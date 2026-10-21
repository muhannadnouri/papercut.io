//! Stage cached and fresh provider replies in source order before the runner admits a whole batch.
use super::batch_input;
use crate::translation::{
    cache::TranslationSegmentCache,
    engine::{validate_mapped_outputs, TranslationEngine, TranslationSegmentOutput},
    inline_markup::protected::ProtectedPlan,
    job::{TranslationBatchPlan, TranslationJobPlan},
};
use std::collections::BTreeMap;

/// Never append cache hits ahead of earlier pending segments or expose a partly validated reply.
/// The returned dirty flag covers inference and materialized text-memory hits, not ordinary cache hits.
pub(super) fn batch(
    engine: &mut dyn TranslationEngine,
    plan: &TranslationJobPlan,
    batch: &TranslationBatchPlan,
    mappings: &BTreeMap<String, ProtectedPlan>,
    cache: &TranslationSegmentCache,
    check: impl Fn() -> Result<(), String>,
    mut report_inference: impl FnMut() -> Result<(), String>,
) -> Result<(Vec<TranslationSegmentOutput>, usize, bool), String> {
    let mut outputs = vec![None; batch.segments.len()];
    let mut pending = TranslationBatchPlan {
        index: batch.index,
        segments: Vec::new(),
    };
    let mut positions = Vec::new();
    let mut reused = 0;
    let mut materialized_memory = false;
    for (index, segment) in batch.segments.iter().enumerate() {
        check()?;
        let cached = if let Some(mapping) = mappings.get(&segment.id) {
            cache.mapped_output_for(segment, mapping)
        } else {
            let text = cache.translated_text_for(segment).or_else(|| {
                let remembered = cache
                    .translated_text_for_source(segment)
                    .filter(|text| !text.trim().is_empty());
                materialized_memory |= remembered.is_some();
                remembered
            });
            text.map(|text| TranslationSegmentOutput {
                id: segment.id.clone(),
                source_text: segment.text.clone(),
                text: text.to_string(),
                markup: None,
                needs_review: false,
            })
        };
        if let Some(output) = cached {
            reused += 1;
            outputs[index] = Some(output);
        } else {
            positions.push(index);
            pending.segments.push(segment.clone());
        }
    }
    let mut input = batch_input(plan, &pending);
    for segment in &mut input.segments {
        segment.markup = mappings.get(&segment.id).cloned();
    }
    if !pending.segments.is_empty() {
        report_inference()?;
        check()?;
        let fresh = engine
            .translate_batch(input.clone())
            .map_err(|e| format!("In-memory translation did not complete: {e}"))?;
        check()?;
        validate_mapped_outputs(&input.segments, &fresh)?;
        for (index, output) in positions.into_iter().zip(fresh) {
            outputs[index] = Some(output);
        }
    }
    let outputs = outputs
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or("Translation batch is missing a validated result")?;
    let mut whole = batch_input(plan, batch);
    for segment in &mut whole.segments {
        segment.markup = mappings.get(&segment.id).cloned();
    }
    validate_mapped_outputs(&whole.segments, &outputs)?;
    check()?;
    Ok((
        outputs,
        reused,
        !pending.segments.is_empty() || materialized_memory,
    ))
}
