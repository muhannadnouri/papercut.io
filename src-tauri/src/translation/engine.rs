//! Native translation engine boundary.
//!
//! This module intentionally does not depend on CTranslate2 directly. Engine
//! adapters implement these contracts so command DTOs and storage semantics
//! stay stable while runtimes change.

use super::types::{TranslationGlossaryEntry, TranslationRepairMode};

/// One bounded batch for an engine, with the job settings that produced it.
///
/// The CTranslate2/OPUS-MT adapter reads only `segments`: pair models encode
/// the language direction, and Marian cannot consume glossary, quality, or
/// repair instructions. Prompt-driven engines such as HY-MT2 also read the
/// language and job-setting fields.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct TranslationBatchInput {
    pub(crate) model_id: String,
    pub(crate) source_language: String,
    pub(crate) target_language: String,
    pub(crate) quality_mode: String,
    pub(crate) repair_mode: TranslationRepairMode,
    pub(crate) glossary: Vec<TranslationGlossaryEntry>,
    pub(crate) segments: Vec<TranslationSegmentInput>,
}

#[derive(Debug, Clone)]
pub(crate) struct TranslationSegmentInput {
    pub(crate) id: String,
    pub(crate) text: String,
    /// Unread by the OPUS-MT adapter; see `TranslationSegmentContext`.
    #[allow(dead_code)]
    pub(crate) context: TranslationSegmentContext,
}

/// Quality hints attached to one segment.
///
/// Only glossary hints exist today. OPUS-MT ignores free-form context; HY-MT2
/// can consume glossary instructions once the UI exposes them. A future
/// document memory packet (title, headings, neighboring text) belongs here.
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub(crate) struct TranslationSegmentContext {
    pub(crate) glossary: Vec<TranslationGlossaryEntry>,
}

#[derive(Debug, Clone)]
pub(crate) struct TranslationSegmentOutput {
    pub(crate) id: String,
    pub(crate) text: String,
}

pub(crate) trait TranslationEngine {
    /// Translate a bounded batch of document segments.
    ///
    /// Engines should preserve segment ids exactly so the caller can rebuild
    /// document order and cache completed work. Context fields are hints for
    /// quality, not text that should be emitted into the translated output.
    fn translate_batch(
        &mut self,
        input: TranslationBatchInput,
    ) -> Result<Vec<TranslationSegmentOutput>, String>;
}
