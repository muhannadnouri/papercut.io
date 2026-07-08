//! Optional text preprocessing applied immediately before synthesis.
//!
//! Source chunks remain unchanged for cache signatures, playback navigation, and
//! DOM highlighting. Only the string passed to the selected TTS model is transformed.

use super::models::{ModelDefinition, TEXT_PREPROCESSOR_NONE};

#[cfg(feature = "native-text-preprocessing")]
use std::panic::{catch_unwind, AssertUnwindSafe};
#[cfg(feature = "native-text-preprocessing")]
use std::sync::OnceLock;

#[cfg(feature = "native-text-preprocessing")]
use libtashkeel_base::{create_inference_engine, do_tashkeel, DynamicInferenceEngine};

/// Loaded synthesis-text transformer selected from model capabilities.
///
/// It never mutates canonical source chunks, preserving cache signatures and
/// DOM highlighting while allowing model-specific pronunciation preparation.
pub(super) struct TextPreprocessor {
    id: &'static str,
    backend: TextPreprocessorBackend,
}

enum TextPreprocessorBackend {
    Identity,
    #[cfg(feature = "native-text-preprocessing")]
    Libtashkeel(DynamicInferenceEngine),
}

impl TextPreprocessor {
    /// Validate the capability and initialize its backend once per save job.
    /// Native setup panics are converted into recoverable command errors.
    pub(super) fn create(
        model: &'static ModelDefinition,
        requested_id: &str,
    ) -> Result<Self, String> {
        let definition = model
            .text_preprocessors
            .iter()
            .find(|item| item.id == requested_id)
            .ok_or_else(|| {
                format!(
                    "Text preprocessor {requested_id:?} is not supported by model {}",
                    model.display_name
                )
            })?;

        let backend = match definition.id {
            TEXT_PREPROCESSOR_NONE => TextPreprocessorBackend::Identity,
            #[cfg(feature = "native-text-preprocessing")]
            super::models::TEXT_PREPROCESSOR_LIBTASHKEEL => {
                initialize_ort()?;
                let engine = catch_unwind(create_inference_engine_default).map_err(|panic| {
                    format!(
                        "Libtashkeel initialization panicked: {}",
                        panic_message(panic)
                    )
                })??;
                TextPreprocessorBackend::Libtashkeel(engine)
            }
            _ => {
                return Err(format!(
                    "Text preprocessor {:?} is unavailable in this build",
                    definition.id
                ))
            }
        };

        Ok(Self {
            id: definition.id,
            backend,
        })
    }

    /// Return the versioned id persisted in manifests, bundles, and cache keys.
    pub(super) fn id(&self) -> &str {
        self.id
    }

    /// Produce synthesis text while leaving the caller source text untouched.
    /// Non-Arabic chunks bypass Libtashkeel to avoid unnecessary inference.
    pub(super) fn process(&self, source: &str) -> Result<String, String> {
        match &self.backend {
            TextPreprocessorBackend::Identity => Ok(source.to_string()),
            #[cfg(feature = "native-text-preprocessing")]
            TextPreprocessorBackend::Libtashkeel(engine) => {
                if !source.chars().any(is_arabic_character) {
                    return Ok(source.to_string());
                }
                catch_unwind(AssertUnwindSafe(|| do_tashkeel(engine, source, None, true)))
                    .map_err(|panic| {
                        format!("Libtashkeel inference panicked: {}", panic_message(panic))
                    })?
                    .map_err(|err| format!("Libtashkeel failed to diacritize text: {err}"))
            }
        }
    }
}

#[cfg(feature = "native-text-preprocessing")]
/// Initialize `ort` once and load sherpa-onnx packaged ONNX Runtime.
/// Dynamic builds load the packaged runtime; iOS links the same runtime statically.
fn initialize_ort() -> Result<(), String> {
    static RESULT: OnceLock<Result<(), String>> = OnceLock::new();
    RESULT
        .get_or_init(|| {
            catch_unwind(commit_ort_environment)
                .map_err(|panic| {
                    format!(
                        "ONNX Runtime initialization panicked: {}",
                        panic_message(panic)
                    )
                })?
                .map_err(|err| format!("Failed to initialize ONNX Runtime for Libtashkeel: {err}"))
        })
        .clone()
}

#[cfg(all(feature = "native-text-preprocessing", feature = "native-text-preprocessing-dynamic"))]
/// Load the packaged shared ONNX Runtime used by desktop and Android.
fn commit_ort_environment() -> Result<(), ort::Error> {
    let library = format!(
        "{}onnxruntime{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    );
    ort::init_from(library).commit()
}

#[cfg(all(feature = "native-text-preprocessing", not(feature = "native-text-preprocessing-dynamic")))]
/// Use the statically linked ONNX Runtime archive prepared for iOS.
fn commit_ort_environment() -> Result<(), ort::Error> {
    ort::init().commit()
}

#[cfg(feature = "native-text-preprocessing")]
/// Load the bundled Libtashkeel ONNX model behind its ORT engine interface.
fn create_inference_engine_default() -> Result<DynamicInferenceEngine, String> {
    create_inference_engine(None)
        .map_err(|err| format!("Failed to load bundled Libtashkeel model: {err}"))
}

#[cfg(feature = "native-text-preprocessing")]
/// Detect Arabic Unicode blocks to decide whether preprocessing is useful.
fn is_arabic_character(character: char) -> bool {
    matches!(
        character as u32,
        0x0600..=0x06ff | 0x0750..=0x077f | 0x08a0..=0x08ff | 0xfb50..=0xfdff | 0xfe70..=0xfeff
    )
}

#[cfg(feature = "native-text-preprocessing")]
/// Convert an unwound backend panic payload into a useful command error.
fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_tts::engine::models::{model_definition, DEFAULT_MODEL_ID};

    #[test]
    fn identity_preserves_source_exactly() {
        let model = model_definition(DEFAULT_MODEL_ID).unwrap();
        let preprocessor = TextPreprocessor::create(model, TEXT_PREPROCESSOR_NONE).unwrap();
        let source = "Text  with punctuation - and العربية.";
        assert_eq!(preprocessor.process(source).unwrap(), source);
    }
    #[cfg(feature = "native-text-preprocessing")]
    #[test]
    fn libtashkeel_adds_arabic_diacritics() {
        let model = model_definition("sherpa-onnx/vits-piper-ar_JO-kareem-medium").unwrap();
        let preprocessor =
            TextPreprocessor::create(model, super::super::models::TEXT_PREPROCESSOR_LIBTASHKEEL)
                .unwrap();
        let output = preprocessor.process("كتب الطالب الدرس").unwrap();

        assert!(output
            .chars()
            .any(|character| matches!(character as u32, 0x064b..=0x0652)));
    }
}
