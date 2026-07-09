//! Native saved-audiobook cache: scanning chunk WAVs and parsing WAV headers.
//!
//! Read playback and the save job both ask "which chunks already exist on disk,
//! and how long/large are they?" This module answers that by scanning the
//! per-audiobook chunk directory and parsing minimal RIFF/WAVE metadata without
//! decoding audio. [`WavMetadata`] additionally exposes the `data` chunk extent
//! so the export path can concatenate chunk payloads into a single track.

use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use base64::{engine::general_purpose, Engine as _};

use super::manifest::{manifest_has_complete_index, read_manifest, read_or_rebuild_manifest_index};
use super::paths::{audiobook_dir, chunk_path, chunk_source_signature};
use crate::native_tts::types::{
    NativeAudiobookChunkRequest, NativeAudiobookStatusRequest, NativeAudiobookStatusResponse,
    NativeTtsChunkResponse, NativeTtsInputChunk,
};

/// Running totals from scanning an audiobook directory. `#[derive(Default)]`
/// gives a zeroed starting value via `AudiobookScan::default()`.
#[derive(Default)]
pub(super) struct AudiobookScan {
    pub(super) cached_chunks: usize,
    pub(super) audio_duration_sec: f32,
    pub(super) wav_bytes: usize,
}

/// The cheap facts about one WAV file: how long it plays and its total size.
pub(super) struct WavInfo {
    pub(super) audio_duration_sec: f32,
    pub(super) wav_bytes: usize,
}

/// Full WAV header details needed to splice files together: the raw `fmt ` block
/// plus where the `data` samples start and how many bytes they span.
pub(super) struct WavMetadata {
    pub(super) fmt_payload: Vec<u8>,
    pub(super) precise_audio_duration_sec: f64,
    pub(super) data_offset: usize,
    pub(super) data_bytes: usize,
    pub(super) info: WavInfo,
}

/// Report saved-audiobook availability without receiving the full chunk list over IPC.
///
/// The caller sends the same ordered source signature used by the manifest. A
/// matching current manifest is a single-read metadata fast path with no per-WAV
/// scan. Incomplete current manifests fall back to one verification scan and are
/// indexed only after every expected WAV exists.
pub(crate) fn native_audiobook_status(
    app: tauri::AppHandle,
    request: NativeAudiobookStatusRequest,
) -> Result<NativeAudiobookStatusResponse, String> {
    let dir = audiobook_dir(&app, &request.audiobook_id)?;
    let manifest = match read_manifest(&dir) {
        Ok(manifest)
            if chunk_source_signature(&manifest.chunks) == request.source_signature
                && manifest.chunks.len() == request.total_chunks =>
        {
            manifest
        }
        _ => {
            return Ok(NativeAudiobookStatusResponse {
                cached_chunks: 0,
                total_chunks: request.total_chunks,
                complete: false,
                dir: dir.display().to_string(),
                audio_duration_sec: 0.0,
                wav_bytes: 0,
            });
        }
    };

    // Complete current manifests contain totals and exact chunk boundaries,
    // so large books avoid opening hundreds or thousands of WAV files here.
    if manifest_has_complete_index(&manifest) {
        return Ok(NativeAudiobookStatusResponse {
            cached_chunks: manifest.chunks.len(),
            total_chunks: manifest.chunks.len(),
            complete: true,
            dir: dir.display().to_string(),
            audio_duration_sec: manifest.audio_duration_sec as f32,
            wav_bytes: manifest.wav_bytes,
        });
    }

    // Recovery path: verify actual chunk files, then persist the timing index
    // once complete so later checks use the manifest fast path.
    let scan = scan_audiobook(&dir, &manifest.chunks, false);
    let complete = !manifest.chunks.is_empty() && scan.cached_chunks == manifest.chunks.len();
    let indexed = if complete {
        read_or_rebuild_manifest_index(&dir).ok()
    } else {
        None
    };
    Ok(NativeAudiobookStatusResponse {
        cached_chunks: scan.cached_chunks,
        total_chunks: manifest.chunks.len(),
        complete,
        dir: dir.display().to_string(),
        audio_duration_sec: indexed
            .as_ref()
            .map(|value| value.audio_duration_sec as f32)
            .unwrap_or(scan.audio_duration_sec),
        wav_bytes: indexed
            .as_ref()
            .map(|value| value.wav_bytes)
            .unwrap_or(scan.wav_bytes),
    })
}

/// Read one already-saved chunk WAV off disk and return it base64-encoded for
/// the WebView to play. (sample_rate/duration are left 0 here; playback reads
/// them from the WAV itself.)
pub(crate) fn get_native_audiobook_chunk(
    app: tauri::AppHandle,
    request: NativeAudiobookChunkRequest,
) -> Result<NativeTtsChunkResponse, String> {
    let dir = audiobook_dir(&app, &request.audiobook_id)?;
    let path = chunk_path(&dir, request.index, &request.chunk);
    let wav = fs::read(&path).map_err(|err| {
        format!(
            "Failed to read native audiobook chunk {}: {err}",
            path.display()
        )
    })?;
    let wav_bytes = wav.len();
    Ok(NativeTtsChunkResponse {
        chunk_id: Some(request.chunk.id),
        wav_base64: general_purpose::STANDARD.encode(wav),
        sample_rate: 0,
        audio_duration_sec: 0.0,
        wav_bytes,
        generate_ms: 0,
        backend: format!("native-audiobook-cache:{}", dir.display()),
    })
}

/// Walk the expected chunk paths and tally which valid WAVs exist, plus their
/// total duration/size. With `prune_invalid` set, any present-but-unparseable
/// WAV is deleted so the save job will regenerate it.
pub(super) fn scan_audiobook(
    dir: &Path,
    chunks: &[NativeTtsInputChunk],
    prune_invalid: bool,
) -> AudiobookScan {
    let mut scan = AudiobookScan::default();
    for (index, chunk) in chunks.iter().enumerate() {
        let path = chunk_path(dir, index, chunk);
        // `let Some(..) = .. else { continue }` handles the "no valid WAV" case
        // and moves on; otherwise `info` is the parsed metadata.
        let Some(info) = wav_info(&path) else {
            if prune_invalid && path.exists() {
                let _ = fs::remove_file(path);
            }
            continue;
        };
        scan.cached_chunks += 1;
        scan.audio_duration_sec += info.audio_duration_sec;
        scan.wav_bytes += info.wav_bytes;
    }
    scan
}

/// Convenience wrapper: parse a WAV and keep only the cheap `WavInfo`. Returns
/// `None` (not an error) if the file is missing or not a valid WAV.
pub(super) fn wav_info(path: &Path) -> Option<WavInfo> {
    wav_metadata(path).map(|metadata| metadata.info)
}

/// Parse only RIFF/WAVE headers needed for validation, duration, and stitching.
///
/// The file is streamed and seeked rather than loaded into memory. This matters
/// when status/import/playback touch many chunks from multi-hour audiobooks.
/// Returns `None` for missing, truncated, unsupported, or malformed input.
///
/// WAV layout: `RIFF<size>WAVE`, then a sequence of `<id><size><payload>`
/// chunks. We walk them, capture the `fmt ` block (channels/sample rate/bit
/// depth) and the `data` extent, then derive seconds from byte counts.
pub(super) fn wav_metadata(path: &Path) -> Option<WavMetadata> {
    let mut file = File::open(path).ok()?;
    let wav_bytes = file.metadata().ok()?.len() as usize;
    let mut header = [0u8; 12];
    file.read_exact(&mut header).ok()?;
    if wav_bytes < 44 || &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
        return None;
    }

    let mut offset = 12u64;
    let mut fmt_payload = Vec::new();
    let mut channels = 0u16;
    let mut sample_rate = 0u32;
    let mut bits_per_sample = 0u16;
    let mut audio_offset = 0usize;
    let mut data_bytes = 0usize;

    while offset + 8 <= wav_bytes as u64 {
        let mut chunk_header = [0u8; 8];
        file.read_exact(&mut chunk_header).ok()?;
        let id = &chunk_header[0..4];
        let size = u32::from_le_bytes(chunk_header[4..8].try_into().ok()?) as usize;
        let payload_offset = offset + 8;
        if payload_offset + size as u64 > wav_bytes as u64 {
            return None;
        }

        if id == b"fmt " && size >= 16 {
            fmt_payload.resize(size, 0);
            file.read_exact(&mut fmt_payload).ok()?;
            channels = u16::from_le_bytes(fmt_payload[2..4].try_into().ok()?);
            sample_rate = u32::from_le_bytes(fmt_payload[4..8].try_into().ok()?);
            bits_per_sample = u16::from_le_bytes(fmt_payload[14..16].try_into().ok()?);
        } else if id == b"data" {
            audio_offset = payload_offset as usize;
            data_bytes = size;
            break;
        } else {
            file.seek(SeekFrom::Current(size as i64)).ok()?;
        }

        if size % 2 != 0 {
            file.seek(SeekFrom::Current(1)).ok()?;
        }
        offset = payload_offset + size as u64 + (size % 2) as u64;
    }

    if fmt_payload.is_empty()
        || channels == 0
        || sample_rate == 0
        || bits_per_sample == 0
        || audio_offset == 0
        || data_bytes == 0
    {
        return None;
    }
    let bytes_per_sample = (bits_per_sample as f64 / 8.0).max(1.0);
    let precise_audio_duration_sec =
        data_bytes as f64 / (sample_rate as f64 * channels as f64 * bytes_per_sample);
    Some(WavMetadata {
        fmt_payload,
        precise_audio_duration_sec,
        data_offset: audio_offset,
        data_bytes,
        info: WavInfo {
            audio_duration_sec: precise_audio_duration_sec as f32,
            wav_bytes,
        },
    })
}
