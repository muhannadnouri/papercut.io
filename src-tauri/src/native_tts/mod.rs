//! Native sherpa-onnx offline TTS feature.
//!
//! Splits the synthesis / audiobook-save backend into focused submodules so each
//! concern can grow independently. Dependencies only point downward:
//!
//! ```text
//! commands -> { engine | stub } -> { platform, types }
//! engine: model | synth | save | cache | bundle -> { paths, config } -> types
//! state  -> engine (LoadedTtsEngine slot)
//! ```
//!
//! - [`commands`]: the thin `#[tauri::command]` edge; dispatches to `engine` or
//!   `stub` via one `#[cfg]` switch so both build configs share identical bodies.
//! - [`engine`]: the real implementation, compiled only with `native-tts-core`.
//! - `stub`: the "native TTS not compiled" fallbacks, same signatures as `engine`.
//! - [`platform`]: the OS-specific tuning seam (thread counts, future providers).
//! - [`state`]: the Tauri-managed [`NativeTtsState`].
//! - [`types`]: serde DTOs shared across the boundary (leaf module).

// `commands` is `pub(crate)` so `generate_handler!` in `lib.rs` can reach both
// each command and the hidden `__cmd__*` helper the macro generates beside it.
pub(crate) mod commands;
mod state;
mod types;

#[cfg(feature = "native-tts-core")]
mod engine;
#[cfg(feature = "native-tts-core")]
mod platform;
#[cfg(not(feature = "native-tts-core"))]
mod stub;

pub use state::NativeTtsState;

#[cfg(feature = "native-tts-core")]
pub(crate) use engine::{
    audiobooks_dir, imported_upload_dir, imported_upload_id_from_document_url,
    list_audiobook_transfer_payloads, validate_transferred_audiobook,
};
#[cfg(not(feature = "native-tts-core"))]
pub(crate) use types::NativeAudiobookTransferPayload;
pub(crate) use types::NativeSavedAudiobookRecord;

#[cfg(not(feature = "native-tts-core"))]
pub(crate) fn list_audiobook_transfer_payloads(
    _app: &tauri::AppHandle,
) -> Result<Vec<NativeAudiobookTransferPayload>, String> {
    Ok(Vec::new())
}

#[cfg(not(feature = "native-tts-core"))]
pub(crate) fn validate_transferred_audiobook(
    _dir: &std::path::Path,
    _expected_id: &str,
) -> Result<NativeSavedAudiobookRecord, String> {
    Err("Native audiobook support is not available in this build".into())
}

#[cfg(not(feature = "native-tts-core"))]
pub(crate) fn audiobooks_dir(_app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Err("Native audiobook support is not available in this build".into())
}

#[cfg(not(feature = "native-tts-core"))]
pub(crate) fn imported_upload_id_from_document_url(_document_url: &str) -> Result<String, String> {
    Err("Native audiobook support is not available in this build".into())
}

#[cfg(not(feature = "native-tts-core"))]
pub(crate) fn imported_upload_dir(
    _app: &tauri::AppHandle,
    _upload_id: &str,
) -> Result<std::path::PathBuf, String> {
    Err("Native audiobook support is not available in this build".into())
}
