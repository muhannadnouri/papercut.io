//! Translation job planning.
//!
//! The runner does three separate things: read source blocks, plan bounded
//! batches, then translate/write/cache those batches. This module owns the
//! middle step so performance limits stay testable without loading a model.

use super::hash::StableHasher;
use super::segment::{segment_text_blocks, TranslationTextSegment};
use super::types::TranslationStartRequest;

const MAX_GLOSSARY_TERMS: usize = 200;

#[derive(Debug, Clone)]
pub(crate) struct TranslationJobPlan {
    pub(crate) cache_key: String,
    pub(crate) request: TranslationStartRequest,
    pub(crate) batches: Vec<TranslationBatchPlan>,
    pub(crate) total_segments: usize,
    pub(crate) max_segment_chars: usize,
    pub(crate) batch_segment_limit: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct TranslationBatchPlan {
    pub(crate) index: usize,
    pub(crate) segments: Vec<TranslationTextSegment>,
}

/// Convert source text blocks into bounded engine batches for one job.
///
/// The planner validates only structural constraints: non-empty languages,
/// model id, quality mode, and segment/batch limits. Model availability and
/// language-pair support stay in the runner's model/runtime preflight.
pub(crate) fn plan_translation_job<I, S>(
    request: TranslationStartRequest,
    source_blocks: I,
    max_segment_chars: usize,
    batch_segment_limit: usize,
) -> Result<TranslationJobPlan, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    validate_request_shape(&request)?;
    if batch_segment_limit == 0 {
        return Err("Translation batch size must be greater than zero".into());
    }

    let segments = segment_text_blocks(source_blocks, max_segment_chars)?;
    if segments.is_empty() {
        return Err("Document has no translatable text".into());
    }

    let batches = segments
        .chunks(batch_segment_limit)
        .enumerate()
        .map(|(index, chunk)| TranslationBatchPlan {
            index,
            segments: chunk.to_vec(),
        })
        .collect::<Vec<_>>();

    Ok(TranslationJobPlan {
        cache_key: build_translation_cache_key(&request),
        request,
        total_segments: segments.len(),
        max_segment_chars,
        batch_segment_limit,
        batches,
    })
}

/// Build a stable key for completed segment caches.
///
/// A later implementation should include source content hashes per segment too.
/// This higher-level key intentionally captures settings that make translated
/// output incompatible across jobs.
pub(crate) fn build_translation_cache_key(request: &TranslationStartRequest) -> String {
    let mut hasher = StableHasher::new();
    hasher.write_str(&request.document_url);
    hasher.write_str(&request.source_language);
    hasher.write_str(&request.target_language);
    hasher.write_str(&request.model_id);
    hasher.write_str(&request.quality_mode);
    hasher.write_str(request.repair_mode.label());
    for entry in &request.glossary {
        hasher.write_str(entry.source.trim());
        hasher.write_str(entry.target.trim());
        hasher.write_str(entry.note.as_deref().unwrap_or("").trim());
    }
    hasher.finish_hex()
}

fn validate_request_shape(request: &TranslationStartRequest) -> Result<(), String> {
    if request.document_url.trim().is_empty() {
        return Err("Translation document URL is required".into());
    }
    if request.source_language.trim().is_empty() {
        return Err("Translation source language is required".into());
    }
    if request.target_language.trim().is_empty() {
        return Err("Translation target language is required".into());
    }
    if request.model_id.trim().is_empty() {
        return Err("Translation model id is required".into());
    }
    if request.quality_mode.trim().is_empty() {
        return Err("Translation quality mode is required".into());
    }
    validate_glossary_shape(request)?;
    Ok(())
}

/// Validate user glossary shape before it enters cache keys or engine prompts.
///
/// This stays deliberately light: real glossary management comes later, but the
/// job planner must already reject empty/prohibitively large protected-term
/// payloads so engine adapters never receive ambiguous instructions.
fn validate_glossary_shape(request: &TranslationStartRequest) -> Result<(), String> {
    if request.glossary.len() > MAX_GLOSSARY_TERMS {
        return Err(format!(
            "Translation glossary has too many entries. Maximum is {MAX_GLOSSARY_TERMS}."
        ));
    }
    let mut mappings = std::collections::BTreeMap::<String, String>::new();
    for (index, entry) in request.glossary.iter().enumerate() {
        if entry.source.trim().is_empty() {
            return Err(format!(
                "Translation glossary entry {} has an empty source term",
                index + 1
            ));
        }
        if entry.target.trim().is_empty() {
            return Err(format!(
                "Translation glossary entry {} has an empty target term",
                index + 1
            ));
        }
        let source_key = normalize_glossary_key(&entry.source);
        let target_key = normalize_glossary_key(&entry.target);
        if let Some(existing_target) = mappings.get(&source_key) {
            if existing_target != &target_key {
                return Err(format!(
                    "Translation glossary maps source term {:?} to multiple targets",
                    entry.source.trim()
                ));
            }
        } else {
            mappings.insert(source_key, target_key);
        }
    }
    Ok(())
}

fn normalize_glossary_key(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::{build_translation_cache_key, plan_translation_job};
    use crate::translation::types::TranslationStartRequest;

    fn request() -> TranslationStartRequest {
        TranslationStartRequest {
            job_id: None,
            document_url: "app://document/example".into(),
            source_language: "ar".into(),
            target_language: "en".into(),
            model_id: "opus-ar-en".into(),
            quality_mode: "balanced".into(),
            repair_mode: Default::default(),
            glossary: Vec::new(),
        }
    }

    #[test]
    fn plans_batches_from_bounded_segments() {
        let plan = plan_translation_job(request(), ["One. Two. Three. Four."], 6, 2).expect("plan");

        assert_eq!(plan.total_segments, 4);
        assert_eq!(plan.batches.len(), 2);
        assert_eq!(plan.batches[0].index, 0);
        assert_eq!(plan.batches[0].segments.len(), 2);
        assert_eq!(plan.batches[1].index, 1);
        assert_eq!(plan.batches[1].segments.len(), 2);
    }

    #[test]
    fn rejects_empty_source_text() {
        let error = plan_translation_job(request(), ["   "], 100, 4).expect_err("empty text");

        assert!(error.contains("no translatable text"));
    }

    #[test]
    fn rejects_empty_batch_limit() {
        let error = plan_translation_job(request(), ["Text."], 100, 0).expect_err("zero batch");

        assert!(error.contains("batch size"));
    }

    #[test]
    fn cache_key_changes_with_translation_settings() {
        let mut first = request();
        let mut second = request();
        second.target_language = "fr".into();

        assert_ne!(
            build_translation_cache_key(&first),
            build_translation_cache_key(&second)
        );
        first.target_language = "fr".into();
        assert_eq!(
            build_translation_cache_key(&first),
            build_translation_cache_key(&second)
        );
    }

    #[test]
    fn cache_key_changes_with_glossary_terms() {
        let first = request();
        let mut second = request();
        second
            .glossary
            .push(crate::translation::types::TranslationGlossaryEntry {
                source: "Estado".into(),
                target: "State".into(),
                note: Some("Political term".into()),
            });

        assert_ne!(
            build_translation_cache_key(&first),
            build_translation_cache_key(&second)
        );
    }

    #[test]
    fn cache_key_changes_with_repair_mode() {
        let first = request();
        let mut second = request();
        second.repair_mode = crate::translation::types::TranslationRepairMode::Chapter;

        assert_ne!(
            build_translation_cache_key(&first),
            build_translation_cache_key(&second)
        );
    }

    #[test]
    fn rejects_empty_glossary_terms() {
        let mut bad = request();
        bad.glossary
            .push(crate::translation::types::TranslationGlossaryEntry {
                source: " ".into(),
                target: "State".into(),
                note: None,
            });

        let error = plan_translation_job(bad, ["Text."], 100, 4).expect_err("bad glossary");

        assert!(error.contains("glossary"));
    }

    #[test]
    fn rejects_conflicting_glossary_targets_for_same_source() {
        let mut bad = request();
        bad.glossary
            .push(crate::translation::types::TranslationGlossaryEntry {
                source: "chapter title".into(),
                target: "chapter title".into(),
                note: None,
            });
        bad.glossary
            .push(crate::translation::types::TranslationGlossaryEntry {
                source: " chapter title ".into(),
                target: "section title".into(),
                note: None,
            });

        let error = plan_translation_job(bad, ["Text."], 100, 4).expect_err("conflict");

        assert!(error.contains("multiple targets"));
    }
}
