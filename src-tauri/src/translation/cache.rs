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
    cache_key: String,
    model_id: String,
    source_language: String,
    target_language: String,
    quality_mode: String,
    max_segment_chars: usize,
    batch_segment_limit: usize,
    segments: BTreeMap<String, CachedTranslationSegment>,
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
}

impl TranslationSegmentCache {
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
    let Ok(cache) = serde_json::from_slice::<TranslationSegmentCache>(&bytes) else {
        log::warn!(
            "translation: ignoring unreadable segment cache {}; starting fresh",
            path.display()
        );
        return Ok(new_segment_cache(plan));
    };
    if cache_is_compatible(&cache, plan) {
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
        cache_key: plan.cache_key.clone(),
        model_id: plan.request.model_id.clone(),
        source_language: plan.request.source_language.clone(),
        target_language: plan.request.target_language.clone(),
        quality_mode: plan.request.quality_mode.clone(),
        max_segment_chars: plan.max_segment_chars,
        batch_segment_limit: plan.batch_segment_limit,
        segments: BTreeMap::new(),
        memory: BTreeMap::new(),
        inline_alignment_version: INLINE_ALIGNMENT_VERSION,
    }
}

fn cache_is_compatible(cache: &TranslationSegmentCache, plan: &TranslationJobPlan) -> bool {
    cache.version == SEGMENT_CACHE_VERSION
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
        cache.inline_alignment_version = 0;
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
