//! Serde DTOs crossing the native-TTS Tauri boundary.
//!
//! This is the leaf of the module tree: it depends on nothing else in the
//! feature and is shared by `commands`, `engine`, and `stub` alike. Fields are
//! `pub(crate)` so the feature modules can construct/read them, but they stay
//! internal to the crate.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Stable command error code plus the original diagnostic detail.
pub(crate) struct NativeTtsCommandError {
    pub(crate) code: &'static str,
    pub(crate) message: String,
}

impl NativeTtsCommandError {
    pub(crate) fn new(message: String) -> Self {
        Self {
            code: native_tts_error_code(&message),
            message,
        }
    }
}

impl From<String> for NativeTtsCommandError {
    fn from(message: String) -> Self {
        Self::new(message)
    }
}

/// Classify only expected user-actionable failures; unknown details stay generic.
fn native_tts_error_code(message: &str) -> &'static str {
    let message = message.to_ascii_lowercase();
    if message.contains("cancelled") {
        return "operation-cancelled";
    }
    if message.contains("not compiled")
        || message.contains("currently supported on linux x64 only")
        || message.contains("not supported on this platform yet")
        || message.contains("native tts is not available")
    {
        return "native-tts-unavailable";
    }
    if message.contains("unsupported native tts model")
        || message.contains("is not supported by model")
    {
        return "unsupported-model";
    }
    if message.contains("already in progress") {
        return "operation-in-progress";
    }
    if message.contains("runtime pack is not installed")
        || message.contains("worker not found")
        || message.contains("missing worker executable")
    {
        return "runtime-not-installed";
    }
    if message.contains("offline voice model is not installed")
        || (message.starts_with("missing ") && message.contains("voice model"))
        || message.contains("required model files")
        || message.contains("missing required files")
    {
        return "model-not-installed";
    }
    if message.contains("no speakable audiobook chunks")
        || message.contains("audiobook with no chunks")
    {
        return "no-speakable-text";
    }
    if message.contains("does not match the current document chunks") {
        return "audiobook-cache-mismatch";
    }
    if message.contains("4 gb riff/wav limit") {
        return "wav-too-large";
    }
    if message.contains("audiobook bundle")
        && (message.contains("not a current")
            || message.contains("not a supported")
            || message.contains("did not contain")
            || message.contains("does not contain")
            || message.contains("invalid")
            || message.contains("ended unexpectedly")
            || message.contains("without a content type"))
    {
        return "invalid-audiobook-bundle";
    }
    "native-tts-failed"
}

/// IPC requests from older frontends retain historical identity processing.
fn default_text_preprocessor() -> String {
    "none".into()
}

fn default_audiobook_export_format() -> String {
    "bundle".into()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Whether native TTS is available, plus platform and thread defaults.
pub(crate) struct NativeTtsCapabilities {
    pub(crate) available: bool,
    pub(crate) backend: String,
    pub(crate) reason: String,
    pub(crate) model_dir: Option<String>,
    pub(crate) platform: String,
    pub(crate) default_thread_count: i32,
    pub(crate) max_thread_count: i32,
    pub(crate) models: Vec<NativeTtsModelInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeTtsModelInfo {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) family: String,
    pub(crate) language: String,
    pub(crate) language_label: String,
    pub(crate) default_voice: String,
    pub(crate) voices: Vec<NativeTtsVoiceInfo>,
    pub(crate) default_text_preprocessor: String,
    pub(crate) text_preprocessors: Vec<NativeTextPreprocessorInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeTtsVoiceInfo {
    pub(crate) id: String,
    pub(crate) name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeTextPreprocessorInfo {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) description: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Voice-model install state and the pinned source metadata.
pub(crate) struct NativeTtsModelStatus {
    pub(crate) model_id: String,
    pub(crate) installed: bool,
    pub(crate) installing: bool,
    pub(crate) install_supported: bool,
    pub(crate) runtime_installed: bool,
    pub(crate) model_dir: Option<String>,
    pub(crate) runtime_dir: Option<String>,
    pub(crate) source_url: String,
    pub(crate) source_label: String,
    pub(crate) archive_bytes: u64,
    pub(crate) installed_bytes: u64,
    pub(crate) sha256: String,
    pub(crate) message: String,
    pub(crate) runtime_message: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
/// Streamed progress while downloading/extracting the model.
#[cfg_attr(not(feature = "native-tts-core"), allow(dead_code))]
pub(crate) struct NativeTtsModelInstallProgress {
    pub(crate) model_id: String,
    pub(crate) status: String,
    pub(crate) message: String,
    pub(crate) downloaded_bytes: u64,
    pub(crate) total_bytes: u64,
    pub(crate) percent: u8,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Result of a completed model install (final dir + size).
pub(crate) struct NativeTtsModelInstallResponse {
    pub(crate) model_id: String,
    pub(crate) model_dir: String,
    pub(crate) bytes: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Dev/prototype health check for the desktop SILMA Python worker.
pub(crate) struct NativeSilmaSidecarProbeResponse {
    pub(crate) worker_path: String,
    pub(crate) python_command: String,
    pub(crate) probe_wav_path: String,
    pub(crate) health_version: String,
    pub(crate) sample_rate: i32,
    pub(crate) audio_duration_sec: f32,
    pub(crate) wav_bytes: usize,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
/// Runtime-only source locator for a chunk in the reader's readable DOM segments.
#[cfg_attr(not(feature = "native-tts-core"), allow(dead_code))]
pub(crate) struct NativeTtsChunkSourceSpan {
    pub(crate) start_segment_index: usize,
    pub(crate) start_offset: usize,
    pub(crate) end_segment_index: usize,
    pub(crate) end_offset: usize,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
/// One narration chunk: id/text identity plus optional hash and reader locator.
pub(crate) struct NativeTtsInputChunk {
    pub(crate) id: String,
    pub(crate) text: String,
    pub(crate) text_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) source_span: Option<NativeTtsChunkSourceSpan>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Lightweight status identity; full chunk data lives in the persisted manifest.
#[cfg_attr(not(feature = "native-tts-core"), allow(dead_code))]
pub(crate) struct NativeAudiobookStatusRequest {
    pub(crate) audiobook_id: String,
    pub(crate) source_signature: String,
    pub(crate) total_chunks: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Saved-chunk counts, totals, and the cache directory.
pub(crate) struct NativeAudiobookStatusResponse {
    pub(crate) cached_chunks: usize,
    pub(crate) total_chunks: usize,
    pub(crate) complete: bool,
    pub(crate) dir: String,
    pub(crate) audio_duration_sec: f32,
    pub(crate) wav_bytes: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// One completed audiobook discovered from its native on-disk manifest.
pub(crate) struct NativeSavedAudiobookRecord {
    pub(crate) id: String,
    pub(crate) document_url: String,
    pub(crate) title: String,
    pub(crate) voice: String,
    pub(crate) speed: f32,
    pub(crate) model_id: String,
    pub(crate) text_preprocessor: String,
    pub(crate) silma_nfe_step: Option<i32>,
    pub(crate) cache_version: String,
    pub(crate) dtype: String,
    pub(crate) saved_at: u64,
    pub(crate) chunks: usize,
    pub(crate) audio_duration_sec: f64,
    pub(crate) wav_bytes: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Request to read one saved chunk WAV from the cache.
#[cfg_attr(not(feature = "native-tts-core"), allow(dead_code))]
pub(crate) struct NativeAudiobookChunkRequest {
    pub(crate) audiobook_id: String,
    pub(crate) chunk: NativeTtsInputChunk,
    pub(crate) index: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Identify saved chunks to prepare without resending 1,000+ chunk texts over IPC.
#[cfg_attr(not(feature = "native-tts-core"), allow(dead_code))]
pub(crate) struct NativeAudiobookPlaybackRequest {
    pub(crate) audiobook_id: String,
    pub(crate) source_signature: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
/// One chunk position inside the stitched native playback track.
pub(crate) struct NativeAudiobookPlaybackChunk {
    pub(crate) index: usize,
    pub(crate) chunk_id: String,
    pub(crate) start_sec: f64,
    pub(crate) duration_sec: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Native playback source plus global chunk boundaries.
pub(crate) struct NativeAudiobookPlaybackResponse {
    pub(crate) audio_url: String,
    pub(crate) audio_duration_sec: f64,
    pub(crate) wav_bytes: usize,
    pub(crate) chunks: Vec<NativeAudiobookPlaybackChunk>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Parameters for a full-audiobook save/resume job.
#[cfg_attr(not(feature = "native-tts-core"), allow(dead_code))]
pub(crate) struct NativeAudiobookSaveRequest {
    pub(crate) job_id: String,
    pub(crate) audiobook_id: String,
    pub(crate) document_url: String,
    pub(crate) title: String,
    pub(crate) chunks: Vec<NativeTtsInputChunk>,
    pub(crate) model_id: String,
    #[serde(default = "default_text_preprocessor")]
    pub(crate) text_preprocessor: String,
    pub(crate) voice: String,
    pub(crate) speed: f32,
    pub(crate) thread_count: Option<i32>,
    pub(crate) silma_nfe_step: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Inputs for exporting a saved audiobook to a re-importable bundle or plain WAV.
#[cfg_attr(not(feature = "native-tts-core"), allow(dead_code))]
pub(crate) struct NativeAudiobookExportRequest {
    pub(crate) audiobook_id: String,
    pub(crate) document_url: String,
    pub(crate) title: String,
    pub(crate) source_html: Option<String>,
    pub(crate) chunks: Vec<NativeTtsInputChunk>,
    pub(crate) model_id: String,
    #[serde(default = "default_text_preprocessor")]
    pub(crate) text_preprocessor: String,
    pub(crate) voice: String,
    pub(crate) speed: f32,
    pub(crate) dtype: String,
    pub(crate) silma_nfe_step: Option<i32>,
    #[serde(default = "default_audiobook_export_format")]
    pub(crate) export_format: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Paths and totals describing a written audiobook export.
pub(crate) struct NativeAudiobookExportResponse {
    pub(crate) path: String,
    pub(crate) audio_path: String,
    pub(crate) metadata_path: String,
    pub(crate) html_path: String,
    pub(crate) chunks: usize,
    pub(crate) audio_duration_sec: f32,
    pub(crate) wav_bytes: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Lookup of an imported document's stored source HTML.
#[cfg_attr(not(feature = "native-tts-core"), allow(dead_code))]
pub(crate) struct NativeImportedAudiobookSourceRequest {
    pub(crate) document_url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Original bundle metadata used when replaying an imported audiobook.
pub(crate) struct NativeImportedAudiobookMetadataResponse {
    pub(crate) document_url: String,
    pub(crate) title: String,
    pub(crate) model_id: String,
    pub(crate) text_preprocessor: String,
    pub(crate) voice: String,
    pub(crate) speed: f32,
    pub(crate) dtype: String,
    pub(crate) silma_nfe_step: Option<i32>,
    pub(crate) chunks: Vec<NativeTtsInputChunk>,
    pub(crate) audio_duration_sec: f32,
    pub(crate) wav_bytes: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Metadata about an audiobook restored from a bundle.
pub(crate) struct NativeAudiobookImportResponse {
    pub(crate) document_url: String,
    pub(crate) title: String,
    pub(crate) model_id: String,
    pub(crate) text_preprocessor: String,
    pub(crate) voice: String,
    pub(crate) speed: f32,
    pub(crate) dtype: String,
    pub(crate) silma_nfe_step: Option<i32>,
    pub(crate) chunks: usize,
    pub(crate) audio_duration_sec: f32,
    pub(crate) wav_bytes: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Which saved audiobook (and optional upload) to delete.
#[cfg_attr(not(feature = "native-tts-core"), allow(dead_code))]
pub(crate) struct NativeAudiobookDeleteRequest {
    pub(crate) audiobook_id: String,
    pub(crate) document_url: String,
    pub(crate) delete_user_upload: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// What was deleted and how many bytes were freed.
pub(crate) struct NativeAudiobookDeleteResponse {
    pub(crate) deleted_audio: bool,
    pub(crate) deleted_user_upload: bool,
    pub(crate) bytes_freed: u64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
/// Per-chunk progress event emitted during a save.
#[cfg_attr(not(feature = "native-tts-core"), allow(dead_code))]
pub(crate) struct NativeAudiobookSaveProgress {
    pub(crate) job_id: String,
    pub(crate) status: String,
    pub(crate) message: String,
    pub(crate) cached_chunks: usize,
    pub(crate) total_chunks: usize,
    pub(crate) generated_chunks: usize,
    pub(crate) chunk_id: Option<String>,
    pub(crate) chunk_number: Option<usize>,
    pub(crate) text_chars: Option<usize>,
    pub(crate) text_preview: Option<String>,
    pub(crate) generate_ms: Option<u128>,
    pub(crate) preprocess_ms: Option<u128>,
    pub(crate) synthesis_ms: Option<u128>,
    pub(crate) write_ms: Option<u128>,
    pub(crate) validate_ms: Option<u128>,
    pub(crate) indexing_ms: Option<u128>,
    pub(crate) synthesis_text_chars: Option<usize>,
    pub(crate) total_source_chars: Option<usize>,
    pub(crate) total_synthesis_chars: Option<usize>,
    pub(crate) audio_duration_sec: Option<f32>,
    pub(crate) wav_bytes: Option<usize>,
    pub(crate) total_audio_duration_sec: f32,
    pub(crate) total_wav_bytes: usize,
    pub(crate) applied_thread_count: i32,
    pub(crate) backend: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Final totals returned when a save completes.
pub(crate) struct NativeAudiobookSaveResponse {
    pub(crate) job_id: String,
    pub(crate) cached_chunks: usize,
    pub(crate) total_chunks: usize,
    pub(crate) generated_chunks: usize,
    pub(crate) complete: bool,
    pub(crate) dir: String,
    pub(crate) generate_ms: u128,
    // Canonical total measured from WAV headers in `build_playback_index`, so the
    // reported duration matches the persisted manifest instead of drifting from a
    // per-chunk f32 accumulation over a long book.
    pub(crate) audio_duration_sec: f64,
    pub(crate) wav_bytes: usize,
    pub(crate) applied_thread_count: i32,
    pub(crate) backend: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// A synthesized or cached chunk returned as a base64 WAV.
pub(crate) struct NativeTtsChunkResponse {
    pub(crate) chunk_id: Option<String>,
    pub(crate) wav_base64: String,
    pub(crate) sample_rate: i32,
    pub(crate) audio_duration_sec: f32,
    pub(crate) wav_bytes: usize,
    pub(crate) generate_ms: u128,
    pub(crate) backend: String,
}

#[cfg(test)]
mod tests {
    use super::native_tts_error_code;

    #[test]
    fn expected_native_tts_errors_have_stable_codes() {
        assert_eq!(
            native_tts_error_code("Offline voice model is not installed"),
            "model-not-installed"
        );
        assert_eq!(
            native_tts_error_code("Audiobook export cancelled"),
            "operation-cancelled"
        );
        assert_eq!(
            native_tts_error_code("Selected file is not a supported Papercut audiobook bundle"),
            "invalid-audiobook-bundle"
        );
        assert_eq!(
            native_tts_error_code("unexpected library detail"),
            "native-tts-failed"
        );
    }
}
