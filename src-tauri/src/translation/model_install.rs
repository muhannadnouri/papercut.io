//! Offline translation model installation and removal.
//!
//! Model installation stays separate from inference: large files are streamed,
//! SHA-256 verified, staged beside their destination, then promoted with recovery
//! only after every required file is checked. Translation jobs
//! load a successfully installed model through the engine boundary.

use std::fs;
use std::path::Path;

mod transaction;
#[cfg(test)]
use transaction::WorkDirGuard;

use tauri::{Emitter, Runtime};

use super::config::TRANSLATION_MODEL_INSTALL_PROGRESS_EVENT;
use super::model_store::{
    installed_translation_model_dir, manifest_by_id, translation_model_work_dir,
    TranslationModelFile, TranslationModelManifest,
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
    // Retired packs remain manageable, but must reject before leases, disk recovery or network I/O.
    let manifest = manifest_by_id(&model_id)
        .ok_or_else(|| format!("Translation model {model_id:?} is not in the planned catalog"))?;
    if manifest.is_auxiliary() {
        return Err("Strict formatting is not available: the alignment pack is retired and cannot be installed or repaired".into());
    }
    super::models::admit_provider(&model_id)?;
    if !cfg!(feature = "native-translation") {
        return Err("Offline translation model downloads are not compiled in this build".into());
    }

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
        install_model_blocking(app_for_task, manifest)
    })
    .await
    .map_err(|err| format!("Translation model install task failed: {err}"))
    .and_then(|inner| inner)
}

/// Remove one trusted model/auxiliary pack while preserving translations and segment caches.
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

/// Report removable storage without treating partial files as an installed model or changing recovery state.
pub(super) fn model_storage<R: Runtime>(
    app: &tauri::AppHandle<R>,
    manifest: TranslationModelManifest,
) -> Result<(bool, u64), String> {
    if !manifest.installable {
        return Ok((false, 0));
    }
    manifest.validate()?;
    let paths = transaction::Paths::new(installed_translation_model_dir(app, manifest)?)?;
    let (present, bytes) = paths.storage()?;
    let (legacy_present, legacy_bytes) =
        transaction::storage(&translation_model_work_dir(app, manifest)?)?;
    Ok((
        present || legacy_present,
        bytes
            .checked_add(legacy_bytes)
            .ok_or("Model storage size overflow")?,
    ))
}

/// Delete only installer-owned roots; translated documents and caches live elsewhere.
fn remove_model_blocking<R: Runtime>(
    app: &tauri::AppHandle<R>,
    manifest: TranslationModelManifest,
) -> Result<TranslationModelRemoveResponse, String> {
    manifest.validate()?;
    let paths = transaction::Paths::new(installed_translation_model_dir(app, manifest)?)?;
    let work_dir = translation_model_work_dir(app, manifest)?;
    let (legacy_exists, legacy_bytes) = transaction::clear_legacy(&work_dir)?;
    let (removed, bytes_freed) = paths.remove()?;
    let removed = removed || legacy_exists;
    let bytes_freed = bytes_freed
        .checked_add(legacy_bytes)
        .ok_or("Model storage size overflow")?;

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
    manifest.validate()?;
    let paths = transaction::Paths::new(installed_translation_model_dir(&app, manifest)?)?;
    // A pre-upgrade interrupted download must not consume the space needed by its retry.
    transaction::clear_legacy(&translation_model_work_dir(&app, manifest)?)?;
    emit_model_progress(
        &app,
        manifest,
        "starting",
        if manifest.is_auxiliary() {
            "Preparing auxiliary alignment assets"
        } else {
            "Preparing translation model download"
        },
        0,
    );
    let bytes = transaction::install(
        &paths,
        manifest,
        |file, target, total| download_and_verify_file(&app, manifest, file, target, total),
        |parent, bytes| {
            crate::library_transfer::ensure_available_space(parent, bytes)
                .map_err(|e| e.to_string())
        },
        |from, to| fs::rename(from, to),
    )?;

    emit_model_progress(
        &app,
        manifest,
        "installed",
        if manifest.is_auxiliary() {
            "Auxiliary alignment assets installed"
        } else {
            "Translation model installed"
        },
        manifest.total_bytes(),
    );
    Ok(TranslationModelInstallResponse {
        model_id: manifest.model_id.into(),
        model_dir: paths.final_dir.display().to_string(),
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

#[cfg(test)]
mod tests;
