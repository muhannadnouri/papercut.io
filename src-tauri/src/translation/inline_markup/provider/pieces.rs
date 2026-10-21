//! Strict-only adapters, projection and receipts; normal translation imports the shared planner directly.
use super::super::{
    pieces::{
        scalar_bytes, ExactPieceTranslator, Kind, Side, SourcePlan, Translation, Unit, MAX_UNITS,
    },
    protected::{ProtectedPlan, ProtectedSource, MAX_HTML_BYTES},
};
use crate::translation::segment::TranslationTextSegment;
use unicode_segmentation::UnicodeSegmentation;

mod projection;
pub(crate) use projection::Projection;
mod cached;
pub(crate) use cached::MappedSegment;

/// One job-owned backend supplies checked maps; engines and cache code never interpret embeddings.
pub(crate) trait PieceAligner {
    fn versions(&self, model: &str) -> Result<super::contract::Versions, String>;
    fn text_fits(&self, text: &str, side: Side) -> Result<bool, String>;
    fn map(
        &mut self,
        translation: &Translation<'_>,
    ) -> Result<Vec<super::contract::NodeMapping>, String>;
}

impl SourcePlan {
    /// Start from the same DOM-normalized segment and reject a pre-existing cut through an authored identity.
    pub(crate) fn new(
        segment: &TranslationTextSegment,
        source: &ProtectedSource,
        translator: &dyn ExactPieceTranslator,
        aligner_fits: impl Fn(&str, Side) -> Result<bool, String>,
    ) -> Result<Self, String> {
        if segment.id.is_empty() || segment.id.len() > 256 || segment.text.len() > MAX_HTML_BYTES {
            return Err("Exact-piece source exceeds its bound".into());
        }
        let protected = source.fragment(segment.source_start..segment.source_end, &segment.text)?;
        Self::from_protected(segment, protected, translator, aligner_fits)
    }

    /// The runner already retains each bounded DOM fragment; do not extract or clip its identities again.
    pub(crate) fn from_protected(
        segment: &TranslationTextSegment,
        protected: ProtectedPlan,
        translator: &dyn ExactPieceTranslator,
        aligner_fits: impl Fn(&str, Side) -> Result<bool, String>,
    ) -> Result<Self, String> {
        Self::plan(segment, protected, translator, aligner_fits, false)
    }
}
