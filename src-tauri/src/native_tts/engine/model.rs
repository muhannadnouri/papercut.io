//! Voice-model lifecycle: download, verify, extract, status, and capabilities.
//!
//! Each catalog archive is downloaded into an app cache work directory,
//! SHA-256 verified, extracted, checked for required files, then atomically
//! moved into app data. Progress is streamed on [`MODEL_INSTALL_PROGRESS_EVENT`].
//! [`native_capabilities`] and [`model_status`] report install/availability to
//! the frontend without mutating anything.
//!
//! Rust notes for a JS reader: `&app` passes a borrowed reference (read access
//! without taking ownership). `?` after a fallible call returns early on error.
//! `let _ = some_call();` deliberately ignores a result we don't need to check.

use std::fs;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use bzip2::read::BzDecoder;
use sha2::{Digest, Sha256};
use tauri::Emitter;

use super::config::MODEL_INSTALL_PROGRESS_EVENT;
use super::models::{model_definition, visible_models, ModelDefinition, TtsModelBackend};
use super::paths::{
    directory_size, has_required_model_files, installed_model_dir, model_work_dir,
    resolve_model_dir, runtime_model_dir,
};
use super::silma_sidecar::silma_runtime_status;
use crate::native_tts::platform::{default_thread_count, max_thread_count};
use crate::native_tts::state::NativeTtsState;
use crate::native_tts::types::{
    NativeTtsCapabilities, NativeTtsModelInstallProgress, NativeTtsModelInstallResponse,
    NativeTtsModelStatus,
};

/// Report runtime support and the model catalog. Model installation is queried separately.
pub(crate) fn native_capabilities(_app: tauri::AppHandle) -> NativeTtsCapabilities {
    NativeTtsCapabilities {
        available: true,
        backend: "sherpa-onnx".into(),
        reason: "ready".into(),
        model_dir: None,
        platform: std::env::consts::OS.into(),
        default_thread_count: default_thread_count(),
        max_thread_count: max_thread_count(),
        models: visible_models().map(ModelDefinition::to_info).collect(),
    }
}

/// Report install state and source metadata for one catalog model.
pub(crate) fn model_status(
    app: tauri::AppHandle,
    state: tauri::State<'_, NativeTtsState>,
    model_id: String,
) -> NativeTtsModelStatus {
    let Ok(model) = model_definition(&model_id) else {
        return NativeTtsModelStatus {
            model_id,
            installed: false,
            installing: false,
            install_supported: false,
            runtime_installed: false,
            model_dir: None,
            runtime_dir: None,
            source_url: String::new(),
            source_label: "Unsupported model".into(),
            archive_bytes: 0,
            installed_bytes: 0,
            sha256: String::new(),
            message: "Unsupported native TTS model".into(),
            runtime_message: "Unsupported native TTS model".into(),
        };
    };
    let runtime_status = if matches!(model.backend, TtsModelBackend::SilmaSidecar) {
        Some(silma_runtime_status(&app))
    } else {
        None
    };
    let installing = state
        .model_installing
        .lock()
        .map(|guard| guard.contains(model.directory_name))
        .unwrap_or(false);
    match runtime_model_dir(&app, model) {
        Ok(model_dir) if has_required_model_files(model, &model_dir) => NativeTtsModelStatus {
            model_id: model.id.into(),
            installed: true,
            installing,
            install_supported: model.install_supported(),
            runtime_installed: model_runtime_installed(runtime_status.as_ref()),
            installed_bytes: directory_size(&model_dir).unwrap_or(0),
            model_dir: Some(model_dir.display().to_string()),
            runtime_dir: model_runtime_dir(runtime_status.as_ref()),
            source_url: model.source_url.into(),
            source_label: model.source_label.into(),
            archive_bytes: model.archive_bytes,
            sha256: model.sha256.into(),
            message: "Offline voice model installed".into(),
            runtime_message: model_runtime_message(runtime_status.as_ref()),
        },
        Ok(model_dir) => NativeTtsModelStatus {
            model_id: model.id.into(),
            installed: false,
            installing,
            install_supported: model.install_supported(),
            runtime_installed: model_runtime_installed(runtime_status.as_ref()),
            model_dir: missing_model_dir(model, &model_dir),
            runtime_dir: model_runtime_dir(runtime_status.as_ref()),
            source_url: model.source_url.into(),
            source_label: model.source_label.into(),
            archive_bytes: model.archive_bytes,
            installed_bytes: directory_size(&model_dir).unwrap_or(0),
            sha256: model.sha256.into(),
            message: missing_model_message(model, &model_dir, installing),
            runtime_message: model_runtime_message(runtime_status.as_ref()),
        },
        Err(err) => NativeTtsModelStatus {
            model_id: model.id.into(),
            installed: false,
            installing,
            install_supported: model.install_supported(),
            runtime_installed: model_runtime_installed(runtime_status.as_ref()),
            model_dir: None,
            runtime_dir: model_runtime_dir(runtime_status.as_ref()),
            source_url: model.source_url.into(),
            source_label: model.source_label.into(),
            archive_bytes: model.archive_bytes,
            installed_bytes: 0,
            sha256: model.sha256.into(),
            message: err,
            runtime_message: model_runtime_message(runtime_status.as_ref()),
        },
    }
}

/// Non-SILMA models are compiled into the native backend, so their runtime is always present.
fn model_runtime_installed(status: Option<&super::silma_sidecar::SilmaRuntimeStatus>) -> bool {
    status.map(|status| status.installed).unwrap_or(true)
}

/// Report the SILMA runtime folder when status can resolve one.
fn model_runtime_dir(status: Option<&super::silma_sidecar::SilmaRuntimeStatus>) -> Option<String> {
    status
        .and_then(|status| status.runtime_dir.as_ref())
        .map(|path| path.display().to_string())
}

/// Keep a friendly runtime message for sherpa entries while exposing SILMA details.
fn model_runtime_message(status: Option<&super::silma_sidecar::SilmaRuntimeStatus>) -> String {
    status
        .map(|status| status.message.clone())
        .unwrap_or_else(|| "Native TTS runtime available".into())
}

/// Install one catalog model without blocking the async runtime.
///
/// A per-model set prevents duplicate downloads while still allowing the state
/// shape to support independent catalog entries. The guard is always cleared.
pub(crate) async fn install_model(
    app: tauri::AppHandle,
    state: tauri::State<'_, NativeTtsState>,
    model_id: String,
) -> Result<NativeTtsModelInstallResponse, String> {
    let model = model_definition(&model_id)?;
    if !matches!(model.backend, TtsModelBackend::SherpaOnnx) {
        let model_dir = runtime_model_dir(&app, model)?;
        return Err(format!(
            "{} uses the SILMA sidecar backend; app download is not implemented yet. Place {} in {}.",
            model.display_name,
            model.required_files.join(", "),
            model_dir.display()
        ));
    }
    if let Ok(model_dir) = resolve_model_dir(&app, model) {
        return Ok(NativeTtsModelInstallResponse {
            model_id: model.id.into(),
            bytes: directory_size(&model_dir).unwrap_or(0),
            model_dir: model_dir.display().to_string(),
        });
    }

    let installing = state.model_installing.clone();
    {
        let mut guard = installing
            .lock()
            .map_err(|_| "Native TTS model install lock poisoned".to_string())?;
        if !guard.insert(model.directory_name.to_string()) {
            return Err(format!(
                "{} download is already in progress",
                model.display_name
            ));
        }
    }

    let app_for_task = app.clone();
    let result =
        tauri::async_runtime::spawn_blocking(move || install_model_blocking(app_for_task, model))
            .await
            .map_err(|err| format!("Native TTS model install task failed: {err}"))
            .and_then(|inner| inner);

    if let Ok(mut guard) = installing.lock() {
        guard.remove(model.directory_name);
    }
    result
}

/// Return an absent-model message that matches the backend's real install path.
fn missing_model_message(model: &ModelDefinition, model_dir: &Path, installing: bool) -> String {
    if installing {
        return "Offline voice model download in progress".into();
    }
    if matches!(model.backend, TtsModelBackend::SilmaSidecar) {
        return format!(
            "Place SILMA model files ({}) in {}. App download is not implemented yet.",
            model.required_files.join(", "),
            model_dir.display()
        );
    }
    "Offline voice model is not installed".into()
}

/// Keep sherpa's historical missing-model shape while exposing SILMA's manual path.
fn missing_model_dir(model: &ModelDefinition, model_dir: &Path) -> Option<String> {
    if matches!(model.backend, TtsModelBackend::SilmaSidecar) {
        return Some(model_dir.display().to_string());
    }
    None
}

/// Run the checked install transaction: download, hash, extract, validate, then promote.
/// Work remains isolated until the complete model directory can be atomically moved.
fn install_model_blocking(
    app: tauri::AppHandle,
    model: &ModelDefinition,
) -> Result<NativeTtsModelInstallResponse, String> {
    let final_dir = installed_model_dir(&app, model)?;
    emit_model_progress(
        &app,
        model,
        "starting",
        "Preparing offline voice model download",
        0,
    );

    let work_root = model_work_dir(&app, model)?;
    let archive_path = work_root.join(format!("{}.tar.bz2", model.directory_name));
    let extract_dir = work_root.join("extract");
    let temp_model_dir = work_root.join(format!("{}.installing", model.directory_name));
    let _ = fs::remove_dir_all(&work_root);
    fs::create_dir_all(&work_root).map_err(|err| {
        format!(
            "Failed to create model installer work directory {}: {err}",
            work_root.display()
        )
    })?;
    let work_guard = WorkDirGuard::new(work_root.clone());

    download_model_archive(&app, &archive_path, model)?;
    verify_model_archive(&archive_path, model)?;
    emit_model_progress(
        &app,
        model,
        "extracting",
        "Extracting offline voice model",
        model.archive_bytes,
    );
    fs::create_dir_all(&extract_dir).map_err(|err| {
        format!(
            "Failed to create model extraction directory {}: {err}",
            extract_dir.display()
        )
    })?;
    extract_model_archive(&archive_path, &extract_dir)?;

    let extracted_model_dir = extract_dir.join(model.directory_name);
    if !model.has_required_files(&extracted_model_dir) {
        return Err("Downloaded voice model is missing required files after extraction".into());
    }

    let _ = fs::remove_dir_all(&temp_model_dir);
    fs::rename(&extracted_model_dir, &temp_model_dir).map_err(|err| {
        format!(
            "Failed to stage extracted voice model {}: {err}",
            temp_model_dir.display()
        )
    })?;
    if let Some(parent) = final_dir.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "Failed to create model directory {}: {err}",
                parent.display()
            )
        })?;
    }
    let _ = fs::remove_dir_all(&final_dir);
    fs::rename(&temp_model_dir, &final_dir).map_err(|err| {
        format!(
            "Failed to install offline voice model {}: {err}",
            final_dir.display()
        )
    })?;
    work_guard.disarm();
    let _ = fs::remove_dir_all(&work_root);

    let bytes = directory_size(&final_dir).unwrap_or(0);
    emit_model_progress(
        &app,
        model,
        "installed",
        "Offline voice model installed",
        model.archive_bytes,
    );
    Ok(NativeTtsModelInstallResponse {
        model_id: model.id.into(),
        model_dir: final_dir.display().to_string(),
        bytes,
    })
}

struct WorkDirGuard {
    path: PathBuf,
    armed: bool,
}

impl WorkDirGuard {
    /// Arm cleanup for a model install work directory until success explicitly disarms it.
    fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }

    /// Consume the guard after a successful install so Drop skips failure cleanup.
    fn disarm(mut self) {
        self.armed = false;
    }
}

impl Drop for WorkDirGuard {
    /// Best-effort cleanup; install failure should not leave the large model work tree behind.
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

/// Stream the pinned model archive from GitHub to `archive_path`, emitting
/// throttled download-progress events (every ~2% and at 100%). Reads in 256 KB
/// blocks so memory stays flat regardless of the ~333 MB total.
fn download_model_archive(
    app: &tauri::AppHandle,
    archive_path: &Path,
    model: &ModelDefinition,
) -> Result<(), String> {
    let message = format!("Downloading {}", model.display_name);
    emit_model_progress(app, model, "downloading", &message, 0);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60 * 30))
        .user_agent("Papercut native TTS model installer")
        .build()
        .map_err(|err| format!("Failed to create model downloader: {err}"))?;
    let mut response = client
        .get(model.source_url)
        .send()
        .map_err(|err| {
            format!(
                "Failed to download offline voice model from {}: {err}",
                model.source_url
            )
        })?
        .error_for_status()
        .map_err(|err| {
            format!(
                "Failed to download offline voice model from {}: {err}",
                model.source_url
            )
        })?;
    let total = response.content_length().unwrap_or(model.archive_bytes);
    let file = fs::File::create(archive_path).map_err(|err| {
        format!(
            "Failed to create model archive {}: {err}",
            archive_path.display()
        )
    })?;
    let mut writer = BufWriter::new(file);
    let mut downloaded = 0u64;
    let mut last_percent = 0u8;
    let mut buffer = [0u8; 256 * 1024];
    loop {
        let read = response
            .read(&mut buffer)
            .map_err(|err| format!("Failed while downloading offline voice model: {err}"))?;
        if read == 0 {
            break; // end of stream
        }
        writer.write_all(&buffer[..read]).map_err(|err| {
            format!(
                "Failed to write model archive {}: {err}",
                archive_path.display()
            )
        })?;
        downloaded += read as u64;
        // Throttle progress events to avoid flooding the frontend.
        let percent = download_percent(downloaded, total);
        if percent >= last_percent.saturating_add(2) || percent == 100 {
            last_percent = percent;
            emit_model_progress(app, model, "downloading", &message, downloaded);
        }
    }
    writer.flush().map_err(|err| {
        format!(
            "Failed to finish model archive {}: {err}",
            archive_path.display()
        )
    })?;
    Ok(())
}

/// Verify the downloaded archive's SHA-256 matches the pinned hash, reading in
/// 256 KB blocks. Guards against corrupt or tampered downloads before extract.
fn verify_model_archive(archive_path: &Path, model: &ModelDefinition) -> Result<(), String> {
    let file = fs::File::open(archive_path).map_err(|err| {
        format!(
            "Failed to open model archive {}: {err}",
            archive_path.display()
        )
    })?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 256 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|err| format!("Failed to hash model archive: {err}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let actual = format!("{:x}", hasher.finalize());
    if actual != model.sha256 {
        return Err(format!(
            "Downloaded voice model checksum mismatch. Expected {}, got {actual}",
            model.sha256
        ));
    }
    Ok(())
}

/// Decompress (bzip2) and untar the verified archive into `extract_dir`.
/// Safe to unpack directly because the archive was checksum-pinned above.
fn extract_model_archive(archive_path: &Path, extract_dir: &Path) -> Result<(), String> {
    let file = fs::File::open(archive_path).map_err(|err| {
        format!(
            "Failed to open model archive {}: {err}",
            archive_path.display()
        )
    })?;
    let decoder = BzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    // The archive is checksum-pinned above. tar::Archive::unpack preserves the
    // directory tree from the trusted archive into our app-owned temp folder.
    archive
        .unpack(extract_dir)
        .map_err(|err| format!("Failed to extract offline voice model: {err}"))
}

/// Emit one install-progress event (status + message + byte count + percent).
/// Best-effort: a failed emit is ignored so it can't abort the install.
fn emit_model_progress(
    app: &tauri::AppHandle,
    model: &ModelDefinition,
    status: &str,
    message: &str,
    downloaded_bytes: u64,
) {
    let _ = app.emit(
        MODEL_INSTALL_PROGRESS_EVENT,
        NativeTtsModelInstallProgress {
            model_id: model.id.into(),
            status: status.into(),
            message: message.into(),
            downloaded_bytes,
            total_bytes: model.archive_bytes,
            percent: download_percent(downloaded_bytes, model.archive_bytes),
        },
    );
}

/// Integer percentage of `downloaded` out of `total`, clamped to 0..=100 and
/// using saturating math so it can never overflow or divide by zero.
fn download_percent(downloaded: u64, total: u64) -> u8 {
    if total == 0 {
        return 0;
    }
    ((downloaded.saturating_mul(100) / total).min(100)) as u8
}
