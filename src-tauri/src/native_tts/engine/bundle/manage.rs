//! Housekeeping for imported/saved audiobooks: read source HTML and delete.
//!
//! These are the small, non-streaming operations that don't belong to the
//! export or import pipelines but still act on the same on-disk layout.

use std::fs;

use serde::Deserialize;

use super::super::models::{DEFAULT_MODEL_ID, TEXT_PREPROCESSOR_NONE};
use super::super::paths::{
    audiobook_dir, imported_upload_dir, imported_upload_id_from_document_url,
    remove_dir_and_count_bytes,
};
use crate::document_uploads::sanitize_html;
use crate::native_tts::types::{
    NativeAudiobookDeleteRequest, NativeAudiobookDeleteResponse,
    NativeImportedAudiobookMetadataResponse, NativeImportedAudiobookSourceRequest,
    NativeTtsInputChunk,
};

/// Default old Kokoro-only export metadata to Kokoro's catalog id.
fn default_model_id() -> String {
    DEFAULT_MODEL_ID.into()
}

/// Default pre-diacritization bundle metadata to original source text.
fn default_text_preprocessor() -> String {
    TEXT_PREPROCESSOR_NONE.into()
}

/// Minimal shape of the `metadata.json` copied from an audiobook bundle.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportedAudiobookMetadataFile {
    title: String,
    voice: String,
    speed: f32,
    dtype: String,
    silma_nfe_step: Option<i32>,
    #[serde(default = "default_model_id")]
    model_id: String,
    #[serde(default = "default_text_preprocessor")]
    text_preprocessor: String,
    chunks: Vec<NativeTtsInputChunk>,
    audio: Option<ImportedAudiobookAudioMetadata>,
}

/// Optional aggregate audio totals from the bundle sidecar.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImportedAudiobookAudioMetadata {
    duration_sec: f32,
    bytes: usize,
}

/// Return the stored `source.html` for an imported audiobook document.
///
/// Imported documents have URLs like `/user-uploads/<id>.html`; this extracts
/// the id, locates the upload directory, and reads the HTML back as a String
/// for the reader UI.
pub(crate) fn get_imported_audiobook_source(
    app: tauri::AppHandle,
    request: NativeImportedAudiobookSourceRequest,
) -> Result<String, String> {
    let upload_id = imported_upload_id_from_document_url(&request.document_url)?;
    let path = imported_upload_dir(&app, &upload_id)?.join("source.html");
    let source = fs::read_to_string(&path).map_err(|err| {
        format!(
            "Failed to read imported audiobook source {}: {err}",
            path.display()
        )
    })?;
    // Imported bundles can outlive the sanitizer version that first accepted
    // them, so the reader receives content checked against today's allowlist.
    Ok(sanitize_html(&source))
}

/// Return original bundle chunks/options for an imported audiobook document.
///
/// Playback/status use these chunks as the authority for imported bundles because
/// they are the exact chunk ids/text hashes that the restored WAV files were
/// generated from. Re-chunking the source HTML with newer app code can legitimately
/// produce different boundaries, especially for legacy bundles.
pub(crate) fn get_imported_audiobook_metadata(
    app: tauri::AppHandle,
    request: NativeImportedAudiobookSourceRequest,
) -> Result<NativeImportedAudiobookMetadataResponse, String> {
    let upload_id = imported_upload_id_from_document_url(&request.document_url)?;
    let path = imported_upload_dir(&app, &upload_id)?.join("metadata.json");
    let bytes = fs::read(&path).map_err(|err| {
        format!(
            "Failed to read imported audiobook metadata {}: {err}",
            path.display()
        )
    })?;
    let metadata =
        serde_json::from_slice::<ImportedAudiobookMetadataFile>(&bytes).map_err(|err| {
            format!(
                "Failed to parse imported audiobook metadata {}: {err}",
                path.display()
            )
        })?;
    let audio = metadata.audio.unwrap_or(ImportedAudiobookAudioMetadata {
        duration_sec: 0.0,
        bytes: 0,
    });

    Ok(NativeImportedAudiobookMetadataResponse {
        document_url: request.document_url,
        title: metadata.title,
        model_id: metadata.model_id,
        text_preprocessor: metadata.text_preprocessor,
        voice: metadata.voice,
        speed: metadata.speed,
        dtype: metadata.dtype,
        silma_nfe_step: metadata.silma_nfe_step,
        chunks: metadata
            .chunks
            .into_iter()
            .filter(|chunk| !chunk.text.trim().is_empty())
            .collect(),
        audio_duration_sec: audio.duration_sec,
        wav_bytes: audio.bytes,
    })
}

/// Delete a saved audiobook's audio cache, and optionally its imported source.
///
/// Always removes the per-audiobook audio directory. When `delete_user_upload`
/// is set and the document is an import, also removes the stored source upload.
/// Returns what was deleted and how many bytes were reclaimed.
///
/// Rust note: `remove_dir_and_count_bytes` returns a `(bool, u64)` tuple
/// (deleted?, bytes); `result.0` / `result.1` access those positional fields.
pub(crate) fn delete_audiobook_native(
    app: tauri::AppHandle,
    request: NativeAudiobookDeleteRequest,
) -> Result<NativeAudiobookDeleteResponse, String> {
    let audio_dir = audiobook_dir(&app, &request.audiobook_id)?;
    let (deleted_audio, audio_bytes) =
        remove_dir_and_count_bytes(&audio_dir, "audiobook audio cache")?;

    let mut deleted_user_upload = false;
    let mut upload_bytes = 0u64;
    if request.delete_user_upload {
        // Only imported documents have a deletable upload dir; a non-import URL
        // simply fails the id parse and is skipped.
        if let Ok(upload_id) = imported_upload_id_from_document_url(&request.document_url) {
            let upload_dir = imported_upload_dir(&app, &upload_id)?;
            let result = remove_dir_and_count_bytes(&upload_dir, "imported audiobook source")?;
            deleted_user_upload = result.0;
            upload_bytes = result.1;
        }
    }

    Ok(NativeAudiobookDeleteResponse {
        deleted_audio,
        deleted_user_upload,
        bytes_freed: audio_bytes + upload_bytes,
    })
}
