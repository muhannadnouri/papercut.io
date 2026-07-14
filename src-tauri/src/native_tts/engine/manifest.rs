//! Saved-audiobook manifest schema, persistence, and playback index recovery.
//!
//! The save job, import path, status checks, and native playback all need the
//! same durable source identity plus chunk timing metadata. Keeping it here means
//! the long-running save loop can stay focused on generation while every reader
//! validates the same manifest invariants.

use std::fs;
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::cache::wav_metadata;
use super::config::AUDIOBOOK_MANIFEST_VERSION;
use super::file_commit::commit_staged_file;
use super::models::DEFAULT_MODEL_ID;
use super::paths::{
    chunk_path, chunk_source_signature, playback_metadata_path, playback_track_path,
};
use crate::native_tts::types::{
    NativeAudiobookPlaybackChunk, NativeAudiobookSaveRequest, NativeTtsInputChunk,
};

const PLAYBACK_TIMING_TOLERANCE_SEC: f64 = 0.05;

/// Durable source identity and playback index stored beside canonical chunk WAVs.
///
/// The current schema requires its compact source signature, aggregate totals,
/// and chunk boundaries. Older schemas are intentionally rejected instead of
/// being carried through the runtime as partially defaulted state.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct NativeAudiobookManifest {
    pub(super) version: u8,
    pub(super) document_url: String,
    pub(super) title: String,
    #[serde(default = "default_model_id")]
    pub(super) model_id: String,
    #[serde(default = "default_text_preprocessor")]
    pub(super) text_preprocessor: String,
    pub(super) voice: String,
    pub(super) speed: f32,
    pub(super) thread_count: i32,
    pub(super) chunks: Vec<NativeTtsInputChunk>,
    pub(super) generated_at_ms: u128,
    pub(super) source_signature: String,
    pub(super) audio_duration_sec: f64,
    pub(super) wav_bytes: usize,
    pub(super) playback_chunks: Vec<NativeAudiobookPlaybackChunk>,
}

#[derive(Deserialize)]
struct NativeAudiobookManifestHeader {
    version: u8,
}

/// Preserve manifests written before model selection existed by treating them as Kokoro.
fn default_model_id() -> String {
    DEFAULT_MODEL_ID.into()
}

/// Preserve pre-diacritization manifests by treating absent metadata as original text.
fn default_text_preprocessor() -> String {
    "none".into()
}

/// Persist chunk identity before synthesis begins.
///
/// Resume/status can then discover an interrupted job from disk using only a
/// source signature. Timing totals remain empty until all chunk WAVs are valid.
pub(super) fn write_pending_manifest(
    dir: &Path,
    request: &NativeAudiobookSaveRequest,
    chunks: &[NativeTtsInputChunk],
) -> Result<(), String> {
    let generated_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("System clock error: {err}"))?
        .as_millis();
    let manifest = NativeAudiobookManifest {
        version: AUDIOBOOK_MANIFEST_VERSION,
        document_url: request.document_url.clone(),
        title: request.title.clone(),
        model_id: request.model_id.clone(),
        text_preprocessor: request.text_preprocessor.clone(),
        voice: request.voice.clone(),
        speed: request.speed,
        thread_count: request.thread_count.unwrap_or(0),
        chunks: chunks.to_vec(),
        generated_at_ms,
        source_signature: chunk_source_signature(chunks),
        audio_duration_sec: 0.0,
        wav_bytes: 0,
        playback_chunks: Vec::new(),
    };
    write_manifest_file(dir, &manifest)?;
    remove_legacy_playback_files(dir);
    Ok(())
}

/// Finalize a complete audiobook manifest from actual WAV headers.
///
/// Chunk start times come from measured audio durations, not text estimates. Any
/// prior derived track is invalidated because Save or Import changed canonical data.
/// Returns the canonical `(audio_duration_sec, wav_bytes)` totals it persisted so
/// callers report the manifest's measured values rather than re-deriving them.
pub(super) fn write_manifest(
    dir: &Path,
    request: &NativeAudiobookSaveRequest,
    chunks: &[NativeTtsInputChunk],
    thread_count: i32,
) -> Result<(f64, usize), String> {
    let generated_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("System clock error: {err}"))?
        .as_millis();
    let index_started = Instant::now();
    let (playback_chunks, audio_duration_sec, wav_bytes) = build_playback_index(dir, chunks)?;
    log::info!(
        "Built native audiobook manifest index: chunks={}, elapsed_ms={}",
        chunks.len(),
        index_started.elapsed().as_millis()
    );
    let manifest = NativeAudiobookManifest {
        version: AUDIOBOOK_MANIFEST_VERSION,
        document_url: request.document_url.clone(),
        title: request.title.clone(),
        model_id: request.model_id.clone(),
        text_preprocessor: request.text_preprocessor.clone(),
        voice: request.voice.clone(),
        speed: request.speed,
        thread_count,
        chunks: chunks.to_vec(),
        generated_at_ms,
        source_signature: chunk_source_signature(chunks),
        audio_duration_sec,
        wav_bytes,
        playback_chunks,
    };
    write_manifest_file(dir, &manifest)?;
    remove_legacy_playback_files(dir);
    Ok((audio_duration_sec, wav_bytes))
}

/// Read a manifest only when its schema exactly matches this app version.
///
/// Parsing the small header first produces a clear unsupported-version error
/// before the full current-schema deserialize requires all current fields.
pub(super) fn read_manifest(dir: &Path) -> Result<NativeAudiobookManifest, String> {
    let path = dir.join("manifest.json");
    let bytes = fs::read(&path).map_err(|err| {
        format!(
            "Failed to read native audiobook manifest {}: {err}",
            path.display()
        )
    })?;
    let header =
        serde_json::from_slice::<NativeAudiobookManifestHeader>(&bytes).map_err(|err| {
            format!(
                "Failed to read native audiobook manifest version {}: {err}",
                path.display()
            )
        })?;
    if header.version != AUDIOBOOK_MANIFEST_VERSION {
        return Err(format!(
            "Unsupported native audiobook manifest version {} (expected {})",
            header.version, AUDIOBOOK_MANIFEST_VERSION
        ));
    }
    serde_json::from_slice::<NativeAudiobookManifest>(&bytes).map_err(|err| {
        format!(
            "Failed to parse native audiobook manifest {}: {err}",
            path.display()
        )
    })
}

/// Return a current manifest with a complete, internally consistent index.
///
/// A current pending manifest intentionally has no timing index while generation
/// is incomplete. Once every WAV exists, this recovery path rebuilds metadata
/// from headers and commits it without regenerating or decoding audio.
pub(super) fn read_or_rebuild_manifest_index(
    dir: &Path,
) -> Result<NativeAudiobookManifest, String> {
    let mut manifest = read_manifest(dir)?;
    if !manifest_has_complete_index(&manifest) {
        let (playback_chunks, audio_duration_sec, wav_bytes) =
            build_playback_index(dir, &manifest.chunks)?;
        manifest.source_signature = chunk_source_signature(&manifest.chunks);
        manifest.audio_duration_sec = audio_duration_sec;
        manifest.wav_bytes = wav_bytes;
        manifest.playback_chunks = playback_chunks;
        write_manifest_file(dir, &manifest)?;
        remove_legacy_playback_files(dir);
    }
    Ok(manifest)
}

/// Validate every invariant needed by the status and native playback fast paths.
///
/// This is deliberately centralized so a manifest cannot be considered complete
/// by status but malformed by playback (or vice versa).
pub(super) fn manifest_has_complete_index(manifest: &NativeAudiobookManifest) -> bool {
    manifest.version == AUDIOBOOK_MANIFEST_VERSION
        && manifest.source_signature == chunk_source_signature(&manifest.chunks)
        && manifest.wav_bytes > 0
        && playback_index_matches(
            &manifest.chunks,
            &manifest.playback_chunks,
            manifest.audio_duration_sec,
        )
}

/// Check ordered chunk identity and a contiguous global playback timeline.
///
/// The small tolerance permits WAV/container duration rounding while still
/// rejecting stale, reordered, missing, duplicated, or non-finite boundaries.
pub(super) fn playback_index_matches(
    chunks: &[NativeTtsInputChunk],
    playback_chunks: &[NativeAudiobookPlaybackChunk],
    audio_duration_sec: f64,
) -> bool {
    if chunks.is_empty()
        || chunks.len() != playback_chunks.len()
        || !audio_duration_sec.is_finite()
        || audio_duration_sec <= 0.0
    {
        return false;
    }

    let mut expected_start_sec = 0.0;
    for (index, (chunk, timing)) in chunks.iter().zip(playback_chunks).enumerate() {
        if timing.index != index
            || timing.chunk_id != chunk.id
            || !timing.start_sec.is_finite()
            || !timing.duration_sec.is_finite()
            || timing.duration_sec <= 0.0
            || (timing.start_sec - expected_start_sec).abs() > PLAYBACK_TIMING_TOLERANCE_SEC
        {
            return false;
        }
        expected_start_sec += timing.duration_sec;
    }

    (expected_start_sec - audio_duration_sec).abs() <= PLAYBACK_TIMING_TOLERANCE_SEC
}

/// Derive global chunk boundaries and aggregate bytes by reading each WAV header.
/// Fails closed if any expected chunk is missing or invalid; a complete manifest
/// must never advertise timing that cannot be played.
fn build_playback_index(
    dir: &Path,
    chunks: &[NativeTtsInputChunk],
) -> Result<(Vec<NativeAudiobookPlaybackChunk>, f64, usize), String> {
    let mut playback_chunks = Vec::with_capacity(chunks.len());
    let mut start_sec = 0f64;
    let mut wav_bytes = 0usize;
    for (index, chunk) in chunks.iter().enumerate() {
        let path = chunk_path(dir, index, chunk);
        let metadata = wav_metadata(&path).ok_or_else(|| {
            format!(
                "Missing or invalid saved audiobook chunk {}/{}: {}",
                index + 1,
                chunks.len(),
                path.display()
            )
        })?;
        let duration_sec = metadata.precise_audio_duration_sec;
        playback_chunks.push(NativeAudiobookPlaybackChunk {
            index,
            chunk_id: chunk.id.clone(),
            start_sec,
            duration_sec,
        });
        start_sec += duration_sec;
        wav_bytes += metadata.info.wav_bytes;
    }
    Ok((playback_chunks, start_sec, wav_bytes))
}

/// Replace manifest JSON through a complete same-directory staged file.
///
/// Readers see either the previous valid manifest or the new valid manifest,
/// never partially written JSON.
fn write_manifest_file(dir: &Path, manifest: &NativeAudiobookManifest) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(manifest)
        .map_err(|err| format!("Failed to serialize native audiobook manifest: {err}"))?;
    let path = dir.join("manifest.json");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("System clock error: {err}"))?
        .as_nanos();
    let temp_path = dir.join(format!("manifest.{nonce}.tmp"));
    fs::write(&temp_path, json)
        .map_err(|err| format!("Failed to write native audiobook manifest: {err}"))?;
    commit_staged_file(&temp_path, &path, "native audiobook manifest")
}

/// Invalidate derived track artifacts whenever canonical manifest/chunks change.
/// They are rebuilt or restored on demand and must never outlive source identity.
fn remove_legacy_playback_files(dir: &Path) {
    let _ = fs::remove_file(playback_track_path(dir));
    let _ = fs::remove_file(playback_metadata_path(dir));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunks() -> Vec<NativeTtsInputChunk> {
        vec![
            NativeTtsInputChunk {
                id: "a".into(),
                text: "First".into(),
                text_hash: Some("hash-a".into()),
                source_span: None,
            },
            NativeTtsInputChunk {
                id: "b".into(),
                text: "Second".into(),
                text_hash: Some("hash-b".into()),
                source_span: None,
            },
        ]
    }

    #[test]
    fn playback_index_requires_ordered_contiguous_current_chunks() {
        let chunks = chunks();
        let timings = vec![
            NativeAudiobookPlaybackChunk {
                index: 0,
                chunk_id: "a".into(),
                start_sec: 0.0,
                duration_sec: 1.25,
            },
            NativeAudiobookPlaybackChunk {
                index: 1,
                chunk_id: "b".into(),
                start_sec: 1.25,
                duration_sec: 2.0,
            },
        ];

        assert!(playback_index_matches(&chunks, &timings, 3.25));

        let mut stale = timings;
        stale[1].chunk_id = "wrong".into();
        assert!(!playback_index_matches(&chunks, &stale, 3.25));
    }

    #[test]
    fn legacy_manifest_without_model_id_defaults_to_kokoro() {
        let manifest: NativeAudiobookManifest = serde_json::from_value(serde_json::json!({
            "version": AUDIOBOOK_MANIFEST_VERSION,
            "documentUrl": "/legacy.html",
            "title": "Legacy",
            "voice": "af_heart",
            "speed": 1.0,
            "threadCount": 1,
            "chunks": [],
            "generatedAtMs": 0,
            "sourceSignature": "legacy",
            "audioDurationSec": 0.0,
            "wavBytes": 0,
            "playbackChunks": []
        }))
        .expect("deserialize legacy manifest");

        assert_eq!(manifest.model_id, DEFAULT_MODEL_ID);
        assert_eq!(manifest.text_preprocessor, "none");
    }

    #[test]
    fn new_save_writes_current_pending_and_complete_manifests() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("papercut-save-manifest-{nonce}"));
        fs::create_dir_all(dir.join("chunks")).expect("create test cache");
        let chunks = chunks();
        let request = NativeAudiobookSaveRequest {
            job_id: "test-save".into(),
            audiobook_id: "test-audiobook".into(),
            document_url: "/test.html".into(),
            title: "Test".into(),
            model_id: DEFAULT_MODEL_ID.into(),
            text_preprocessor: "none".into(),
            chunks: chunks.clone(),
            voice: "af_heart".into(),
            speed: 1.0,
            thread_count: Some(1),
            silma_nfe_step: None,
        };

        write_pending_manifest(&dir, &request, &chunks).expect("write pending manifest");
        let pending = read_manifest(&dir).expect("read pending manifest");
        assert_eq!(pending.version, AUDIOBOOK_MANIFEST_VERSION);
        assert!(!manifest_has_complete_index(&pending));

        for (index, chunk) in chunks.iter().enumerate() {
            let path = chunk_path(&dir, index, chunk);
            let data = [0u8; 4];
            let mut wav = Vec::new();
            wav.extend_from_slice(b"RIFF");
            wav.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
            wav.extend_from_slice(b"WAVEfmt ");
            wav.extend_from_slice(&16u32.to_le_bytes());
            wav.extend_from_slice(&[1, 0, 1, 0, 0x40, 0x1f, 0, 0, 0x80, 0x3e, 0, 0, 2, 0, 16, 0]);
            wav.extend_from_slice(b"data");
            wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
            wav.extend_from_slice(&data);
            fs::write(path, wav).expect("write test WAV");
        }

        write_manifest(&dir, &request, &chunks, 1).expect("write complete manifest");
        let complete = read_manifest(&dir).expect("read complete manifest");
        assert!(manifest_has_complete_index(&complete));
        assert_eq!(complete.playback_chunks.len(), chunks.len());
        let _ = fs::remove_dir_all(dir);
    }
}
