//! Fresh-file preparation only: generated/restored reader HTML must not be re-normalized.

use super::{emphasis::Normalizer, emphasis_policy::POLICY, parse_html_document};
use crate::document_uploads::parsed::{ParsedDocument, SourceFormatting};

/// Resolve supported inline/embedded presentation while the raw head and ancestors still exist,
/// then retain the existing sanitizer as the only reader-HTML security boundary.
pub(crate) fn prepare_html_document(html: &str) -> ParsedDocument {
    let normalized = Normalizer::default().normalize(html);
    let mut parsed = parse_html_document(&normalized.html);
    parsed.source_formatting = Some(SourceFormatting {
        policy: POLICY.into(),
        warnings: normalized.warning.into_iter().map(str::to_owned).collect(),
    });
    parsed
}

#[cfg(test)]
#[path = "prepare_tests.rs"]
mod tests;
