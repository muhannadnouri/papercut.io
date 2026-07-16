//! Supported offline TTS models, backend identity, and loading metadata.

use std::path::Path;

use crate::native_tts::types::{
    NativeTextPreprocessorInfo, NativeTtsModelInfo, NativeTtsVoiceInfo,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Runtime backend that owns inference for a model catalog entry.
pub(super) enum TtsModelBackend {
    SherpaOnnx,
    #[allow(dead_code)]
    SilmaSidecar,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// User-visible model family; this is intentionally not the same as backend.
pub(super) enum TtsModelFamily {
    Kokoro,
    Supertonic,
    Vits,
    #[allow(dead_code)]
    SilmaF5,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// sherpa-onnx configuration family used to build the correct native model block.
pub(super) enum SherpaModelFamily {
    Supertonic,
    Kokoro,
    Vits,
}

pub(super) const TEXT_PREPROCESSOR_NONE: &str = "none";
#[cfg(feature = "native-text-preprocessing-core")]
pub(super) const TEXT_PREPROCESSOR_LIBTASHKEEL: &str = "libtashkeel-1.5.0";

#[derive(Clone, Copy, Debug)]
/// User-visible, versioned preprocessing capability advertised by one model.
pub(super) struct TextPreprocessorDefinition {
    pub(super) id: &'static str,
    pub(super) name: &'static str,
    pub(super) description: &'static str,
}

#[derive(Clone, Copy, Debug)]
/// Catalog voice mapped to the numeric speaker id expected by sherpa-onnx.
pub(super) struct VoiceDefinition {
    pub(super) id: &'static str,
    pub(super) name: &'static str,
    pub(super) speaker_id: i32,
}

#[derive(Clone, Copy, Debug)]
/// Complete source, validation, loading, voice, and preprocessing contract for a model.
pub(super) struct ModelDefinition {
    pub(super) id: &'static str,
    pub(super) directory_name: &'static str,
    pub(super) display_name: &'static str,
    pub(super) backend: TtsModelBackend,
    pub(super) family: TtsModelFamily,
    pub(super) sherpa_family: Option<SherpaModelFamily>,
    pub(super) language: &'static str,
    pub(super) language_label: &'static str,
    pub(super) supertonic_lang: Option<&'static str>,
    pub(super) source_label: &'static str,
    pub(super) source_url: &'static str,
    pub(super) sha256: &'static str,
    pub(super) archive_bytes: u64,
    pub(super) model_file: &'static str,
    pub(super) required_files: &'static [&'static str],
    pub(super) default_voice: &'static str,
    pub(super) voices: &'static [VoiceDefinition],
    pub(super) default_text_preprocessor: &'static str,
    pub(super) text_preprocessors: &'static [TextPreprocessorDefinition],
}

impl ModelDefinition {
    /// Return whether this entry should be advertised to the UI catalog.
    pub(super) fn is_catalog_visible(&self) -> bool {
        self.is_supported_on_current_platform()
    }

    /// Return whether this model's backend can run on the current build target.
    pub(super) fn is_supported_on_current_platform(&self) -> bool {
        match self.backend {
            TtsModelBackend::SherpaOnnx => true,
            TtsModelBackend::SilmaSidecar => {
                cfg!(all(target_os = "linux", target_arch = "x86_64"))
            }
        }
    }

    /// App-data subdirectory used for this backend's installed model files.
    pub(super) fn model_storage_dir_name(&self) -> &'static str {
        match self.backend {
            TtsModelBackend::SherpaOnnx => "sherpa-onnx",
            TtsModelBackend::SilmaSidecar => "silma-tts",
        }
    }

    /// Whether the current in-app installer knows how to fetch this model.
    pub(super) fn install_supported(&self) -> bool {
        matches!(self.backend, TtsModelBackend::SherpaOnnx)
    }

    /// Return true only when every file required by this model family is installed.
    pub(super) fn has_required_files(&self, dir: &Path) -> bool {
        self.required_files
            .iter()
            .all(|path| dir.join(path).is_file())
    }

    /// Resolve a catalog voice to its native speaker id; reject cross-model voices.
    pub(super) fn speaker_id(&self, voice_id: &str) -> Result<i32, String> {
        self.voices
            .iter()
            .find(|voice| voice.id == voice_id)
            .map(|voice| voice.speaker_id)
            .ok_or_else(|| {
                format!(
                    "Voice {voice_id:?} is not supported by model {}",
                    self.display_name
                )
            })
    }

    /// Validate a requested preprocessor against capabilities advertised by this model.
    pub(super) fn supports_text_preprocessor(&self, id: &str) -> bool {
        self.text_preprocessors.iter().any(|item| item.id == id)
    }

    /// Return the sherpa family only for sherpa-backed entries.
    ///
    /// SILMA and future non-sherpa models should fail here until their own
    /// backend routes are implemented, rather than pretending to be VITS/Kokoro.
    pub(super) fn require_sherpa_family(&self) -> Result<SherpaModelFamily, String> {
        self.sherpa_family.ok_or_else(|| {
            format!(
                "Model {} is not a sherpa-onnx model and cannot use the sherpa loader",
                self.display_name
            )
        })
    }

    /// English-only synthesis-text normalization (year expansion, roman numerals,
    /// semicolon/decimal cleanup) only helps the English eSpeak/Kokoro path. Other
    /// languages (e.g. Arabic Piper) must never have Western number words or
    /// English-specific punctuation rewrites spliced into their synthesis text.
    pub(super) fn english_text_normalization(&self) -> bool {
        matches!(self.family, TtsModelFamily::Kokoro) && self.language.starts_with("en")
    }

    /// Stable diagnostic label identifying the active model backend and family.
    pub(super) fn backend_name(&self) -> &'static str {
        match (self.backend, self.family) {
            (TtsModelBackend::SherpaOnnx, TtsModelFamily::Kokoro) => "sherpa-onnx-kokoro",
            (TtsModelBackend::SherpaOnnx, TtsModelFamily::Supertonic) => "sherpa-onnx-supertonic",
            (TtsModelBackend::SherpaOnnx, TtsModelFamily::Vits) => "sherpa-onnx-vits",
            (TtsModelBackend::SilmaSidecar, TtsModelFamily::SilmaF5) => "silma-sidecar-f5",
            _ => "native-tts-unknown",
        }
    }

    /// Project internal catalog metadata into the serializable frontend capability DTO.
    pub(super) fn to_info(&self) -> NativeTtsModelInfo {
        NativeTtsModelInfo {
            id: self.id.into(),
            name: self.display_name.into(),
            family: match self.family {
                TtsModelFamily::Kokoro => "kokoro",
                TtsModelFamily::Supertonic => "supertonic",
                TtsModelFamily::Vits => "vits",
                TtsModelFamily::SilmaF5 => "silma-f5",
            }
            .into(),
            language: self.language.into(),
            language_label: self.language_label.into(),
            default_voice: self.default_voice.into(),
            voices: self
                .voices
                .iter()
                .map(|voice| NativeTtsVoiceInfo {
                    id: voice.id.into(),
                    name: voice.name.into(),
                })
                .collect(),
            default_text_preprocessor: self.default_text_preprocessor.into(),
            text_preprocessors: self
                .text_preprocessors
                .iter()
                .map(|preprocessor| NativeTextPreprocessorInfo {
                    id: preprocessor.id.into(),
                    name: preprocessor.name.into(),
                    description: preprocessor.description.into(),
                })
                .collect(),
        }
    }
}

const KOKORO_VOICES: &[VoiceDefinition] = &[
    VoiceDefinition {
        id: "af_alloy",
        name: "Alloy",
        speaker_id: 0,
    },
    VoiceDefinition {
        id: "af_aoede",
        name: "Aoede",
        speaker_id: 1,
    },
    VoiceDefinition {
        id: "af_bella",
        name: "Bella",
        speaker_id: 2,
    },
    VoiceDefinition {
        id: "af_heart",
        name: "Heart",
        speaker_id: 3,
    },
    VoiceDefinition {
        id: "af_jessica",
        name: "Jessica",
        speaker_id: 4,
    },
    VoiceDefinition {
        id: "af_kore",
        name: "Kore",
        speaker_id: 5,
    },
    VoiceDefinition {
        id: "af_nicole",
        name: "Nicole",
        speaker_id: 6,
    },
    VoiceDefinition {
        id: "af_nova",
        name: "Nova",
        speaker_id: 7,
    },
    VoiceDefinition {
        id: "af_river",
        name: "River",
        speaker_id: 8,
    },
    VoiceDefinition {
        id: "af_sarah",
        name: "Sarah",
        speaker_id: 9,
    },
    VoiceDefinition {
        id: "af_sky",
        name: "Sky",
        speaker_id: 10,
    },
    VoiceDefinition {
        id: "am_echo",
        name: "Echo",
        speaker_id: 12,
    },
    VoiceDefinition {
        id: "am_eric",
        name: "Eric",
        speaker_id: 13,
    },
    VoiceDefinition {
        id: "am_fenrir",
        name: "Fenrir",
        speaker_id: 14,
    },
    VoiceDefinition {
        id: "am_liam",
        name: "Liam",
        speaker_id: 15,
    },
    VoiceDefinition {
        id: "am_michael",
        name: "Michael",
        speaker_id: 16,
    },
    VoiceDefinition {
        id: "am_onyx",
        name: "Onyx",
        speaker_id: 17,
    },
    VoiceDefinition {
        id: "am_puck",
        name: "Puck",
        speaker_id: 18,
    },
    VoiceDefinition {
        id: "am_santa",
        name: "Santa",
        speaker_id: 19,
    },
    VoiceDefinition {
        id: "bf_alice",
        name: "Alice",
        speaker_id: 20,
    },
    VoiceDefinition {
        id: "bf_emma",
        name: "Emma",
        speaker_id: 21,
    },
    VoiceDefinition {
        id: "bf_isabella",
        name: "Isabella",
        speaker_id: 22,
    },
    VoiceDefinition {
        id: "bf_lily",
        name: "Lily",
        speaker_id: 23,
    },
    VoiceDefinition {
        id: "bm_daniel",
        name: "Daniel",
        speaker_id: 24,
    },
    VoiceDefinition {
        id: "bm_fable",
        name: "Fable",
        speaker_id: 25,
    },
    VoiceDefinition {
        id: "bm_george",
        name: "George",
        speaker_id: 26,
    },
    VoiceDefinition {
        id: "bm_lewis",
        name: "Lewis",
        speaker_id: 27,
    },
];

const SUPERTONIC_VOICES: &[VoiceDefinition] = &[VoiceDefinition {
    id: "speaker_6",
    name: "Speaker 6",
    speaker_id: 6,
}];

const PIPER_KAREEM_VOICES: &[VoiceDefinition] = &[VoiceDefinition {
    id: "kareem",
    name: "Kareem",
    speaker_id: 0,
}];

const SILMA_VOICES: &[VoiceDefinition] = &[VoiceDefinition {
    id: "silma-ar-default",
    name: "SILMA Arabic Reference",
    speaker_id: 0,
}];

const IDENTITY_TEXT_PREPROCESSORS: &[TextPreprocessorDefinition] = &[TextPreprocessorDefinition {
    id: TEXT_PREPROCESSOR_NONE,
    name: "Original text",
    description: "Synthesize source text without language preprocessing.",
}];

const SILMA_TEXT_PREPROCESSORS: &[TextPreprocessorDefinition] = &[TextPreprocessorDefinition {
    id: "silma-default",
    name: "SILMA default",
    description: "Use SILMA's default Arabic text processing before synthesis.",
}];

#[cfg(feature = "native-text-preprocessing-core")]
const PIPER_TEXT_PREPROCESSORS: &[TextPreprocessorDefinition] = &[
    TextPreprocessorDefinition {
        id: TEXT_PREPROCESSOR_NONE,
        name: "Original text",
        description: "Synthesize Arabic source text without automatic diacritization.",
    },
    TextPreprocessorDefinition {
        id: TEXT_PREPROCESSOR_LIBTASHKEEL,
        name: "Auto diacritization",
        description: "Restore Arabic tashkeel with Libtashkeel before Piper synthesis.",
    },
];
#[cfg(not(feature = "native-text-preprocessing-core"))]
const PIPER_TEXT_PREPROCESSORS: &[TextPreprocessorDefinition] = IDENTITY_TEXT_PREPROCESSORS;

#[cfg(feature = "native-text-preprocessing-core")]
const PIPER_DEFAULT_TEXT_PREPROCESSOR: &str = TEXT_PREPROCESSOR_LIBTASHKEEL;
#[cfg(not(feature = "native-text-preprocessing-core"))]
const PIPER_DEFAULT_TEXT_PREPROCESSOR: &str = TEXT_PREPROCESSOR_NONE;

const KOKORO_REQUIRED_FILES: &[&str] = &[
    "model.onnx",
    "voices.bin",
    "tokens.txt",
    "espeak-ng-data/phontab",
    "espeak-ng-data/en_dict",
    "lexicon-us-en.txt",
];

const SUPERTONIC_REQUIRED_FILES: &[&str] = &[
    "duration_predictor.int8.onnx",
    "text_encoder.int8.onnx",
    "vector_estimator.int8.onnx",
    "vocoder.int8.onnx",
    "tts.json",
    "unicode_indexer.bin",
    "voice.bin",
];

const PIPER_REQUIRED_FILES: &[&str] = &[
    "ar_JO-kareem-medium.onnx",
    "tokens.txt",
    "espeak-ng-data/phontab",
    "espeak-ng-data/ar_dict",
];

const SILMA_REQUIRED_FILES: &[&str] = &[
    "models--silma-ai--silma-tts/snapshots/d2515317033803648ecb8844765db9e583afecf9/model.pt",
    "models--silma-ai--silma-tts/snapshots/d2515317033803648ecb8844765db9e583afecf9/vocab.txt",
];

pub(super) const DEFAULT_MODEL_ID: &str = "sherpa-onnx/kokoro-multi-lang-v1_0";
pub(super) const SILMA_MODEL_ID: &str = "silma-ai/silma-tts";
pub(super) const SILMA_HF_CACHE_REPO_DIR: &str = "models--silma-ai--silma-tts";
pub(super) const SILMA_HF_REVISION: &str = "d2515317033803648ecb8844765db9e583afecf9";

pub(super) const MODELS: &[ModelDefinition] = &[
    ModelDefinition {
        id: DEFAULT_MODEL_ID,
        directory_name: "kokoro-multi-lang-v1_0",
        display_name: "Kokoro v1.0",
        backend: TtsModelBackend::SherpaOnnx,
        family: TtsModelFamily::Kokoro,
        sherpa_family: Some(SherpaModelFamily::Kokoro),
        language: "en-US",
        language_label: "English",
        supertonic_lang: None,
        source_label: "k2-fsa/sherpa-onnx Kokoro multi-lang v1.0",
        source_url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/kokoro-multi-lang-v1_0.tar.bz2",
        sha256: "c133d26353d776da730870dac7da07dbfc9a5e3bc80cc5e8e83ab6e823be7046",
        archive_bytes: 349_418_188,
        model_file: "model.onnx",
        required_files: KOKORO_REQUIRED_FILES,
        default_voice: "af_heart",
        voices: KOKORO_VOICES,
        default_text_preprocessor: TEXT_PREPROCESSOR_NONE,
        text_preprocessors: IDENTITY_TEXT_PREPROCESSORS,
    },
    ModelDefinition {
        id: "sherpa-onnx/supertonic-3-en",
        directory_name: "sherpa-onnx-supertonic-3-tts-int8-2026-05-11",
        display_name: "Supertonic 3 English",
        backend: TtsModelBackend::SherpaOnnx,
        family: TtsModelFamily::Supertonic,
        sherpa_family: Some(SherpaModelFamily::Supertonic),
        language: "en-US",
        language_label: "English",
        supertonic_lang: Some("en"),
        source_label: "k2-fsa/sherpa-onnx SupertonicTTS 3 int8",
        source_url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/sherpa-onnx-supertonic-3-tts-int8-2026-05-11.tar.bz2",
        sha256: "82fa96f91c4ef8abaae3a14a3f4153facf88bed821d1f7331cec2700f432c427",
        archive_bytes: 123_000_000,
        model_file: "vocoder.int8.onnx",
        required_files: SUPERTONIC_REQUIRED_FILES,
        default_voice: "speaker_6",
        voices: SUPERTONIC_VOICES,
        default_text_preprocessor: TEXT_PREPROCESSOR_NONE,
        text_preprocessors: IDENTITY_TEXT_PREPROCESSORS,
    },
    ModelDefinition {
        id: "sherpa-onnx/supertonic-3-ar",
        directory_name: "sherpa-onnx-supertonic-3-tts-int8-2026-05-11",
        display_name: "Supertonic 3 Arabic",
        backend: TtsModelBackend::SherpaOnnx,
        family: TtsModelFamily::Supertonic,
        sherpa_family: Some(SherpaModelFamily::Supertonic),
        language: "ar",
        language_label: "Arabic",
        supertonic_lang: Some("ar"),
        source_label: "k2-fsa/sherpa-onnx SupertonicTTS 3 int8",
        source_url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/sherpa-onnx-supertonic-3-tts-int8-2026-05-11.tar.bz2",
        sha256: "82fa96f91c4ef8abaae3a14a3f4153facf88bed821d1f7331cec2700f432c427",
        archive_bytes: 123_000_000,
        model_file: "vocoder.int8.onnx",
        required_files: SUPERTONIC_REQUIRED_FILES,
        default_voice: "speaker_6",
        voices: SUPERTONIC_VOICES,
        default_text_preprocessor: TEXT_PREPROCESSOR_NONE,
        text_preprocessors: IDENTITY_TEXT_PREPROCESSORS,
    },
    ModelDefinition {
        id: "sherpa-onnx/vits-piper-ar_JO-kareem-medium",
        directory_name: "vits-piper-ar_JO-kareem-medium",
        display_name: "Piper Kareem Medium",
        backend: TtsModelBackend::SherpaOnnx,
        family: TtsModelFamily::Vits,
        sherpa_family: Some(SherpaModelFamily::Vits),
        language: "ar-JO",
        language_label: "Arabic (Jordan)",
        supertonic_lang: None,
        source_label: "k2-fsa/sherpa-onnx Piper ar_JO Kareem medium",
        source_url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/tts-models/vits-piper-ar_JO-kareem-medium.tar.bz2",
        sha256: "9ebbcea30e0fbd588f7b2cb45ee897d6aeb1bf5791cbc037a7b5a3f641e3dbce",
        archive_bytes: 67_177_830,
        model_file: "ar_JO-kareem-medium.onnx",
        required_files: PIPER_REQUIRED_FILES,
        default_voice: "kareem",
        voices: PIPER_KAREEM_VOICES,
        default_text_preprocessor: PIPER_DEFAULT_TEXT_PREPROCESSOR,
        text_preprocessors: PIPER_TEXT_PREPROCESSORS,
    },
    ModelDefinition {
        id: SILMA_MODEL_ID,
        directory_name: "silma-tts",
        display_name: "SILMA Arabic TTS",
        backend: TtsModelBackend::SilmaSidecar,
        family: TtsModelFamily::SilmaF5,
        sherpa_family: None,
        language: "ar",
        language_label: "Arabic",
        supertonic_lang: None,
        source_label: "silma-ai/silma-tts",
        source_url: "https://huggingface.co/silma-ai/silma-tts",
        sha256: "",
        archive_bytes: 2_603_245_629,
        model_file: "model.pt",
        required_files: SILMA_REQUIRED_FILES,
        default_voice: "silma-ar-default",
        voices: SILMA_VOICES,
        default_text_preprocessor: "silma-default",
        text_preprocessors: SILMA_TEXT_PREPROCESSORS,
    },
];

/// Models advertised to the frontend capability catalog.
pub(super) fn visible_models() -> impl Iterator<Item = &'static ModelDefinition> {
    MODELS.iter().filter(|model| model.is_catalog_visible())
}

/// Resolve the authoritative catalog entry used by install, synthesis, and import.
pub(super) fn model_definition(model_id: &str) -> Result<&'static ModelDefinition, String> {
    MODELS
        .iter()
        .find(|model| model.id == model_id && model.is_supported_on_current_platform())
        .ok_or_else(|| format!("Unsupported native TTS model: {model_id}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn catalog_ids_are_unique_and_default_exists() {
        let ids = MODELS.iter().map(|model| model.id).collect::<HashSet<_>>();
        assert_eq!(ids.len(), MODELS.len());
        assert_eq!(
            model_definition(DEFAULT_MODEL_ID).unwrap().family,
            TtsModelFamily::Kokoro
        );
    }

    #[test]
    fn supertonic_entries_share_archive_and_have_lang_codes() {
        let en = model_definition("sherpa-onnx/supertonic-3-en").unwrap();
        let ar = model_definition("sherpa-onnx/supertonic-3-ar").unwrap();
        assert_eq!(en.backend, TtsModelBackend::SherpaOnnx);
        assert!(en.install_supported());
        assert_eq!(en.family, TtsModelFamily::Supertonic);
        assert_eq!(ar.family, TtsModelFamily::Supertonic);
        assert_eq!(
            en.require_sherpa_family().unwrap(),
            SherpaModelFamily::Supertonic
        );
        assert_eq!(en.directory_name, ar.directory_name);
        assert_eq!(en.supertonic_lang, Some("en"));
        assert_eq!(ar.supertonic_lang, Some("ar"));
        assert_eq!(en.speaker_id("speaker_6").unwrap(), 6);
    }

    #[test]
    fn piper_kareem_has_one_valid_voice() {
        let model = model_definition("sherpa-onnx/vits-piper-ar_JO-kareem-medium").unwrap();
        assert_eq!(model.family, TtsModelFamily::Vits);
        assert_eq!(
            model.require_sherpa_family().unwrap(),
            SherpaModelFamily::Vits
        );
        assert_eq!(model.speaker_id("kareem").unwrap(), 0);
        assert!(model.speaker_id("af_heart").is_err());
    }

    #[test]
    fn silma_backend_metadata_has_its_own_storage_and_label() {
        let model = ModelDefinition {
            id: "silma/smoke",
            directory_name: "silma-smoke",
            display_name: "SILMA Smoke",
            backend: TtsModelBackend::SilmaSidecar,
            family: TtsModelFamily::SilmaF5,
            sherpa_family: None,
            language: "ar",
            language_label: "Arabic",
            supertonic_lang: None,
            source_label: "SILMA smoke",
            source_url: "",
            sha256: "",
            archive_bytes: 0,
            model_file: "model.pt",
            required_files: &["model.pt"],
            default_voice: "silma-ar-default",
            voices: &[],
            default_text_preprocessor: TEXT_PREPROCESSOR_NONE,
            text_preprocessors: IDENTITY_TEXT_PREPROCESSORS,
        };

        assert_eq!(model.model_storage_dir_name(), "silma-tts");
        assert_eq!(model.backend_name(), "silma-sidecar-f5");
        assert!(!model.install_supported());
        assert!(model.require_sherpa_family().is_err());
        assert_eq!(model.to_info().family, "silma-f5");
    }

    #[test]
    fn silma_catalog_entry_is_release_visible_on_linux_x64() {
        if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            assert!(model_definition(SILMA_MODEL_ID).is_err());
            assert!(visible_models().all(|item| item.id != SILMA_MODEL_ID));
            return;
        }
        let model = model_definition(SILMA_MODEL_ID).unwrap();
        assert_eq!(model.backend, TtsModelBackend::SilmaSidecar);
        assert_eq!(model.model_storage_dir_name(), "silma-tts");
        assert!(model.is_catalog_visible());
        assert!(visible_models().any(|item| item.id == SILMA_MODEL_ID));
    }
}
