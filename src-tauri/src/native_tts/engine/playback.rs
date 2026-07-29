//! Native mobile playback preparation backed by saved per-chunk WAV files.
//!
//! Mobile players need one seekable source for background/lock-screen playback.
//! Preparation prefers an already validated track, repairs missing metadata for an
//! imported track, and stitches chunk WAVs only as the final fallback.

use std::fs;
use std::path::Path;
use std::time::Instant;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::bundle::stitch_audiobook_wav;
use super::cache::wav_metadata;
use super::file_commit::commit_staged_file;
use super::manifest::{playback_index_matches, read_or_rebuild_manifest_index};
use super::paths::{audiobook_dir, playback_metadata_path, playback_track_path};
use crate::native_tts::types::{NativeAudiobookPlaybackChunk, NativeTtsInputChunk};
use crate::native_tts::types::{NativeAudiobookPlaybackRequest, NativeAudiobookPlaybackResponse};

const PLAYBACK_METADATA_VERSION: u32 = 1;

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlaybackMetadata {
    version: u32,
    source_signature: String,
    audio_duration_sec: f64,
    wav_bytes: usize,
    chunks: Vec<NativeAudiobookPlaybackChunk>,
}

/// Validate requested audiobook identity and return one native-playable file URL
/// plus global chunk boundaries used by React to recover chunk-local state.
pub(crate) fn prepare_native_audiobook_playback(
    app: tauri::AppHandle,
    request: NativeAudiobookPlaybackRequest,
) -> Result<NativeAudiobookPlaybackResponse, String> {
    let started = Instant::now();
    let dir = audiobook_dir(&app, &request.audiobook_id)?;
    let manifest = read_or_rebuild_manifest_index(&dir)?;
    if manifest.chunks.is_empty() || manifest.source_signature != request.source_signature {
        return Err("Saved audiobook does not match the current document chunks".into());
    }

    let metadata = prepare_single_track(
        &dir,
        &manifest.chunks,
        &manifest.source_signature,
        &manifest.playback_chunks,
        manifest.audio_duration_sec,
    )?;
    let track_path = playback_track_path(&dir);
    let audio_url = tauri::Url::from_file_path(&track_path)
        .map_err(|_| {
            format!(
                "Failed to convert playback path to a file URL: {}",
                track_path.display()
            )
        })?
        .to_string();
    log::info!(
        "Prepared native audiobook track: chunks={}, duration_sec={:.2}, elapsed_ms={}",
        metadata.chunks.len(),
        metadata.audio_duration_sec,
        started.elapsed().as_millis()
    );
    Ok(NativeAudiobookPlaybackResponse {
        audio_url,
        audio_duration_sec: metadata.audio_duration_sec,
        wav_bytes: metadata.wav_bytes,
        chunks: metadata.chunks,
    })
}

/// Resolve the cheapest valid single-track representation in three stages:
/// 1. reuse track + matching sidecar; 2. validate imported track and rebuild only
/// sidecar metadata; 3. stream-stitch canonical chunk WAVs into a new track.
fn prepare_single_track(
    dir: &Path,
    chunks: &[NativeTtsInputChunk],
    source_signature: &str,
    playback_chunks: &[NativeAudiobookPlaybackChunk],
    expected_duration_sec: f64,
) -> Result<PlaybackMetadata, String> {
    let track_path = playback_track_path(dir);
    let metadata_path = playback_metadata_path(dir);
    // Fast path: sidecar signature and byte count prove this track belongs to
    // current chunks without reading audio data or walking per-chunk files.
    if track_path.is_file() && metadata_path.is_file() {
        if let Ok(bytes) = fs::read(&metadata_path) {
            if let Ok(metadata) = serde_json::from_slice::<PlaybackMetadata>(&bytes) {
                let track_bytes = fs::metadata(&track_path)
                    .map(|value| value.len() as usize)
                    .unwrap_or(0);
                if playback_metadata_matches(
                    &metadata,
                    chunks,
                    source_signature,
                    expected_duration_sec,
                    track_bytes,
                ) {
                    return Ok(metadata);
                }
            }
        }
    }

    // Imported bundles may restore playback.wav without device-local sidecar.
    // Validate duration against manifest boundaries, then reconstruct sidecar.
    if track_path.is_file() && !playback_chunks.is_empty() {
        if let Some(track) = wav_metadata(&track_path) {
            if (track.precise_audio_duration_sec - expected_duration_sec).abs() <= 0.05 {
                let metadata = PlaybackMetadata {
                    version: PLAYBACK_METADATA_VERSION,
                    source_signature: source_signature.to_string(),
                    audio_duration_sec: track.precise_audio_duration_sec,
                    wav_bytes: track.info.wav_bytes,
                    chunks: playback_chunks.to_vec(),
                };
                write_metadata_atomically(&metadata_path, &metadata)?;
                return Ok(metadata);
            }
        }
    }

    // No reusable track remains. Stream canonical chunks into an atomic track.
    let summary = stitch_audiobook_wav(dir, chunks, &track_path)?;
    let metadata = PlaybackMetadata {
        version: PLAYBACK_METADATA_VERSION,
        source_signature: source_signature.to_string(),
        audio_duration_sec: summary.audio_duration_sec as f64,
        wav_bytes: summary.wav_bytes,
        chunks: summary.chunk_timings,
    };
    write_metadata_atomically(&metadata_path, &metadata)?;
    Ok(metadata)
}

/// Validate a cached sidecar against both the track and canonical manifest.
///
/// Matching only the file size is insufficient: stale metadata could otherwise
/// map native time to reordered or missing chunks after a cache change.
fn playback_metadata_matches(
    metadata: &PlaybackMetadata,
    chunks: &[NativeTtsInputChunk],
    source_signature: &str,
    expected_duration_sec: f64,
    track_bytes: usize,
) -> bool {
    metadata.version == PLAYBACK_METADATA_VERSION
        && metadata.source_signature == source_signature
        && metadata.wav_bytes == track_bytes
        && metadata.audio_duration_sec.is_finite()
        && metadata.audio_duration_sec > 0.0
        && (metadata.audio_duration_sec - expected_duration_sec).abs() <= 0.05
        && playback_index_matches(chunks, &metadata.chunks, metadata.audio_duration_sec)
}

/// Commit playback sidecar through a temporary file so interruption never leaves
/// partially written JSON that could be mistaken for valid cached metadata.
fn write_metadata_atomically(path: &Path, metadata: &PlaybackMetadata) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(metadata)
        .map_err(|err| format!("Failed to serialize playback metadata: {err}"))?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("System clock error: {err}"))?
        .as_nanos();
    let temp_path = path.with_extension(format!("json.{nonce}.tmp"));
    fs::write(&temp_path, bytes).map_err(|err| {
        format!(
            "Failed to write playback metadata {}: {err}",
            temp_path.display()
        )
    })?;
    commit_staged_file(&temp_path, path, "playback metadata")
}
