//! Job formatting semantics; engine transport capabilities and public admission remain separate.
use super::types::TranslationStartRequest;

pub(super) const SENTENCE_REVIEW_POLICY: &str = "ctranslate2-sentences-review-v1";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum FormattingPolicy {
    /// The adapter supplies validated protected markup or the existing conservative text/probe path.
    #[default]
    Provider,
    SentenceReview,
    Strict,
}

impl FormattingPolicy {
    /// Select from requested semantics, never installed assets or the presence of cached markup.
    pub(crate) fn for_request(request: &TranslationStartRequest) -> Self {
        if request.preserve_formatting {
            Self::Strict
        } else if Self::supports_sentence_review(&request.model_id) {
            Self::SentenceReview
        } else {
            Self::Provider
        }
    }

    /// Recheck the model at the persistence trust boundary without inferring policy from result fragments.
    pub(crate) fn supports_sentence_review(model: &str) -> bool {
        matches!(
            model,
            "opus-mt-fr-en-ctranslate2" | "opus-mt-es-en-ctranslate2"
        )
    }

    /// Keep existing cache namespaces byte-for-byte; provider caches predate explicit formatting policies.
    pub(crate) fn cache_tag(self) -> Option<&'static str> {
        match self {
            Self::Provider => None,
            Self::SentenceReview => Some(SENTENCE_REVIEW_POLICY),
            Self::Strict => Some("strict-formatting-v1"),
        }
    }
}
