//! Fallback backend used when the native engine is not compiled.
//!
//! Every function mirrors the signature of its `engine` counterpart so
//! [`super::commands`] can swap backends with one `#[cfg]` switch. Capability
//! and status queries report "unavailable"; everything that would synthesize or
//! persist audio returns a build-hint error. The whole module is gated off in
//! native builds, so no per-function `#[cfg]` attributes are needed here.

use super::state::NativeTtsState;
use super::types::{
    NativeAudiobookChunkRequest, NativeAudiobookDeleteRequest, NativeAudiobookDeleteResponse,
    NativeAudiobookExportRequest, NativeAudiobookExportResponse, NativeAudiobookImportResponse,
    NativeAudiobookPlaybackRequest, NativeAudiobookPlaybackResponse, NativeAudiobookSaveRequest,
    NativeAudiobookSaveResponse, NativeAudiobookStatusRequest, NativeAudiobookStatusResponse,
    NativeImportedAudiobookMetadataResponse, NativeImportedAudiobookSourceRequest,
    NativeSavedAudiobookRecord, NativeSilmaSidecarProbeResponse, NativeTtsCapabilities,
    NativeTtsChunkResponse, NativeTtsModelInstallResponse, NativeTtsModelStatus,
};

const NOT_COMPILED: &str =
    "Native sherpa-onnx TTS was not compiled. Build Tauri with --features native-tts.";

pub(super) fn native_capabilities(_app: tauri::AppHandle) -> NativeTtsCapabilities {
    NativeTtsCapabilities {
        available: false,
        backend: "native-unavailable".into(),
        reason: NOT_COMPILED.into(),
        model_dir: None,
        platform: std::env::consts::OS.into(),
        compiled_execution_providers: Vec::new(),
        execution_provider_probe_error: None,
        default_thread_count: 1,
        max_thread_count: 1,
        models: Vec::new(),
    }
}

pub(super) fn model_status(
    _app: tauri::AppHandle,
    _state: tauri::State<'_, NativeTtsState>,
    model_id: String,
) -> NativeTtsModelStatus {
    NativeTtsModelStatus {
        model_id,
        installed: false,
        installing: false,
        install_supported: false,
        runtime_installed: false,
        model_dir: None,
        runtime_dir: None,
        source_url: String::new(),
        source_label: "sherpa-onnx offline TTS".into(),
        archive_bytes: 0,
        installed_bytes: 0,
        sha256: String::new(),
        message: NOT_COMPILED.into(),
        runtime_message: NOT_COMPILED.into(),
    }
}

pub(super) async fn install_model(
    _app: tauri::AppHandle,
    _state: tauri::State<'_, NativeTtsState>,
    _model_id: String,
) -> Result<NativeTtsModelInstallResponse, String> {
    Err(NOT_COMPILED.into())
}

pub(super) fn native_audiobook_status(
    _app: tauri::AppHandle,
    _request: NativeAudiobookStatusRequest,
) -> Result<NativeAudiobookStatusResponse, String> {
    Err(NOT_COMPILED.into())
}

pub(super) fn list_saved_audiobooks(
    _app: tauri::AppHandle,
) -> Result<Vec<NativeSavedAudiobookRecord>, String> {
    Ok(Vec::new())
}

pub(super) fn get_native_audiobook_chunk(
    _app: tauri::AppHandle,
    _request: NativeAudiobookChunkRequest,
) -> Result<NativeTtsChunkResponse, String> {
    Err(NOT_COMPILED.into())
}

pub(super) fn prepare_native_audiobook_playback(
    _app: tauri::AppHandle,
    _request: NativeAudiobookPlaybackRequest,
) -> Result<NativeAudiobookPlaybackResponse, String> {
    Err(NOT_COMPILED.into())
}

pub(super) async fn save_audiobook_native(
    _app: tauri::AppHandle,
    _state: tauri::State<'_, NativeTtsState>,
    _request: NativeAudiobookSaveRequest,
) -> Result<NativeAudiobookSaveResponse, String> {
    Err(NOT_COMPILED.into())
}

pub(super) fn cancel_audiobook_save(
    _state: tauri::State<'_, NativeTtsState>,
    _job_id: String,
) -> Result<(), String> {
    Ok(())
}

pub(super) fn export_audiobook_native(
    _app: tauri::AppHandle,
    _request: NativeAudiobookExportRequest,
) -> Result<NativeAudiobookExportResponse, String> {
    Err(NOT_COMPILED.into())
}

pub(super) fn import_audiobook_native(
    _app: tauri::AppHandle,
) -> Result<NativeAudiobookImportResponse, String> {
    Err(NOT_COMPILED.into())
}

pub(super) fn get_imported_audiobook_source(
    _app: tauri::AppHandle,
    _request: NativeImportedAudiobookSourceRequest,
) -> Result<String, String> {
    Err(NOT_COMPILED.into())
}

pub(super) fn get_imported_audiobook_metadata(
    _app: tauri::AppHandle,
    _request: NativeImportedAudiobookSourceRequest,
) -> Result<NativeImportedAudiobookMetadataResponse, String> {
    Err(NOT_COMPILED.into())
}

pub(super) fn delete_audiobook_native(
    _app: tauri::AppHandle,
    _request: NativeAudiobookDeleteRequest,
) -> Result<NativeAudiobookDeleteResponse, String> {
    Err(NOT_COMPILED.into())
}

pub(super) fn probe_silma_sidecar(
    _app: tauri::AppHandle,
) -> Result<NativeSilmaSidecarProbeResponse, String> {
    Err(NOT_COMPILED.into())
}
