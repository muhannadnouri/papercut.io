//! HY-MT2 translation through the pinned llama.cpp Rust bindings.
//!
//! The model is loaded once per Papercut translation job. Each planned batch
//! reuses one llama context and clears its KV cache between independent
//! segments, avoiding repeated model loads without mixing segment state.

use std::path::Path;

use super::engine::{TranslationBatchInput, TranslationEngine, TranslationSegmentOutput};
use super::types::{TranslationGlossaryEntry, TranslationHardwareAcceleration};

#[cfg(feature = "native-translation-llama")]
const HY_MT2_MODEL_FILE: &str = "Hy-MT2-1.8B-Q8_0.gguf";
#[cfg(feature = "native-translation-llama")]
const CONTEXT_TOKENS: u32 = 4_096;
#[cfg(feature = "native-translation-llama")]
const MAX_GENERATED_TOKENS: usize = 2_048;
#[cfg(feature = "native-translation-llama")]
const SAMPLING_SEED: u32 = 42;

#[cfg(feature = "native-translation-llama")]
use llama_cpp_2::context::params::LlamaContextParams;
#[cfg(feature = "native-translation-llama")]
use llama_cpp_2::llama_backend::LlamaBackend;
#[cfg(feature = "native-translation-llama")]
use llama_cpp_2::llama_batch::LlamaBatch;
#[cfg(feature = "native-translation-llama")]
use llama_cpp_2::model::params::LlamaModelParams;
#[cfg(feature = "native-translation-llama")]
use llama_cpp_2::model::{AddBos, LlamaModel};
#[cfg(feature = "native-translation-llama")]
use llama_cpp_2::sampling::LlamaSampler;
#[cfg(feature = "native-translation-llama")]
use llama_cpp_2::{list_llama_ggml_backend_devices, LlamaBackendDevice, LlamaBackendDeviceType};
#[cfg(feature = "native-translation-llama")]
use std::num::NonZeroU32;

pub(crate) struct HyMt2Engine {
    // Rust drops struct fields in declaration order. The model must be freed
    // before the backend that owns llama.cpp's process-wide native resources.
    #[cfg(feature = "native-translation-llama")]
    model: LlamaModel,
    #[cfg(feature = "native-translation-llama")]
    backend: LlamaBackend,
}

impl HyMt2Engine {
    /// Load the verified Q8 GGUF once for the lifetime of one translation job.
    pub(crate) fn for_installed_model(
        model_dir: &Path,
        use_hardware_acceleration: bool,
    ) -> Result<Self, String> {
        #[cfg(feature = "native-translation-llama")]
        {
            let model_path = model_dir.join(HY_MT2_MODEL_FILE);
            if !model_path.is_file() {
                return Err(format!(
                    "HY-MT2 model file is missing: {}",
                    model_path.display()
                ));
            }
            let mut backend = LlamaBackend::init()
                .map_err(|err| format!("Failed to initialize llama.cpp: {err}"))?;
            backend.void_logs();
            let model_params = if use_hardware_acceleration {
                let device = preferred_vulkan_device(&backend).ok_or_else(|| {
                    "Vulkan acceleration is no longer available. Turn off hardware acceleration and try again."
                        .to_string()
                })?;
                LlamaModelParams::default()
                    .with_devices(&[device.index])
                    .map_err(|err| format!("Failed to select Vulkan device: {err}"))?
            } else {
                // Vulkan-enabled builds otherwise inherit llama.cpp's default
                // all-layer offload, even when the user left acceleration off.
                LlamaModelParams::default().with_n_gpu_layers(0)
            };
            let model = LlamaModel::load_from_file(&backend, &model_path, &model_params).map_err(
                |err| {
                    format!(
                        "Failed to load HY-MT2 model {}: {err}",
                        model_path.display()
                    )
                },
            )?;
            Ok(Self { model, backend })
        }

        #[cfg(not(feature = "native-translation-llama"))]
        {
            let _ = (model_dir, use_hardware_acceleration);
            Err("HY-MT2 translation was not compiled with native-translation-llama.".into())
        }
    }
}

/// Probe the compiled llama.cpp backends and expose only a real Vulkan GPU.
///
/// Software Vulkan devices are reported as CPUs by llama.cpp and are skipped,
/// so the UI does not offer an acceleration switch that merely moves CPU work
/// through a graphics API.
pub(crate) fn hy_mt2_hardware_acceleration() -> Option<TranslationHardwareAcceleration> {
    #[cfg(feature = "native-translation-llama")]
    {
        let mut backend = LlamaBackend::init().ok()?;
        backend.void_logs();
        let device = preferred_vulkan_device(&backend)?;
        let device_name = if device.description.trim().is_empty() {
            device.name
        } else {
            device.description
        };
        return Some(TranslationHardwareAcceleration {
            backend: device.backend,
            device: device_name,
        });
    }

    #[cfg(not(feature = "native-translation-llama"))]
    None
}

#[cfg(feature = "native-translation-llama")]
fn preferred_vulkan_device(backend: &LlamaBackend) -> Option<LlamaBackendDevice> {
    if !backend.supports_gpu_offload() {
        return None;
    }
    list_llama_ggml_backend_devices()
        .into_iter()
        .filter(|device| {
            device.backend.eq_ignore_ascii_case("vulkan")
                && matches!(
                    device.device_type,
                    LlamaBackendDeviceType::Gpu | LlamaBackendDeviceType::IntegratedGpu
                )
        })
        .max_by_key(|device| device.memory_total)
}

impl TranslationEngine for HyMt2Engine {
    fn translate_batch(
        &mut self,
        input: TranslationBatchInput,
    ) -> Result<Vec<TranslationSegmentOutput>, String> {
        #[cfg(feature = "native-translation-llama")]
        {
            if input.segments.is_empty() {
                return Ok(Vec::new());
            }
            let threads = std::thread::available_parallelism()
                .map(|count| count.get().min(8) as i32)
                .unwrap_or(4);
            let context_params = LlamaContextParams::default()
                .with_n_ctx(NonZeroU32::new(CONTEXT_TOKENS))
                .with_n_batch(CONTEXT_TOKENS)
                .with_n_threads(threads)
                .with_n_threads_batch(threads);
            let mut context = self
                .model
                .new_context(&self.backend, context_params)
                .map_err(|err| format!("Failed to create HY-MT2 context: {err}"))?;
            let target_language = language_name(&input.target_language)?;
            let mut outputs = Vec::with_capacity(input.segments.len());

            for segment in input.segments {
                context.clear_kv_cache();
                let prompt =
                    translation_prompt(target_language, &segment.text, &segment.context.glossary);
                let text = generate_translation(&self.model, &mut context, &prompt)?;
                outputs.push(TranslationSegmentOutput {
                    id: segment.id,
                    text,
                });
            }
            Ok(outputs)
        }

        #[cfg(not(feature = "native-translation-llama"))]
        {
            let _ = input;
            Err("HY-MT2 translation was not compiled with native-translation-llama.".into())
        }
    }
}

/// Generate one bounded completion using HY-MT2's published sampler settings.
#[cfg(feature = "native-translation-llama")]
fn generate_translation(
    model: &LlamaModel,
    context: &mut llama_cpp_2::context::LlamaContext<'_>,
    prompt: &str,
) -> Result<String, String> {
    let prompt_tokens = model
        .str_to_token(prompt, AddBos::Always)
        .map_err(|err| format!("Failed to tokenize HY-MT2 prompt: {err}"))?;
    let available_output = (context.n_ctx() as usize).saturating_sub(prompt_tokens.len());
    let output_limit = available_output.min(MAX_GENERATED_TOKENS);
    if prompt_tokens.is_empty() || output_limit == 0 {
        return Err(format!(
            "HY-MT2 prompt requires {} tokens but the context holds {}",
            prompt_tokens.len(),
            context.n_ctx()
        ));
    }

    let mut batch = LlamaBatch::new(prompt_tokens.len(), 1);
    let last_index = prompt_tokens.len() - 1;
    for (index, token) in prompt_tokens.iter().copied().enumerate() {
        batch
            .add(token, index as i32, &[0], index == last_index)
            .map_err(|err| format!("Failed to prepare HY-MT2 prompt: {err}"))?;
    }
    context
        .decode(&mut batch)
        .map_err(|err| format!("Failed to evaluate HY-MT2 prompt: {err}"))?;

    let mut sampler = LlamaSampler::chain_simple([
        LlamaSampler::penalties(-1, 1.05, 0.0, 0.0),
        LlamaSampler::top_k(20),
        LlamaSampler::top_p(0.6, 1),
        LlamaSampler::temp(0.7),
        LlamaSampler::dist(SAMPLING_SEED),
    ])
    .with_tokens(&prompt_tokens);
    let mut decoder = encoding_rs::UTF_8.new_decoder();
    let mut output = String::new();
    let mut position = batch.n_tokens();

    for _ in 0..output_limit {
        let token = sampler.sample(context, batch.n_tokens() - 1);
        sampler.accept(token);
        if model.is_eog_token(token) {
            break;
        }
        output.push_str(
            &model
                .token_to_piece(token, &mut decoder, false, None)
                .map_err(|err| format!("Failed to decode HY-MT2 output: {err}"))?,
        );
        batch.clear();
        batch
            .add(token, position, &[0], true)
            .map_err(|err| format!("Failed to continue HY-MT2 generation: {err}"))?;
        position += 1;
        context
            .decode(&mut batch)
            .map_err(|err| format!("Failed to generate HY-MT2 output: {err}"))?;
    }

    let output = output.trim().to_string();
    if output.is_empty() {
        Err("HY-MT2 returned an empty translation.".into())
    } else {
        Ok(output)
    }
}

/// Keep the no-glossary prompt identical to Tencent's published HY-MT2 form.
fn translation_prompt(
    target_language: &str,
    source_text: &str,
    glossary: &[TranslationGlossaryEntry],
) -> String {
    let glossary = glossary_prompt(glossary);
    format!(
        "Translate the following text into {target_language}. {glossary}Note that you should only output the translated result without any additional explanation:\n{source_text}"
    )
}

fn glossary_prompt(glossary: &[TranslationGlossaryEntry]) -> String {
    if glossary.is_empty() {
        return String::new();
    }
    let mappings = glossary
        .iter()
        .map(|entry| format!("{} = {}", entry.source.trim(), entry.target.trim()))
        .collect::<Vec<_>>()
        .join("; ");
    format!("Use these terminology mappings when applicable: {mappings}. ")
}

fn language_name(code: &str) -> Result<&'static str, String> {
    match code {
        "en" => Ok("English"),
        _ => Err(format!("HY-MT2 target language {code:?} is not supported")),
    }
}

#[cfg(test)]
mod tests {
    use super::{language_name, translation_prompt};
    use crate::translation::types::TranslationGlossaryEntry;

    #[test]
    fn default_prompt_matches_hy_mt2_instruction() {
        assert_eq!(
            translation_prompt("English", "Hola.", &[]),
            "Translate the following text into English. Note that you should only output the translated result without any additional explanation:\nHola."
        );
    }

    #[test]
    fn prompt_includes_only_supplied_terminology() {
        let glossary = vec![TranslationGlossaryEntry {
            source: "Estado".into(),
            target: "State".into(),
            note: None,
        }];

        let prompt = translation_prompt("English", "El Estado.", &glossary);

        assert!(prompt.contains("Estado = State"));
        assert!(prompt.ends_with("\nEl Estado."));
    }

    #[test]
    fn target_language_names_are_explicit() {
        assert_eq!(language_name("en"), Ok("English"));
        assert!(language_name("xx").is_err());
    }
}
