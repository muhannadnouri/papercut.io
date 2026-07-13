//! Generic sherpa-onnx engine loading and single-chunk synthesis.
//!
//! Owns the loaded [`LoadedTtsEngine`] (rebuilt when the requested thread
//! count changes), the text sanitization applied before native tokenization,
//! and the saved-audiobook synthesis sink: [`synthesize_to_file`], which writes
//! validated WAV chunks atomically into the audiobook cache.
//!
//! Rust notes for a JS reader: `spawn_blocking` runs CPU-heavy work on a
//! background thread pool (like a Web Worker) so the async runtime that handles
//! UI messages isn't blocked. A `Mutex` is a lock guaranteeing one thread
//! touches the engine at a time; `.lock()` is like awaiting that lock.

use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use sherpa_onnx::{
    GeneratedAudio, GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsKokoroModelConfig,
    OfflineTtsModelConfig, OfflineTtsSupertonicModelConfig, OfflineTtsVitsModelConfig,
};

use super::models::{model_definition, ModelDefinition, SherpaModelFamily, TtsModelBackend};
use super::paths::{audio_duration_sec, resolve_model_dir};
use super::silma_sidecar::SilmaSidecar;
use super::text_normalization::{normalize_english_synthesis_text, sanitize_tts_text};
use super::wav_sink::{
    commit_synthesis_wav, write_silent_placeholder, FileSynthesisResult, SILENT_PLACEHOLDER_SEC,
};
use crate::native_tts::platform::resolve_thread_count;

/// A loaded sherpa-onnx model plus the settings it was built with. Kept alive
/// in shared state and reused across syntheses; rebuilt when the requested
/// model or thread count differs (see [`ensure_sherpa_engine`]).
pub(crate) struct SherpaTtsEngine {
    pub(super) tts: OfflineTts,
    pub(super) model: &'static ModelDefinition,
    pub(super) model_dir: std::path::PathBuf,
    pub(super) num_threads: i32,
}

/// Loaded SILMA worker plus the model directory it was initialized with.
#[allow(dead_code)]
pub(crate) struct SilmaTtsEngine {
    pub(super) sidecar: SilmaSidecar,
    pub(super) model: &'static ModelDefinition,
    pub(super) model_dir: std::path::PathBuf,
    pub(super) sample_rate: i32,
}

/// Single runtime engine slot. Only sherpa synthesis is implemented today; the
/// SILMA variant proves load/retention before chunk generation is wired.
pub(crate) enum LoadedTtsEngine {
    Sherpa(SherpaTtsEngine),
    Silma(SilmaTtsEngine),
}

impl LoadedTtsEngine {
    /// Return the loaded sherpa engine only when it matches the requested model
    /// and thread count; otherwise the caller must rebuild the slot.
    fn matching_sherpa(&self, model_id: &str, threads: i32) -> Option<&SherpaTtsEngine> {
        match self {
            LoadedTtsEngine::Sherpa(engine)
                if engine.model.id == model_id && engine.num_threads == threads =>
            {
                Some(engine)
            }
            _ => None,
        }
    }

    /// Return the loaded SILMA worker only when it matches the requested model
    /// and model directory; otherwise the caller must start/load a fresh worker.
    #[allow(dead_code)]
    fn matching_silma(&self, model_id: &str, model_dir: &Path) -> Option<&SilmaTtsEngine> {
        match self {
            LoadedTtsEngine::Silma(engine)
                if engine.model.id == model_id && engine.model_dir.as_path() == model_dir =>
            {
                Some(engine)
            }
            _ => None,
        }
    }
}

/// Return a ready engine from the shared slot, (re)building it if absent or if
/// the thread count changed. `guard` is the locked `Option<engine>`; the
/// returned `&SherpaTtsEngine` borrows from it for the rest of the call.
pub(super) fn ensure_sherpa_engine<'a>(
    app: &tauri::AppHandle,
    guard: &'a mut Option<LoadedTtsEngine>,
    model_id: &str,
    thread_count: Option<i32>,
) -> Result<&'a SherpaTtsEngine, String> {
    let model = model_definition(model_id)?;
    if !matches!(model.backend, TtsModelBackend::SherpaOnnx) {
        return Err(format!(
            "{} uses the SILMA sidecar backend; audiobook synthesis is not wired yet",
            model.display_name
        ));
    }
    let requested_threads = resolve_thread_count(thread_count);
    // Rebuild only when there's no engine yet, or the desired thread count
    // differs from the loaded one (changing threads needs a fresh engine).
    let should_create = guard
        .as_ref()
        .and_then(|engine| engine.matching_sherpa(model.id, requested_threads))
        .is_none();

    if should_create {
        let model_dir = resolve_model_dir(app, model)?;
        *guard = Some(LoadedTtsEngine::Sherpa(SherpaTtsEngine {
            tts: create_engine(model, &model_dir, requested_threads)?,
            model,
            model_dir,
            num_threads: requested_threads,
        }));
    }

    guard
        .as_ref()
        .and_then(|engine| engine.matching_sherpa(model.id, requested_threads))
        .ok_or_else(|| "Native sherpa TTS engine unavailable".to_string())
}

/// Start/load a SILMA worker into the shared engine slot.
#[allow(dead_code)]
pub(super) fn ensure_silma_engine<'a>(
    app: &tauri::AppHandle,
    guard: &'a mut Option<LoadedTtsEngine>,
    model_id: &str,
) -> Result<&'a SilmaTtsEngine, String> {
    let model = model_definition(model_id)?;
    if !matches!(model.backend, TtsModelBackend::SilmaSidecar) {
        return Err(format!(
            "{} is not a SILMA sidecar model",
            model.display_name
        ));
    }
    let model_dir = resolve_model_dir(app, model)?;
    let should_create = guard
        .as_ref()
        .and_then(|engine| engine.matching_silma(model.id, &model_dir))
        .is_none();

    if should_create {
        let mut sidecar = SilmaSidecar::start_dev()?;
        let sample_rate = sidecar.load_model(&model_dir)?;
        *guard = Some(LoadedTtsEngine::Silma(SilmaTtsEngine {
            sidecar,
            model,
            model_dir: model_dir.clone(),
            sample_rate,
        }));
    }

    guard
        .as_ref()
        .and_then(|engine| engine.matching_silma(model.id, &model_dir))
        .ok_or_else(|| "Native SILMA sidecar engine unavailable".to_string())
}

/// Synthesize `text` straight to `output_path` (for saving). Writes to a temp
/// file, validates it parses as WAV, then atomically renames into place so a
/// crash mid-write never leaves a corrupt chunk in the cache.
pub(super) fn synthesize_to_file(
    engine: &SherpaTtsEngine,
    text: &str,
    voice: &str,
    speed: f32,
    output_path: &Path,
) -> Result<FileSynthesisResult, String> {
    let started = Instant::now();
    // A chunk whose text is empty after sanitization (e.g. a paragraph of only
    // emoji or other dropped symbols) has nothing to synthesize. Failing here
    // would abort the whole save and wedge resume on the same chunk forever, so
    // instead write a short silent WAV: the playback timeline stays contiguous
    // and the job completes. `fallback_duration_sec` guards against a generated
    // WAV whose header rounds its duration down to zero.
    let synthesis_started = Instant::now();
    let generated_audio = generate_audio(engine, text, voice, speed)?;
    let synthesis_ms = synthesis_started.elapsed().as_millis();

    commit_synthesis_wav(
        started,
        synthesis_ms,
        output_path,
        |temp_path| match generated_audio {
            Some(audio) => {
                let duration = audio_duration_sec(audio.samples().len(), audio.sample_rate());
                if !audio.save(&temp_path.display().to_string()) {
                    return Err(format!(
                        "sherpa-onnx failed to write generated WAV {}",
                        temp_path.display()
                    ));
                }
                Ok(duration)
            }
            None => {
                let sample_rate = engine.tts.sample_rate();
                log::warn!(
                    "Audiobook chunk had no speakable text after sanitization; writing {SILENT_PLACEHOLDER_SEC}s silent placeholder at {} ({sample_rate} Hz)",
                    output_path.display(),
                );
                write_silent_placeholder(temp_path, sample_rate)
            }
        },
    )
}

/// Synthesize one chunk through the SILMA sidecar and commit it through the
/// same validated WAV path sherpa uses.
pub(super) fn synthesize_silma_to_file(
    engine: &mut SilmaTtsEngine,
    text: &str,
    voice: &str,
    speed: f32,
    output_path: &Path,
) -> Result<FileSynthesisResult, String> {
    engine.model.speaker_id(voice)?;
    let started = Instant::now();
    if text.trim().is_empty() {
        return commit_synthesis_wav(started, 0, output_path, |temp_path| {
            write_silent_placeholder(temp_path, engine.sample_rate)
        });
    }

    let mut sidecar_synthesis_ms = 0;
    let result = commit_synthesis_wav(started, 0, output_path, |temp_path| {
        let sidecar_result = engine.sidecar.synthesize_to_wav(text, temp_path, speed)?;
        sidecar_synthesis_ms = sidecar_result.synthesis_ms;
        Ok(sidecar_result.audio_duration_sec)
    })?;
    Ok(FileSynthesisResult {
        synthesis_ms: sidecar_synthesis_ms,
        ..result
    })
}

/// Run sherpa-onnx inference for one piece of text. Normalizes speed, maps the
/// voice name to a speaker id, sanitizes the text, and asks the engine to
/// generate. Returns `Ok(None)` when the text is empty after sanitization (the
/// chunk has nothing speakable, so the caller writes a silent placeholder rather
/// than failing the save); errors only when generation itself fails.
fn generate_audio(
    engine: &SherpaTtsEngine,
    text: &str,
    voice: &str,
    speed: f32,
) -> Result<Option<GeneratedAudio>, String> {
    let speed = if speed.is_finite() && speed > 0.0 {
        speed
    } else {
        1.0
    };
    let extra = engine.model.supertonic_lang.map(|lang| {
        let mut extra = HashMap::new();
        extra.insert("lang".to_string(), serde_json::json!(lang));
        extra
    });
    let generation = GenerationConfig {
        speed,
        sid: engine.model.speaker_id(voice)?,
        extra,
        ..Default::default()
    };

    let mut sanitized = sanitize_tts_text(text);
    // Year expansion, roman-numeral expansion, and semicolon/decimal cleanup are
    // English-only; gate them so non-English models (e.g. Arabic Piper) never get
    // Western number words or English punctuation rewrites in their synthesis text.
    if engine.model.english_text_normalization() {
        sanitized = normalize_english_synthesis_text(&sanitized);
    }
    if sanitized.trim().is_empty() {
        return Ok(None);
    }

    // The last arg is an optional progress callback; `None::<fn...>` means none.
    engine
        .tts
        .generate_with_config(&sanitized, &generation, None::<fn(&[f32], f32) -> bool>)
        .map(Some)
        .ok_or_else(|| "sherpa-onnx failed to synthesize audio".to_string())
}

/// Construct one sherpa-onnx engine from catalog metadata.
fn create_engine(
    model: &ModelDefinition,
    model_dir: &Path,
    thread_count: i32,
) -> Result<OfflineTts, String> {
    let mut model_config = OfflineTtsModelConfig {
        num_threads: thread_count,
        provider: Some("cpu".into()),
        ..Default::default()
    };

    match model.require_sherpa_family()? {
        SherpaModelFamily::Kokoro => {
            let lexicon = [
                model_dir.join("lexicon-us-en.txt"),
                model_dir.join("lexicon-zh.txt"),
            ]
            .iter()
            .filter(|path| path.is_file())
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(",");
            model_config.kokoro = OfflineTtsKokoroModelConfig {
                model: Some(model_dir.join(model.model_file).display().to_string()),
                voices: Some(model_dir.join("voices.bin").display().to_string()),
                tokens: Some(model_dir.join("tokens.txt").display().to_string()),
                data_dir: Some(model_dir.join("espeak-ng-data").display().to_string()),
                lexicon: (!lexicon.is_empty()).then_some(lexicon),
                lang: Some("en-us".into()),
                ..Default::default()
            };
        }
        SherpaModelFamily::Supertonic => {
            model_config.supertonic = OfflineTtsSupertonicModelConfig {
                duration_predictor: Some(
                    model_dir
                        .join("duration_predictor.int8.onnx")
                        .display()
                        .to_string(),
                ),
                text_encoder: Some(
                    model_dir
                        .join("text_encoder.int8.onnx")
                        .display()
                        .to_string(),
                ),
                vector_estimator: Some(
                    model_dir
                        .join("vector_estimator.int8.onnx")
                        .display()
                        .to_string(),
                ),
                vocoder: Some(model_dir.join("vocoder.int8.onnx").display().to_string()),
                tts_json: Some(model_dir.join("tts.json").display().to_string()),
                unicode_indexer: Some(model_dir.join("unicode_indexer.bin").display().to_string()),
                voice_style: Some(model_dir.join("voice.bin").display().to_string()),
            };
        }
        SherpaModelFamily::Vits => {
            model_config.vits = OfflineTtsVitsModelConfig {
                model: Some(model_dir.join(model.model_file).display().to_string()),
                tokens: Some(model_dir.join("tokens.txt").display().to_string()),
                data_dir: Some(model_dir.join("espeak-ng-data").display().to_string()),
                ..Default::default()
            };
        }
    }

    let config = OfflineTtsConfig {
        model: model_config,
        max_num_sentences: 1,
        ..Default::default()
    };
    OfflineTts::create(&config)
        .ok_or_else(|| format!("Failed to create {} engine", model.display_name))
}

#[cfg(test)]
mod tests {
    use super::super::models::{model_definition, DEFAULT_MODEL_ID};
    use super::*;

    #[test]
    fn only_english_models_normalize_text() {
        assert!(model_definition(DEFAULT_MODEL_ID)
            .unwrap()
            .english_text_normalization());
        assert!(
            !model_definition("sherpa-onnx/vits-piper-ar_JO-kareem-medium")
                .unwrap()
                .english_text_normalization()
        );
    }
}
