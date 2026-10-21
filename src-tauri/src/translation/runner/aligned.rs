//! Internal strict batch admission. Public commands do not opt in while semantic gates remain unmet.
use super::*;
use crate::translation::inline_markup::{
    protected::ProtectedPlan,
    provider::pieces::{MappedSegment, PieceAligner, SourcePlan},
};

/// Stage every result before updating the shared cache; partial or late-cancelled batches commit nothing.
pub(super) fn batch(
    engine: &mut dyn TranslationEngine,
    aligner: &mut dyn PieceAligner,
    model: &str,
    segments: &[super::super::segment::TranslationTextSegment],
    plans: &BTreeMap<String, ProtectedPlan>,
    cache: &TranslationSegmentCache,
    check_cancelled: impl Fn() -> Result<(), String>,
    mut report: impl FnMut(&str) -> Result<(), String>,
) -> Result<(Vec<MappedSegment>, usize), String> {
    let versions = aligner.versions(model)?;
    let translator = engine
        .exact_translator()
        .ok_or("Engine has no exact-piece contract")?;
    let mut completed = Vec::new();
    let mut reused = 0;
    for segment in segments {
        check_cancelled()?;
        let protected = plans
            .get(&segment.id)
            .ok_or("Missing strict source plan")?
            .clone();
        let plan = SourcePlan::from_protected(segment, protected, translator, |text, side| {
            aligner.text_fits(text, side)
        })?;
        let cached = cache.aligned_for(&segment.id);
        let resumed = cached.and_then(|c| {
            c.translation(&plan, &versions, |text, side| aligner.text_fits(text, side))
        });
        let reused_text = resumed.is_some();
        let translated = if let Some(translated) = resumed {
            reused += 1;
            translated
        } else {
            report("translating-batch")?;
            check_cancelled()?;
            plan.translate(
                translator,
                |text, side| aligner.text_fits(text, side),
                || check_cancelled().is_err(),
            )?
        };
        check_cancelled()?;
        let receipt = if let Some(cached) = cached.filter(|c| {
            reused_text
                && c.binding.versions == versions
                && c.binding.target == translated.text
                && c.restore(&plan.protected, segment).is_ok()
        }) {
            cached.clone()
        } else {
            report("aligning-batch")?;
            check_cancelled()?;
            let nodes = aligner.map(&translated)?;
            check_cancelled()?;
            MappedSegment::new(&translated, versions.clone(), nodes)?
        };
        // Rendering is part of admission even on a cache hit, before any progress becomes reusable.
        report("reconstructing-batch")?;
        check_cancelled()?;
        receipt.restore(&plan.protected, segment)?;
        check_cancelled()?;
        completed.push(receipt);
    }
    check_cancelled()?;
    Ok((completed, reused))
}

#[cfg(test)]
mod tests;
