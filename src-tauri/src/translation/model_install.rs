//! Offline translation model installation and removal.
//!
//! Model installation stays separate from inference: large files are streamed,
//! SHA-256 verified, staged in a cache work directory, then promoted into app
//! data only after every required file is present and checked. Translation jobs
//! load a successfully installed model through the engine boundary.

use std::fs;
use std::path::{Path, PathBuf};

use tauri::{Emitter, Runtime};

use super::config::TRANSLATION_MODEL_INSTALL_PROGRESS_EVENT;
use super::model_store::{
    directory_size, installed_translation_model_dir, manifest_by_id, resolve_translation_model_dir,
    translation_model_work_dir, TranslationModelFile, TranslationModelManifest,
};
use super::state::TranslationState;
use super::types::{
    TranslationModelInstallProgress, TranslationModelInstallResponse,
    TranslationModelRemoveResponse,
};

pub(crate) async fn install_translation_model<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, TranslationState>,
    model_id: String,
) -> Result<TranslationModelInstallResponse, String> {
    if !cfg!(feature = "native-translation") {
        return Err("Offline translation model downloads are not compiled in this build".into());
    }

    let manifest = manifest_by_id(&model_id)
        .ok_or_else(|| format!("Translation model {model_id:?} is not in the planned catalog"))?;
    if !manifest.installable {
        return Err(format!(
            "{} is not installable yet; its model manifest is not pinned for downloads.",
            manifest.model_id
        ));
    }
    let operation = state.claim_model_install(manifest.directory_name)?;

    let app_for_task = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        if let Ok(model_dir) = resolve_translation_model_dir(&app_for_task, manifest) {
            return Ok(TranslationModelInstallResponse {
                model_id: manifest.model_id.into(),
                model_dir: model_dir.display().to_string(),
                bytes: directory_size(&model_dir).unwrap_or(0),
            });
        }
        install_model_blocking(app_for_task, manifest)
    })
    .await
    .map_err(|err| format!("Translation model install task failed: {err}"))
    .and_then(|inner| inner)
}

/// Remove one catalog model while preserving translations and segment caches.
pub(crate) async fn remove_translation_model<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, TranslationState>,
    model_id: String,
) -> Result<TranslationModelRemoveResponse, String> {
    let manifest = manifest_by_id(&model_id)
        .ok_or_else(|| format!("Translation model {model_id:?} is not in the planned catalog"))?;
    if !manifest.installable {
        return Err(format!(
            "{} does not have installed model files.",
            manifest.model_id
        ));
    }
    let operation = state.claim_model_removal(manifest.directory_name)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _operation = operation;
        remove_model_blocking(&app, manifest)
    })
    .await
    .map_err(|err| format!("Translation model removal task failed: {err}"))
    .and_then(|inner| inner)
}

/// Delete only installer-owned roots; translated documents and caches live elsewhere.
fn remove_model_blocking<R: Runtime>(
    app: &tauri::AppHandle<R>,
    manifest: TranslationModelManifest,
) -> Result<TranslationModelRemoveResponse, String> {
    let model_dir = installed_translation_model_dir(app, manifest)?;
    let work_dir = translation_model_work_dir(app, manifest)?;
    let bytes_freed = directory_size(&model_dir)?.saturating_add(directory_size(&work_dir)?);
    let removed = model_dir.exists() || work_dir.exists();

    if work_dir.exists() {
        fs::remove_dir_all(&work_dir).map_err(|err| {
            format!(
                "Failed to remove translation model work directory {}: {err}",
                work_dir.display()
            )
        })?;
    }
    if model_dir.exists() {
        fs::remove_dir_all(&model_dir).map_err(|err| {
            format!(
                "Failed to remove translation model directory {}: {err}",
                model_dir.display()
            )
        })?;
    }

    Ok(TranslationModelRemoveResponse {
        model_id: manifest.model_id.into(),
        removed,
        bytes_freed,
    })
}

fn install_model_blocking<R: Runtime>(
    app: tauri::AppHandle<R>,
    manifest: TranslationModelManifest,
) -> Result<TranslationModelInstallResponse, String> {
    let final_dir = installed_translation_model_dir(&app, manifest)?;
    let work_root = translation_model_work_dir(&app, manifest)?;
    let temp_model_dir = work_root.join(format!("{}.installing", manifest.directory_name));
    let files_dir = work_root.join("files");

    emit_model_progress(
        &app,
        manifest,
        "starting",
        "Preparing translation model download",
        0,
    );
    let _ = fs::remove_dir_all(&work_root);
    fs::create_dir_all(&files_dir).map_err(|err| {
        format!(
            "Failed to create translation model work directory {}: {err}",
            files_dir.display()
        )
    })?;
    let work_guard = WorkDirGuard::new(work_root.clone());

    let mut downloaded_total = 0u64;
    for file in manifest.files {
        let target = files_dir.join(file.path);
        download_and_verify_file(&app, manifest, file, &target, &mut downloaded_total)?;
    }

    if !manifest.has_required_files(&files_dir) {
        return Err("Downloaded translation model is missing required files".into());
    }

    let _ = fs::remove_dir_all(&temp_model_dir);
    fs::rename(&files_dir, &temp_model_dir).map_err(|err| {
        format!(
            "Failed to stage translation model {}: {err}",
            temp_model_dir.display()
        )
    })?;
    if let Some(parent) = final_dir.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "Failed to create translation model directory {}: {err}",
                parent.display()
            )
        })?;
    }
    let _ = fs::remove_dir_all(&final_dir);
    fs::rename(&temp_model_dir, &final_dir).map_err(|err| {
        format!(
            "Failed to install translation model {}: {err}",
            final_dir.display()
        )
    })?;

    work_guard.disarm();
    let _ = fs::remove_dir_all(&work_root);
    let bytes = directory_size(&final_dir).unwrap_or(0);
    emit_model_progress(
        &app,
        manifest,
        "installed",
        "Translation model installed",
        manifest.total_bytes(),
    );
    Ok(TranslationModelInstallResponse {
        model_id: manifest.model_id.into(),
        model_dir: final_dir.display().to_string(),
        bytes,
    })
}

#[cfg(feature = "native-translation")]
fn download_and_verify_file<R: Runtime>(
    app: &tauri::AppHandle<R>,
    manifest: TranslationModelManifest,
    file: &TranslationModelFile,
    target: &Path,
    downloaded_total: &mut u64,
) -> Result<(), String> {
    use std::io::{BufWriter, Read, Write};
    use std::time::Duration;

    use sha2::{Digest, Sha256};

    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "Failed to create translation model file directory {}: {err}",
                parent.display()
            )
        })?;
    }

    let url = model_file_url(manifest, file);
    let message = format!("Downloading {}", file.path);
    emit_model_progress(app, manifest, "downloading", &message, *downloaded_total);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60 * 30))
        .user_agent("Papercut offline translation model installer")
        .build()
        .map_err(|err| format!("Failed to create translation model downloader: {err}"))?;
    let mut response = client
        .get(&url)
        .send()
        .map_err(|err| format!("Failed to download translation model file from {url}: {err}"))?
        .error_for_status()
        .map_err(|err| format!("Failed to download translation model file from {url}: {err}"))?;
    let output = fs::File::create(target).map_err(|err| {
        format!(
            "Failed to create translation model file {}: {err}",
            target.display()
        )
    })?;
    let mut writer = BufWriter::new(output);
    let mut hasher = Sha256::new();
    let mut downloaded_file = 0u64;
    let mut last_percent = download_percent(*downloaded_total, manifest.total_bytes());
    let mut buffer = [0u8; 256 * 1024];
    loop {
        let read = response
            .read(&mut buffer)
            .map_err(|err| format!("Failed while downloading translation model file: {err}"))?;
        if read == 0 {
            break;
        }
        if downloaded_file.saturating_add(read as u64) > file.bytes {
            return Err("Translation model download exceeded its pinned size".into());
        }
        writer.write_all(&buffer[..read]).map_err(|err| {
            format!(
                "Failed to write translation model file {}: {err}",
                target.display()
            )
        })?;
        hasher.update(&buffer[..read]);
        downloaded_file += read as u64;
        *downloaded_total += read as u64;
        let percent = download_percent(*downloaded_total, manifest.total_bytes());
        if percent >= last_percent.saturating_add(2) || percent == 100 {
            last_percent = percent;
            emit_model_progress(app, manifest, "downloading", &message, *downloaded_total);
        }
    }
    writer.flush().map_err(|err| {
        format!(
            "Failed to finish translation model file {}: {err}",
            target.display()
        )
    })?;

    if downloaded_file != file.bytes {
        return Err(format!(
            "Translation model file {} size mismatch. Expected {} bytes, got {} bytes",
            file.path, file.bytes, downloaded_file
        ));
    }
    let actual = format!("{:x}", hasher.finalize());
    if actual != file.sha256 {
        return Err(format!(
            "Translation model file {} checksum mismatch. Expected {}, got {actual}",
            file.path, file.sha256
        ));
    }
    Ok(())
}

#[cfg(not(feature = "native-translation"))]
fn download_and_verify_file<R: Runtime>(
    _app: &tauri::AppHandle<R>,
    _manifest: TranslationModelManifest,
    _file: &TranslationModelFile,
    _target: &Path,
    _downloaded_total: &mut u64,
) -> Result<(), String> {
    Err("Offline translation model downloads are not compiled in this build".into())
}

// Only the feature-gated downloader builds URLs; keep the helper compiled in
// all builds so it stays reviewed alongside the manifest shape.
#[cfg_attr(not(feature = "native-translation"), allow(dead_code))]
fn model_file_url(manifest: TranslationModelManifest, file: &TranslationModelFile) -> String {
    if let Some(url) = file.download_url {
        return url.into();
    }
    format!(
        "https://huggingface.co/{}/resolve/{}/{}",
        manifest.source_label, manifest.revision, file.path
    )
}

fn emit_model_progress<R: Runtime>(
    app: &tauri::AppHandle<R>,
    manifest: TranslationModelManifest,
    status: &str,
    message: &str,
    downloaded_bytes: u64,
) {
    let _ = app.emit(
        TRANSLATION_MODEL_INSTALL_PROGRESS_EVENT,
        TranslationModelInstallProgress {
            model_id: manifest.model_id.into(),
            status: status.into(),
            message: message.into(),
            downloaded_bytes,
            total_bytes: manifest.total_bytes(),
            percent: download_percent(downloaded_bytes, manifest.total_bytes()),
        },
    );
}

fn download_percent(downloaded: u64, total: u64) -> u8 {
    if total == 0 {
        return 0;
    }
    ((downloaded.saturating_mul(100) / total).min(100)) as u8
}

struct WorkDirGuard {
    path: PathBuf,
    armed: bool,
}

impl WorkDirGuard {
    fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }

    fn disarm(mut self) {
        self.armed = false;
    }
}

impl Drop for WorkDirGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::download_percent;

    #[test]
    fn progress_percent_clamps_and_handles_zero_total() {
        assert_eq!(download_percent(0, 0), 0);
        assert_eq!(download_percent(50, 100), 50);
        assert_eq!(download_percent(250, 100), 100);
    }

    #[test]
    fn explicit_mozilla_urls_do_not_change_hugging_face_pins() {
        use crate::translation::model_store::manifest_by_id;
        let opus = manifest_by_id("opus-mt-fr-en-ctranslate2").unwrap();
        assert_eq!(super::model_file_url(opus, &opus.files[0]),
            "https://huggingface.co/michaelfeil/ct2fast-opus-mt-fr-en/resolve/cb3b2d680bf35591a508d8479e2c99c44e281ef3/config.json");
        let worker = manifest_by_id("bergamot-fr-en-worker").unwrap();
        assert_eq!(
            super::model_file_url(worker, &worker.files[0]),
            worker.files[0].download_url.unwrap()
        );
    }

    #[cfg(feature = "native-translation")]
    #[test]
    fn actual_downloader_verifies_bytes_and_rejects_oversize_before_writing() {
        use crate::translation::model_store::{manifest_by_id, TranslationModelFile};
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::time::{Duration, Instant};
        for body in [b"abc".as_slice(), b"abd", b"abcd"] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let url: &'static str = Box::leak(
                format!("http://{}/asset", listener.local_addr().unwrap()).into_boxed_str(),
            );
            let body = body.to_vec();
            let good = body == b"abc";
            let oversize = body.len() > 3;
            let server = std::thread::spawn(move || {
                let deadline = Instant::now() + Duration::from_secs(5);
                let mut socket = loop {
                    if let Ok((socket, _)) = listener.accept() {
                        break socket;
                    }
                    assert!(Instant::now() < deadline, "test downloader did not connect");
                    std::thread::sleep(Duration::from_millis(5));
                };
                socket
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut request = [0; 2048];
                socket.read(&mut request).unwrap();
                write!(
                    socket,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                socket.write_all(&body).unwrap();
            });
            let manifest = manifest_by_id("bergamot-fr-en-worker").unwrap();
            let pin = TranslationModelFile {
                path: "fixture.bin",
                download_url: Some(url),
                bytes: 3,
                sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            };
            let mut nonce = [0u8; 16];
            getrandom::fill(&mut nonce).unwrap();
            let dir = std::env::temp_dir().join(format!("papercut-download-{nonce:x?}"));
            let guard = super::WorkDirGuard::new(dir.clone());
            let target = dir.join(pin.path);
            let app = tauri::test::mock_builder()
                .build(tauri::test::mock_context(tauri::test::noop_assets()))
                .unwrap();
            let mut total = 0;
            let result =
                super::download_and_verify_file(app.handle(), manifest, &pin, &target, &mut total);
            server.join().unwrap();
            assert_eq!(result.is_ok(), good);
            if good {
                assert_eq!(std::fs::read(&target).unwrap(), b"abc");
            }
            if oversize {
                assert_eq!(std::fs::metadata(&target).unwrap().len(), 0);
                assert_eq!(total, 0);
            }
            drop(guard);
            assert!(!dir.exists());
        }
    }
}
