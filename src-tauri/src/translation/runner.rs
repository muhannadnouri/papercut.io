//! Translation job runner.
//!
//! Owns the long-running translate/persist path: preflight, cache reuse,
//! native CTranslate2 batches, inline phrase probes, progress/cancellation
//! events, and handoff to durable storage. Capability/status reporting lives
//! in `capabilities`; batch planning lives in `job`.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use tauri::Emitter;

use super::cache::{load_segment_cache, save_segment_cache, TranslationSegmentCache};
use super::config::{
    DEFAULT_BATCH_SEGMENT_LIMIT, DEFAULT_MAX_SEGMENT_CHARS, DEFAULT_TRANSLATION_QUALITY_MODE,
    TRANSLATION_JOB_PROGRESS_EVENT,
};
use super::ctranslate2::CTranslate2Engine;
use super::engine::{
    TranslationBatchInput, TranslationEngine, TranslationSegmentContext, TranslationSegmentInput,
};
use super::hy_mt2::HyMt2Engine;
use super::inline_markup::{inline_phrase_probes_by_block, InlinePhraseProbe};
use super::job::{
    build_translation_cache_key, plan_translation_job, TranslationBatchPlan, TranslationJobPlan,
};
use super::model_store::{manifest_for, resolve_translation_model_dir};
use super::models::{find_planned_model, TranslationModelDefinition};
use super::quality::lowered_contains_word_bounded;
use super::source::{load_translation_source_document, TranslationSourceDocument};
use super::state::TranslationState;
use super::storage::{
    persist_translated_document, PersistTranslationFragment, PersistTranslationInlinePhrase,
    PersistTranslationRequest, PersistTranslationSection,
};
use super::types::{
    TranslationCancelRequest, TranslationGlossaryEntry, TranslationJobProgress,
    TranslationRepairMode, TranslationStartRequest, TranslationStartResponse,
};

const MAX_INLINE_PHRASE_PROBES_PER_SEGMENT: usize = 8;

/// Start a translation job and persist the completed output as a derived upload.
pub(super) fn start_translation<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    state: &TranslationState,
    request: TranslationStartRequest,
) -> Result<TranslationStartResponse, String> {
    let model = find_planned_model(&request.model_id).ok_or_else(|| {
        format!(
            "Translation model {:?} is not in the planned catalog",
            request.model_id
        )
    })?;
    validate_model_request(model, &request)?;
    let manifest = manifest_for(model);
    if !manifest.installable {
        return Err(format!(
            "{} is still a planning candidate and cannot run translation preflight yet.",
            model.name
        ));
    }
    let model_dir = resolve_translation_model_dir(app, manifest).map_err(|_| {
        format!(
            "{} must be installed before translation can run. Open the Translation tab and install the model first.",
            model.name
        )
    })?;

    let _active_job = state.claim_job(
        &build_translation_cache_key(&request),
        manifest.directory_name,
    )?;
    let source = load_translation_source_document(app, &request.document_url)?;
    let source_blocks = source.blocks.iter().map(|block| block.text.as_str());
    let plan = plan_translation_job(
        request,
        source_blocks,
        DEFAULT_MAX_SEGMENT_CHARS,
        DEFAULT_BATCH_SEGMENT_LIMIT,
    )?;
    let job_id = plan
        .request
        .job_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&plan.cache_key)
        .to_string();
    clear_cancelled(&state.cancelled_jobs, &job_id)?;
    let started = Instant::now();
    emit_translation_progress(
        app,
        progress(
            &job_id,
            "loading-model",
            "Loading translation model",
            &plan,
            started,
            None,
            0,
            0,
            0,
            0,
            "",
        ),
    )?;
    let mut engine = load_translation_engine(model, &model_dir)?;
    match run_translation_batches(
        app,
        state,
        engine.as_mut(),
        &plan,
        &source,
        &job_id,
        started,
    ) {
        Ok(summary) => {
            emit_translation_progress(
                app,
                progress(
                    &job_id,
                    "validating",
                    "Validating translated document",
                    &plan,
                    started,
                    None,
                    plan.total_segments,
                    summary.cached_segments,
                    0,
                    plan.batches.len(),
                    &summary.preview,
                ),
            )?;
            let stored = persist_translated_document(
                app,
                PersistTranslationRequest {
                    source: source.clone(),
                    source_language: plan.request.source_language.clone(),
                    target_language: plan.request.target_language.clone(),
                    model_id: plan.request.model_id.clone(),
                    quality_mode: plan.request.quality_mode.clone(),
                    repair_mode: plan.request.repair_mode.clone(),
                    job_id: job_id.clone(),
                    glossary: plan.request.glossary.clone(),
                    translated_sections: summary.sections,
                },
            )?;
            emit_translation_progress(
                app,
                progress(
                    &job_id,
                    "stored",
                    "Translated document stored",
                    &plan,
                    started,
                    None,
                    plan.total_segments,
                    summary.cached_segments,
                    0,
                    plan.batches.len(),
                    &summary.preview,
                ),
            )?;
            Ok(TranslationStartResponse {
                job_id,
                status: "stored".into(),
                message: format!(
                    "Stored translated document '{}': {} segment(s), {} batch(es), preview: {}",
                    stored.title,
                    plan.total_segments,
                    plan.batches.len(),
                    summary.preview
                ),
            })
        }
        Err(err) => {
            let _ = clear_cancelled(&state.cancelled_jobs, &job_id);
            if err.status != "cancelled" {
                let _ = emit_translation_progress(
                    app,
                    progress(
                        &job_id,
                        err.status,
                        &err.message,
                        &plan,
                        started,
                        None,
                        err.completed_segments,
                        err.cached_segments,
                        0,
                        err.completed_batches,
                        &err.preview,
                    ),
                );
            }
            Err(format!(
                "Translation did not complete. Planned {} translatable segments in {} batches for '{}', using installed model {} at {}. {}",
                plan.total_segments,
                plan.batches.len(),
                source.title,
                plan.request.model_id,
                model_dir.display(),
                err.message
            ))
        }
    }
}

/// Mark a running job as cancelled.
///
/// The batch loop checks this cooperative flag between batches so cancellation
/// stays simple and avoids interrupting native inference in the middle of a
/// backend call.
pub(super) fn cancel_translation(
    state: &TranslationState,
    request: TranslationCancelRequest,
) -> Result<(), String> {
    let mut cancelled = state
        .cancelled_jobs
        .lock()
        .map_err(|_| "Translation cancellation lock poisoned".to_string())?;
    cancelled.insert(request.job_id);
    Ok(())
}

struct TranslationRunSummary {
    preview: String,
    cached_segments: usize,
    sections: Vec<PersistTranslationSection>,
}

struct TranslationRunFailure {
    status: &'static str,
    message: String,
    completed_segments: usize,
    cached_segments: usize,
    completed_batches: usize,
    preview: String,
}

/// Mutable run counters shared by every progress event and failure report.
///
/// Centralizing these keeps the batch loop's many error paths from carrying
/// six positional arguments each; the values always reflect the loop's current
/// position, so a failure snapshot is just a read of this struct.
struct BatchRunState<'a> {
    job_id: &'a str,
    plan: &'a TranslationJobPlan,
    started: Instant,
    completed_segments: usize,
    cached_segments: usize,
    completed_batches: usize,
    current_heading: Option<String>,
    preview: String,
}

impl BatchRunState<'_> {
    fn failure(&self, status: &'static str, message: impl Into<String>) -> TranslationRunFailure {
        TranslationRunFailure {
            status,
            message: message.into(),
            completed_segments: self.completed_segments,
            cached_segments: self.cached_segments,
            completed_batches: self.completed_batches,
            preview: truncate_preview(self.preview.trim(), 180),
        }
    }

    fn progress(
        &self,
        status: &str,
        message: &str,
        reused_segments_in_batch: usize,
    ) -> TranslationJobProgress {
        progress(
            self.job_id,
            status,
            message,
            self.plan,
            self.started,
            self.current_heading.clone(),
            self.completed_segments,
            self.cached_segments,
            reused_segments_in_batch,
            self.completed_batches,
            &self.preview,
        )
    }

    fn note_preview(&mut self, text: &str) {
        if self.preview.is_empty() && !text.trim().is_empty() {
            self.preview = text.trim().to_string();
        }
    }
}

/// Translate every planned batch and keep source-block order intact.
///
/// Segments are batched for engine throughput, but storage needs section-sized
/// text again. The source block index lets us stitch translated segments back
/// into their original document sections before the durable variant is written.
#[allow(clippy::too_many_arguments)]
fn run_translation_batches<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    state: &TranslationState,
    engine: &mut dyn TranslationEngine,
    plan: &TranslationJobPlan,
    source: &TranslationSourceDocument,
    job_id: &str,
    started: Instant,
) -> Result<TranslationRunSummary, TranslationRunFailure> {
    let mut run = BatchRunState {
        job_id,
        plan,
        started,
        completed_segments: 0,
        cached_segments: 0,
        completed_batches: 0,
        current_heading: None,
        preview: String::new(),
    };
    emit_translation_progress(
        app,
        run.progress("starting", "Preparing translation batches", 0),
    )
    .map_err(|err| run.failure("failed", err))?;

    let mut translated_blocks = vec![String::new(); source.blocks.len()];
    let mut translated_fragments =
        vec![Vec::<PersistTranslationFragment>::new(); source.blocks.len()];
    let inline_phrase_probes = {
        let probes = inline_phrase_probes_by_block(&source.view_html);
        // Probe offsets are only meaningful when reader-DOM blocks line up
        // one-to-one with planned source blocks; on mismatch, skipping hints
        // beats attaching them to the wrong segments.
        if probes.len() == source.blocks.len() {
            probes
        } else {
            log::warn!(
                "translation: skipping inline phrase probes for '{}': reader DOM has {} blocks, planner has {}",
                source.title,
                probes.len(),
                source.blocks.len()
            );
            Vec::new()
        }
    };
    let mut cache = load_segment_cache(app, plan).map_err(|err| run.failure("failed", err))?;
    for batch in &plan.batches {
        run.completed_batches = batch.index;
        run.current_heading = batch
            .segments
            .first()
            .and_then(|segment| source.blocks.get(segment.source_block_index))
            .and_then(|block| block.heading.clone());
        if is_cancelled(&state.cancelled_jobs, job_id).map_err(|err| run.failure("failed", err))? {
            emit_translation_progress(app, run.progress("cancelled", "Translation cancelled", 0))
                .map_err(|err| run.failure("failed", err))?;
            clear_cancelled(&state.cancelled_jobs, job_id)
                .map_err(|err| run.failure("failed", err))?;
            return Err(run.failure("cancelled", "Translation cancelled"));
        }

        let (inline_phrase_hints, stored_probe_translations) =
            translate_inline_phrase_hints(engine, plan, batch, &inline_phrase_probes, &mut cache);
        let mut pending_batch = TranslationBatchPlan {
            index: batch.index,
            segments: Vec::new(),
        };
        let mut reused_segments_in_batch = 0;
        let mut materialized_memory_reuse = false;
        for segment in &batch.segments {
            if let Some(cached_text) = cache.translated_text_for(segment).map(str::to_owned) {
                run.completed_segments += 1;
                run.cached_segments += 1;
                reused_segments_in_batch += 1;
                append_translated_segment(
                    &mut translated_blocks,
                    &mut translated_fragments,
                    segment,
                    segment.source_block_index,
                    &cached_text,
                    inline_phrase_hints
                        .get(&segment.id)
                        .cloned()
                        .unwrap_or_default(),
                );
                run.note_preview(&cached_text);
            } else if let Some(memory_text) =
                cache.translated_text_for_source(segment).map(str::to_owned)
            {
                run.completed_segments += 1;
                run.cached_segments += 1;
                reused_segments_in_batch += 1;
                materialized_memory_reuse = true;
                append_translated_segment(
                    &mut translated_blocks,
                    &mut translated_fragments,
                    segment,
                    segment.source_block_index,
                    &memory_text,
                    inline_phrase_hints
                        .get(&segment.id)
                        .cloned()
                        .unwrap_or_default(),
                );
                run.note_preview(&memory_text);
                cache.store_translation(segment, memory_text);
            } else {
                pending_batch.segments.push(segment.clone());
            }
        }

        let reused_only = pending_batch.segments.is_empty();
        if !reused_only {
            let outputs = engine
                .translate_batch(batch_input(plan, &pending_batch))
                .map_err(|err| {
                    run.failure(
                        "failed",
                        format!("Native in-memory translation did not complete: {err}"),
                    )
                })?;
            let mut translated_count = 0;
            for (segment, output) in pending_batch.segments.iter().zip(outputs.iter()) {
                let translated_text = output.text.trim().to_string();
                if translated_text.is_empty() {
                    return Err(run.failure(
                        "failed",
                        format!(
                            "Native translation returned empty output for source section {}",
                            segment.source_block_index + 1
                        ),
                    ));
                }
                translated_count += 1;
                append_translated_segment(
                    &mut translated_blocks,
                    &mut translated_fragments,
                    segment,
                    segment.source_block_index,
                    &translated_text,
                    inline_phrase_hints
                        .get(&segment.id)
                        .cloned()
                        .unwrap_or_default(),
                );
                run.note_preview(&translated_text);
                cache.store_translation(segment, translated_text);
            }
            run.completed_segments += translated_count;
            save_segment_cache(app, plan, &cache).map_err(|err| run.failure("failed", err))?;
        } else if materialized_memory_reuse || stored_probe_translations {
            save_segment_cache(app, plan, &cache).map_err(|err| run.failure("failed", err))?;
        }

        run.completed_batches = batch.index + 1;
        emit_translation_progress(
            app,
            run.progress(
                "translating",
                if reused_only {
                    "Reused cached batch"
                } else {
                    "Translated batch"
                },
                reused_segments_in_batch,
            ),
        )
        .map_err(|err| run.failure("failed", err))?;
    }

    emit_translation_progress(
        app,
        run.progress("completed", "Translation completed in memory", 0),
    )
    .map_err(|err| run.failure("failed", err))?;
    clear_cancelled(&state.cancelled_jobs, job_id).map_err(|err| run.failure("failed", err))?;
    let mut current_translated_heading: Option<String> = None;
    let sections = translated_blocks
        .into_iter()
        .enumerate()
        .map(|(index, text)| {
            let text = text.trim().to_string();
            let source_block = source.blocks.get(index);
            let heading = source_block.and_then(|block| block.heading.clone());
            let is_heading = source_block.is_some_and(|block| {
                block
                    .heading
                    .as_deref()
                    .is_some_and(|heading| heading.trim() == block.text.trim())
            });
            if is_heading {
                current_translated_heading = Some(text.clone());
            }
            PersistTranslationSection {
                heading: current_translated_heading
                    .clone()
                    .or_else(|| heading.clone()),
                source_heading: heading,
                source_ordinal: source_block.map(|block| block.ordinal).unwrap_or(index),
                is_heading,
                text,
                fragments: translated_fragments.get(index).cloned().unwrap_or_default(),
            }
        })
        .collect();
    Ok(TranslationRunSummary {
        preview: truncate_preview(run.preview.trim(), 240),
        cached_segments: run.cached_segments,
        sections,
    })
}

/// Translate emphasized source phrases as small repair hints.
///
/// Main OPUS-MT output does not expose word alignment. These probes give the
/// renderer a target phrase to exact-match when bold/italic source text was
/// translated rather than carried over unchanged. Probe failure is non-fatal:
/// plain/proportional inline rendering is safer than failing a completed book.
///
/// Probe phrases go through the shared translation memory first, so resumed or
/// repeated jobs do not re-run the engine for every emphasized phrase. Returns
/// the hints keyed by owning segment id, plus whether new probe translations
/// entered the cache and need a save.
fn translate_inline_phrase_hints(
    engine: &mut dyn TranslationEngine,
    plan: &TranslationJobPlan,
    batch: &TranslationBatchPlan,
    probes_by_block: &[Vec<InlinePhraseProbe>],
    cache: &mut TranslationSegmentCache,
) -> (BTreeMap<String, Vec<PersistTranslationInlinePhrase>>, bool) {
    let mut hints = BTreeMap::<String, Vec<PersistTranslationInlinePhrase>>::new();
    let mut owners = Vec::new();
    let mut inputs = Vec::new();
    for segment in &batch.segments {
        let probes = inline_phrase_probes_for_segment(segment, probes_by_block);
        for (index, probe) in probes.into_iter().enumerate() {
            if let Some(remembered) = cache.translated_probe_text(&probe.text) {
                hints
                    .entry(segment.id.clone())
                    .or_default()
                    .push(PersistTranslationInlinePhrase {
                        source_text: probe.text.clone(),
                        text: remembered.to_string(),
                    });
                continue;
            }
            let id = format!("{}:inline:{index}", segment.id);
            owners.push((id.clone(), segment.id.clone(), probe.text.clone()));
            inputs.push(TranslationSegmentInput {
                id,
                text: probe.text,
                context: TranslationSegmentContext {
                    glossary: glossary_for_segment(&plan.request.glossary, &segment.text),
                },
            });
        }
    }
    if inputs.is_empty() {
        return (hints, false);
    }

    let owner_by_probe = owners
        .into_iter()
        .map(|(probe_id, owner_id, source_text)| (probe_id, (owner_id, source_text)))
        .collect::<BTreeMap<_, _>>();
    let outputs = match engine.translate_batch(TranslationBatchInput {
        model_id: plan.request.model_id.clone(),
        source_language: plan.request.source_language.clone(),
        target_language: plan.request.target_language.clone(),
        quality_mode: plan.request.quality_mode.clone(),
        repair_mode: plan.request.repair_mode.clone(),
        glossary: plan.request.glossary.clone(),
        segments: inputs,
    }) {
        Ok(outputs) => outputs,
        Err(err) => {
            // Non-fatal by design: the job continues with proportional inline
            // projection, but the degradation must be visible in logs.
            log::warn!("translation: inline phrase probe batch failed: {err}");
            return (hints, false);
        }
    };

    let mut stored_new_translation = false;
    for output in outputs {
        let Some((owner_id, source_text)) = owner_by_probe.get(&output.id) else {
            continue;
        };
        let translated = output.text.trim();
        if translated.is_empty() {
            continue;
        }
        cache.store_probe_translation(source_text, translated.to_string());
        stored_new_translation = true;
        hints
            .entry(owner_id.clone())
            .or_default()
            .push(PersistTranslationInlinePhrase {
                source_text: source_text.clone(),
                text: translated.to_string(),
            });
    }
    (hints, stored_new_translation)
}

fn inline_phrase_probes_for_segment(
    segment: &super::segment::TranslationTextSegment,
    probes_by_block: &[Vec<InlinePhraseProbe>],
) -> Vec<InlinePhraseProbe> {
    let Some(probes) = probes_by_block.get(segment.source_block_index) else {
        return Vec::new();
    };
    let mut seen = BTreeSet::new();
    probes
        .iter()
        .filter(|probe| {
            probe.source_start >= segment.source_start && probe.source_end <= segment.source_end
        })
        .filter(|probe| seen.insert(probe.text.to_lowercase()))
        .take(MAX_INLINE_PHRASE_PROBES_PER_SEGMENT)
        .cloned()
        .collect()
}

fn append_translated_segment(
    blocks: &mut [String],
    fragments: &mut [Vec<PersistTranslationFragment>],
    segment: &super::segment::TranslationTextSegment,
    source_block_index: usize,
    text: &str,
    inline_phrases: Vec<PersistTranslationInlinePhrase>,
) {
    let Some(block_text) = blocks.get_mut(source_block_index) else {
        return;
    };
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    if !block_text.is_empty() {
        block_text.push(' ');
    }
    block_text.push_str(text);
    if let Some(block_fragments) = fragments.get_mut(source_block_index) {
        block_fragments.push(PersistTranslationFragment {
            source_start: segment.source_start,
            source_end: segment.source_end,
            source_text: segment.text.trim().to_string(),
            text: text.to_string(),
            inline_phrases,
        });
    }
}

/// Convert a planned batch into the engine-agnostic input shape.
///
/// Keeping this small boundary lets future engines add context/glossary fields
/// without changing the planner or storage code.
fn batch_input(plan: &TranslationJobPlan, batch: &TranslationBatchPlan) -> TranslationBatchInput {
    TranslationBatchInput {
        model_id: plan.request.model_id.clone(),
        source_language: plan.request.source_language.clone(),
        target_language: plan.request.target_language.clone(),
        quality_mode: plan.request.quality_mode.clone(),
        repair_mode: plan.request.repair_mode.clone(),
        glossary: plan.request.glossary.clone(),
        segments: batch
            .segments
            .iter()
            .map(|segment| TranslationSegmentInput {
                id: segment.id.clone(),
                text: segment.text.clone(),
                context: TranslationSegmentContext {
                    glossary: glossary_for_segment(&plan.request.glossary, &segment.text),
                },
            })
            .collect(),
    }
}

/// Select glossary entries relevant to one segment by exact source-term match.
///
/// This keeps prompt/context payloads bounded today. Matching is word-bounded
/// so short terms do not attach to unrelated segments through substrings
/// ("cat" inside "category"); fuzzy matching can come later if miss rates
/// justify a text-similarity dependency.
fn glossary_for_segment(
    glossary: &[TranslationGlossaryEntry],
    segment_text: &str,
) -> Vec<TranslationGlossaryEntry> {
    let lower_segment = segment_text.to_lowercase();
    glossary
        .iter()
        .filter(|entry| {
            lowered_contains_word_bounded(&lower_segment, &entry.source.trim().to_lowercase())
        })
        .cloned()
        .collect()
}

/// Build the frontend progress event from source/batch counters.
///
/// `cached_segments` and `reused_segments_in_batch` are separate so the UI can
/// distinguish overall resume wins from a single batch that was fully reused.
#[allow(clippy::too_many_arguments)]
fn progress(
    job_id: &str,
    status: &str,
    message: &str,
    plan: &TranslationJobPlan,
    started: Instant,
    current_heading: Option<String>,
    completed_segments: usize,
    cached_segments: usize,
    reused_segments_in_batch: usize,
    completed_batches: usize,
    preview: &str,
) -> TranslationJobProgress {
    let percent = if plan.total_segments == 0 {
        0
    } else {
        ((completed_segments.saturating_mul(100)) / plan.total_segments).min(100) as u8
    };
    TranslationJobProgress {
        job_id: job_id.into(),
        status: status.into(),
        message: message.into(),
        model_id: plan.request.model_id.clone(),
        elapsed_ms: started.elapsed().as_millis() as u64,
        current_heading,
        completed_segments,
        total_segments: plan.total_segments,
        cached_segments,
        translated_segments: completed_segments.saturating_sub(cached_segments),
        reused_segments_in_batch,
        completed_batches,
        total_batches: plan.batches.len(),
        percent,
        preview: truncate_preview(preview.trim(), 180),
    }
}

/// Emit progress through the single event name the React hook listens to.
fn emit_translation_progress<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    progress: TranslationJobProgress,
) -> Result<(), String> {
    app.emit(TRANSLATION_JOB_PROGRESS_EVENT, progress)
        .map_err(|err| format!("Failed to emit translation progress: {err}"))
}

/// Check the cooperative cancellation set without exposing the lock shape.
fn is_cancelled(
    cancelled_jobs: &Arc<Mutex<HashSet<String>>>,
    job_id: &str,
) -> Result<bool, String> {
    let cancelled = cancelled_jobs
        .lock()
        .map_err(|_| "Translation cancellation lock poisoned".to_string())?;
    Ok(cancelled.contains(job_id))
}

/// Remove a completed/cancelled job id so a later retry starts cleanly.
fn clear_cancelled(
    cancelled_jobs: &Arc<Mutex<HashSet<String>>>,
    job_id: &str,
) -> Result<(), String> {
    let mut cancelled = cancelled_jobs
        .lock()
        .map_err(|_| "Translation cancellation lock poisoned".to_string())?;
    cancelled.remove(job_id);
    Ok(())
}

/// Keep progress previews readable and event payloads small.
fn truncate_preview(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let preview = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{preview}...")
    } else {
        preview
    }
}

/// Validate request languages and settings against the selected model.
///
/// The UI should prevent invalid pairs, but this backend check protects saved
/// jobs, direct command calls, and future import/export replay paths.
fn validate_model_request(
    model: TranslationModelDefinition,
    request: &TranslationStartRequest,
) -> Result<(), String> {
    let source_ok = model
        .source_languages
        .iter()
        .any(|language| *language == request.source_language);
    if !source_ok {
        return Err(format!(
            "{} does not support source language {:?}. Supported: {}",
            model.name,
            request.source_language,
            model.source_languages.join(", ")
        ));
    }

    if !model
        .target_languages
        .iter()
        .any(|language| *language == request.target_language)
    {
        return Err(format!(
            "{} does not support target language {:?}. Supported: {}",
            model.name,
            request.target_language,
            model.target_languages.join(", ")
        ));
    }

    if request.quality_mode != DEFAULT_TRANSLATION_QUALITY_MODE {
        return Err(format!(
            "{} supports only the default {:?} quality setting.",
            model.name, DEFAULT_TRANSLATION_QUALITY_MODE
        ));
    }
    if request.repair_mode != TranslationRepairMode::Off {
        return Err(format!("{} does not support chapter repair.", model.name));
    }

    // OPUS-MT pair models translate plain segment text only. HY-MT2 can add
    // bounded terminology mappings to its prompt, so glossary requests remain
    // valid for that engine.
    if model.engine == "ctranslate2" {
        if !request.glossary.is_empty() {
            return Err(format!(
                "{} does not support glossary instructions.",
                model.name
            ));
        }
    }

    Ok(())
}

/// Construct the native adapter selected by catalog metadata.
///
/// The planner and persistence pipeline use only `TranslationEngine`; keeping
/// this match here prevents engine-specific loading details from spreading
/// into commands, caches, or the frontend.
fn load_translation_engine(
    model: TranslationModelDefinition,
    model_dir: &std::path::Path,
) -> Result<Box<dyn TranslationEngine>, String> {
    match model.engine {
        "ctranslate2" => Ok(Box::new(CTranslate2Engine::for_installed_model(
            model.id.to_string(),
            model_dir.to_path_buf(),
        )?)),
        "llama.cpp" if model.id == "hy-mt2-1.8b-q8" => {
            Ok(Box::new(HyMt2Engine::for_installed_model(model_dir)?))
        }
        engine => Err(format!(
            "{} uses unsupported translation engine {engine:?}",
            model.name
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::validate_model_request;
    use crate::translation::models::PLANNED_TRANSLATION_MODELS;
    use crate::translation::types::{
        TranslationGlossaryEntry, TranslationRepairMode, TranslationStartRequest,
    };

    fn request(source_language: &str) -> TranslationStartRequest {
        TranslationStartRequest {
            job_id: None,
            document_url: "uploaded://document".into(),
            source_language: source_language.into(),
            target_language: "en".into(),
            model_id: "opus-mt-es-en-ctranslate2".into(),
            quality_mode: "balanced".into(),
            repair_mode: TranslationRepairMode::Off,
            glossary: Vec::new(),
        }
    }

    #[test]
    fn fixed_pair_model_requires_its_actual_source_language() {
        let model = PLANNED_TRANSLATION_MODELS[0];

        assert!(validate_model_request(model, &request("es")).is_ok());
        assert!(validate_model_request(model, &request("auto")).is_err());
    }

    #[test]
    fn fixed_pair_model_rejects_settings_it_cannot_honor() {
        let model = PLANNED_TRANSLATION_MODELS[0];

        let mut quality = request("es");
        quality.quality_mode = "quality".into();
        assert!(validate_model_request(model, &quality).is_err());

        let mut repair = request("es");
        repair.repair_mode = TranslationRepairMode::Chapter;
        assert!(validate_model_request(model, &repair).is_err());

        let mut glossary = request("es");
        glossary.glossary.push(TranslationGlossaryEntry {
            source: "Estado".into(),
            target: "State".into(),
            note: None,
        });
        assert!(validate_model_request(model, &glossary).is_err());
    }

    #[test]
    fn hy_mt2_accepts_supported_language_and_glossary() {
        let model = PLANNED_TRANSLATION_MODELS
            .iter()
            .copied()
            .find(|model| model.id == "hy-mt2-1.8b-q8")
            .expect("HY-MT2 model");
        let mut request = request("es");
        request.model_id = model.id.into();
        request.glossary.push(TranslationGlossaryEntry {
            source: "Estado".into(),
            target: "State".into(),
            note: None,
        });

        assert!(validate_model_request(model, &request).is_ok());
    }
}
