//! CTranslate2 engine adapter for the OPUS-MT/Marian MVP.
//!
//! The real native binding remains feature-gated so normal builds do not pull
//! in C++/SentencePiece dependencies. When `native-translation-ctranslate2` is
//! enabled, this adapter loads `ct2rs::Translator` from a verified on-disk
//! model directory and translates bounded batches for the document pipeline.

// Several config fields and helpers are only read when the native feature is
// compiled in; the blanket allow keeps the non-native build warning-free
// without scattering cfg_attr over every conditional item.
#![allow(dead_code)]

use std::path::PathBuf;

use unicode_segmentation::UnicodeSegmentation;

#[cfg(feature = "native-translation-ctranslate2")]
use super::engine::TranslationSegmentInput;
use super::engine::{TranslationBatchInput, TranslationEngine, TranslationSegmentOutput};

#[cfg(feature = "native-translation-ctranslate2")]
type NativeTranslator = ct2rs::Translator<ct2rs::tokenizers::auto::Tokenizer>;
#[cfg(feature = "native-translation-ctranslate2")]
type NativeTokenizer = ct2rs::tokenizers::auto::Tokenizer;

const MARIAN_POSITION_LIMIT_TOKENS: usize = 512;
const MARIAN_SAFE_SOURCE_TOKENS: usize = 448;

#[derive(Debug, Clone)]
pub(crate) struct CTranslate2EngineConfig {
    pub(crate) model_id: String,
    pub(crate) model_dir: PathBuf,
    pub(crate) device: CTranslate2Device,
    pub(crate) inter_threads: usize,
    pub(crate) intra_threads: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CTranslate2Device {
    Cpu,
}

pub(crate) struct CTranslate2Engine {
    config: CTranslate2EngineConfig,
    #[cfg(feature = "native-translation-ctranslate2")]
    translator: Option<NativeTranslator>,
    #[cfg(feature = "native-translation-ctranslate2")]
    tokenizer: Option<NativeTokenizer>,
}

impl CTranslate2Engine {
    /// Load the native runtime from a verified on-disk model folder.
    ///
    /// This is intentionally separate from `new` because loading CTranslate2
    /// can mmap model weights, initialize tokenizers, and fail on platform
    /// linkage. Callers should do this only after model-manifest validation.
    pub(crate) fn for_installed_model(
        model_id: impl Into<String>,
        model_dir: PathBuf,
    ) -> Result<Self, String> {
        let config = CTranslate2EngineConfig {
            model_id: model_id.into(),
            model_dir,
            device: CTranslate2Device::Cpu,
            inter_threads: 1,
            intra_threads: default_intra_threads(),
        };
        Self::load(config)
    }

    /// Create a CTranslate2 adapter without loading native code.
    ///
    /// The real implementation should validate converted model files here, then
    /// initialize the chosen binding/wrapper. Keeping construction explicit
    /// avoids hiding expensive model loads inside job planning code.
    pub(crate) fn new(config: CTranslate2EngineConfig) -> Self {
        Self {
            config,
            #[cfg(feature = "native-translation-ctranslate2")]
            translator: None,
            #[cfg(feature = "native-translation-ctranslate2")]
            tokenizer: None,
        }
    }

    #[cfg(test)]
    fn config(&self) -> &CTranslate2EngineConfig {
        &self.config
    }

    /// Initialize the native translator only when the feature is compiled in.
    ///
    /// Non-native builds still construct the adapter so shared planning/storage
    /// code compiles, but translation returns a clear feature-gate error.
    fn load(config: CTranslate2EngineConfig) -> Result<Self, String> {
        #[cfg(feature = "native-translation-ctranslate2")]
        {
            let translator_config = native_config(&config);
            let translator = ct2rs::Translator::new(&config.model_dir, &translator_config)
                .map_err(|err| {
                    format!(
                        "Failed to load CTranslate2 model {} at {}: {err}",
                        config.model_id,
                        config.model_dir.display()
                    )
                })?;
            let tokenizer =
                ct2rs::tokenizers::auto::Tokenizer::new(&config.model_dir).map_err(|err| {
                    format!(
                        "Failed to load CTranslate2 tokenizer {} at {}: {err}",
                        config.model_id,
                        config.model_dir.display()
                    )
                })?;
            return Ok(Self {
                config,
                translator: Some(translator),
                tokenizer: Some(tokenizer),
            });
        }

        #[cfg(not(feature = "native-translation-ctranslate2"))]
        {
            Ok(Self::new(config))
        }
    }
}

impl TranslationEngine for CTranslate2Engine {
    fn translate_batch(
        &mut self,
        input: TranslationBatchInput,
    ) -> Result<Vec<TranslationSegmentOutput>, String> {
        #[cfg(feature = "native-translation-ctranslate2")]
        {
            let translator = self.translator.as_ref().ok_or_else(|| {
                "CTranslate2 runtime was not loaded. Use for_installed_model before translation."
                    .to_string()
            })?;
            let tokenizer = self.tokenizer.as_ref().ok_or_else(|| {
                "CTranslate2 tokenizer was not loaded. Use for_installed_model before translation."
                    .to_string()
            })?;
            if input.segments.is_empty() {
                return Ok(Vec::new());
            }
            let expanded_sources = prepare_token_bounded_sources(
                tokenizer,
                &input.segments,
                source_token_budget(&self.config.model_id),
            )?;
            let sources = expanded_sources
                .iter()
                .map(|source| source.text.as_str())
                .collect::<Vec<_>>();
            let mut options: ct2rs::TranslationOptions<String, String> = Default::default();
            options.max_batch_size = sources.len().max(1);
            options.replace_unknowns = true;

            let translated = translator
                .translate_batch(&sources, &options, None)
                .map_err(|err| format!("CTranslate2 batch translation failed: {err}"))?;
            if translated.len() != expanded_sources.len() {
                return Err(format!(
                    "CTranslate2 returned {} outputs for {} token-bounded source pieces",
                    translated.len(),
                    expanded_sources.len()
                ));
            }

            let mut joined_outputs = vec![String::new(); input.segments.len()];
            for (source, (text, _score)) in expanded_sources.iter().zip(translated) {
                append_translated_part(&mut joined_outputs[source.owner_index], &text);
            }

            Ok(input
                .segments
                .into_iter()
                .enumerate()
                .map(|(index, segment)| TranslationSegmentOutput {
                    id: segment.id,
                    text: joined_outputs[index].clone(),
                })
                .collect())
        }

        #[cfg(not(feature = "native-translation-ctranslate2"))]
        {
            let _ = input;
            Err(
                "CTranslate2 translation is selected for the MVP, but this build was not compiled with native-translation-ctranslate2."
                    .into(),
            )
        }
    }
}

/// Convert Papercut's small runtime config into ct2rs options.
///
/// The MVP is CPU-only because that is the common desktop/Android baseline.
/// GPU/device selection should be added here later without touching callers.
#[cfg(feature = "native-translation-ctranslate2")]
fn native_config(config: &CTranslate2EngineConfig) -> ct2rs::Config {
    let mut native = ct2rs::Config {
        device: match config.device {
            CTranslate2Device::Cpu => ct2rs::Device::CPU,
        },
        num_threads_per_replica: config.intra_threads,
        max_queued_batches: config.inter_threads.max(1) as i32,
        ..Default::default()
    };
    native.device_indices = vec![0];
    native
}

/// Pick a conservative CPU thread count for long-running translation jobs.
///
/// Translation runs can sit beside UI, TTS, and downloads. Capping at 8 avoids
/// oversubscribing large desktops while still using enough parallelism to make
/// OPUS-MT practical.
fn default_intra_threads() -> usize {
    std::thread::available_parallelism()
        .map(|count| count.get().clamp(1, 8))
        .unwrap_or(1)
}

/// Pick source-token budget by model family.
///
/// OPUS-MT/Marian reports a hard 512-position ceiling from CTranslate2. The
/// 448 budget leaves space for tokenizer/model special tokens and avoids
/// running right against the edge on different language pairs.
fn source_token_budget(model_id: &str) -> usize {
    if model_id.contains("opus-mt") {
        MARIAN_SAFE_SOURCE_TOKENS
    } else {
        MARIAN_SAFE_SOURCE_TOKENS.min(MARIAN_POSITION_LIMIT_TOKENS - 1)
    }
}

#[cfg(feature = "native-translation-ctranslate2")]
struct ExpandedTranslationSource {
    owner_index: usize,
    text: String,
}

/// Split source text by the model tokenizer before CTranslate2 sees it.
///
/// OPUS-MT/Marian models have fixed positional embeddings, so a segment that is
/// safe by character count can still exceed the source-token window. We keep the
/// public job/cache segment ids stable by translating subsegments internally and
/// joining their outputs back into the original segment result.
#[cfg(feature = "native-translation-ctranslate2")]
fn prepare_token_bounded_sources(
    tokenizer: &NativeTokenizer,
    segments: &[TranslationSegmentInput],
    max_source_tokens: usize,
) -> Result<Vec<ExpandedTranslationSource>, String> {
    use ct2rs::Tokenizer as _;

    let mut expanded = Vec::new();
    for (owner_index, segment) in segments.iter().enumerate() {
        let parts = split_text_by_token_budget(&segment.text, max_source_tokens, |text| {
            tokenizer
                .encode(text)
                .map(|tokens| tokens.len())
                .map_err(|err| {
                    format!(
                        "CTranslate2 tokenizer failed while sizing segment {}: {err}",
                        segment.id
                    )
                })
        })?;
        for text in parts {
            expanded.push(ExpandedTranslationSource { owner_index, text });
        }
    }
    Ok(expanded)
}

/// Build model-sized source pieces using tokenizer counts, not character counts.
///
/// The planner already gives us stable document/cache segments. This lower
/// layer only protects the native model's context window, so it must not create
/// new public segment ids or progress units.
fn split_text_by_token_budget<F>(
    text: &str,
    max_tokens: usize,
    count_tokens: F,
) -> Result<Vec<String>, String>
where
    F: Fn(&str) -> Result<usize, String> + Copy,
{
    if max_tokens == 0 {
        return Err("Translation source token budget must be greater than zero".into());
    }
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    if count_tokens(trimmed)? <= max_tokens {
        return Ok(vec![trimmed.to_string()]);
    }

    let mut parts = Vec::new();
    let mut current = String::new();
    for sentence in sentence_like_parts(trimmed) {
        if count_tokens(sentence)? > max_tokens {
            push_token_part(&mut parts, &mut current);
            parts.extend(split_oversized_token_part(
                sentence,
                max_tokens,
                count_tokens,
            )?);
            continue;
        }

        let proposed = join_parts(&current, sentence);
        if !current.is_empty() && count_tokens(&proposed)? > max_tokens {
            push_token_part(&mut parts, &mut current);
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(sentence);
    }
    push_token_part(&mut parts, &mut current);

    Ok(parts)
}

/// Split a single sentence-like part that is already too large for the model.
///
/// We prefer word boundaries because OPUS-MT quality drops sharply if common
/// prose is chopped mid-word. Character splitting exists only for pathological
/// long tokens such as pasted URLs or malformed markup text.
fn split_oversized_token_part<F>(
    text: &str,
    max_tokens: usize,
    count_tokens: F,
) -> Result<Vec<String>, String>
where
    F: Fn(&str) -> Result<usize, String> + Copy,
{
    let mut parts = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if count_tokens(word)? > max_tokens {
            push_token_part(&mut parts, &mut current);
            parts.extend(split_oversized_token_word(word, max_tokens, count_tokens)?);
            continue;
        }

        let proposed = join_parts(&current, word);
        if !current.is_empty() && count_tokens(&proposed)? > max_tokens {
            push_token_part(&mut parts, &mut current);
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    push_token_part(&mut parts, &mut current);

    Ok(parts)
}

/// Last-resort split for one token-like run with no usable whitespace.
///
/// A single character can theoretically still exceed a tiny fake test budget,
/// so this function always makes forward progress instead of looping forever.
fn split_oversized_token_word<F>(
    word: &str,
    max_tokens: usize,
    count_tokens: F,
) -> Result<Vec<String>, String>
where
    F: Fn(&str) -> Result<usize, String> + Copy,
{
    let mut parts = Vec::new();
    let mut current = String::new();
    for ch in word.chars() {
        let mut proposed = current.clone();
        proposed.push(ch);
        if !current.is_empty() && count_tokens(&proposed)? > max_tokens {
            parts.push(std::mem::take(&mut current));
        }
        current.push(ch);
    }
    if !current.is_empty() {
        parts.push(current);
    }
    Ok(parts)
}

/// Split into sentence-sized parts with the same Unicode rules as the planner.
///
/// The previous hand-rolled terminal-character list split inside abbreviations
/// ("U.S." became two parts) and disagreed with `segment.rs`, which already
/// uses UAX #29 sentence boundaries. One boundary definition across both
/// layers keeps token-budget subdivision aligned with planned segments.
fn sentence_like_parts(text: &str) -> Vec<&str> {
    text.split_sentence_bounds()
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect()
}

fn join_parts(left: &str, right: &str) -> String {
    if left.is_empty() {
        right.to_string()
    } else {
        format!("{left} {right}")
    }
}

fn push_token_part(parts: &mut Vec<String>, current: &mut String) {
    if current.is_empty() {
        return;
    }
    parts.push(std::mem::take(current));
}

fn append_translated_part(target: &mut String, part: &str) {
    let trimmed = part.trim();
    if trimmed.is_empty() {
        return;
    }
    if !target.is_empty() {
        target.push(' ');
    }
    target.push_str(trimmed);
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{
        append_translated_part, split_text_by_token_budget, CTranslate2Device, CTranslate2Engine,
        CTranslate2EngineConfig, MARIAN_SAFE_SOURCE_TOKENS,
    };

    #[test]
    fn stores_engine_config_without_loading_native_runtime() {
        let engine = CTranslate2Engine::new(CTranslate2EngineConfig {
            model_id: "opus-mt-es-en-ctranslate2".into(),
            model_dir: PathBuf::from("/tmp/model"),
            device: CTranslate2Device::Cpu,
            inter_threads: 1,
            intra_threads: 4,
        });

        assert_eq!(engine.config().model_id, "opus-mt-es-en-ctranslate2");
        assert_eq!(engine.config().device, CTranslate2Device::Cpu);
    }

    #[cfg(not(feature = "native-translation-ctranslate2"))]
    #[test]
    fn prepares_engine_from_installed_model_dir_without_native_feature() {
        let engine = CTranslate2Engine::for_installed_model(
            "opus-mt-fr-en-ctranslate2",
            PathBuf::from("/tmp/fr-en"),
        )
        .expect("engine");

        assert_eq!(engine.config().model_id, "opus-mt-fr-en-ctranslate2");
        assert_eq!(engine.config().model_dir, PathBuf::from("/tmp/fr-en"));
        assert!(engine.config().intra_threads >= 1);
    }

    #[test]
    fn splits_text_by_token_budget_without_changing_public_segment_ids() {
        let text = "Uno dos tres cuatro. Cinco seis siete ocho. Nueve diez once doce.";
        let parts = split_text_by_token_budget(text, 4, whitespace_token_count).expect("parts");

        assert_eq!(
            parts,
            vec![
                "Uno dos tres cuatro.",
                "Cinco seis siete ocho.",
                "Nueve diez once doce."
            ]
        );
    }

    #[test]
    fn splits_unpunctuated_text_by_token_budget() {
        let text = "uno dos tres cuatro cinco seis siete ocho";
        let parts = split_text_by_token_budget(text, 3, whitespace_token_count).expect("parts");

        assert_eq!(
            parts,
            vec!["uno dos tres", "cuatro cinco seis", "siete ocho"]
        );
    }

    #[test]
    fn sentence_parts_keep_lowercase_abbreviations_together() {
        // The old terminal-character splitter broke after every period,
        // separating abbreviations from their sentence before token budgeting.
        let parts = super::sentence_like_parts("El caso p. ej. sigue abierto. Fin del texto.");

        assert_eq!(
            parts,
            vec!["El caso p. ej. sigue abierto.", "Fin del texto."]
        );
    }

    #[test]
    fn joins_translated_subsegments_for_original_output() {
        let mut output = String::new();

        append_translated_part(&mut output, "The first sentence.");
        append_translated_part(&mut output, "The second sentence.");

        assert_eq!(output, "The first sentence. The second sentence.");
    }

    #[test]
    fn marian_source_budget_leaves_room_below_position_limit() {
        assert!(MARIAN_SAFE_SOURCE_TOKENS < 512);
    }

    fn whitespace_token_count(text: &str) -> Result<usize, String> {
        Ok(text.split_whitespace().count().max(1))
    }
}
