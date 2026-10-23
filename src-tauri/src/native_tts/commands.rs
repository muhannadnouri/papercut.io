//! The `#[tauri::command]` edge exposed to the frontend.
//!
//! Each command is a thin wrapper that marshals arguments and delegates to the
//! active backend. A single `#[cfg]` switch picks `engine` (real synthesis) or
//! `stub` (native TTS not compiled); both expose the same function names and
//! signatures, so the command bodies below are identical across build configs.
//! Commands that wrap a blocking backend call move that work onto the blocking
//! thread pool so the async runtime is never stalled by filesystem or native
//! inference I/O.

use super::state::NativeTtsState;
use super::types::{
    NativeAudiobookChunkRequest, NativeAudiobookDeleteRequest, NativeAudiobookDeleteResponse,
    NativeAudiobookExportRequest, NativeAudiobookExportResponse, NativeAudiobookImportResponse,
    NativeAudiobookPlaybackRequest, NativeAudiobookPlaybackResponse, NativeAudiobookSaveRequest,
    NativeAudiobookSaveResponse, NativeAudiobookStatusRequest, NativeAudiobookStatusResponse,
    NativeImportedAudiobookMetadataResponse, NativeImportedAudiobookSourceRequest,
    NativeSavedAudiobookRecord, NativeSilmaSidecarProbeResponse, NativeTtsCapabilities,
    NativeTtsChunkResponse, NativeTtsCommandError, NativeTtsModelInstallResponse,
    NativeTtsModelStatus, NativeTtsPreviewRequest,
};

#[cfg(feature = "native-tts-core")]
use super::engine::{
    cancel_audiobook_save, delete_audiobook_native, export_audiobook_native,
    get_imported_audiobook_metadata, get_imported_audiobook_source, get_native_audiobook_chunk,
    import_audiobook_native, install_model, list_saved_audiobooks, model_status,
    native_audiobook_status, native_capabilities, prepare_native_audiobook_playback, preview_voice,
    probe_silma_sidecar, save_audiobook_native,
};

#[cfg(not(feature = "native-tts-core"))]
use super::stub::{
    cancel_audiobook_save, delete_audiobook_native, export_audiobook_native,
    get_imported_audiobook_metadata, get_imported_audiobook_source, get_native_audiobook_chunk,
    import_audiobook_native, install_model, list_saved_audiobooks, model_status,
    native_audiobook_status, native_capabilities, prepare_native_audiobook_playback, preview_voice,
    probe_silma_sidecar, save_audiobook_native,
};

/// Is native TTS usable on this build/device, and is the voice model installed?
#[tauri::command]
pub fn tts_native_capabilities(app: tauri::AppHandle) -> NativeTtsCapabilities {
    native_capabilities(app)
}

/// Voice-model install state (installed?/installing?, size, source metadata).
#[tauri::command]
pub fn tts_model_status(
    app: tauri::AppHandle,
    state: tauri::State<'_, NativeTtsState>,
    model_id: String,
) -> NativeTtsModelStatus {
    model_status(app, state, model_id)
}

/// Download + install the pinned voice model (one-time, idempotent).
#[tauri::command]
pub async fn tts_install_model(
    app: tauri::AppHandle,
    state: tauri::State<'_, NativeTtsState>,
    model_id: String,
) -> Result<NativeTtsModelInstallResponse, NativeTtsCommandError> {
    install_model(app, state, model_id)
        .await
        .map_err(Into::into)
}

/// How many of a document's chunks are already saved (for the "Saved" UI).
#[tauri::command]
pub async fn tts_native_audiobook_status(
    app: tauri::AppHandle,
    request: NativeAudiobookStatusRequest,
) -> Result<NativeAudiobookStatusResponse, NativeTtsCommandError> {
    run_audiobook_read("tts_native_audiobook_status", move || {
        native_audiobook_status(app, request)
    })
    .await
}

/// Return the completed-audiobook registry derived from native manifests.
#[tauri::command]
pub async fn tts_list_saved_audiobooks(
    app: tauri::AppHandle,
) -> Result<Vec<NativeSavedAudiobookRecord>, NativeTtsCommandError> {
    run_audiobook_read("tts_list_saved_audiobooks", move || {
        list_saved_audiobooks(app)
    })
    .await
}

/// Read one already-saved chunk WAV from the cache (returns a base64 WAV).
#[tauri::command]
pub async fn tts_get_native_audiobook_chunk(
    app: tauri::AppHandle,
    request: NativeAudiobookChunkRequest,
) -> Result<NativeTtsChunkResponse, NativeTtsCommandError> {
    run_audiobook_read("tts_get_native_audiobook_chunk", move || {
        get_native_audiobook_chunk(app, request)
    })
    .await
}

/// Generate a short disposable preview with the selected model and voice.
#[tauri::command]
pub async fn tts_preview_voice(
    app: tauri::AppHandle,
    state: tauri::State<'_, NativeTtsState>,
    request: NativeTtsPreviewRequest,
) -> Result<NativeTtsChunkResponse, NativeTtsCommandError> {
    preview_voice(app, state, request).await.map_err(Into::into)
}

/// Prepare/reuse one seekable native track and return global chunk boundaries.
/// Heavy file work runs off Tauri's async command thread.
#[tauri::command]
pub async fn tts_prepare_native_audiobook_playback(
    app: tauri::AppHandle,
    request: NativeAudiobookPlaybackRequest,
) -> Result<NativeAudiobookPlaybackResponse, NativeTtsCommandError> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        prepare_native_audiobook_playback(app, request)
    })
    .await
    .map_err(|err| {
        NativeTtsCommandError::new(format!(
            "Native audiobook playback preparation failed: {err}"
        ))
    })?;
    result.map_err(Into::into)
}

/// Start (or resume) a full-audiobook save; streams progress events as it runs.
#[tauri::command]
pub async fn tts_save_audiobook_native(
    app: tauri::AppHandle,
    state: tauri::State<'_, NativeTtsState>,
    request: NativeAudiobookSaveRequest,
) -> Result<NativeAudiobookSaveResponse, NativeTtsCommandError> {
    save_audiobook_native(app, state, request)
        .await
        .map_err(Into::into)
}

/// Request cancellation of an in-progress save by job id.
#[tauri::command]
pub fn tts_cancel_audiobook_save(
    state: tauri::State<'_, NativeTtsState>,
    job_id: String,
) -> Result<(), NativeTtsCommandError> {
    cancel_audiobook_save(state, job_id).map_err(Into::into)
}

/// Export a saved audiobook to a single `.papercut-audiobook` bundle file.
///
/// Wrapped in `spawn_blocking` because the export does synchronous file I/O;
/// this keeps the async UI thread responsive while it runs.
#[tauri::command]
pub async fn tts_export_audiobook_native(
    app: tauri::AppHandle,
    request: NativeAudiobookExportRequest,
) -> Result<NativeAudiobookExportResponse, NativeTtsCommandError> {
    let result =
        tauri::async_runtime::spawn_blocking(move || export_audiobook_native(app, request))
            .await
            .map_err(|err| {
                NativeTtsCommandError::new(format!("Native audiobook export task failed: {err}"))
            })?;
    result.map_err(Into::into)
}

/// Import a `.papercut-audiobook` bundle and restore it into app data.
#[tauri::command]
pub async fn tts_import_audiobook_native(
    app: tauri::AppHandle,
) -> Result<NativeAudiobookImportResponse, NativeTtsCommandError> {
    let result = tauri::async_runtime::spawn_blocking(move || import_audiobook_native(app))
        .await
        .map_err(|err| {
            NativeTtsCommandError::new(format!("Native audiobook import task failed: {err}"))
        })?;
    result.map_err(Into::into)
}

/// Read the stored source HTML of an imported audiobook document.
#[tauri::command]
pub async fn tts_get_imported_audiobook_source(
    app: tauri::AppHandle,
    request: NativeImportedAudiobookSourceRequest,
) -> Result<String, NativeTtsCommandError> {
    run_audiobook_read("tts_get_imported_audiobook_source", move || {
        get_imported_audiobook_source(app, request)
    })
    .await
}

/// Read the original bundle chunks/options for an imported audiobook document.
#[tauri::command]
pub async fn tts_get_imported_audiobook_metadata(
    app: tauri::AppHandle,
    request: NativeImportedAudiobookSourceRequest,
) -> Result<NativeImportedAudiobookMetadataResponse, NativeTtsCommandError> {
    run_audiobook_read("tts_get_imported_audiobook_metadata", move || {
        get_imported_audiobook_metadata(app, request)
    })
    .await
}

/// Delete a saved audiobook's audio (and optionally its imported source).
#[tauri::command]
pub async fn tts_delete_audiobook_native(
    app: tauri::AppHandle,
    request: NativeAudiobookDeleteRequest,
) -> Result<NativeAudiobookDeleteResponse, NativeTtsCommandError> {
    let result =
        tauri::async_runtime::spawn_blocking(move || delete_audiobook_native(app, request))
            .await
            .map_err(|err| {
                NativeTtsCommandError::new(format!("Native audiobook delete task failed: {err}"))
            })?;
    result.map_err(Into::into)
}

/// Dev/prototype probe for Stage 1 SILMA sidecar mechanics.
#[tauri::command]
pub async fn tts_probe_silma_sidecar(
    app: tauri::AppHandle,
) -> Result<NativeSilmaSidecarProbeResponse, NativeTtsCommandError> {
    let result = tauri::async_runtime::spawn_blocking(move || probe_silma_sidecar(app))
        .await
        .map_err(|err| {
            NativeTtsCommandError::new(format!("SILMA sidecar probe task failed: {err}"))
        })?;
    result.map_err(Into::into)
}

/// Disk reads and WAV encoding must not run on either the UI or async executor thread.
/// Keep timing numeric: never log book paths, request contents or audio payloads.
async fn run_audiobook_read<T: Send + 'static>(
    operation: &'static str,
    read: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, NativeTtsCommandError> {
    let queued = std::time::Instant::now();
    tauri::async_runtime::spawn_blocking(move || {
        let started = std::time::Instant::now();
        let result = read();
        log::info!(
            "[tts-native-read] operation={operation} queue_ms={} work_ms={} ok={}",
            started.duration_since(queued).as_millis(),
            started.elapsed().as_millis(),
            result.is_ok(),
        );
        result.map_err(Into::into)
    })
    .await
    .map_err(|err| {
        NativeTtsCommandError::new(format!("Native audiobook read task failed: {err}"))
    })?
}

#[cfg(test)]
mod read_tests {
    use super::run_audiobook_read;

    #[test]
    fn disk_work_runs_off_the_calling_thread() {
        let caller = std::thread::current().id();
        let worker = tauri::async_runtime::block_on(run_audiobook_read("test", || {
            Ok(std::thread::current().id())
        }))
        .unwrap();
        assert_ne!(caller, worker);
    }

    #[test]
    fn backend_errors_keep_their_original_message() {
        let error = tauri::async_runtime::block_on(run_audiobook_read::<()>("test", || {
            Err("missing saved chunk".into())
        }))
        .unwrap_err();
        assert_eq!(error.message, "missing saved chunk");
        assert_eq!(error.code, "native-tts-failed");
    }

    #[test]
    fn worker_panics_become_command_errors() {
        let error = tauri::async_runtime::block_on(run_audiobook_read::<()>("test", || {
            panic!("test worker failure")
        }))
        .unwrap_err();
        assert!(error.message.contains("Native audiobook read task failed:"));
    }
}
