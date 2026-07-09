//! Export saved audiobook audio as a `.papercut-audiobook` bundle or plain WAV.
//!
//! Bundle export stitches the per-chunk WAVs into one combined WAV, writes JSON
//! metadata and source HTML sidecars, then packs those files behind the bundle
//! header. WAV export reuses the same stitching path and writes only the final
//! audio file.
//!
//! Rust notes for a JS reader: `Result<T, String>` is this codebase's "either a
//! value or an error message" type — the trailing `?` after a call means "if it
//! was an error, return that error now" (like rethrowing). A `&` in front of a
//! type (`&Path`) is a borrowed reference: the function reads the value without
//! taking ownership, similar to passing an object you promise not to keep.

use std::fs;
use std::io::{BufWriter, Read, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_fs::{FilePath, FsExt, OpenOptions};

use super::super::config::{BUNDLE_MAGIC, CACHE_VERSION};
use super::super::paths::{
    audiobook_dir, chunk_path, sanitize_export_basename, speakable_chunks, unique_export_work_dir,
};
use super::{stitch_audiobook_wav, WavExportSummary};
use crate::native_tts::types::{
    NativeAudiobookExportRequest, NativeAudiobookExportResponse, NativeTtsInputChunk,
};

/// Top-level export entry point. The frontend chooses between the re-importable
/// Papercut bundle and a plain stitched WAV for use outside Papercut.
pub(crate) fn export_audiobook_native(
    app: tauri::AppHandle,
    request: NativeAudiobookExportRequest,
) -> Result<NativeAudiobookExportResponse, String> {
    match request.export_format.as_str() {
        "bundle" => export_audiobook_bundle(app, request),
        "wav" => export_audiobook_wav(app, request),
        value => Err(format!("Unsupported audiobook export format {value:?}")),
    }
}

/// Asks the OS for a save location, builds the combined WAV + sidecars in a
/// temporary work directory, writes the final bundle to the chosen path, then
/// cleans up the work directory. Returns metadata about what was written.
fn export_audiobook_bundle(
    app: tauri::AppHandle,
    request: NativeAudiobookExportRequest,
) -> Result<NativeAudiobookExportResponse, String> {
    // Drop blank chunks up front; an export with nothing to say is an error.
    let chunks = speakable_chunks(&request.chunks);
    if chunks.is_empty() {
        return Err("No speakable audiobook chunks to export".into());
    }
    let source_html = request
        .source_html
        .as_deref()
        .ok_or_else(|| "Source HTML is required for audiobook bundle export".to_string())?;

    // Open the native "Save As" dialog. `blocking_save_file` returns None if the
    // user cancels, which `ok_or_else` turns into an error.
    let basename = sanitize_export_basename(&request.title);
    let destination = app
        .dialog()
        .file()
        .set_title("Export Audiobook Bundle")
        .set_file_name(format!("{basename}.papercut-audiobook"))
        .add_filter("Papercut Audiobook", &["papercut-audiobook"])
        .blocking_save_file()
        .ok_or_else(|| "Audiobook export cancelled".to_string())?;
    let destination_label = destination.to_string();

    // Build artifacts in a throwaway work dir so a failure never leaves a
    // half-written file at the user's chosen path.
    let dir = audiobook_dir(&app, &request.audiobook_id)?;
    let export_dir = unique_export_work_dir(&app, &request.title)?;
    fs::create_dir_all(&export_dir).map_err(|err| {
        format!(
            "Failed to create audiobook export work directory {}: {err}",
            export_dir.display()
        )
    })?;

    let audio_filename = format!("{basename}.wav");
    let audio_path = export_dir.join(&audio_filename);
    let metadata_path = export_dir.join("metadata.json");
    let html_path = export_dir.join("source.html");
    let export = stitch_audiobook_wav(&dir, &chunks, &audio_path)?;
    write_export_sidecars(
        &request,
        source_html,
        &chunks,
        &export,
        &audio_path,
        &metadata_path,
        &html_path,
    )?;
    write_export_bundle(
        &app,
        destination,
        &request,
        &chunks,
        &export,
        &audio_path,
        &metadata_path,
        &html_path,
        &audio_filename,
    )?;
    let _ = fs::remove_dir_all(&export_dir);

    Ok(NativeAudiobookExportResponse {
        path: destination_label,
        audio_path: audio_filename,
        metadata_path: "metadata.json".into(),
        html_path: "source.html".into(),
        chunks: export.chunks,
        audio_duration_sec: export.audio_duration_sec,
        wav_bytes: export.wav_bytes,
    })
}

/// Export only the stitched WAV, omitting Papercut metadata/source sidecars.
///
/// The WAV is still built in Papercut's temporary export directory first so
/// failed stitching never leaves a partial file at the user's selected path.
/// The final copy goes through Tauri's FS plugin so mobile file-provider paths
/// keep working the same way bundle export already does.
fn export_audiobook_wav(
    app: tauri::AppHandle,
    request: NativeAudiobookExportRequest,
) -> Result<NativeAudiobookExportResponse, String> {
    let chunks = speakable_chunks(&request.chunks);
    if chunks.is_empty() {
        return Err("No speakable audiobook chunks to export".into());
    }

    let basename = sanitize_export_basename(&request.title);
    let destination = app
        .dialog()
        .file()
        .set_title("Export Audiobook WAV")
        .set_file_name(format!("{basename}.wav"))
        .add_filter("WAV Audio", &["wav"])
        .blocking_save_file()
        .ok_or_else(|| "Audiobook export cancelled".to_string())?;
    let destination_label = destination.to_string();

    let dir = audiobook_dir(&app, &request.audiobook_id)?;
    let export_dir = unique_export_work_dir(&app, &request.title)?;
    fs::create_dir_all(&export_dir).map_err(|err| {
        format!(
            "Failed to create audiobook export work directory {}: {err}",
            export_dir.display()
        )
    })?;

    let audio_filename = format!("{basename}.wav");
    let audio_path = export_dir.join(&audio_filename);
    let export = stitch_audiobook_wav(&dir, &chunks, &audio_path)?;
    write_wav_destination(&app, destination, &audio_path)?;
    let _ = fs::remove_dir_all(&export_dir);

    Ok(NativeAudiobookExportResponse {
        path: destination_label,
        audio_path: audio_filename,
        metadata_path: String::new(),
        html_path: String::new(),
        chunks: export.chunks,
        audio_duration_sec: export.audio_duration_sec,
        wav_bytes: export.wav_bytes,
    })
}

/// Write the two human/portable sidecar files next to the combined WAV: the
/// original `source.html` and a `metadata.json` describing the export (voice,
/// speed, model id, chunk list, etc.). These are also packed into the bundle.
fn write_export_sidecars(
    request: &NativeAudiobookExportRequest,
    source_html: &str,
    chunks: &[NativeTtsInputChunk],
    export: &WavExportSummary,
    audio_path: &Path,
    metadata_path: &Path,
    html_path: &Path,
) -> Result<(), String> {
    fs::write(html_path, source_html.as_bytes())
        .map_err(|err| format!("Failed to write source HTML {}: {err}", html_path.display()))?;

    let exported_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("System clock error: {err}"))?
        .as_millis();
    let audio_file = audio_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("audiobook.wav");
    // `json!` builds a JSON value inline, much like writing an object literal.
    let metadata = json!({
        "version": 1,
        "kind": "papercut-audiobook-export",
        "documentUrl": request.document_url,
        "title": request.title,
        "voice": request.voice,
        "speed": request.speed,
        "dtype": request.dtype,
        "modelId": request.model_id,
        "textPreprocessor": request.text_preprocessor,
        "cacheVersion": CACHE_VERSION,
        "audiobookId": request.audiobook_id,
        "exportedAtMs": exported_at_ms,
        "files": {
            "audio": audio_file,
            "sourceHtml": "source.html"
        },
        "audio": {
            "format": "wav",
            "singleTrack": true,
            "durationSec": export.audio_duration_sec,
            "bytes": export.wav_bytes
        },
        "chunks": chunks,
    });
    let json = serde_json::to_vec_pretty(&metadata)
        .map_err(|err| format!("Failed to serialize audiobook export metadata: {err}"))?;
    fs::write(metadata_path, json).map_err(|err| {
        format!(
            "Failed to write audiobook export metadata {}: {err}",
            metadata_path.display()
        )
    })
}

/// Pack everything into the final `.papercut-audiobook` file at `destination`.
///
/// Builds the manifest entry list first (computing each payload's running byte
/// offset), serializes the manifest to JSON, then writes:
/// magic bytes → manifest length → manifest JSON → each payload in order.
/// Payload order here must match the offsets recorded in the manifest.
#[allow(clippy::too_many_arguments)]
fn write_export_bundle(
    app: &tauri::AppHandle,
    destination: FilePath,
    request: &NativeAudiobookExportRequest,
    chunks: &[NativeTtsInputChunk],
    export: &WavExportSummary,
    audio_path: &Path,
    metadata_path: &Path,
    html_path: &Path,
    audio_filename: &str,
) -> Result<(), String> {
    let dir = audiobook_dir(app, &request.audiobook_id)?;
    let metadata_bytes = fs::metadata(metadata_path)
        .map_err(|err| format!("Failed to inspect export metadata: {err}"))?
        .len();
    let html_bytes = fs::metadata(html_path)
        .map_err(|err| format!("Failed to inspect export HTML: {err}"))?
        .len();
    let exported_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("System clock error: {err}"))?
        .as_millis();

    // Describe each packed file. `push_bundle_entry` advances `payload_offset`
    // by the entry size so offsets line up with the payload write order below.
    let mut payload_offset = 0u64;
    let mut manifest_entries = Vec::with_capacity(chunks.len() + 3);
    push_bundle_entry(
        &mut manifest_entries,
        "metadata.json",
        "metadata",
        "application/json",
        metadata_bytes,
        &mut payload_offset,
        None,
    );
    push_bundle_entry(
        &mut manifest_entries,
        "source.html",
        "sourceHtml",
        "text/html; charset=utf-8",
        html_bytes,
        &mut payload_offset,
        None,
    );

    for (index, chunk) in chunks.iter().enumerate() {
        if chunk.text.trim().is_empty() {
            continue;
        }
        let path = chunk_path(&dir, index, chunk);
        let bytes = fs::metadata(&path)
            .map_err(|err| {
                format!(
                    "Failed to inspect saved audiobook chunk {}: {err}",
                    path.display()
                )
            })?
            .len();
        push_bundle_entry(
            &mut manifest_entries,
            &format!("chunks/{:05}.wav", index + 1),
            "chunkWav",
            "audio/wav",
            bytes,
            &mut payload_offset,
            Some(index),
        );
    }

    push_bundle_entry(
        &mut manifest_entries,
        &format!("audio/{audio_filename}"),
        "singleTrackWav",
        "audio/wav",
        export.wav_bytes as u64,
        &mut payload_offset,
        None,
    );

    let manifest = json!({
        "version": 2,
        "kind": "papercut-audiobook-bundle",
        "sourceDocumentUrl": request.document_url,
        "title": request.title,
        "voice": request.voice,
        "speed": request.speed,
        "dtype": request.dtype,
        "modelId": request.model_id,
        "textPreprocessor": request.text_preprocessor,
        "cacheVersion": CACHE_VERSION,
        "exportedAtMs": exported_at_ms,
        "files": manifest_entries,
        "audio": {
            "format": "wav",
            "singleTrack": true,
            "durationSec": export.audio_duration_sec,
            "bytes": export.wav_bytes,
        },
        "chunks": chunks,
    });
    let manifest_json = serde_json::to_vec_pretty(&manifest)
        .map_err(|err| format!("Failed to serialize audiobook bundle manifest: {err}"))?;

    // The destination came from the dialog plugin, so open it through the fs
    // plugin (which understands those handles) rather than std::fs.
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    let file = app
        .fs()
        .open(destination, options)
        .map_err(|err| format!("Failed to open the selected audiobook export file: {err}"))?;
    let mut writer = BufWriter::new(file);
    // Header: magic + manifest length (u64, little-endian) + manifest JSON.
    writer.write_all(BUNDLE_MAGIC).map_err(write_export_err)?;
    writer
        .write_all(&(manifest_json.len() as u64).to_le_bytes())
        .map_err(write_export_err)?;
    writer.write_all(&manifest_json).map_err(write_export_err)?;
    // Payloads, in the exact order their offsets were assigned above.
    write_file_payload(&mut writer, metadata_path)?;
    write_file_payload(&mut writer, html_path)?;
    for (index, chunk) in chunks.iter().enumerate() {
        if chunk.text.trim().is_empty() {
            continue;
        }
        write_file_payload(&mut writer, &chunk_path(&dir, index, chunk))?;
    }
    write_file_payload(&mut writer, audio_path)?;
    writer.flush().map_err(write_export_err)
}

fn write_wav_destination(
    app: &tauri::AppHandle,
    destination: FilePath,
    audio_path: &Path,
) -> Result<(), String> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    let file = app
        .fs()
        .open(destination, options)
        .map_err(|err| format!("Failed to open the selected WAV export file: {err}"))?;
    let mut writer = BufWriter::new(file);
    write_file_payload(&mut writer, audio_path)?;
    writer.flush().map_err(write_export_err)
}

/// Append one file's manifest entry and bump the running payload offset.
///
/// `entries` and `payload_offset` are `&mut` (mutable borrows) so this helper
/// edits the caller's vector and counter in place. `chunk_index` is `Option`
/// (Some(i) for chunk WAVs, None otherwise) — Rust's null-free "maybe a value".
fn push_bundle_entry(
    entries: &mut Vec<serde_json::Value>,
    path: &str,
    role: &str,
    content_type: &str,
    bytes: u64,
    payload_offset: &mut u64,
    chunk_index: Option<usize>,
) {
    entries.push(json!({
        "path": path,
        "role": role,
        "contentType": content_type,
        "bytes": bytes,
        "payloadOffset": *payload_offset,
        "chunkIndex": chunk_index,
    }));
    *payload_offset += bytes;
}

/// Stream one file's bytes into an export writer in 64 KB blocks.
///
/// Generic over `W: Write` so it works with any writer (here a buffered file).
/// The loop reads until `read == 0` (end of file), copying each block out.
fn write_file_payload<W: Write>(writer: &mut W, path: &Path) -> Result<(), String> {
    let file = fs::File::open(path).map_err(|err| {
        format!(
            "Failed to open audiobook export payload {}: {err}",
            path.display()
        )
    })?;
    let mut reader = std::io::BufReader::new(file);
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer).map_err(|err| {
            format!(
                "Failed to read audiobook export payload {}: {err}",
                path.display()
            )
        })?;
        if read == 0 {
            break;
        }
        writer
            .write_all(&buffer[..read])
            .map_err(write_export_err)?;
    }
    Ok(())
}

/// Small adapter turning a low-level I/O error into our `String` error type, so
/// the many `writer.write_all(...).map_err(write_export_err)?` calls stay terse.
fn write_export_err(err: std::io::Error) -> String {
    format!("Failed to write audiobook export: {err}")
}
