//! Resume cache for translated text segments.
//!
//! This is intentionally separate from translated-document storage. The cache
//! is a retry/resume aid for in-progress jobs, while `storage.rs` owns durable
//! reader/search variants after a whole job succeeds.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{Manager, Runtime};

use super::formatting::FormattingPolicy;
use super::hash::stable_text_hash;
use super::inline_markup::INLINE_ALIGNMENT_VERSION;
use super::job::TranslationJobPlan;
use super::segment::TranslationTextSegment;

const SEGMENT_CACHE_VERSION: u32 = 1;
const SEGMENT_CACHE_FILE: &str = "segments.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TranslationSegmentCache {
    version: u32,
    /// Bound from the admitted job after loading; do not change the on-disk cache schema.
    #[serde(skip)]
    formatting: FormattingPolicy,
    #[serde(default)]
    provider_identity: Option<String>,
    cache_key: String,
    model_id: String,
    source_language: String,
    target_language: String,
    quality_mode: String,
    max_segment_chars: usize,
    batch_segment_limit: usize,
    segments: BTreeMap<String, CachedTranslationSegment>,
    /// Separate admission: neither legacy text memory nor worker markup is a checked native mapping.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    aligned: BTreeMap<String, super::inline_markup::provider::pieces::MappedSegment>,
    #[serde(default)]
    memory: BTreeMap<String, String>,
    /// Inline-alignment strategy version active when this cache was written.
    ///
    /// The memory map stores probe phrase translations whose extraction and
    /// trimming rules live in `inline_markup`; mixing entries produced under a
    /// different strategy could feed stale hints into rendering.
    #[serde(default)]
    inline_alignment_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CachedTranslationSegment {
    source_hash: String,
    translated_text: String,
    #[serde(default)]
    markup: Option<String>,
    #[serde(default)]
    markup_identity: Option<String>,
    #[serde(default)]
    needs_review: bool,
}

impl TranslationSegmentCache {
    pub(crate) fn aligned_for(
        &self,
        id: &str,
    ) -> Option<&super::inline_markup::provider::pieces::MappedSegment> {
        self.aligned.get(id)
    }

    /// The runner commits these together only after every strict result in the batch has reconstructed.
    pub(crate) fn store_aligned_batch(
        &mut self,
        values: Vec<super::inline_markup::provider::pieces::MappedSegment>,
    ) {
        for value in values {
            self.aligned.insert(value.binding.segment_id.clone(), value);
        }
    }

    /// Rich resume is location/DOM-bound and revalidated; plain translation memory cannot supply mappings.
    pub(crate) fn mapped_output_for(
        &self,
        segment: &TranslationTextSegment,
        plan: &super::inline_markup::protected::ProtectedPlan,
    ) -> Option<super::engine::TranslationSegmentOutput> {
        let cached = self.segments.get(&segment.id)?;
        if cached.needs_review
            || cached.source_hash != hash_segment_text(&segment.text)
            || cached.markup_identity.as_deref() != Some(self.markup_identity(plan).as_str())
        {
            return None;
        }
        let markup = cached.markup.as_ref()?;
        let restored = match self.formatting {
            FormattingPolicy::SentenceReview => plan.restore_boundaries(markup),
            FormattingPolicy::Provider => plan.restore(markup),
            FormattingPolicy::Strict => return None,
        };
        if restored.ok()?.text != cached.translated_text {
            return None;
        }
        Some(super::engine::TranslationSegmentOutput {
            id: segment.id.clone(),
            source_text: segment.text.clone(),
            text: cached.translated_text.clone(),
            markup: Some(markup.clone()),
            needs_review: false,
        })
    }

    /// Review cache entries retain explicit source ownership; missing markup alone never authorizes fallback.
    pub(crate) fn sentence_output_for(
        &self,
        segment: &TranslationTextSegment,
        plan: &super::inline_markup::protected::ProtectedPlan,
        needs_review: bool,
    ) -> Option<super::engine::TranslationSegmentOutput> {
        if !needs_review {
            return self.mapped_output_for(segment, plan);
        }
        let cached = self.segments.get(&segment.id)?;
        if self.formatting != FormattingPolicy::SentenceReview
            || !cached.needs_review
            || cached.markup.is_some()
            || cached.source_hash != hash_segment_text(&segment.text)
            || cached.markup_identity.as_deref() != Some(self.markup_identity(plan).as_str())
            || cached.translated_text.trim().is_empty()
            || cached.translated_text.len() > super::inline_markup::protected::MAX_HTML_BYTES
        {
            return None;
        }
        Some(super::engine::TranslationSegmentOutput {
            id: segment.id.clone(),
            source_text: segment.text.clone(),
            text: cached.translated_text.clone(),
            markup: None,
            needs_review: true,
        })
    }

    /// Context participates in boundary cache identity because table children need their source parser context.
    fn markup_identity(&self, plan: &super::inline_markup::protected::ProtectedPlan) -> String {
        if self.formatting == FormattingPolicy::SentenceReview {
            plan.alignment_identity()
        } else {
            plan.identity()
        }
    }

    /// Persist only validated rich replies, without adding provider HTML to plain-text translation memory.
    pub(crate) fn store_mapped_output(
        &mut self,
        segment: &TranslationTextSegment,
        plan: &super::inline_markup::protected::ProtectedPlan,
        output: &super::engine::TranslationSegmentOutput,
    ) {
        self.segments.insert(
            segment.id.clone(),
            CachedTranslationSegment {
                source_hash: hash_segment_text(&segment.text),
                translated_text: output.text.clone(),
                markup: output.markup.clone(),
                markup_identity: Some(self.markup_identity(plan)),
                needs_review: output.needs_review,
            },
        );
    }

    /// Return a cached translation only when the segment id and source text
    /// still match.
    ///
    /// Segment ids are stable by source block/part order, but a user can edit or
    /// reimport a document at the same URL. The source hash prevents stale text
    /// from being reused just because the positional id stayed the same.
    pub(crate) fn translated_text_for(&self, segment: &TranslationTextSegment) -> Option<&str> {
        let cached = self.segments.get(&segment.id)?;
        if cached.translated_text.trim().is_empty() {
            return None;
        }
        if cached.source_hash == hash_segment_text(&segment.text) {
            Some(cached.translated_text.as_str())
        } else {
            None
        }
    }

    /// Return a translation for repeated source text regardless of segment id.
    ///
    /// This is the first translation-memory slice: exact repeated paragraphs or
    /// sentences should not call the native engine again just because they live
    /// in a different block. The source hash keeps reuse scoped to identical
    /// text under the same job settings.
    pub(crate) fn translated_text_for_source(
        &self,
        segment: &TranslationTextSegment,
    ) -> Option<&str> {
        self.memory
            .get(&hash_segment_text(&segment.text))
            .map(String::as_str)
    }

    /// Return a remembered translation for one emphasized phrase probe.
    ///
    /// Probes share the exact-match translation memory with whole segments, so
    /// repeated jobs and repeated emphasized phrases skip native inference.
    pub(crate) fn translated_probe_text(&self, source_text: &str) -> Option<&str> {
        self.memory
            .get(&hash_segment_text(source_text))
            .map(String::as_str)
            .filter(|text| !text.trim().is_empty())
    }

    pub(crate) fn store_probe_translation(&mut self, source_text: &str, translated_text: String) {
        if translated_text.trim().is_empty() {
            return;
        }
        self.memory
            .insert(hash_segment_text(source_text), translated_text);
    }

    pub(crate) fn store_translation(
        &mut self,
        segment: &TranslationTextSegment,
        translated_text: String,
    ) {
        if translated_text.trim().is_empty() {
            return;
        }
        let source_hash = hash_segment_text(&segment.text);
        self.memory
            .insert(source_hash.clone(), translated_text.clone());
        self.segments.insert(
            segment.id.clone(),
            CachedTranslationSegment {
                source_hash,
                translated_text,
                markup: None,
                markup_identity: None,
                needs_review: false,
            },
        );
    }
}

/// Load a cache manifest if it belongs to the exact same job settings.
///
/// Corrupt, stale, or incompatible manifests are ignored instead of failing the
/// translation. A bad cache should only cost time, never prevent a fresh run.
pub(crate) fn load_segment_cache<R: Runtime>(
    app: &tauri::AppHandle<R>,
    plan: &TranslationJobPlan,
) -> Result<TranslationSegmentCache, String> {
    let path = segment_cache_path(app, &plan.cache_key)?;
    let Ok(bytes) = fs::read(&path) else {
        return Ok(new_segment_cache(plan));
    };
    let Ok(mut cache) = serde_json::from_slice::<TranslationSegmentCache>(&bytes) else {
        log::warn!(
            "translation: ignoring unreadable segment cache {}; starting fresh",
            path.display()
        );
        return Ok(new_segment_cache(plan));
    };
    if cache_is_compatible(&cache, plan) {
        cache.formatting = plan.formatting;
        Ok(cache)
    } else {
        log::warn!(
            "translation: segment cache {} does not match current job settings; starting fresh",
            path.display()
        );
        Ok(new_segment_cache(plan))
    }
}

/// Persist the manifest after a batch finishes.
///
/// Saving after each batch is the main resume guarantee: if a large book is
/// cancelled, crashes, or fails on a later batch, completed batches are already
/// reusable on the next run.
pub(crate) fn save_segment_cache<R: Runtime>(
    app: &tauri::AppHandle<R>,
    plan: &TranslationJobPlan,
    cache: &TranslationSegmentCache,
) -> Result<(), String> {
    let path = segment_cache_path(app, &plan.cache_key)?;
    let Some(parent) = path.parent() else {
        return Err("Translation cache path has no parent directory".into());
    };
    fs::create_dir_all(parent).map_err(|err| {
        format!(
            "Failed to create translation segment cache directory {}: {err}",
            parent.display()
        )
    })?;
    let temp_path = path.with_extension("json.tmp");
    // Compact JSON: the whole manifest is rewritten after every batch, and
    // pretty-printing only inflates that repeated I/O for a machine-read file.
    let bytes = serde_json::to_vec(cache)
        .map_err(|err| format!("Failed to serialize translation segment cache: {err}"))?;
    fs::write(&temp_path, bytes).map_err(|err| {
        format!(
            "Failed to write translation segment cache {}: {err}",
            temp_path.display()
        )
    })?;
    promote_cache_file(&temp_path, &path)
}

fn new_segment_cache(plan: &TranslationJobPlan) -> TranslationSegmentCache {
    TranslationSegmentCache {
        version: SEGMENT_CACHE_VERSION,
        formatting: plan.formatting,
        provider_identity: super::bergamot::assets::provider_identity(&plan.request.model_id),
        cache_key: plan.cache_key.clone(),
        model_id: plan.request.model_id.clone(),
        source_language: plan.request.source_language.clone(),
        target_language: plan.request.target_language.clone(),
        quality_mode: plan.request.quality_mode.clone(),
        max_segment_chars: plan.max_segment_chars,
        batch_segment_limit: plan.batch_segment_limit,
        segments: BTreeMap::new(),
        aligned: BTreeMap::new(),
        memory: BTreeMap::new(),
        inline_alignment_version: INLINE_ALIGNMENT_VERSION,
    }
}

fn cache_is_compatible(cache: &TranslationSegmentCache, plan: &TranslationJobPlan) -> bool {
    cache.provider_identity == super::bergamot::assets::provider_identity(&plan.request.model_id)
        && cache.version == SEGMENT_CACHE_VERSION
        && cache.cache_key == plan.cache_key
        && cache.model_id == plan.request.model_id
        && cache.source_language == plan.request.source_language
        && cache.target_language == plan.request.target_language
        && cache.quality_mode == plan.request.quality_mode
        && cache.max_segment_chars == plan.max_segment_chars
        && cache.batch_segment_limit == plan.batch_segment_limit
        && cache.inline_alignment_version == INLINE_ALIGNMENT_VERSION
}

fn segment_cache_path<R: Runtime>(
    app: &tauri::AppHandle<R>,
    cache_key: &str,
) -> Result<PathBuf, String> {
    if cache_key.is_empty()
        || !cache_key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err("Translation cache key is invalid".into());
    }
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|err| format!("Failed to resolve app data dir for translation cache: {err}"))?;
    Ok(app_data
        .join("translation")
        .join("segment-cache")
        .join(cache_key)
        .join(SEGMENT_CACHE_FILE))
}

fn promote_cache_file(temp_path: &std::path::Path, path: &std::path::Path) -> Result<(), String> {
    match fs::rename(temp_path, path) {
        Ok(()) => Ok(()),
        Err(first_err) => {
            if !path.exists() {
                return Err(format!(
                    "Failed to promote translation segment cache {}: {first_err}",
                    path.display()
                ));
            }
            fs::remove_file(path).map_err(|remove_err| {
                format!(
                    "Failed to replace translation segment cache {} after rename error {first_err}: {remove_err}",
                    path.display()
                )
            })?;
            fs::rename(temp_path, path).map_err(|rename_err| {
                format!(
                    "Failed to promote replacement translation segment cache {}: {rename_err}",
                    path.display()
                )
            })
        }
    }
}

fn hash_segment_text(text: &str) -> String {
    stable_text_hash(text)
}

#[cfg(test)]
mod tests {
    use super::{cache_is_compatible, hash_segment_text, new_segment_cache};
    use crate::translation::job::plan_translation_job;
    use crate::translation::types::TranslationStartRequest;

    #[test]
    fn boundary_cache_rejects_reordered_missing_markup_and_changed_fragment_context() {
        use crate::translation::{
            dom::DomPlan, engine::TranslationSegmentOutput,
            inline_markup::protected::ProtectedSource, segment::TranslationTextSegment,
        };
        let mut job = plan();
        job.request.model_id = "opus-mt-fr-en-ctranslate2".into();
        let mut cache = new_segment_cache(&job);
        let dom = DomPlan::new("<p><strong>un</strong> et <em>deux</em></p>");
        let source = &dom.blocks[0];
        let segment = TranslationTextSegment {
            id: "b0:s0".into(),
            source_block_index: 0,
            source_start: 0,
            source_end: source.inline.text.chars().count(),
            text: source.inline.text.clone(),
        };
        let mapping = ProtectedSource::for_boundaries(&source.node)
            .unwrap()
            .fragment(0..segment.source_end, &segment.text)
            .unwrap();
        let output = TranslationSegmentOutput {
            needs_review: false,
            id: segment.id.clone(),
            source_text: segment.text.clone(),
            text: "one and two".into(),
            markup: Some("<span data-pc='0'>one</span> and <span data-pc='1'>two</span>".into()),
        };
        cache.store_mapped_output(&segment, &mapping, &output);
        assert!(cache.mapped_output_for(&segment, &mapping).is_some());
        let list = DomPlan::new("<ul><li><strong>un</strong> et <em>deux</em></li></ul>");
        let changed = ProtectedSource::for_boundaries(&list.blocks[0].node)
            .unwrap()
            .fragment(0..segment.source_end, &segment.text)
            .unwrap();
        assert!(cache.mapped_output_for(&segment, &changed).is_none());
        cache.segments.get_mut(&segment.id).unwrap().markup =
            Some("<span data-pc='1'>one</span> and <span data-pc='0'>two</span>".into());
        assert!(cache.mapped_output_for(&segment, &mapping).is_none());
        cache.store_translation(&segment, "plaintext memory".into());
        assert!(cache.translated_text_for_source(&segment).is_some());
        assert!(cache.mapped_output_for(&segment, &mapping).is_none());
    }

    #[test]
    fn worker_resume_requires_current_provider_identity_without_invalidating_native_caches() {
        let native = plan();
        let cache = new_segment_cache(&native);
        assert!(cache.provider_identity.is_none());
        let mut json = serde_json::to_value(&cache).unwrap();
        json.as_object_mut().unwrap().remove("providerIdentity");
        let legacy = serde_json::from_value(json).unwrap();
        assert!(cache_is_compatible(&legacy, &native));
        let mut worker = native.clone();
        worker.request.model_id = "bergamot-fr-en-worker".into();
        let mut cache = new_segment_cache(&worker);
        assert!(cache_is_compatible(&cache, &worker));
        cache.provider_identity = cache
            .provider_identity
            .map(|identity| identity.replace("protected-codes-v6", "protected-codes-v5"));
        assert!(!cache_is_compatible(&cache, &worker));
        cache.provider_identity = Some("previous-runtime-or-model".into());
        assert!(!cache_is_compatible(&cache, &worker));
        cache.provider_identity = None;
        assert!(!cache_is_compatible(&cache, &worker));
    }

    #[test]
    fn rich_cache_requires_structure_binding_and_revalidates_payload_instead_of_reusing_text_memory(
    ) {
        use crate::translation::{
            dom::DomPlan, engine::TranslationSegmentOutput, inline_markup::protected::plan_fragment,
        };
        let job = plan();
        let mut segment = job.batches[0].segments[0].clone();
        segment.text = "Une bague".into();
        let mapping = |tag: &str| {
            let dom = DomPlan::new(&format!("<p>Une <{tag}>bague</{tag}></p>"));
            plan_fragment(&dom.blocks[0].node, 0..9, &segment.text).unwrap()
        };
        let strong = mapping("strong");
        let emphasis = mapping("em");
        let mut cache = new_segment_cache(&job);
        cache.store_translation(&segment, "A ring".into());
        assert!(cache.mapped_output_for(&segment, &strong).is_none());
        let output = TranslationSegmentOutput {
            needs_review: false,
            id: segment.id.clone(),
            source_text: segment.text.clone(),
            text: "A ring".into(),
            markup: Some("A <span data-pc='0'>ring</span>".into()),
        };
        cache.store_mapped_output(&segment, &strong, &output);
        assert!(cache.mapped_output_for(&segment, &strong).is_some());
        assert!(cache.mapped_output_for(&segment, &emphasis).is_none());
        let mut moved = segment.clone();
        moved.id.push_str("-different-location");
        assert!(cache.mapped_output_for(&moved, &strong).is_none());
        cache.segments.get_mut(&segment.id).unwrap().markup = Some("A ring".into());
        assert!(cache.mapped_output_for(&segment, &strong).is_none());
        cache.store_mapped_output(&segment, &strong, &output);
        cache.segments.get_mut(&segment.id).unwrap().translated_text = "different prose".into();
        assert!(cache.mapped_output_for(&segment, &strong).is_none());
        let roundtrip: super::TranslationSegmentCache =
            serde_json::from_str(&serde_json::to_string(&cache).unwrap()).unwrap();
        assert!(roundtrip.mapped_output_for(&segment, &strong).is_none());
    }

    /// A review outcome survives JSON/resume, but neither missing flags nor foreign DOM grant fallback.
    #[test]
    fn sentence_review_cache_requires_explicit_outcome_and_current_source_identity() {
        use crate::translation::{
            dom::DomPlan, engine::TranslationSegmentOutput,
            inline_markup::protected::ProtectedSource,
        };
        let mut job = plan();
        job.request.model_id = "opus-mt-fr-en-ctranslate2".into();
        let mut segment = job.batches[0].segments[0].clone();
        segment.text = "Je ne sais pas.".into();
        segment.source_start = 0;
        segment.source_end = segment.text.chars().count();
        let mapping = |tag: &str| {
            let dom = DomPlan::new(&format!("<p>Je <{tag}>ne</{tag}> sais pas.</p>"));
            ProtectedSource::for_boundaries(&dom.blocks[0].node)
                .unwrap()
                .fragment(0..segment.source_end, &segment.text)
                .unwrap()
        };
        let source = mapping("em");
        let mut cache = new_segment_cache(&job);
        let output = TranslationSegmentOutput {
            id: segment.id.clone(),
            source_text: segment.text.clone(),
            text: "I don't know.".into(),
            markup: None,
            needs_review: true,
        };
        cache.store_mapped_output(&segment, &source, &output);
        let mut valid: super::TranslationSegmentCache =
            serde_json::from_str(&serde_json::to_string(&cache).unwrap()).unwrap();
        assert!(cache_is_compatible(&valid, &job));
        // Deserialization does not admit a policy; the real disk-load regression covers this binding.
        valid.formatting = job.formatting;
        let resumed = valid.sentence_output_for(&segment, &source, true).unwrap();
        assert_eq!(resumed.text, "I don't know.");
        assert!(resumed.needs_review && resumed.markup.is_none());
        assert!(valid.mapped_output_for(&segment, &source).is_none());
        assert!(valid
            .sentence_output_for(&segment, &source, false)
            .is_none());
        assert!(valid
            .sentence_output_for(&segment, &mapping("strong"), true)
            .is_none());
        assert!(valid.translated_text_for_source(&segment).is_none());
        for corruption in ["missing-flag", "markup", "empty", "oversize", "source"] {
            let mut changed = valid.clone();
            let entry = changed.segments.get_mut(&segment.id).unwrap();
            match corruption {
                "missing-flag" => entry.needs_review = false,
                "markup" => entry.markup = Some("I don't know.".into()),
                "empty" => entry.translated_text = " ".into(),
                "oversize" => {
                    entry.translated_text =
                        "x".repeat(super::super::inline_markup::protected::MAX_HTML_BYTES + 1)
                }
                _ => entry.source_hash = hash_segment_text("another source"),
            }
            assert!(
                changed
                    .sentence_output_for(&segment, &source, true)
                    .is_none(),
                "{corruption}"
            );
        }
        let mut json = serde_json::to_value(&valid).unwrap();
        json["segments"][&segment.id]
            .as_object_mut()
            .unwrap()
            .remove("needsReview");
        let missing: super::TranslationSegmentCache = serde_json::from_value(json).unwrap();
        assert!(missing
            .sentence_output_for(&segment, &source, true)
            .is_none());
        cache.store_translation(&segment, "legacy text".into());
        assert!(cache.sentence_output_for(&segment, &source, true).is_none());
    }

    #[test]
    fn source_hash_changes_when_text_changes() {
        assert_ne!(hash_segment_text("one"), hash_segment_text("two"));
    }

    #[test]
    fn cached_segment_requires_matching_source_hash() {
        let plan = plan();
        let segment = plan.batches[0].segments[0].clone();
        let mut cache = new_segment_cache(&plan);
        cache.store_translation(&segment, "uno".into());

        assert_eq!(cache.translated_text_for(&segment), Some("uno"));
        let mut changed = segment.clone();
        changed.text = "Changed text".into();
        assert_eq!(cache.translated_text_for(&changed), None);
    }

    #[test]
    fn cache_reuses_exact_source_text_across_segment_ids() {
        let plan = plan();
        let first = plan.batches[0].segments[0].clone();
        let mut second = plan.batches[0].segments[0].clone();
        second.id = "source-block-99-segment-1".into();
        let mut cache = new_segment_cache(&plan);
        cache.store_translation(&first, "uno".into());

        assert_eq!(cache.translated_text_for(&second), None);
        assert_eq!(cache.translated_text_for_source(&second), Some("uno"));
    }

    #[test]
    fn empty_translation_is_not_cached() {
        let plan = plan();
        let segment = plan.batches[0].segments[0].clone();
        let mut cache = new_segment_cache(&plan);
        cache.store_translation(&segment, "   ".into());

        assert_eq!(cache.translated_text_for(&segment), None);
        assert_eq!(cache.translated_text_for_source(&segment), None);
    }

    #[test]
    fn cache_settings_must_match_job_plan() {
        let mut plan = plan();
        let cache = new_segment_cache(&plan);

        assert!(cache_is_compatible(&cache, &plan));
        plan.request.quality_mode = "quality".into();
        assert!(!cache_is_compatible(&cache, &plan));
    }

    #[test]
    fn probe_translations_reuse_shared_memory() {
        let plan = plan();
        let mut cache = new_segment_cache(&plan);

        assert_eq!(cache.translated_probe_text("frase corta"), None);
        cache.store_probe_translation("frase corta", "short phrase".into());
        assert_eq!(
            cache.translated_probe_text("frase corta"),
            Some("short phrase")
        );

        cache.store_probe_translation("otra frase", "   ".into());
        assert_eq!(cache.translated_probe_text("otra frase"), None);
    }

    #[test]
    fn stale_inline_alignment_version_invalidates_cache() {
        let plan = plan();
        let mut cache = new_segment_cache(&plan);

        assert!(cache_is_compatible(&cache, &plan));
        cache.inline_alignment_version = super::INLINE_ALIGNMENT_VERSION - 1;
        assert!(!cache_is_compatible(&cache, &plan));
    }

    fn plan() -> crate::translation::job::TranslationJobPlan {
        plan_translation_job(
            TranslationStartRequest {
                job_id: None,
                document_url: "/uploads/book.html".into(),
                source_language: "es".into(),
                target_language: "en".into(),
                model_id: "opus-mt-es-en-ctranslate2".into(),
                quality_mode: "balanced".into(),
                use_hardware_acceleration: false,
                preserve_formatting: false,
                repair_mode: Default::default(),
                glossary: Vec::new(),
            },
            ["Hola mundo."],
            100,
            4,
        )
        .expect("plan")
    }
}
