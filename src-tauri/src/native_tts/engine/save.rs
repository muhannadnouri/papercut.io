//! Long-running native audiobook save jobs.
//!
//! A single blocking job generates every missing chunk in sequence, writing WAV
//! files straight to app data and emitting per-chunk progress on
//! [`SAVE_PROGRESS_EVENT`]. Cancellation is cooperative: the command inserts the
//! job id into the shared cancelled set and the loop checks it between chunks.
//! Manifest writing and cache pruning live in sibling modules so this file stays
//! focused on the save lifecycle.
//!
//! Rust notes for a JS reader: `Arc<Mutex<T>>` is a thread-safe shared handle —
//! `Arc` lets multiple owners point at the same value (reference counted) and
//! `Mutex` ensures one thread mutates it at a time. `.clone()` on an `Arc`
//! copies the handle, not the data, so the save thread and the command share one
//! engine/cancellation set.

use std::collections::HashSet;
use std::fs;
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime};

use tauri::Emitter;

use super::cache::{scan_audiobook, wav_info};
use super::config::SAVE_PROGRESS_EVENT;
use super::manifest::{write_manifest, write_pending_manifest};
use super::models::{model_definition, TtsModelBackend};
use super::paths::{audiobook_dir, chunk_path, speakable_chunks};
use super::preprocess::TextPreprocessor;
use super::prune::{prune_orphan_chunk_files, prune_stale_temp_files};
use super::silma_sidecar::normalize_silma_nfe_step;
use super::synth::{
    ensure_sherpa_engine, ensure_silma_engine, synthesize_silma_to_file, synthesize_to_file,
    LoadedTtsEngine,
};
use super::text_normalization::text_preview;
use crate::native_tts::platform::resolve_thread_count;
use crate::native_tts::state::NativeTtsState;
use crate::native_tts::types::{
    NativeAudiobookSaveProgress, NativeAudiobookSaveRequest, NativeAudiobookSaveResponse,
};

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
    engine_state: Arc<Mutex<Option<LoadedTtsEngine>>>,
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
    let backend = model.backend_name().to_string();
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
    let backend = match model.backend {
        TtsModelBackend::SherpaOnnx => {
            let engine =
                ensure_sherpa_engine(&app, &mut guard, &request.model_id, Some(thread_count))?;
            format!(
                "{}:{}:{}:threads={}",
                engine.model.backend_name(),
                engine.model.id,
                engine.model_dir.display(),
                engine.num_threads
            )
        }
        TtsModelBackend::SilmaSidecar => {
            let engine =
                ensure_silma_engine(&app, &mut guard, &request.model_id, request.thread_count)?;
            format!(
                "{}:{}:{}:sample_rate={}:device={}:torch_threads={}:torch_interop={}",
                engine.model.backend_name(),
                engine.model.id,
                engine.model_dir.display(),
                engine.sample_rate,
                engine.device,
                engine.torch_threads,
                engine.torch_interop_threads,
            )
        }
    };
    let text_preprocessor = TextPreprocessor::create(model, &request.text_preprocessor)?;
    let silma_nfe_step = normalize_silma_nfe_step(request.silma_nfe_step.unwrap_or(16));
    let backend = match model.backend {
        TtsModelBackend::SilmaSidecar => format!(
            "{backend}:preprocessor={}:nfe={}",
            text_preprocessor.id(),
            silma_nfe_step
        ),
        TtsModelBackend::SherpaOnnx => format!("{backend}:preprocessor={}", text_preprocessor.id()),
    };

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
        let result = match model.backend {
            TtsModelBackend::SherpaOnnx => match guard.as_ref() {
                Some(LoadedTtsEngine::Sherpa(engine)) => synthesize_to_file(
                    engine,
                    &synthesis_text,
                    &request.voice,
                    request.speed,
                    &output_path,
                ),
                _ => Err("Native sherpa TTS engine unavailable".into()),
            },
            TtsModelBackend::SilmaSidecar => match guard.as_mut() {
                Some(LoadedTtsEngine::Silma(engine)) => synthesize_silma_to_file(
                    engine,
                    &synthesis_text,
                    &request.voice,
                    request.speed,
                    silma_nfe_step,
                    &output_path,
                ),
                _ => Err("Native SILMA sidecar engine unavailable".into()),
            },
        }?;
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
