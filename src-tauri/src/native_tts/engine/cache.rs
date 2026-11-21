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

use super::generation::required_restart_token;
use super::manifest::{manifest_has_complete_index, read_manifest, read_or_rebuild_manifest_index};
use super::paths::{audiobook_dir, chunk_path, chunk_source_signature};
use crate::native_tts::lock_audiobook_directory;
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
/// matching current-revision manifest can use the metadata fast path. Unknown/old
/// revisions verify WAV coverage so completed legacy audio is exempt from restart.
/// Timing repair never starts synthesis or grants restart consent.
pub(crate) fn native_audiobook_status(
    app: tauri::AppHandle,
    request: NativeAudiobookStatusRequest,
) -> Result<NativeAudiobookStatusResponse, String> {
    let dir = audiobook_dir(&app, &request.audiobook_id)?;
    audiobook_status_for_dir(&dir, &request)
}

/// Keep PDF source-edit behavior separate from engine restart; status never discards partial progress.
/// Completed matching manifests keep the fast path; timing repair requires a writer lease and preserves provenance.
pub(super) fn audiobook_status_for_dir(
    dir: &Path,
    request: &NativeAudiobookStatusRequest,
) -> Result<NativeAudiobookStatusResponse, String> {
    let manifest = read_manifest(dir).ok();
    let matches = manifest.as_ref().is_some_and(|manifest| {
        chunk_source_signature(&manifest.chunks) == request.source_signature
            && manifest.chunks.len() == request.total_chunks
    });
    let restart_generation_token = required_restart_token(dir)?;
    let changed_pdf = !matches
        && manifest.as_ref().is_some_and(|manifest| {
            manifest
                .document_url
                .split(['?', '#'])
                .next()
                .unwrap_or_default()
                .ends_with(".pdf")
        });
    let regeneration_reason = if restart_generation_token.is_some() {
        Some("synthesis")
    } else if changed_pdf {
        Some("source")
    } else {
        None
    };
    let requires_regeneration = regeneration_reason.is_some();
    let manifest = match manifest.filter(|_| matches) {
        Some(manifest) => manifest,
        _ => {
            return Ok(NativeAudiobookStatusResponse {
                cached_chunks: 0,
                total_chunks: request.total_chunks,
                complete: false,
                requires_regeneration,
                regeneration_reason,
                restart_generation_token,
                dir: dir.display().to_string(),
                audio_duration_sec: 0.0,
                wav_bytes: 0,
            });
        }
    };

    // Complete current manifests contain totals and exact chunk boundaries,
    // so large books avoid opening hundreds or thousands of WAV files here.
    if manifest_has_complete_index(&manifest) && !requires_regeneration {
        return Ok(NativeAudiobookStatusResponse {
            cached_chunks: manifest.chunks.len(),
            total_chunks: manifest.chunks.len(),
            complete: true,
            requires_regeneration: false,
            regeneration_reason: None,
            restart_generation_token: None,
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
        // A save may have reached its final chunk; do not race its manifest writer.
        lock_audiobook_directory(dir)
            .ok()
            .and_then(|_lease| read_or_rebuild_manifest_index(&dir).ok())
    } else {
        None
    };
    Ok(NativeAudiobookStatusResponse {
        cached_chunks: scan.cached_chunks,
        total_chunks: manifest.chunks.len(),
        complete,
        requires_regeneration,
        regeneration_reason,
        restart_generation_token,
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

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::super::config::AUDIOBOOK_MANIFEST_VERSION;
    use super::super::manifest::write_pending_manifest;
    use super::super::prune::prune_orphan_chunk_files;
    use super::*;
    use crate::native_tts::types::NativeAudiobookSaveRequest;

    /// Exercise the existing replacement/resume sequence with real WAVs, without loading a voice model.
    #[test]
    fn corrected_pdf_restart_reuses_only_current_chunk_audio() {
        let dir = std::env::temp_dir().join(format!(
            "papercut-pdf-restart-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(dir.join("chunks")).unwrap();
        let old = vec![
            NativeTtsInputChunk {
                id: "c1".into(),
                text: "An envi- ronment.".into(),
                text_hash: None,
                source_span: None,
            },
            NativeTtsInputChunk {
                id: "c2".into(),
                text: "Unchanged sentence.".into(),
                text_hash: None,
                source_span: None,
            },
        ];
        let request = NativeAudiobookSaveRequest {
            job_id: "restart".into(),
            audiobook_id: "fixture".into(),
            document_url: "/uploads/ab.pdf".into(),
            title: "PDF".into(),
            model_id: "sherpa-onnx/kokoro-multi-lang-v1_0".into(),
            text_preprocessor: "none".into(),
            chunks: old.clone(),
            voice: "af_heart".into(),
            speed: 1.0,
            thread_count: Some(1),
            silma_nfe_step: None,
            restart_generation_token: None,
        };
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&40u32.to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&[1, 0, 1, 0, 0x40, 0x1f, 0, 0, 0x80, 0x3e, 0, 0, 2, 0, 16, 0]);
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&4u32.to_le_bytes());
        wav.extend_from_slice(&[0u8; 4]);
        write_pending_manifest(&dir, &request, &old).unwrap();
        for (index, chunk) in old.iter().enumerate() {
            fs::write(chunk_path(&dir, index, chunk), &wav).unwrap();
        }
        let mut corrected = old.clone();
        corrected[0].text = "An environment.".into();
        write_pending_manifest(&dir, &request, &corrected).unwrap();
        prune_orphan_chunk_files(&dir, &corrected);
        assert!(!chunk_path(&dir, 0, &old[0]).exists());
        assert_eq!(fs::read(chunk_path(&dir, 1, &old[1])).unwrap(), wav);
        let status_request = NativeAudiobookStatusRequest {
            audiobook_id: "fixture".into(),
            source_signature: chunk_source_signature(&corrected),
            total_chunks: 2,
        };
        let resumed = audiobook_status_for_dir(&dir, &status_request).unwrap();
        assert!(!resumed.requires_regeneration && !resumed.complete);
        assert_eq!(resumed.cached_chunks, 1);
        fs::write(chunk_path(&dir, 0, &corrected[0]), &wav).unwrap();
        let finished = audiobook_status_for_dir(&dir, &status_request).unwrap();
        assert!(finished.complete && !finished.requires_regeneration);
        assert_eq!(finished.cached_chunks, 2);
        fs::remove_dir_all(dir).unwrap();
    }

    /// Use actual manifests/files so read-only checks cannot silently prune old audio or rewrite metadata.
    #[test]
    fn changed_source_status_preserves_completed_and_interrupted_saves() {
        for url in [
            "/uploads/ab.pdf",
            "/uploads/ab.pdf?view=1#page=2",
            "/uploads/ab.html",
        ] {
            for complete in [false, true] {
                let dir = std::env::temp_dir().join(format!(
                    "papercut-pdf-status-{}",
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                ));
                fs::create_dir_all(dir.join("chunks")).unwrap();
                let chunks = vec![NativeTtsInputChunk {
                    id: "c1".into(),
                    text: "An envi- ronment.".into(),
                    text_hash: None,
                    source_span: None,
                }];
                let audio = chunk_path(&dir, 0, &chunks[0]);
                let old_audio = if complete {
                    // A complete legacy fixture needs actual WAV coverage, not just a claimed timing index.
                    let mut wav = Vec::new();
                    wav.extend_from_slice(b"RIFF");
                    wav.extend_from_slice(&16036u32.to_le_bytes());
                    wav.extend_from_slice(b"WAVEfmt ");
                    wav.extend_from_slice(&16u32.to_le_bytes());
                    wav.extend_from_slice(&[
                        1, 0, 1, 0, 0x40, 0x1f, 0, 0, 0x80, 0x3e, 0, 0, 2, 0, 16, 0,
                    ]);
                    wav.extend_from_slice(b"data");
                    wav.extend_from_slice(&16000u32.to_le_bytes());
                    wav.resize(16044, 0);
                    wav
                } else {
                    b"old audio must stay untouched".to_vec()
                };
                fs::write(&audio, &old_audio).unwrap();
                let path = dir.join("manifest.json");
                let manifest = serde_json::to_vec(&serde_json::json!({
                    "version": AUDIOBOOK_MANIFEST_VERSION, "documentUrl": url, "title": "Fixture",
                    "voice": "af_heart", "speed": 1.0, "threadCount": 1,
                    "chunks": chunks, "generatedAtMs": 0, "sourceSignature": chunk_source_signature(&chunks),
                    "audioDurationSec": if complete { 1.0 } else { 0.0 },
                    "wavBytes": if complete { old_audio.len() } else { 0 },
                    "playbackChunks": if complete { serde_json::json!([{"index": 0, "chunkId": "c1", "startSec": 0.0, "durationSec": 1.0}]) } else { serde_json::json!([]) }
                })).unwrap();
                fs::write(&path, &manifest).unwrap();
                let mut request = NativeAudiobookStatusRequest {
                    audiobook_id: "fixture".into(),
                    total_chunks: 1,
                    source_signature: chunk_source_signature(&chunks),
                };
                let matching = audiobook_status_for_dir(&dir, &request).unwrap();
                assert_eq!(matching.requires_regeneration, !complete);
                assert_eq!(matching.complete, complete);

                request.source_signature = chunk_source_signature(&[NativeTtsInputChunk {
                    id: "c1".into(),
                    text: "An environment.".into(),
                    text_hash: None,
                    source_span: None,
                }]);
                for total in [1, 2] {
                    request.total_chunks = total;
                    let changed = audiobook_status_for_dir(&dir, &request).unwrap();
                    assert_eq!(
                        changed.requires_regeneration,
                        !complete || url.contains(".pdf")
                    );
                    assert!(!changed.complete);
                    assert_eq!(changed.cached_chunks, 0);
                }
                assert_eq!(fs::read(&path).unwrap(), manifest);
                assert_eq!(fs::read(&audio).unwrap(), old_audio);
                if complete {
                    request.source_signature = chunk_source_signature(&chunks);
                    fs::write(&audio, b"damaged WAV despite complete index").unwrap();
                    let damaged = audiobook_status_for_dir(&dir, &request).unwrap();
                    assert!(!damaged.complete && damaged.requires_regeneration);
                    assert!(damaged.restart_generation_token.is_some());
                    assert_eq!(fs::read(&path).unwrap(), manifest);
                }
                fs::remove_file(&path).unwrap();
                assert!(
                    audiobook_status_for_dir(&dir, &request)
                        .unwrap()
                        .requires_regeneration
                );
                fs::write(&path, b"invalid JSON").unwrap();
                assert!(
                    audiobook_status_for_dir(&dir, &request)
                        .unwrap()
                        .requires_regeneration
                );
                fs::remove_dir_all(dir).unwrap();
            }
        }
    }
}
