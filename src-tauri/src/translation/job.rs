//! Translation job planning.
//!
//! The runner does three separate things: read source blocks, plan bounded
//! batches, then translate/write/cache those batches. This module owns the
//! middle step so performance limits stay testable without loading a model.

use super::hash::StableHasher;
use super::segment::{segment_bergamot_blocks, segment_text_blocks, TranslationTextSegment};
use super::types::TranslationStartRequest;

const MAX_GLOSSARY_TERMS: usize = 200;
// App-side bound: the pinned WASM does not wire max-length-break into its sentence processor.
const BERGAMOT_MAX_SEGMENT_CHARS: usize = 320;

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
    if max_segment_chars == 0 {
        return Err("Translation segment size must be greater than zero".into());
    }
    if batch_segment_limit == 0 {
        return Err("Translation batch size must be greater than zero".into());
    }

    let bergamot = super::bergamot::assets::manifest_by_id(&request.model_id).is_some();
    let max_segment_chars = if bergamot {
        max_segment_chars.min(BERGAMOT_MAX_SEGMENT_CHARS)
    } else {
        max_segment_chars
    };
    // Keep source identities intact; the boundary adapter still caps actual MT inputs at 900 scalars and the model's token budget.
    let boundary = super::boundary::applies(&request.model_id) && !request.preserve_formatting;
    let max_segment_chars = if boundary {
        super::inline_markup::protected::MAX_HTML_BYTES
    } else {
        max_segment_chars
    };
    let segments = if boundary {
        let blocks = source_blocks
            .into_iter()
            .map(|b| b.as_ref().split_whitespace().collect::<Vec<_>>().join(" "))
            .collect::<Vec<_>>();
        if blocks.iter().any(|b| b.len() > max_segment_chars) {
            return Err("Boundary translation source block exceeds its bound".into());
        }
        segment_text_blocks(blocks, max_segment_chars)?
    } else if bergamot {
        segment_bergamot_blocks(source_blocks, max_segment_chars)?
    } else {
        segment_text_blocks(source_blocks, max_segment_chars)?
    };
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
    // Strict keys stay stable; boundary OPUS jobs must not reuse legacy text/probe caches.
    if request.preserve_formatting {
        hasher.write_str("strict-formatting-v1");
    } else if super::boundary::applies(&request.model_id) {
        hasher.write_str(super::boundary::POLICY);
    }
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
            use_hardware_acceleration: false,
            preserve_formatting: false,
            repair_mode: Default::default(),
            glossary: Vec::new(),
        }
    }

    #[test]
    fn strict_request_cannot_resume_a_plain_job_cache() {
        let mut request = request();
        let plain = build_translation_cache_key(&request);
        request.preserve_formatting = true;
        assert_ne!(plain, build_translation_cache_key(&request));
        request.preserve_formatting = false;
        assert_eq!(plain, build_translation_cache_key(&request));
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

    /// Worker planning must not change native segmentation or silently retain the 900-character policy.
    #[test]
    fn bergamot_uses_sentence_and_clause_segments_without_losing_source_ranges() {
        let text = format!(
            "{}: {}; {}. Fin.",
            "Premier élément avec du café 🙂 ".repeat(6),
            "Segundo elemento de la lista ".repeat(6),
            "Dernier élément de la liste ".repeat(6)
        );
        let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let mut worker_request = request();
        worker_request.model_id = "bergamot-fr-en-worker".into();
        let worker = plan_translation_job(worker_request.clone(), [&text], 900, 2).unwrap();
        let native = plan_translation_job(request(), [&text], 900, 2).unwrap();
        assert_eq!(native.total_segments, 1);
        assert_eq!(native.max_segment_chars, 900);
        assert_eq!(worker.max_segment_chars, 320);
        assert!(worker.total_segments > 2);
        let segments = worker
            .batches
            .iter()
            .flat_map(|b| &b.segments)
            .collect::<Vec<_>>();
        assert_eq!(segments.last().unwrap().text, "Fin.");
        assert!(segments[0].text.ends_with(':'));
        let chars = normalized.chars().collect::<Vec<_>>();
        let mut cursor = 0;
        for (index, segment) in segments.iter().enumerate() {
            assert_eq!(segment.id, format!("b0:s{index}"));
            assert!(segment.text.chars().count() <= 320);
            assert!(chars[cursor..segment.source_start]
                .iter()
                .all(|c| c.is_whitespace()));
            assert_eq!(
                chars[segment.source_start..segment.source_end]
                    .iter()
                    .collect::<String>(),
                segment.text
            );
            cursor = segment.source_end;
        }
        assert_eq!(cursor, chars.len());
        let small = plan_translation_job(worker_request, ["One. Two. Three."], 6, 2).unwrap();
        assert_eq!(small.max_segment_chars, 6);
        assert!(small
            .batches
            .iter()
            .flat_map(|b| &b.segments)
            .all(|s| s.text.chars().count() <= 6));
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
    fn cache_key_ignores_hardware_acceleration() {
        let first = request();
        let mut second = request();
        second.use_hardware_acceleration = true;

        assert_eq!(
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
