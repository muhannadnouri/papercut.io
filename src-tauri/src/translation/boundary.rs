//! CTranslate2's source-boundary policy is distinct from the disabled semantic alignment candidate.
use super::{
    cache::TranslationSegmentCache,
    engine::{TranslationEngine, TranslationSegmentOutput},
    inline_markup::{
        protected::{ProtectedPlan, RestoredMarkup},
        provider::pieces::SourcePlan,
    },
    segment::TranslationTextSegment,
};
use std::collections::BTreeMap;

pub(super) const POLICY: &str = "ctranslate2-source-boundaries-v1";

/// Fixed OPUS pairs use source-boundary translation by default; strict requests retain their separate gate.
pub(super) fn applies(model: &str) -> bool {
    matches!(
        model,
        "opus-mt-fr-en-ctranslate2" | "opus-mt-es-en-ctranslate2"
    )
}

/// Worker markup may move notes; boundary output must retain its exact source-owned attachment positions.
pub(super) fn restore(
    model: &str,
    plan: &ProtectedPlan,
    output: &str,
) -> Result<RestoredMarkup, String> {
    if applies(model) {
        plan.restore_boundaries(output)
    } else {
        plan.restore(output)
    }
}

/// Admit a whole batch before caching; native calls and cache hits share cancellation and reconstruction checks.
pub(super) fn batch(
    engine: &mut dyn TranslationEngine,
    segments: &[TranslationTextSegment],
    plans: &BTreeMap<String, ProtectedPlan>,
    cache: &TranslationSegmentCache,
    check: impl Fn() -> Result<(), String>,
) -> Result<(Vec<TranslationSegmentOutput>, usize), String> {
    let mut outputs = Vec::new();
    let mut reused = 0;
    for segment in segments {
        check()?;
        let protected = plans
            .get(&segment.id)
            .ok_or("Missing boundary source plan")?;
        let output = if let Some(output) = cache.mapped_output_for(segment, protected) {
            reused += 1;
            output
        } else {
            let translator = engine
                .exact_translator()
                .ok_or("Engine has no boundary translation contract")?;
            let plan = SourcePlan::from_boundaries(segment, protected.clone(), translator)?;
            check()?;
            let translated = plan.translate(translator, |_, _| Ok(true), || check().is_err())?;
            let markup = protected.boundary_output(&translated)?;
            let restored = protected.restore_boundaries(&markup)?;
            TranslationSegmentOutput {
                id: segment.id.clone(),
                source_text: segment.text.clone(),
                text: restored.text,
                markup: Some(markup),
            }
        };
        check()?;
        outputs.push(output);
    }
    check()?;
    Ok((outputs, reused))
}

#[cfg(test)]
mod tests;
