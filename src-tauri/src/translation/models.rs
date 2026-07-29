//! Offline translation model catalog.
//!
//! Pinned CTranslate2 and llama.cpp entries are installable on supported
//! desktop builds; candidate-only models remain descriptive until their
//! runtime, licensing, and platform constraints are validated.

use super::config::DEFAULT_TRANSLATION_QUALITY_MODE;
use super::types::TranslationModelInfo;

#[derive(Clone, Copy, Debug)]
pub(crate) struct TranslationModelDefinition {
    pub(crate) id: &'static str,
    pub(crate) name: &'static str,
    pub(crate) engine: &'static str,
    pub(crate) tier: &'static str,
    pub(crate) manifest_state: &'static str,
    pub(crate) source_languages: &'static [&'static str],
    pub(crate) target_languages: &'static [&'static str],
    pub(crate) recommended_platforms: &'static [&'static str],
    pub(crate) license_notes: &'static str,
    pub(crate) size_notes: &'static str,
    pub(crate) notes: &'static str,
}

impl TranslationModelDefinition {
    /// Project catalog data into the frontend capability shape.
    pub(crate) fn to_info(self) -> TranslationModelInfo {
        TranslationModelInfo {
            id: self.id.into(),
            name: self.name.into(),
            engine: self.engine.into(),
            tier: self.tier.into(),
            manifest_state: self.manifest_state.into(),
            source_languages: self
                .source_languages
                .iter()
                .map(|language| (*language).into())
                .collect(),
            target_languages: self
                .target_languages
                .iter()
                .map(|language| (*language).into())
                .collect(),
            default_quality_mode: DEFAULT_TRANSLATION_QUALITY_MODE.into(),
            recommended_platforms: self
                .recommended_platforms
                .iter()
                .map(|platform| (*platform).into())
                .collect(),
            license_notes: self.license_notes.into(),
            size_notes: self.size_notes.into(),
            notes: self.notes.into(),
        }
    }
}

/// Translation models and manifest states surfaced to the frontend.
///
/// Runnable rows have pinned file manifests in `model_store`; remaining rows
/// stay candidate-only.
pub(crate) const PLANNED_TRANSLATION_MODELS: &[TranslationModelDefinition] = &[
    TranslationModelDefinition {
        id: "opus-mt-es-en-ctranslate2",
        name: "OPUS-MT Spanish to English",
        engine: "ctranslate2",
        tier: "fast",
        manifest_state: "pinned-file-manifest",
        source_languages: &["es"],
        target_languages: &["en"],
        recommended_platforms: &["desktop", "android"],
        license_notes: "Requires Helsinki-NLP/OPUS-MT model-card and redistribution review before download support.",
        size_notes: "About 159 MB installed; optimized for practical CPU inference.",
        notes: "First CTranslate2 MVP candidate; use this to prove model install, batching, and indexing before wider language support.",
    },
    TranslationModelDefinition {
        id: "opus-mt-fr-en-ctranslate2",
        name: "OPUS-MT French to English",
        engine: "ctranslate2",
        tier: "fast",
        manifest_state: "pinned-file-manifest",
        source_languages: &["fr"],
        target_languages: &["en"],
        recommended_platforms: &["desktop", "android"],
        license_notes: "Requires Helsinki-NLP/OPUS-MT model-card and redistribution review before download support.",
        size_notes: "About 153 MB installed; optimized for practical CPU inference.",
        notes: "Second CTranslate2 MVP candidate; useful comparison against Spanish for quality and packaging behavior.",
    },
    TranslationModelDefinition {
        id: "hy-mt2-1.8b-q8",
        name: "HY-MT2 1.8B Q8",
        engine: "llama.cpp",
        tier: "quality",
        manifest_state: "pinned-file-manifest",
        source_languages: &["ar", "de", "es", "fr", "ru", "zh"],
        target_languages: &["en"],
        recommended_platforms: &["desktop"],
        license_notes: "Tencent HY-MT2 is Apache-2.0; preserve model attribution when redistributing or documenting it.",
        size_notes: "About 1.9 GB for the pinned Q8_0 GGUF; CPU inference is supported, while GPU acceleration remains unverified.",
        notes: "Quality-focused desktop model using llama.cpp. This first integration is CPU-first; CUDA and mobile packaging require separate validation.",
    },
    TranslationModelDefinition {
        id: "qwen3-8b",
        name: "Qwen3 8B",
        engine: "llama.cpp",
        tier: "context",
        manifest_state: "candidate-only",
        source_languages: &["ar", "de", "es", "fr", "ru", "zh"],
        target_languages: &["en"],
        recommended_platforms: &["desktop"],
        license_notes: "Requires model-license, prompt-policy, and redistribution review.",
        size_notes: "Large desktop experiment; not a default mobile candidate.",
        notes: "Context-rich academic-prose experiment; needs strict prompts and QA to avoid paraphrase drift.",
    },
];

/// Return candidate metadata for capabilities/diagnostics.
///
/// Keeping this as a function instead of exporting the const directly gives
/// future storage or platform filters one place to narrow the catalog without
/// making command code understand model-family details.
pub(crate) fn planned_models() -> Vec<TranslationModelInfo> {
    PLANNED_TRANSLATION_MODELS
        .iter()
        .map(|model| model.to_info())
        .collect()
}

pub(crate) fn find_planned_model(id: &str) -> Option<TranslationModelDefinition> {
    PLANNED_TRANSLATION_MODELS
        .iter()
        .copied()
        .find(|model| model.id == id)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{find_planned_model, planned_models, PLANNED_TRANSLATION_MODELS};

    #[test]
    fn planned_model_ids_are_unique() {
        let mut seen = HashSet::new();

        for model in PLANNED_TRANSLATION_MODELS {
            assert!(seen.insert(model.id), "duplicate model id {}", model.id);
        }
    }

    #[test]
    fn planned_models_report_manifest_state_and_review_notes() {
        for model in planned_models() {
            assert!(!model.manifest_state.is_empty());
            assert!(!model.license_notes.is_empty());
            assert!(!model.size_notes.is_empty());
        }
    }

    #[test]
    fn finds_planned_model_by_id() {
        let model = find_planned_model("opus-mt-es-en-ctranslate2").expect("model");

        assert_eq!(model.engine, "ctranslate2");
        assert!(find_planned_model("missing-model").is_none());
    }
}
