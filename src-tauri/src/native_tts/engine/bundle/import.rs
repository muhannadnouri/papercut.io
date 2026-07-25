//! Import a `.papercut-audiobook` bundle and restore it into app data.
//!
//! Flow: read + validate the manifest, then walk its file entries in payload
//! order, copying the source HTML and each chunk WAV out of the bundle and into
//! the app's user-uploads / audiobook-cache directories. Finally write a save
//! manifest so the restored audiobook is indistinguishable from one generated
//! locally. The combined single-track WAV is restored as the native playback cache,
//! while per-chunk files remain the canonical editable/exportable audio.
//!
//! Rust notes: structs that `#[derive(Deserialize)]` can be built directly from
//! JSON by `serde_json`. `#[serde(rename_all = "camelCase")]` maps the JSON's
//! `camelCase` keys onto Rust's `snake_case` field names automatically.

use std::fs;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_fs::{FsExt, OpenOptions};

use super::super::config::{BUNDLE_MAGIC, CACHE_VERSION};
use super::super::file_commit::commit_staged_file;
use super::super::manifest::write_manifest;
use super::super::models::model_definition;
use super::super::paths::{
    audiobook_dir, chunk_path, create_native_audiobook_id, imported_upload_dir,
    playback_track_path, speakable_chunks, stable_hex_hash,
};
use super::super::silma_sidecar::DEFAULT_SILMA_NFE_STEP;
use crate::document_uploads::{restore_audiobook_pdf, sanitize_html};
use crate::native_tts::types::{
    NativeAudiobookImportResponse, NativeAudiobookSaveRequest, NativeTtsInputChunk,
};

const MAX_BUNDLE_SOURCE_HTML_BYTES: u64 = 256 * 1024 * 1024;
const MAX_BUNDLE_SOURCE_PDF_BYTES: u64 = 250 * 1024 * 1024;

/// Version-2 bundles predating preprocessing represent original, undiacritized text.
fn default_text_preprocessor() -> String {
    "none".into()
}

fn default_source_kind() -> String {
    "html".into()
}

/// The bundle's top-level JSON manifest, parsed from the header.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeAudiobookBundleManifest {
    version: u32,
    kind: String,
    #[serde(default = "default_source_kind")]
    source_kind: String,
    source_document_url: String,
    title: String,
    voice: String,
    speed: f32,
    dtype: String,
    silma_nfe_step: Option<i32>,
    model_id: String,
    #[serde(default = "default_text_preprocessor")]
    text_preprocessor: String,
    cache_version: String,
    chunks: Vec<NativeTtsInputChunk>,
    files: Vec<NativeAudiobookBundleFile>,
    audio: NativeAudiobookBundleAudio,
}

/// One packed file inside the bundle: where its bytes live and what it is.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeAudiobookBundleFile {
    path: String,
    role: String,
    content_type: String,
    bytes: u64,
    payload_offset: u64,
    chunk_index: Option<usize>,
}

/// Summary of the combined audio track (used only for the response totals).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeAudiobookBundleAudio {
    format: String,
    single_track: bool,
    duration_sec: f32,
    bytes: usize,
}

/// Top-level import entry point.
///
/// Prompts for a bundle file, reads/validates its manifest, derives a stable
/// upload id (so re-importing the same content reuses the same slot), restores
/// the payloads, and writes a save manifest. Returns metadata for the UI.
pub(crate) fn import_audiobook_native(
    app: tauri::AppHandle,
) -> Result<NativeAudiobookImportResponse, String> {
    // Native "Open File" dialog; None means the user cancelled.
    let source = app
        .dialog()
        .file()
        .set_title("Import Audiobook Bundle")
        .add_filter("Papercut Audiobook", &["papercut-audiobook"])
        .blocking_pick_file()
        .ok_or_else(|| "Audiobook import cancelled".to_string())?;

    let mut options = OpenOptions::new();
    options.read(true);
    let file = app
        .fs()
        .open(source, options)
        .map_err(|err| format!("Failed to open selected audiobook bundle: {err}"))?;
    // Wrap the file in a BufReader so we can read the header, then stream
    // payloads sequentially from the same cursor position.
    let mut reader = BufReader::new(file);
    let manifest = read_bundle_manifest(&mut reader)?;
    validate_bundle_manifest(&manifest)?;

    // HTML bundles retain their audiobook-owned virtual upload. PDF bundles
    // restore their canonical content-addressed URL into the normal library.
    let upload_dir = if manifest.source_kind == "html" {
        let upload_id = imported_upload_id(&manifest);
        let dir = imported_upload_dir(&app, &upload_id)?;
        fs::create_dir_all(&dir).map_err(|err| {
            format!(
                "Failed to create imported audiobook directory {}: {err}",
                dir.display()
            )
        })?;
        Some((upload_id, dir))
    } else {
        None
    };
    let document_url = upload_dir
        .as_ref()
        .map(|(upload_id, _)| format!("/user-uploads/{upload_id}.html"))
        .unwrap_or_else(|| manifest.source_document_url.clone());

    let mut audiobook_id = create_native_audiobook_id(
        &manifest.model_id,
        &document_url,
        &manifest.voice,
        manifest.speed,
        &manifest.dtype,
        &manifest.text_preprocessor,
    );
    if manifest.model_id == "silma-ai/silma-tts"
        && manifest.silma_nfe_step != Some(DEFAULT_SILMA_NFE_STEP)
    {
        if let Some(step) = manifest.silma_nfe_step {
            audiobook_id = add_silma_nfe_to_audiobook_id(&audiobook_id, step);
        }
    }
    let audiobook_dir = audiobook_dir(&app, &audiobook_id)?;
    fs::create_dir_all(audiobook_dir.join("chunks")).map_err(|err| {
        format!(
            "Failed to create imported audiobook cache {}: {err}",
            audiobook_dir.display()
        )
    })?;

    // Walk entries in ascending payload offset so the single read cursor only
    // ever moves forward. `consumed` tracks how many payload bytes we've passed.
    let mut entries = manifest.files.iter().collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.payload_offset);
    let mut consumed = 0u64;
    let mut imported_chunks = 0usize;
    let mut imported_source = false;
    let mut imported_pdf_source = None;
    let mut imported_metadata = false;
    let mut imported_track = false;
    // Keep the imported single track staged until the source document, every
    // canonical chunk, and the new manifest validate. Failed imports cannot
    // replace a working track.
    let imported_track_staging = audiobook_dir.join("playback.import.wav");
    let _ = fs::remove_file(&imported_track_staging);

    for entry in entries {
        // Offsets must be monotonically increasing; a backward offset means a
        // corrupt/hostile bundle we can't stream.
        if entry.payload_offset < consumed {
            return Err(format!(
                "Audiobook bundle entry {} overlaps an earlier payload",
                entry.path
            ));
        }
        // Skip any gap between the cursor and this entry's start.
        if entry.payload_offset > consumed {
            skip_payload(&mut reader, entry.payload_offset - consumed)?;
            consumed = entry.payload_offset;
        }

        // Restore by role. `match` is like a switch but exhaustive; the final
        // `_` arm skips unknown optional file kinds.
        match entry.role.as_str() {
            "sourceHtml" => {
                let upload_dir = upload_dir.as_ref().ok_or_else(|| {
                    "PDF audiobook bundle declared an HTML source payload".to_string()
                })?;
                copy_sanitized_html_payload_to_path(
                    &mut reader,
                    &upload_dir.1.join("source.html"),
                    entry.bytes,
                )?;
                imported_source = true;
            }
            "sourcePdf" => {
                imported_pdf_source = Some(read_pdf_payload(&mut reader, entry.bytes)?);
                imported_source = true;
            }
            "metadata" => {
                if let Some((_, upload_dir)) = &upload_dir {
                    copy_payload_to_path(
                        &mut reader,
                        &upload_dir.join("metadata.json"),
                        entry.bytes,
                    )?;
                } else {
                    skip_payload(&mut reader, entry.bytes)?;
                }
                imported_metadata = true;
            }
            "chunkWav" => {
                let index = entry.chunk_index.ok_or_else(|| {
                    format!(
                        "Audiobook bundle chunk entry {} is missing chunkIndex",
                        entry.path
                    )
                })?;
                let chunk = manifest.chunks.get(index).ok_or_else(|| {
                    format!(
                        "Audiobook bundle chunk entry {} has invalid chunkIndex",
                        entry.path
                    )
                })?;
                let target = chunk_path(&audiobook_dir, index, chunk);
                copy_payload_to_path(&mut reader, &target, entry.bytes)?;
                imported_chunks += 1;
            }
            "singleTrackWav" => {
                copy_payload_to_path(&mut reader, &imported_track_staging, entry.bytes)?;
                imported_track = true;
            }
            _ => {
                skip_payload(&mut reader, entry.bytes)?;
            }
        }
        consumed += entry.bytes;
    }

    // Validate we got everything a usable audiobook needs.
    let speakable = speakable_chunks(&manifest.chunks);
    if !imported_source {
        return Err("Audiobook bundle did not contain its source document".into());
    }
    if imported_chunks != speakable.len() {
        return Err(format!(
            "Audiobook bundle restored {imported_chunks}/{} audio chunks",
            speakable.len()
        ));
    }
    if let Some(source) = imported_pdf_source {
        restore_audiobook_pdf(&app, &document_url, manifest.title.clone(), source)?;
    }
    if !imported_metadata {
        if let Some((_, upload_dir)) = &upload_dir {
            let _ = fs::write(upload_dir.join("metadata.json"), b"{}" as &[u8]);
        }
    }

    // Write the same manifest a local save would, so playback treats this
    // imported audiobook exactly like a generated one.
    let save_request = NativeAudiobookSaveRequest {
        job_id: "import".into(),
        audiobook_id,
        document_url: document_url.clone(),
        title: manifest.title.clone(),
        model_id: manifest.model_id.clone(),
        text_preprocessor: manifest.text_preprocessor.clone(),
        chunks: manifest.chunks.clone(),
        voice: manifest.voice.clone(),
        speed: manifest.speed,
        thread_count: None,
        silma_nfe_step: manifest.silma_nfe_step,
    };
    write_manifest(&audiobook_dir, &save_request, &speakable, 0)?;
    // write_manifest invalidates old derived playback files. Commit staged bundle
    // track afterward so first mobile Play can rebuild only its tiny sidecar.
    if imported_track {
        let track_path = playback_track_path(&audiobook_dir);
        commit_staged_file(
            &imported_track_staging,
            &track_path,
            "imported playback track",
        )?;
    }

    Ok(NativeAudiobookImportResponse {
        document_url,
        source_kind: manifest.source_kind,
        title: manifest.title,
        model_id: manifest.model_id,
        text_preprocessor: manifest.text_preprocessor,
        voice: manifest.voice,
        speed: manifest.speed,
        dtype: manifest.dtype,
        silma_nfe_step: manifest.silma_nfe_step,
        chunks: speakable.len(),
        audio_duration_sec: manifest.audio.duration_sec,
        wav_bytes: manifest.audio.bytes,
    })
}

/// Insert SILMA's NFE cache segment before the normalized document URL.
fn add_silma_nfe_to_audiobook_id(audiobook_id: &str, step: i32) -> String {
    match audiobook_id.rsplit_once('|') {
        Some((prefix, document_url)) => format!("{prefix}|nfe{step}|{document_url}"),
        None => audiobook_id.to_string(),
    }
}

/// Read and JSON-parse the bundle header from the front of `reader`.
///
/// Checks the magic bytes, reads the u64 little-endian manifest length, guards
/// it against an absurd size, then reads exactly that many bytes and parses
/// them. `R: Read` makes this generic over any byte source. After it returns,
/// the reader sits exactly at the first payload byte.
fn read_bundle_manifest<R: Read>(reader: &mut R) -> Result<NativeAudiobookBundleManifest, String> {
    let mut magic = vec![0u8; BUNDLE_MAGIC.len()];
    reader
        .read_exact(&mut magic)
        .map_err(|err| format!("Failed to read audiobook bundle header: {err}"))?;
    if magic != BUNDLE_MAGIC {
        return Err("Selected file is not a current Papercut audiobook bundle".into());
    }

    let mut len_bytes = [0u8; 8];
    reader
        .read_exact(&mut len_bytes)
        .map_err(|err| format!("Failed to read audiobook bundle manifest length: {err}"))?;
    let manifest_len = u64::from_le_bytes(len_bytes);
    if manifest_len == 0 || manifest_len > 32 * 1024 * 1024 {
        return Err("Audiobook bundle manifest has an invalid size".into());
    }

    let mut manifest_bytes = vec![0u8; manifest_len as usize];
    reader
        .read_exact(&mut manifest_bytes)
        .map_err(|err| format!("Failed to read audiobook bundle manifest: {err}"))?;
    serde_json::from_slice(&manifest_bytes)
        .map_err(|err| format!("Failed to parse audiobook bundle manifest: {err}"))
}

/// Reject bundles we can't safely restore: wrong version/kind, a different TTS
/// model or cache version (audio wouldn't match), non-WAV audio, no chunks, or a
/// file entry missing its content type.
fn validate_bundle_manifest(manifest: &NativeAudiobookBundleManifest) -> Result<(), String> {
    if !matches!(manifest.version, 2 | 3) || manifest.kind != "papercut-audiobook-bundle" {
        return Err("Selected file is not a supported Papercut audiobook bundle".into());
    }
    if manifest.version == 2 && manifest.source_kind != "html" {
        return Err("Version 2 audiobook bundles must contain HTML source".into());
    }
    let source_role = match manifest.source_kind.as_str() {
        "html" => "sourceHtml",
        "pdf" if manifest.version == 3 => "sourcePdf",
        _ => return Err("Audiobook bundle has an unsupported source kind".into()),
    };
    let mut source_entries = manifest
        .files
        .iter()
        .filter(|entry| matches!(entry.role.as_str(), "sourceHtml" | "sourcePdf"));
    if source_entries.next().map(|entry| entry.role.as_str()) != Some(source_role)
        || source_entries.next().is_some()
    {
        return Err("Audiobook bundle must contain exactly one source document".into());
    }
    let model = model_definition(&manifest.model_id)?;
    model.speaker_id(&manifest.voice)?;
    if !model.supports_text_preprocessor(&manifest.text_preprocessor) {
        return Err(format!(
            "Audiobook bundle uses unsupported text preprocessor {:?}",
            manifest.text_preprocessor
        ));
    }
    if manifest.cache_version != CACHE_VERSION {
        return Err(
            "Audiobook bundle was generated for an incompatible audio cache version".into(),
        );
    }
    if manifest.audio.format != "wav" || !manifest.audio.single_track {
        return Err("Audiobook bundle does not contain the expected WAV audio".into());
    }
    if manifest.chunks.is_empty() {
        return Err("Audiobook bundle does not contain narration chunks".into());
    }
    if manifest
        .files
        .iter()
        .any(|entry| entry.content_type.trim().is_empty())
    {
        return Err("Audiobook bundle contains a file entry without a content type".into());
    }
    Ok(())
}

/// Copy exactly `bytes` from the bundle reader into a new file at `path`.
///
/// `reader.take(bytes)` caps the copy at one payload's length so we never read
/// into the next entry. Writes to a temp file and renames into place; verifies
/// the copied length matches to catch truncated bundles.
fn copy_payload_to_path<R: Read>(reader: &mut R, path: &Path, bytes: u64) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Failed to create directory {}: {err}", parent.display()))?;
    }
    let temp_path = import_staging_path(path)?;
    let file = fs::File::create(&temp_path).map_err(|err| {
        format!(
            "Failed to create imported audiobook file {}: {err}",
            temp_path.display()
        )
    })?;
    let mut writer = BufWriter::new(file);
    let copied = std::io::copy(&mut reader.take(bytes), &mut writer).map_err(|err| {
        format!(
            "Failed to write imported audiobook file {}: {err}",
            path.display()
        )
    })?;
    writer.flush().map_err(|err| {
        format!(
            "Failed to flush imported audiobook file {}: {err}",
            path.display()
        )
    })?;
    if copied != bytes {
        let _ = fs::remove_file(&temp_path);
        return Err(format!(
            "Audiobook bundle ended while reading payload for {}",
            path.display()
        ));
    }
    commit_staged_file(&temp_path, path, "imported audiobook file")
}

/// Read and sanitize a bundle's HTML payload before atomically exposing it.
///
/// Unlike WAV payloads, source HTML must be parsed before it reaches the stored
/// reader path. Keeping the unsafe bytes in memory also prevents a failed UTF-8
/// decode or sanitizer pass from replacing an existing imported document.
fn copy_sanitized_html_payload_to_path<R: Read>(
    reader: &mut R,
    path: &Path,
    bytes: u64,
) -> Result<(), String> {
    if bytes > MAX_BUNDLE_SOURCE_HTML_BYTES {
        return Err("Audiobook bundle source HTML exceeds the 256 MB limit".into());
    }
    let mut source = Vec::with_capacity(bytes.min(32 * 1024 * 1024) as usize);
    let copied = reader
        .take(bytes)
        .read_to_end(&mut source)
        .map_err(|err| format!("Failed to read imported audiobook HTML: {err}"))?;
    if copied as u64 != bytes {
        return Err("Audiobook bundle ended while reading source HTML".into());
    }
    let source = String::from_utf8(source)
        .map_err(|err| format!("Audiobook bundle source HTML is not valid UTF-8: {err}"))?;
    let sanitized = sanitize_html(&source);

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("Failed to create directory {}: {err}", parent.display()))?;
    }
    let temp_path = import_staging_path(path)?;
    fs::write(&temp_path, sanitized.as_bytes()).map_err(|err| {
        format!(
            "Failed to write imported audiobook HTML {}: {err}",
            temp_path.display()
        )
    })?;
    commit_staged_file(&temp_path, path, "imported audiobook HTML")
}

/// Read one bounded canonical PDF payload before content-id validation/storage.
fn read_pdf_payload<R: Read>(reader: &mut R, bytes: u64) -> Result<Vec<u8>, String> {
    if bytes > MAX_BUNDLE_SOURCE_PDF_BYTES {
        return Err("Audiobook bundle PDF exceeds the 250 MB import limit".into());
    }
    let mut source = Vec::with_capacity(bytes.min(32 * 1024 * 1024) as usize);
    let copied = reader
        .take(bytes)
        .read_to_end(&mut source)
        .map_err(|err| format!("Failed to read audiobook bundle PDF: {err}"))?;
    if copied as u64 != bytes {
        return Err("Audiobook bundle ended while reading source PDF".into());
    }
    Ok(source)
}

fn import_staging_path(path: &Path) -> Result<std::path::PathBuf, String> {
    Ok(path.with_extension(format!(
        "import.{}.tmp",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("System clock error: {err}"))?
            .as_nanos()
    )))
}

/// Discard `bytes` from the reader by copying them into a null sink. Used to
/// jump over gaps and payloads for unknown optional file kinds.
fn skip_payload<R: Read>(reader: &mut R, bytes: u64) -> Result<(), String> {
    let copied = std::io::copy(&mut reader.take(bytes), &mut std::io::sink())
        .map_err(|err| format!("Failed to skip audiobook bundle payload: {err}"))?;
    if copied != bytes {
        return Err("Audiobook bundle ended unexpectedly".into());
    }
    Ok(())
}

/// Derive a stable 24-char hex id from the bundle's identifying fields (title,
/// source URL, model, preprocessor, voice, speed, dtype, and chunk text hashes).
/// Same content in
/// → same id out, so importing the same bundle twice reuses one upload slot.
fn imported_upload_id(manifest: &NativeAudiobookBundleManifest) -> String {
    stable_hex_hash(&format!(
        "{}|{}|{}|{}|{:.2}|{}|{}|{}",
        manifest.model_id,
        manifest.title,
        manifest.source_document_url,
        manifest.voice,
        manifest.speed,
        manifest.dtype,
        manifest.text_preprocessor,
        manifest
            .chunks
            .iter()
            .map(|chunk| chunk
                .text_hash
                .clone()
                .unwrap_or_else(|| stable_hex_hash(&chunk.text)))
            .collect::<Vec<_>>()
            .join("|")
    ))
    .chars()
    .take(24)
    .collect()
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn bundle_manifest(
        version: u32,
        source_kind: Option<&str>,
        source_role: &str,
    ) -> NativeAudiobookBundleManifest {
        let mut value = serde_json::json!({
            "version": version,
            "kind": "papercut-audiobook-bundle",
            "sourceDocumentUrl": "/uploads/0123456789abcdef.pdf",
            "title": "Fixture",
            "voice": "af_heart",
            "speed": 1.0,
            "dtype": "native",
            "modelId": "sherpa-onnx/kokoro-multi-lang-v1_0",
            "cacheVersion": CACHE_VERSION,
            "chunks": [{ "id": "chunk-1", "text": "Hello", "textHash": null }],
            "files": [{
                "path": if source_role == "sourcePdf" { "source.pdf" } else { "source.html" },
                "role": source_role,
                "contentType": if source_role == "sourcePdf" { "application/pdf" } else { "text/html" },
                "bytes": 5,
                "payloadOffset": 0,
                "chunkIndex": null
            }],
            "audio": {
                "format": "wav",
                "singleTrack": true,
                "durationSec": 1.0,
                "bytes": 44
            }
        });
        if let Some(source_kind) = source_kind {
            value["sourceKind"] = serde_json::Value::String(source_kind.into());
        }
        serde_json::from_value(value).expect("fixture bundle manifest")
    }

    #[test]
    fn accepts_legacy_html_and_current_pdf_bundle_sources() {
        let legacy = bundle_manifest(2, None, "sourceHtml");
        assert_eq!(legacy.source_kind, "html");
        validate_bundle_manifest(&legacy).expect("version 2 HTML bundle");

        let pdf = bundle_manifest(3, Some("pdf"), "sourcePdf");
        validate_bundle_manifest(&pdf).expect("version 3 PDF bundle");
    }

    #[test]
    fn rejects_source_kind_and_payload_role_mismatches() {
        let manifest = bundle_manifest(3, Some("pdf"), "sourceHtml");
        assert!(validate_bundle_manifest(&manifest).is_err());
    }

    #[test]
    fn sanitizes_bundle_html_before_committing_it() {
        let source = br##"<body><a href="java&#x73;cript:alert(1)">Bad</a><a href="#note">Note</a><p id="note">Safe</p></body>"##;
        let dir = std::env::temp_dir().join(format!(
            "papercut-bundle-html-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("test clock should be valid")
                .as_nanos()
        ));
        let path = dir.join("source.html");

        copy_sanitized_html_payload_to_path(&mut Cursor::new(source), &path, source.len() as u64)
            .expect("bundle HTML should import");
        let stored = fs::read_to_string(&path).expect("sanitized HTML should be stored");

        assert!(!stored.contains("javascript:"));
        assert!(stored.contains(r##"href="#note""##));
        assert!(stored.contains(r#"id="note""#));
        let _ = fs::remove_dir_all(dir);
    }
}
