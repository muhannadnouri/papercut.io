//! Long-running native audiobook save jobs.
//!
//! A single blocking job generates every missing chunk in sequence, writing WAV
//! files straight to app data and emitting per-chunk progress on
//! [`SAVE_PROGRESS_EVENT`]. Cancellation is cooperative: the command inserts the
//! job id into the shared cancelled set and the loop checks it between chunks.
//! [`write_manifest`] records the save and is reused by the bundle import path.
//!
//! Rust notes for a JS reader: `Arc<Mutex<T>>` is a thread-safe shared handle —
//! `Arc` lets multiple owners point at the same value (reference counted) and
//! `Mutex` ensures one thread mutates it at a time. `.clone()` on an `Arc`
//! copies the handle, not the data, so the save thread and the command share one
//! engine/cancellation set.

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::Emitter;

use super::cache::{scan_audiobook, wav_info, wav_metadata};
use super::config::{AUDIOBOOK_MANIFEST_VERSION, SAVE_PROGRESS_EVENT};
use super::file_commit::commit_staged_file;
use super::models::{model_definition, DEFAULT_MODEL_ID};
use super::paths::{
    audiobook_dir, chunk_path, chunk_source_signature, playback_metadata_path, playback_track_path,
    speakable_chunks,
};
use super::preprocess::TextPreprocessor;
use super::synth::{ensure_engine, synthesize_to_file, SherpaTtsEngine};
use super::text_normalization::text_preview;
use crate::native_tts::platform::resolve_thread_count;
use crate::native_tts::state::NativeTtsState;
use crate::native_tts::types::{
    NativeAudiobookPlaybackChunk, NativeAudiobookSaveProgress, NativeAudiobookSaveRequest,
    NativeAudiobookSaveResponse, NativeTtsInputChunk,
};

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

const PLAYBACK_TIMING_TOLERANCE_SEC: f64 = 0.05;
#[derive(Default)]
struct SavePerformanceStats {
    total_source_chars: usize,
    total_synthesis_chars: usize,
    total_preprocess_ms: u128,
    total_synthesis_ms: u128,
    total_write_ms: u128,
    total_validate_ms: u128,
    total_indexing_ms: u128,
}

/// Preserve manifests written before model selection existed by treating them as Kokoro.
fn default_model_id() -> String {
    DEFAULT_MODEL_ID.into()
}

/// Preserve pre-diacritization manifests by treating absent metadata as original text.
fn default_text_preprocessor() -> String {
    "none".into()
}

/// Tauri command backend: start (or resume) saving the full audiobook.
///
/// Clears any stale cancellation for this job id, then runs the actual work on a
/// blocking thread (it does heavy inference and large file writes). Shared state
/// is `.clone()`d so the spawned closure owns its own handles.
pub(crate) async fn save_audiobook_native(
    app: tauri::AppHandle,
    state: tauri::State<'_, NativeTtsState>,
    request: NativeAudiobookSaveRequest,
) -> Result<NativeAudiobookSaveResponse, String> {
    let engine = state.engine.clone();
    let cancelled_jobs = state.cancelled_jobs.clone();
    {
        let mut cancelled = cancelled_jobs
            .lock()
            .map_err(|_| "Native TTS cancellation lock poisoned".to_string())?;
        cancelled.remove(&request.job_id);
    }

    tauri::async_runtime::spawn_blocking(move || {
        save_audiobook_native_blocking(app, engine, cancelled_jobs, request)
    })
    .await
    .map_err(|err| format!("Native audiobook save task failed: {err}"))?
}

/// Tauri command backend: request cancellation of a running save by job id.
/// Just records the id in the shared set; the save loop notices between chunks.
pub(crate) fn cancel_audiobook_save(
    state: tauri::State<'_, NativeTtsState>,
    job_id: String,
) -> Result<(), String> {
    let mut cancelled = state
        .cancelled_jobs
        .lock()
        .map_err(|_| "Native TTS cancellation lock poisoned".to_string())?;
    cancelled.insert(job_id);
    Ok(())
}

/// The actual save loop, run on a blocking thread.
///
/// Scans which chunks already exist (so Resume skips them), loads the engine
/// once, then for each missing chunk: checks for cancellation, synthesizes it to
/// disk, and emits progress. On success writes the manifest and a final "saved"
/// event. Returns aggregate totals for the whole audiobook.
fn save_audiobook_native_blocking(
    app: tauri::AppHandle,
    engine_state: Arc<Mutex<Option<SherpaTtsEngine>>>,
    cancelled_jobs: Arc<Mutex<HashSet<String>>>,
    request: NativeAudiobookSaveRequest,
) -> Result<NativeAudiobookSaveResponse, String> {
    let started = Instant::now();
    // Wall-clock mark for the final stale-temp sweep: only `.tmp` files left
    // untouched since before this job began are abandoned remnants safe to remove.
    let job_started = SystemTime::now();
    let chunks = speakable_chunks(&request.chunks);
    let total_chunks = chunks.len();
    if total_chunks == 0 {
        return Err("No speakable audiobook chunks to save".into());
    }
    let model = model_definition(&request.model_id)?;
    if !model.supports_text_preprocessor(&request.text_preprocessor) {
        return Err(format!(
            "Text preprocessor {:?} is not supported by model {}",
            request.text_preprocessor, model.display_name
        ));
    }
    let dir = audiobook_dir(&app, &request.audiobook_id)?;
    let chunks_dir = dir.join("chunks");
    fs::create_dir_all(&chunks_dir).map_err(|err| {
        format!(
            "Failed to create native audiobook directory {}: {err}",
            chunks_dir.display()
        )
    })?;

    // Persist the source index before generation so interrupted saves remain
    // discoverable without sending every chunk through later status IPC.
    write_pending_manifest(&dir, &request, &chunks)?;

    // Sweep chunk WAVs left by an earlier save of now-edited source text before
    // regenerating. Editing the source reuses the same audiobook id (its hash
    // omits chunk content), so without this a re-save holds both the stale and
    // new chunk sets on disk until the job finishes. Files whose names match the
    // current chunk set are kept, so this is safe on resume.
    prune_orphan_chunk_files(&dir, &chunks);

    // Scan with prune=true so invalid leftovers are removed and regenerated.
    let backend = "sherpa-onnx".to_string();
    let mut scan = scan_audiobook(&dir, &chunks, true);
    let mut cached_chunks = scan.cached_chunks;
    let mut generated_chunks = 0usize;
    let mut total_generate_ms = 0u128;
    let mut generated_audio_duration_sec = 0f32;
    let mut generated_wav_bytes = 0usize;
    let mut performance = SavePerformanceStats::default();
    let thread_count = resolve_thread_count(request.thread_count);

    // Initial "checking" progress so the UI shows the cache state immediately.
    emit_progress(
        &app,
        NativeAudiobookSaveProgress {
            job_id: request.job_id.clone(),
            status: "checking".into(),
            message: "Checking native audiobook cache".into(),
            cached_chunks,
            total_chunks,
            generated_chunks,
            chunk_id: None,
            chunk_number: None,
            text_chars: None,
            text_preview: None,
            generate_ms: None,
            preprocess_ms: None,
            synthesis_ms: None,
            write_ms: None,
            validate_ms: None,
            indexing_ms: None,
            synthesis_text_chars: None,
            total_source_chars: None,
            total_synthesis_chars: None,
            audio_duration_sec: None,
            wav_bytes: None,
            total_audio_duration_sec: scan.audio_duration_sec,
            total_wav_bytes: scan.wav_bytes,
            applied_thread_count: thread_count,
            backend: backend.clone(),
        },
    );

    // Load the engine once for the whole job, then build a richer backend label
    // that includes the model dir and thread count for diagnostics.
    let mut guard = engine_state
        .lock()
        .map_err(|_| "Native TTS engine lock poisoned".to_string())?;
    let engine = ensure_engine(&app, &mut guard, &request.model_id, Some(thread_count))?;
    let backend = format!(
        "{}:{}:{}:threads={}",
        engine.model.backend_name(),
        engine.model.id,
        engine.model_dir.display(),
        engine.num_threads
    );
    let text_preprocessor = TextPreprocessor::create(engine.model, &request.text_preprocessor)?;
    let backend = format!("{backend}:preprocessor={}", text_preprocessor.id());

    for (index, chunk) in chunks.iter().enumerate() {
        // Cooperative cancellation: bail out cleanly between chunks if asked.
        if is_cancelled(&cancelled_jobs, &request.job_id)? {
            emit_progress(
                &app,
                NativeAudiobookSaveProgress {
                    job_id: request.job_id.clone(),
                    status: "cancelled".into(),
                    message: "Audiobook save cancelled".into(),
                    cached_chunks,
                    total_chunks,
                    generated_chunks,
                    chunk_id: Some(chunk.id.clone()),
                    chunk_number: Some(index + 1),
                    text_chars: None,
                    text_preview: Some(text_preview(&chunk.text)),
                    generate_ms: None,
                    preprocess_ms: None,
                    synthesis_ms: None,
                    write_ms: None,
                    validate_ms: None,
                    indexing_ms: None,
                    synthesis_text_chars: None,
                    total_source_chars: None,
                    total_synthesis_chars: None,
                    audio_duration_sec: None,
                    wav_bytes: None,
                    total_audio_duration_sec: scan.audio_duration_sec,
                    total_wav_bytes: scan.wav_bytes,
                    applied_thread_count: thread_count,
                    backend: backend.clone(),
                },
            );
            return Err("Audiobook save cancelled".into());
        }

        // Skip chunks already saved as valid WAVs (Resume); drop invalid ones.
        let output_path = chunk_path(&dir, index, chunk);
        if output_path.is_file() {
            if wav_info(&output_path).is_some() {
                continue;
            }
            let _ = fs::remove_file(&output_path);
        }

        let source_chars = chunk.text.chars().count();
        // "Generating chunk N/total" before the (slow) synthesis call.
        emit_progress(
            &app,
            NativeAudiobookSaveProgress {
                job_id: request.job_id.clone(),
                status: "saving".into(),
                message: format!("Generating chunk {}/{}", index + 1, total_chunks),
                cached_chunks,
                total_chunks,
                generated_chunks,
                chunk_id: Some(chunk.id.clone()),
                chunk_number: Some(index + 1),
                text_chars: Some(source_chars),
                text_preview: Some(text_preview(&chunk.text)),
                generate_ms: None,
                preprocess_ms: None,
                synthesis_ms: None,
                write_ms: None,
                validate_ms: None,
                indexing_ms: None,
                synthesis_text_chars: None,
                total_source_chars: None,
                total_synthesis_chars: None,
                audio_duration_sec: None,
                wav_bytes: None,
                total_audio_duration_sec: scan.audio_duration_sec,
                total_wav_bytes: scan.wav_bytes,
                applied_thread_count: thread_count,
                backend: backend.clone(),
            },
        );

        // Synthesize this chunk to its file and fold its stats into the totals.
        let preprocess_started = Instant::now();
        let synthesis_text = text_preprocessor.process(&chunk.text)?;
        let preprocess_ms = preprocess_started.elapsed().as_millis();
        let synthesis_chars = synthesis_text.chars().count();
        log::debug!(
            "Prepared synthesis text: preprocessor={}, source_chars={}, synthesis_chars={}, source_preview={:?}, synthesis_preview={:?}",
            text_preprocessor.id(),
            source_chars,
            synthesis_chars,
            text_preview(&chunk.text),
            text_preview(&synthesis_text),
        );
        let result = synthesize_to_file(
            engine,
            &synthesis_text,
            &request.voice,
            request.speed,
            &output_path,
        )?;
        generated_chunks += 1;
        cached_chunks += 1;
        total_generate_ms += result.generate_ms;
        generated_audio_duration_sec += result.audio_duration_sec;
        generated_wav_bytes += result.wav_bytes;
        performance.total_source_chars += source_chars;
        performance.total_synthesis_chars += synthesis_chars;
        performance.total_preprocess_ms += preprocess_ms;
        performance.total_synthesis_ms += result.synthesis_ms;
        performance.total_write_ms += result.write_ms;
        performance.total_validate_ms += result.validate_ms;
        scan.audio_duration_sec += result.audio_duration_sec;
        scan.wav_bytes += result.wav_bytes;

        // "Saved chunk N/total" with this chunk's measured timing/size.
        emit_progress(
            &app,
            NativeAudiobookSaveProgress {
                job_id: request.job_id.clone(),
                status: "saving".into(),
                message: format!("Saved chunk {}/{}", cached_chunks, total_chunks),
                cached_chunks,
                total_chunks,
                generated_chunks,
                chunk_id: Some(chunk.id.clone()),
                chunk_number: Some(index + 1),
                text_chars: Some(source_chars),
                text_preview: Some(text_preview(&chunk.text)),
                generate_ms: Some(result.generate_ms),
                preprocess_ms: Some(preprocess_ms),
                synthesis_ms: Some(result.synthesis_ms),
                write_ms: Some(result.write_ms),
                validate_ms: Some(result.validate_ms),
                indexing_ms: None,
                synthesis_text_chars: Some(synthesis_chars),
                total_source_chars: Some(performance.total_source_chars),
                total_synthesis_chars: Some(performance.total_synthesis_chars),
                audio_duration_sec: Some(result.audio_duration_sec),
                wav_bytes: Some(result.wav_bytes),
                total_audio_duration_sec: scan.audio_duration_sec,
                total_wav_bytes: scan.wav_bytes,
                applied_thread_count: thread_count,
                backend: backend.clone(),
            },
        );
    }

    // Final sweep before the manifest is finalized: catches any orphan that
    // appeared since the start sweep, so disk use tracks exactly the current
    // chunk set. The companion sweep then removes abandoned `.tmp` remnants.
    prune_orphan_chunk_files(&dir, &chunks);
    prune_stale_temp_files(&dir, job_started);

    // Record the manifest and clear any cancellation flag for this job. The
    // returned totals are measured from WAV headers, so they are the canonical
    // values to report instead of the per-chunk f32 running sum.
    let indexing_started = Instant::now();
    let (total_audio_duration_sec, total_wav_bytes) =
        write_manifest(&dir, &request, &chunks, thread_count)?;
    performance.total_indexing_ms = indexing_started.elapsed().as_millis();
    clear_cancelled(&cancelled_jobs, &request.job_id)?;

    // Final "saved" event with whole-job totals.
    emit_progress(
        &app,
        NativeAudiobookSaveProgress {
            job_id: request.job_id.clone(),
            status: "saved".into(),
            message: "Audiobook saved".into(),
            cached_chunks: total_chunks,
            total_chunks,
            generated_chunks,
            chunk_id: None,
            chunk_number: None,
            text_chars: None,
            text_preview: None,
            generate_ms: Some(total_generate_ms),
            preprocess_ms: Some(performance.total_preprocess_ms),
            synthesis_ms: Some(performance.total_synthesis_ms),
            write_ms: Some(performance.total_write_ms),
            validate_ms: Some(performance.total_validate_ms),
            indexing_ms: Some(performance.total_indexing_ms),
            synthesis_text_chars: None,
            total_source_chars: Some(performance.total_source_chars),
            total_synthesis_chars: Some(performance.total_synthesis_chars),
            audio_duration_sec: Some(generated_audio_duration_sec),
            wav_bytes: Some(generated_wav_bytes),
            total_audio_duration_sec: total_audio_duration_sec as f32,
            total_wav_bytes,
            applied_thread_count: thread_count,
            backend: backend.clone(),
        },
    );

    Ok(NativeAudiobookSaveResponse {
        job_id: request.job_id,
        cached_chunks: total_chunks,
        total_chunks,
        generated_chunks,
        complete: true,
        dir: dir.display().to_string(),
        generate_ms: started.elapsed().as_millis(),
        audio_duration_sec: total_audio_duration_sec,
        wav_bytes: total_wav_bytes,
        applied_thread_count: thread_count,
        backend,
    })
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

/// Remove chunk WAVs that no longer belong to the current chunk set.
///
/// Chunk filenames embed a content hash, so editing the source document and
/// re-saving into the same audiobook id writes new filenames while the stale
/// ones linger forever ([`scan_audiobook`] only prunes invalid WAVs at expected
/// paths, never valid WAVs with old names). Sweep them once every current chunk
/// is generated.
///
/// In-flight `.tmp` staging files are skipped on purpose: [`synthesize_to_file`]
/// writes each chunk to `<name>.<nonce>.tmp` and atomically renames it into
/// place, so a `.tmp` here can belong to a concurrent save of the same audiobook
/// id mid-write. Deleting it would break that save's commit rename. Abandoned
/// temps (each `.tmp` carries a unique nonce, so a crashed write is never
/// overwritten by a later attempt) are reclaimed separately by
/// [`prune_stale_temp_files`] using a job-start cutoff.
fn prune_orphan_chunk_files(dir: &Path, chunks: &[NativeTtsInputChunk]) {
    let expected: HashSet<std::ffi::OsString> = chunks
        .iter()
        .enumerate()
        .filter_map(|(index, chunk)| {
            chunk_path(dir, index, chunk)
                .file_name()
                .map(|name| name.to_os_string())
        })
        .collect();

    let Ok(entries) = fs::read_dir(dir.join("chunks")) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry
            .file_type()
            .map(|kind| kind.is_file())
            .unwrap_or(false)
        {
            continue;
        }
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "tmp") {
            continue;
        }
        if !expected.contains(&entry.file_name()) {
            let _ = fs::remove_file(path);
        }
    }
}

/// Reclaim abandoned `.tmp` chunk staging files from crashed earlier writes.
///
/// A successful [`synthesize_to_file`] leaves no temp (it renames into place) and
/// a failed one removes its own, so a `.tmp` still present here is either an
/// orphan from a process that died mid-write or a concurrent save's live staging
/// file. Each carries a unique nonce, so an abandoned one is never overwritten by
/// a later attempt and would otherwise linger forever (inflating disk use and the
/// byte totals reported on delete). Only files last modified before `cutoff` (this
/// job's start) are swept: a concurrent save's in-flight temp is necessarily
/// written after this job began, so its commit rename is never disturbed. Temps
/// with unreadable mtime are left alone rather than risking a live write.
fn prune_stale_temp_files(dir: &Path, cutoff: SystemTime) {
    let Ok(entries) = fs::read_dir(dir.join("chunks")) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry
            .file_type()
            .map(|kind| kind.is_file())
            .unwrap_or(false)
        {
            continue;
        }
        let path = entry.path();
        if !path.extension().is_some_and(|ext| ext == "tmp") {
            continue;
        }
        let modified = entry.metadata().ok().and_then(|meta| meta.modified().ok());
        if modified.is_some_and(|time| time < cutoff) {
            let _ = fs::remove_file(path);
        }
    }
}

/// Invalidate derived track artifacts whenever canonical manifest/chunks change.
/// They are rebuilt or restored on demand and must never outlive source identity.
fn remove_legacy_playback_files(dir: &Path) {
    let _ = fs::remove_file(playback_track_path(dir));
    let _ = fs::remove_file(playback_metadata_path(dir));
}

/// Emit one save-progress event to the frontend. Errors are ignored on purpose
/// (a dropped progress event must never fail the save).
fn emit_progress(app: &tauri::AppHandle, progress: NativeAudiobookSaveProgress) {
    let _ = app.emit(SAVE_PROGRESS_EVENT, progress);
}

/// Has this job been asked to cancel? Reads the shared cancelled-id set.
fn is_cancelled(
    cancelled_jobs: &Arc<Mutex<HashSet<String>>>,
    job_id: &str,
) -> Result<bool, String> {
    let cancelled = cancelled_jobs
        .lock()
        .map_err(|_| "Native TTS cancellation lock poisoned".to_string())?;
    Ok(cancelled.contains(job_id))
}

/// Remove this job's id from the cancelled set once it finishes successfully.
fn clear_cancelled(
    cancelled_jobs: &Arc<Mutex<HashSet<String>>>,
    job_id: &str,
) -> Result<(), String> {
    let mut cancelled = cancelled_jobs
        .lock()
        .map_err(|_| "Native TTS cancellation lock poisoned".to_string())?;
    cancelled.remove(job_id);
    Ok(())
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
    fn prune_removes_orphan_chunk_files_only() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("papercut-prune-orphans-{nonce}"));
        let chunks = chunks();
        fs::create_dir_all(dir.join("chunks")).expect("create chunks dir");

        // Write the expected WAV for each current chunk plus a leftover from an
        // earlier save of different source text (a different content hash).
        for (index, chunk) in chunks.iter().enumerate() {
            fs::write(chunk_path(&dir, index, chunk), b"wav").expect("write expected chunk");
        }
        let orphan = dir.join("chunks").join("00001-a-deadbeefdeadbeef.wav");
        fs::write(&orphan, b"stale").expect("write orphan chunk");
        // A concurrent save of the same audiobook id stages its chunk here mid-write.
        let in_flight = dir.join("chunks").join("00001-a-hash-a.123456789.tmp");
        fs::write(&in_flight, b"writing").expect("write in-flight temp");

        prune_orphan_chunk_files(&dir, &chunks);

        assert!(!orphan.exists(), "orphan chunk should be removed");
        assert!(
            in_flight.exists(),
            "in-flight temp must be left for the concurrent save's commit rename"
        );
        for (index, chunk) in chunks.iter().enumerate() {
            assert!(
                chunk_path(&dir, index, chunk).is_file(),
                "current chunk should be kept"
            );
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn prune_stale_temp_files_removes_only_pre_job_temps() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("papercut-prune-temps-{nonce}"));
        fs::create_dir_all(dir.join("chunks")).expect("create chunks dir");

        // An abandoned temp from a crashed earlier write, then the job-start mark,
        // then a concurrent save's in-flight temp staged after the job began.
        let stale = dir.join("chunks").join("00001-a-hash-a.111.tmp");
        fs::write(&stale, b"abandoned").expect("write stale temp");
        std::thread::sleep(std::time::Duration::from_millis(20));
        let cutoff = SystemTime::now();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let in_flight = dir.join("chunks").join("00002-b-hash-b.222.tmp");
        fs::write(&in_flight, b"writing").expect("write in-flight temp");
        // A committed WAV must be untouched by the temp sweep.
        let committed = dir.join("chunks").join("00001-a-hash-a.wav");
        fs::write(&committed, b"wav").expect("write committed chunk");

        prune_stale_temp_files(&dir, cutoff);

        assert!(!stale.exists(), "abandoned pre-job temp should be removed");
        assert!(
            in_flight.exists(),
            "a concurrent save's temp written after the job started must be kept"
        );
        assert!(committed.exists(), "committed WAVs must not be touched");
        let _ = fs::remove_dir_all(dir);
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
