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
use std::fs::OpenOptions;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use bzip2::read::BzDecoder;
use reqwest::header::RANGE;
use reqwest::StatusCode;
use sha2::{Digest, Sha256};
use tauri::Emitter;

use super::config::MODEL_INSTALL_PROGRESS_EVENT;
use super::models::{
    model_definition, visible_models, ModelDefinition, TtsModelBackend, SILMA_HF_CACHE_REPO_DIR,
    SILMA_HF_REVISION,
};
use super::paths::{
    directory_size, has_required_model_files, installed_model_dir, model_work_dir,
    resolve_model_dir, runtime_model_dir,
};
use super::providers::compiled_sherpa_execution_providers;
use super::silma_sidecar::{install_silma_runtime_pack, silma_runtime_status};
use crate::native_tts::platform::{default_thread_count, max_thread_count};
use crate::native_tts::state::NativeTtsState;
use crate::native_tts::types::{
    NativeTtsCapabilities, NativeTtsModelInstallProgress, NativeTtsModelInstallResponse,
    NativeTtsModelStatus,
};

const SILMA_MODEL_FILES: &[SilmaModelFile] = &[
    SilmaModelFile {
        name: "model.pt",
        blob: "f43256d0b78b8803c638aed0875da5a4b372b4a784690a0156e5baff14f7336c",
        url: "https://huggingface.co/silma-ai/silma-tts/resolve/d2515317033803648ecb8844765db9e583afecf9/model.pt",
        sha256: "f43256d0b78b8803c638aed0875da5a4b372b4a784690a0156e5baff14f7336c",
        bytes: 2_603_209_272,
    },
    SilmaModelFile {
        name: "vocab.txt",
        blob: "b678d831888af2e3c662b94bc248fa6845bebf61",
        url: "https://huggingface.co/silma-ai/silma-tts/resolve/d2515317033803648ecb8844765db9e583afecf9/vocab.txt",
        sha256: "5c2ffc48802a52bbdf715dacf1d6519d3fee96e391aef690261963a692b8e661",
        bytes: 36_357,
    },
];
struct SilmaModelFile {
    name: &'static str,
    blob: &'static str,
    url: &'static str,
    sha256: &'static str,
    bytes: u64,
}

/// Report runtime support and the model catalog. Model installation is queried separately.
pub(crate) fn native_capabilities(_app: tauri::AppHandle) -> NativeTtsCapabilities {
    let (compiled_execution_providers, execution_provider_probe_error) =
        match compiled_sherpa_execution_providers() {
            Ok(providers) => (providers.into_iter().map(str::to_string).collect(), None),
            Err(error) => (vec!["cpu".into()], Some(error)),
        };
    NativeTtsCapabilities {
        available: true,
        backend: "sherpa-onnx".into(),
        reason: "ready".into(),
        model_dir: None,
        platform: std::env::consts::OS.into(),
        compiled_execution_providers,
        execution_provider_probe_error,
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
            install_supported: model_install_supported(model, runtime_status.as_ref()),
            runtime_installed: model_runtime_installed(runtime_status.as_ref()),
            installed_bytes: directory_size(&model_dir).unwrap_or(0),
            model_dir: Some(model_dir.display().to_string()),
            runtime_dir: model_runtime_dir(runtime_status.as_ref()),
            source_url: model.source_url.into(),
            source_label: model.source_label.into(),
            archive_bytes: model_archive_bytes(model, runtime_status.as_ref()),
            sha256: model.sha256.into(),
            message: "Offline voice model installed".into(),
            runtime_message: model_runtime_message(runtime_status.as_ref()),
        },
        Ok(model_dir) => NativeTtsModelStatus {
            model_id: model.id.into(),
            installed: false,
            installing,
            install_supported: model_install_supported(model, runtime_status.as_ref()),
            runtime_installed: model_runtime_installed(runtime_status.as_ref()),
            model_dir: missing_model_dir(model, &model_dir),
            runtime_dir: model_runtime_dir(runtime_status.as_ref()),
            source_url: model.source_url.into(),
            source_label: model.source_label.into(),
            archive_bytes: model_archive_bytes(model, runtime_status.as_ref()),
            installed_bytes: directory_size(&model_dir).unwrap_or(0),
            sha256: model.sha256.into(),
            message: missing_model_message(model, installing),
            runtime_message: model_runtime_message(runtime_status.as_ref()),
        },
        Err(err) => NativeTtsModelStatus {
            model_id: model.id.into(),
            installed: false,
            installing,
            install_supported: model_install_supported(model, runtime_status.as_ref()),
            runtime_installed: model_runtime_installed(runtime_status.as_ref()),
            model_dir: None,
            runtime_dir: model_runtime_dir(runtime_status.as_ref()),
            source_url: model.source_url.into(),
            source_label: model.source_label.into(),
            archive_bytes: model_archive_bytes(model, runtime_status.as_ref()),
            installed_bytes: 0,
            sha256: model.sha256.into(),
            message: err,
            runtime_message: model_runtime_message(runtime_status.as_ref()),
        },
    }
}

/// SILMA's install button may install the runtime pack before model weights exist.
fn model_archive_bytes(
    model: &ModelDefinition,
    status: Option<&super::silma_sidecar::SilmaRuntimeStatus>,
) -> u64 {
    if matches!(model.backend, TtsModelBackend::SilmaSidecar) {
        return status
            .filter(|status| !status.installed && status.archive_bytes > 0)
            .map(|status| status.archive_bytes)
            .unwrap_or(model.archive_bytes);
    }
    model.archive_bytes
}

fn model_install_supported(
    model: &ModelDefinition,
    status: Option<&super::silma_sidecar::SilmaRuntimeStatus>,
) -> bool {
    if matches!(model.backend, TtsModelBackend::SilmaSidecar) {
        return status
            .map(|status| status.installed || status.install_supported)
            .unwrap_or(false);
    }
    model.install_supported()
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
        return install_silma_for_model(app, state, model).await;
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
    if result.is_ok() {
        if let Ok(mut engine) = state.engine.lock() {
            *engine = None;
        }
    }
    result
}

/// Install whichever SILMA piece is missing first: runtime pack, then model files.
async fn install_silma_for_model(
    app: tauri::AppHandle,
    state: tauri::State<'_, NativeTtsState>,
    model: &'static ModelDefinition,
) -> Result<NativeTtsModelInstallResponse, String> {
    let runtime_status = silma_runtime_status(&app);
    if !runtime_status.installed {
        let _ = install_silma_runtime_pack_for_model(app.clone(), &state, model, runtime_status)
            .await?;
    }

    if let Ok(model_dir) = resolve_model_dir(&app, model) {
        return Ok(NativeTtsModelInstallResponse {
            model_id: model.id.into(),
            bytes: directory_size(&model_dir).unwrap_or(0),
            model_dir: model_dir.display().to_string(),
        });
    }

    install_silma_model_files_for_model(app, &state, model).await
}

/// Install the local SILMA runtime pack into app data.
async fn install_silma_runtime_pack_for_model(
    app: tauri::AppHandle,
    state: &tauri::State<'_, NativeTtsState>,
    model: &'static ModelDefinition,
    runtime_status: super::silma_sidecar::SilmaRuntimeStatus,
) -> Result<NativeTtsModelInstallResponse, String> {
    if !runtime_status.install_supported {
        return Err(format!(
            "{}. Run `npm run prepare:silma-sidecar -- --self-test` or add release metadata to src-tauri/tts/silma-runtime-packs.json.",
            runtime_status.message
        ));
    }

    let installing = state.model_installing.clone();
    {
        let mut guard = installing
            .lock()
            .map_err(|_| "Native TTS model install lock poisoned".to_string())?;
        if !guard.insert(model.directory_name.to_string()) {
            return Err("SILMA runtime pack install is already in progress".into());
        }
    }

    emit_model_progress(&app, model, "starting", "Installing SILMA runtime pack", 0);
    let app_for_task = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let runtime_dir =
            install_silma_runtime_pack(&app_for_task, |status, message, downloaded, total| {
                emit_model_progress_total(&app_for_task, model, status, message, downloaded, total);
            })?;
        Ok(NativeTtsModelInstallResponse {
            model_id: model.id.into(),
            bytes: directory_size(&runtime_dir).unwrap_or(0),
            model_dir: runtime_dir.display().to_string(),
        })
    })
    .await
    .map_err(|err| format!("SILMA runtime pack install task failed: {err}"))
    .and_then(|inner| inner);

    if let Ok(mut guard) = installing.lock() {
        guard.remove(model.directory_name);
    }
    if result.is_ok() {
        if let Ok(mut engine) = state.engine.lock() {
            *engine = None;
        }
        emit_model_progress(
            &app,
            model,
            "runtime-installed",
            "SILMA runtime installed; installing model files next",
            0,
        );
    }
    result
}

/// Download SILMA's pinned Hugging Face files into the app-owned model folder.
async fn install_silma_model_files_for_model(
    app: tauri::AppHandle,
    state: &tauri::State<'_, NativeTtsState>,
    model: &'static ModelDefinition,
) -> Result<NativeTtsModelInstallResponse, String> {
    let installing = state.model_installing.clone();
    {
        let mut guard = installing
            .lock()
            .map_err(|_| "Native TTS model install lock poisoned".to_string())?;
        if !guard.insert(model.directory_name.to_string()) {
            return Err("SILMA model install is already in progress".into());
        }
    }

    emit_model_progress(&app, model, "starting", "Installing SILMA model files", 0);
    let app_for_task = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        install_silma_model_files_blocking(app_for_task, model)
    })
    .await
    .map_err(|err| format!("SILMA model install task failed: {err}"))
    .and_then(|inner| inner);

    if let Ok(mut guard) = installing.lock() {
        guard.remove(model.directory_name);
    }
    if result.is_ok() {
        if let Ok(mut engine) = state.engine.lock() {
            *engine = None;
        }
    }
    result
}

/// Return an absent-model message that matches the backend's real install path.
fn missing_model_message(model: &ModelDefinition, installing: bool) -> String {
    if installing {
        return "Offline voice model download in progress".into();
    }
    if matches!(model.backend, TtsModelBackend::SilmaSidecar) {
        return "Open Audio Setup and choose Install SILMA to download the required model files."
            .into();
    }
    "Offline voice model is not installed".into()
}

/// Install SILMA model files through a temp tree, then atomically promote it.
fn install_silma_model_files_blocking(
    app: tauri::AppHandle,
    model: &ModelDefinition,
) -> Result<NativeTtsModelInstallResponse, String> {
    let final_dir = installed_model_dir(&app, model)?;
    let work_root = model_work_dir(&app, model)?;
    let temp_model_dir = work_root.join(format!("{}.installing", model.directory_name));
    fs::create_dir_all(&work_root).map_err(|err| {
        format!(
            "Failed to create SILMA model work directory {}: {err}",
            work_root.display()
        )
    })?;
    fs::create_dir_all(&temp_model_dir).map_err(|err| {
        format!(
            "Failed to create SILMA model staging directory {}: {err}",
            temp_model_dir.display()
        )
    })?;
    let total_bytes = silma_model_files_total_bytes();
    let mut completed_bytes = 0;
    for file in SILMA_MODEL_FILES {
        let destination = silma_hf_blob_path(&temp_model_dir, file);
        seed_silma_hf_blob_from_legacy_file(&final_dir, file, &destination)?;
        download_silma_model_file(
            &app,
            model,
            file,
            &destination,
            completed_bytes,
            total_bytes,
        )?;
        verify_file_sha256(&destination, file.sha256, file.name)?;
        link_silma_hf_snapshot_file(&temp_model_dir, file, &destination)?;
        completed_bytes += file.bytes;
    }
    write_silma_hf_ref(&temp_model_dir)?;
    if !has_required_model_files(model, &temp_model_dir) {
        return Err("Downloaded SILMA model is missing required files".into());
    }
    if let Some(parent) = final_dir.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "Failed to create SILMA model directory {}: {err}",
                parent.display()
            )
        })?;
    }
    let _ = fs::remove_dir_all(&final_dir);
    fs::rename(&temp_model_dir, &final_dir).map_err(|err| {
        format!(
            "Failed to install SILMA model files {}: {err}",
            final_dir.display()
        )
    })?;
    let _ = fs::remove_dir_all(&work_root);
    let bytes = directory_size(&final_dir).unwrap_or(0);
    emit_model_progress_total(
        &app,
        model,
        "installed",
        "SILMA model files installed",
        total_bytes,
        total_bytes,
    );
    Ok(NativeTtsModelInstallResponse {
        model_id: model.id.into(),
        model_dir: final_dir.display().to_string(),
        bytes,
    })
}

fn silma_model_files_total_bytes() -> u64 {
    SILMA_MODEL_FILES.iter().map(|file| file.bytes).sum()
}

fn silma_hf_blob_path(root: &Path, file: &SilmaModelFile) -> PathBuf {
    root.join(SILMA_HF_CACHE_REPO_DIR)
        .join("blobs")
        .join(file.blob)
}

fn silma_hf_snapshot_path(root: &Path, file: &SilmaModelFile) -> PathBuf {
    root.join(SILMA_HF_CACHE_REPO_DIR)
        .join("snapshots")
        .join(SILMA_HF_REVISION)
        .join(file.name)
}

/// Reuse the old flat Papercut download when upgrading to Hugging Face cache layout.
fn seed_silma_hf_blob_from_legacy_file(
    final_dir: &Path,
    file: &SilmaModelFile,
    destination: &Path,
) -> Result<(), String> {
    if destination.exists() {
        return Ok(());
    }
    let legacy_file = final_dir.join(file.name);
    if !legacy_file.is_file() {
        return Ok(());
    }
    copy_or_hard_link_file(&legacy_file, destination)
}

/// Put the blob at SILMA's expected snapshot path without copying 2.5GB when possible.
fn link_silma_hf_snapshot_file(
    root: &Path,
    file: &SilmaModelFile,
    blob_path: &Path,
) -> Result<(), String> {
    let snapshot_path = silma_hf_snapshot_path(root, file);
    if let Some(parent) = snapshot_path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "Failed to create SILMA Hugging Face snapshot directory {}: {err}",
                parent.display()
            )
        })?;
    }
    let _ = fs::remove_file(&snapshot_path);
    create_silma_snapshot_link(file, blob_path, &snapshot_path)
}

/// `cached_path` delegates to `huggingface_hub`, which resolves branch names through refs.
fn write_silma_hf_ref(root: &Path) -> Result<(), String> {
    let ref_path = root.join(SILMA_HF_CACHE_REPO_DIR).join("refs").join("main");
    if let Some(parent) = ref_path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "Failed to create SILMA Hugging Face ref directory {}: {err}",
                parent.display()
            )
        })?;
    }
    fs::write(&ref_path, SILMA_HF_REVISION).map_err(|err| {
        format!(
            "Failed to write SILMA Hugging Face ref {}: {err}",
            ref_path.display()
        )
    })
}

fn copy_or_hard_link_file(source: &Path, destination: &Path) -> Result<(), String> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "Failed to create SILMA model file directory {}: {err}",
                parent.display()
            )
        })?;
    }
    let _ = fs::remove_file(destination);
    match fs::hard_link(source, destination) {
        Ok(()) => Ok(()),
        Err(_) => fs::copy(source, destination).map(|_| ()).map_err(|err| {
            format!(
                "Failed to copy SILMA model file {} to {}: {err}",
                source.display(),
                destination.display()
            )
        }),
    }
}

#[cfg(unix)]
fn create_silma_snapshot_link(
    file: &SilmaModelFile,
    _blob_path: &Path,
    snapshot_path: &Path,
) -> Result<(), String> {
    let relative_blob = Path::new("..").join("..").join("blobs").join(file.blob);
    std::os::unix::fs::symlink(&relative_blob, snapshot_path).map_err(|err| {
        format!(
            "Failed to link SILMA snapshot file {} to {}: {err}",
            snapshot_path.display(),
            relative_blob.display()
        )
    })
}

#[cfg(not(unix))]
fn create_silma_snapshot_link(
    _file: &SilmaModelFile,
    blob_path: &Path,
    snapshot_path: &Path,
) -> Result<(), String> {
    copy_or_hard_link_file(blob_path, snapshot_path)
}

/// Download one pinned SILMA file, resuming an interrupted partial file if possible.
fn download_silma_model_file(
    app: &tauri::AppHandle,
    model: &ModelDefinition,
    file: &SilmaModelFile,
    destination: &Path,
    completed_bytes: u64,
    total_bytes: u64,
) -> Result<(), String> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "Failed to create SILMA model file directory {}: {err}",
                parent.display()
            )
        })?;
    }
    let resume_from = resumable_file_offset(destination, file.bytes)?;
    if resume_from == file.bytes {
        emit_model_progress_total(
            app,
            model,
            "downloading",
            &format!("Downloaded {}", file.name),
            completed_bytes + resume_from,
            total_bytes,
        );
        return Ok(());
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(60 * 60))
        .user_agent("Papercut SILMA model installer")
        .build()
        .map_err(|err| format!("Failed to create SILMA model downloader: {err}"))?;
    let mut request = client.get(file.url);
    if resume_from > 0 {
        request = request.header(RANGE, format!("bytes={resume_from}-"));
    }
    let mut response = request
        .send()
        .map_err(|err| format!("Failed to download SILMA model file {}: {err}", file.name))?
        .error_for_status()
        .map_err(|err| format!("Failed to download SILMA model file {}: {err}", file.name))?;
    let appending = resume_from > 0 && response.status() == StatusCode::PARTIAL_CONTENT;
    let mut downloaded = if appending { resume_from } else { 0 };
    let file_handle = OpenOptions::new()
        .create(true)
        .write(true)
        .append(appending)
        .truncate(!appending)
        .open(destination)
        .map_err(|err| {
            format!(
                "Failed to create SILMA model file {}: {err}",
                destination.display()
            )
        })?;
    let mut writer = BufWriter::new(file_handle);
    let mut last_percent = download_percent(completed_bytes + downloaded, total_bytes);
    let mut buffer = [0u8; 256 * 1024];
    emit_model_progress_total(
        app,
        model,
        "downloading",
        &format!("Downloading {}", file.name),
        completed_bytes + downloaded,
        total_bytes,
    );
    loop {
        let read = response.read(&mut buffer).map_err(|err| {
            format!(
                "Failed while downloading SILMA model file {}: {err}",
                file.name
            )
        })?;
        if read == 0 {
            break;
        }
        writer.write_all(&buffer[..read]).map_err(|err| {
            format!(
                "Failed to write SILMA model file {}: {err}",
                destination.display()
            )
        })?;
        downloaded += read as u64;
        let percent = download_percent(completed_bytes + downloaded, total_bytes);
        if percent >= last_percent.saturating_add(2) || percent == 100 {
            last_percent = percent;
            emit_model_progress_total(
                app,
                model,
                "downloading",
                &format!("Downloading {}", file.name),
                completed_bytes + downloaded,
                total_bytes,
            );
        }
    }
    writer.flush().map_err(|err| {
        format!(
            "Failed to finish SILMA model file {}: {err}",
            destination.display()
        )
    })?;
    Ok(())
}

fn resumable_file_offset(path: &Path, expected_bytes: u64) -> Result<u64, String> {
    let Ok(metadata) = fs::metadata(path) else {
        return Ok(0);
    };
    let size = metadata.len();
    if size > expected_bytes {
        fs::remove_file(path)
            .map_err(|err| format!("Failed to remove oversized file {}: {err}", path.display()))?;
        return Ok(0);
    }
    Ok(size)
}

/// Hash a downloaded SILMA file before it is promoted into the active model dir.
fn verify_file_sha256(path: &Path, expected_sha256: &str, label: &str) -> Result<(), String> {
    let file = fs::File::open(path)
        .map_err(|err| format!("Failed to open downloaded SILMA model file {label}: {err}"))?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 256 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|err| format!("Failed to hash SILMA model file {label}: {err}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let actual = format!("{:x}", hasher.finalize());
    if actual == expected_sha256 {
        Ok(())
    } else {
        let _ = fs::remove_file(path);
        Err(format!(
            "SILMA model file {label} checksum mismatch. Expected {expected_sha256}, got {actual}"
        ))
    }
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

/// Emit progress for downloads whose size is not stored on `ModelDefinition`.
fn emit_model_progress_total(
    app: &tauri::AppHandle,
    model: &ModelDefinition,
    status: &str,
    message: &str,
    downloaded_bytes: u64,
    total_bytes: u64,
) {
    let _ = app.emit(
        MODEL_INSTALL_PROGRESS_EVENT,
        NativeTtsModelInstallProgress {
            model_id: model.id.into(),
            status: status.into(),
            message: message.into(),
            downloaded_bytes,
            total_bytes,
            percent: download_percent(downloaded_bytes, total_bytes),
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
