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
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use base64::{engine::general_purpose, Engine as _};
use sherpa_onnx::{
    GeneratedAudio, GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsKokoroModelConfig,
    OfflineTtsModelConfig, OfflineTtsSupertonicModelConfig, OfflineTtsVitsModelConfig,
};
use tauri::Manager;

use super::models::{model_definition, ModelDefinition, SherpaModelFamily, TtsModelBackend};
use super::paths::{audio_duration_sec, resolve_model_dir};
use super::phoneme_adapter::PhonemeAdapter;
use super::phonemizer::phonemize;
use super::preprocess::TextPreprocessor;
use super::silma_sidecar::{normalize_silma_nfe_step, SilmaSidecar, DEFAULT_SILMA_NFE_STEP};
use super::text_normalization::{normalize_english_synthesis_text, sanitize_tts_text};
use super::wav_sink::{
    commit_synthesis_wav, write_silent_placeholder, FileSynthesisResult, SILENT_PLACEHOLDER_SEC,
};
use crate::native_tts::platform::resolve_thread_count;
use crate::native_tts::state::NativeTtsState;
use crate::native_tts::types::{NativeTtsChunkResponse, NativeTtsPreviewRequest};

const MAX_PREVIEW_TEXT_CHARS: usize = 500;

/// A loaded sherpa-onnx model plus the settings it was built with. Kept alive
/// in shared state and reused across syntheses; rebuilt when the requested
/// model or thread count differs (see [`ensure_sherpa_engine`]).
pub(crate) struct SherpaTtsEngine {
    pub(super) tts: OfflineTts,
    pub(super) model: &'static ModelDefinition,
    pub(super) model_dir: std::path::PathBuf,
    pub(super) num_threads: i32,
    phoneme_adapter: Option<PhonemeAdapter>,
}

/// Loaded SILMA worker plus the runtime settings that decide whether it can be
/// reused for the next audiobook job.
pub(crate) struct SilmaTtsEngine {
    pub(super) sidecar: SilmaSidecar,
    pub(super) model: &'static ModelDefinition,
    pub(super) model_dir: std::path::PathBuf,
    pub(super) sample_rate: i32,
    pub(super) device: String,
    pub(super) torch_threads: i32,
    pub(super) torch_interop_threads: i32,
}

/// Single runtime engine slot shared by all native audiobook jobs.
pub(crate) enum LoadedTtsEngine {
    Sherpa(SherpaTtsEngine),
    Silma(SilmaTtsEngine),
}

/// Generate one short preview in the cache directory, return it to the WebView,
/// and remove it immediately. It shares the normal engine and preprocessing path.
pub(crate) async fn preview_voice(
    app: tauri::AppHandle,
    state: tauri::State<'_, NativeTtsState>,
    request: NativeTtsPreviewRequest,
) -> Result<NativeTtsChunkResponse, String> {
    let engine = state.engine.clone();
    tauri::async_runtime::spawn_blocking(move || preview_voice_blocking(app, engine, request))
        .await
        .map_err(|err| format!("Voice preview task failed: {err}"))?
}

fn preview_voice_blocking(
    app: tauri::AppHandle,
    engine_state: Arc<Mutex<Option<LoadedTtsEngine>>>,
    request: NativeTtsPreviewRequest,
) -> Result<NativeTtsChunkResponse, String> {
    validate_preview_text(&request.text)?;
    let model = model_definition(&request.model_id)?;
    let preprocessor = TextPreprocessor::create(model, &request.text_preprocessor)?;
    let text = preprocessor.process(&request.text)?;
    let thread_count = resolve_thread_count(request.thread_count);
    let preview_dir = app
        .path()
        .app_cache_dir()
        .or_else(|_| app.path().app_data_dir())
        .map_err(|err| format!("Failed to resolve voice preview cache dir: {err}"))?
        .join("voice-previews");
    let output_path = preview_dir.join(format!(
        "preview-{}-{}.wav",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|err| format!("System clock error: {err}"))?
            .as_nanos()
    ));

    let mut guard = engine_state
        .lock()
        .map_err(|_| "Native TTS engine lock poisoned".to_string())?;
    let silma_nfe_step =
        normalize_silma_nfe_step(request.silma_nfe_step.unwrap_or(DEFAULT_SILMA_NFE_STEP));
    let (result, sample_rate, backend) = match model.backend {
        TtsModelBackend::SherpaOnnx => {
            let engine =
                ensure_sherpa_engine(&app, &mut guard, &request.model_id, Some(thread_count))?;
            let result =
                synthesize_to_file(engine, &text, &request.voice, request.speed, &output_path)?;
            (
                result,
                engine.tts.sample_rate(),
                format!(
                    "{}:{}:preview",
                    engine.model.backend_name(),
                    engine.model.id
                ),
            )
        }
        TtsModelBackend::SilmaSidecar => {
            ensure_silma_engine(&app, &mut guard, &request.model_id, Some(thread_count))?;
            let Some(LoadedTtsEngine::Silma(engine)) = guard.as_mut() else {
                return Err("Native SILMA sidecar engine unavailable".into());
            };
            let result = synthesize_silma_to_file(
                engine,
                &text,
                &request.voice,
                request.speed,
                silma_nfe_step,
                &output_path,
            )?;
            (
                result,
                engine.sample_rate,
                format!(
                    "{}:{}:preview",
                    engine.model.backend_name(),
                    engine.model.id
                ),
            )
        }
    };
    drop(guard);

    let wav = fs::read(&output_path);
    let _ = fs::remove_file(&output_path);
    let wav = wav.map_err(|err| format!("Failed to read generated voice preview: {err}"))?;
    Ok(NativeTtsChunkResponse {
        chunk_id: None,
        wav_base64: general_purpose::STANDARD.encode(wav),
        sample_rate,
        audio_duration_sec: result.audio_duration_sec,
        wav_bytes: result.wav_bytes,
        generate_ms: result.generate_ms,
        backend,
    })
}

fn validate_preview_text(text: &str) -> Result<(), String> {
    let text_chars = text.chars().count();
    if text.trim().is_empty() || text_chars > MAX_PREVIEW_TEXT_CHARS {
        return Err(format!(
            "Voice preview text must contain 1 to {MAX_PREVIEW_TEXT_CHARS} characters"
        ));
    }
    Ok(())
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
    fn matching_silma(
        &self,
        model_id: &str,
        model_dir: &Path,
        torch_threads: i32,
    ) -> Option<&SilmaTtsEngine> {
        match self {
            LoadedTtsEngine::Silma(engine)
                if engine.model.id == model_id
                    && engine.model_dir.as_path() == model_dir
                    && engine.torch_threads == torch_threads =>
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
            phoneme_adapter: PhonemeAdapter::load(model.id, &model_dir)?,
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
pub(super) fn ensure_silma_engine<'a>(
    app: &tauri::AppHandle,
    guard: &'a mut Option<LoadedTtsEngine>,
    model_id: &str,
    thread_count: Option<i32>,
) -> Result<&'a SilmaTtsEngine, String> {
    let model = model_definition(model_id)?;
    if !matches!(model.backend, TtsModelBackend::SilmaSidecar) {
        return Err(format!(
            "{} is not a SILMA sidecar model",
            model.display_name
        ));
    }
    let model_dir = resolve_model_dir(app, model)?;
    let requested_threads = resolve_thread_count(thread_count);
    let should_create = guard
        .as_ref()
        .and_then(|engine| engine.matching_silma(model.id, &model_dir, requested_threads))
        .is_none();

    if should_create {
        let mut sidecar = SilmaSidecar::start(app)?;
        let load = sidecar.load_model(&model_dir, requested_threads)?;
        *guard = Some(LoadedTtsEngine::Silma(SilmaTtsEngine {
            sidecar,
            model,
            model_dir: model_dir.clone(),
            sample_rate: load.sample_rate,
            device: load.device,
            torch_threads: load.torch_threads,
            torch_interop_threads: load.torch_interop_threads,
        }));
    }

    guard
        .as_ref()
        .and_then(|engine| engine.matching_silma(model.id, &model_dir, requested_threads))
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
    nfe_step: i32,
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
        let sidecar_result = engine.sidecar.synthesize_to_wav(
            text,
            temp_path,
            speed,
            normalize_silma_nfe_step(nfe_step),
        )?;
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
/// than failing the save). The 7M frontend can also return no phones for a decorative
/// separator; preserve its chunk/timeline with the same placeholder.
/// Word/unsupported-phone failures remain errors,
/// so incomplete speech cannot become a successful saved chunk.
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
    let sid = engine.model.speaker_id(voice)?;
    let extra = generation_language(engine.model, voice).map(|lang| {
        let mut extra = HashMap::new();
        extra.insert("lang".to_string(), serde_json::json!(lang));
        extra
    });
    let mut generation = GenerationConfig {
        speed,
        sid,
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

    // These distills need different training-time phones; changing other models would change reviewed pronunciation.
    if let Some(frontend) = &engine.phoneme_adapter {
        let sentences = frontend.phonemize(&sanitized)?;
        if sentences.is_empty() {
            return Ok(None);
        }
        generation.phoneme_codepoints = Some(sentences);
    } else {
        // Mandarin keeps its lexicon/FST frontend; Supertonic keeps its own text tokenizer.
        let locale = match engine.model.require_sherpa_family()? {
            SherpaModelFamily::Kokoro => generation_language(engine.model, voice),
            SherpaModelFamily::Vits => Some("ar"),
            SherpaModelFamily::Supertonic => None,
        };
        if let Some(locale) = locale {
            let sentences =
                phonemize(&engine.model_dir.join("espeak-ng-data"), &sanitized, locale)?;
            if sentences.is_empty() {
                return Err("Native phonemizer returned no speech for nonempty text".into());
            }
            generation.phoneme_codepoints = Some(sentences);
        }
    }

    // The last arg is an optional progress callback; `None::<fn...>` means none.
    engine
        .tts
        .generate_with_config(&sanitized, &generation, None::<fn(&[f32], f32) -> bool>)
        .map(Some)
        .ok_or_else(|| "sherpa-onnx failed to synthesize audio".to_string())
}

/// Select the phonemizer locale per voice without changing model or voice identity.
///
/// Small models use their explicit language; their shared af_msa name is not a locale.
/// Existing Kokoro voice prefixes are its stable locale convention; Supertonic instead
/// selects one language for the whole catalog entry.
fn generation_language(model: &ModelDefinition, voice: &str) -> Option<&'static str> {
    if let Some(locale) = PhonemeAdapter::locale(model.id) {
        return Some(locale);
    }
    match model.require_sherpa_family().ok()? {
        SherpaModelFamily::Kokoro if voice.starts_with('a') => Some("en-us"),
        // The bundled eSpeak data accepts `en` for British phonemization, not `en-gb`.
        SherpaModelFamily::Kokoro if voice.starts_with('b') => Some("en"),
        SherpaModelFamily::Kokoro if voice.starts_with('e') => Some("es"),
        SherpaModelFamily::Kokoro if voice.starts_with('f') => Some("fr"),
        SherpaModelFamily::Kokoro if voice.starts_with('h') => Some("hi"),
        SherpaModelFamily::Kokoro if voice.starts_with('i') => Some("it"),
        SherpaModelFamily::Kokoro if voice.starts_with('p') => Some("pt-br"),
        // Mandarin uses the loaded Chinese/English lexicons; `zh` is not an eSpeak voice.
        SherpaModelFamily::Kokoro if voice.starts_with('z') => None,
        SherpaModelFamily::Supertonic => model.supertonic_lang,
        _ => None,
    }
}

/// Construct one sherpa-onnx engine from catalog metadata.
fn create_engine(
    model: &ModelDefinition,
    model_dir: &Path,
    thread_count: i32,
) -> Result<OfflineTts, String> {
    // Reject stale packaged libraries before passing the new ABI structures across FFI.
    if sherpa_onnx::version() != "2.0.1" {
        return Err(format!(
            "Incorrect native Sherpa version {}; expected 2.0.1; rebuild/reinstall the app",
            sherpa_onnx::version()
        ));
    }
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
                lexicon: (!lexicon.is_empty()).then_some(lexicon),
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
                ..Default::default()
            };
        }
    }

    // sherpa's Mandarin Kokoro path uses these transducers for phone numbers,
    // dates, and general numeric text. English keeps its historical no-FST path.
    let rule_fsts = model.language.starts_with("zh").then(|| {
        ["phone-zh.fst", "date-zh.fst", "number-zh.fst"]
            .map(|name| model_dir.join(name).display().to_string())
            .join(",")
    });
    let config = OfflineTtsConfig {
        model: model_config,
        rule_fsts,
        max_num_sentences: 1,
        ..Default::default()
    };
    OfflineTts::create(&config)
        .ok_or_else(|| format!("Failed to create {} engine", model.display_name))
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::path::PathBuf;

    use super::super::manifest::NativeAudiobookManifest;
    use super::super::models::{model_definition, DEFAULT_MODEL_ID, KOKORO_ZH_MODEL_ID};
    use super::*;

    #[derive(serde::Deserialize)]
    struct FrontendCase {
        name: String,
        model: String,
        speaker: i32,
        text: String,
        // Frozen reviewed phones supplement audio validity for training-specific adapters.
        #[serde(default)]
        phones: Option<String>,
    }

    /// Corpus metadata must resolve explicitly; reserved candidates may not hide typos or foreign speaker IDs.
    #[test]
    fn frontend_fixture_contracts_are_explicit() {
        let cases: Vec<FrontendCase> =
            serde_json::from_str(include_str!("../../../tts/frontend-smoke.json")).unwrap();
        let mut distills = 0;
        for case in cases {
            let model = model_definition(&case.model).unwrap();
            assert!(model
                .voices
                .iter()
                .any(|voice| voice.speaker_id == case.speaker));
            if PhonemeAdapter::locale(model.id).is_some() {
                assert_eq!(case.speaker, 0);
                assert!(!case.phones.unwrap().is_empty());
                distills += 1;
            } else {
                assert!(case.phones.is_none());
            }
        }
        assert_eq!(distills, 22);
    }

    /// Read cached assets explicitly; never download models during ordinary tests.
    #[cfg(feature = "native-text-preprocessing-core")]
    #[test]
    #[ignore = "requires PAPERCUT_ARABIC_TTS_MODEL_DIR pointing to cached Arabic VITS assets"]
    fn cached_arabic_preprocessing_smoke() {
        let model = model_definition("sherpa-onnx/vits-piper-ar_JO-kareem-medium").unwrap();
        let model_dir = PathBuf::from(
            env::var_os("PAPERCUT_ARABIC_TTS_MODEL_DIR")
                .expect("PAPERCUT_ARABIC_TTS_MODEL_DIR is required"),
        );
        assert!(model.has_required_files(&model_dir));
        let preprocessor =
            TextPreprocessor::create(model, super::super::models::TEXT_PREPROCESSOR_LIBTASHKEEL)
                .unwrap();
        let text = preprocessor.process("كتب الطالب الدرس").unwrap();
        assert_eq!(text, "كَتَبَ الطَّالِبُ الدَّرْس");
        let engine = SherpaTtsEngine {
            phoneme_adapter: PhonemeAdapter::load(model.id, &model_dir).unwrap(),
            tts: create_engine(model, &model_dir, 2).unwrap(),
            model,
            model_dir,
            num_threads: 2,
        };
        let audio = generate_audio(&engine, &text, model.default_voice, 1.0)
            .unwrap()
            .unwrap();
        assert!(audio.sample_rate() > 0);
        assert!(audio.samples().len() > audio.sample_rate() as usize / 4);
        assert!(audio.samples().iter().all(|sample| sample.is_finite()));
        assert!(audio.samples().iter().any(|sample| sample.abs() > 0.001));
    }

    /// Run explicitly after downloading archives with scripts/checks/check-model-downloads.js.
    #[test]
    #[ignore = "requires real model archives; run by the model-download workflow"]
    fn model_archive_smoke() {
        use super::super::{
            model::{extract_model_archive, verify_model_archive},
            models::MODELS,
        };
        let archive_dir = PathBuf::from(
            env::var_os("PAPERCUT_MODEL_ARCHIVE_DIR")
                .expect("PAPERCUT_MODEL_ARCHIVE_DIR is required"),
        );
        let only_kokoro = env::var_os("PAPERCUT_SMOKE_KOKORO_ONLY").is_some();
        // One archive in a fresh process proves a distill does not borrow an installed 82M data tree.
        let archive_name = env::var("PAPERCUT_SMOKE_ARCHIVE_NAME").ok();
        let mut exercised = 0;
        let work = archive_dir.join(format!("smoke-{}", std::process::id()));
        assert!(
            !work.exists(),
            "Smoke directory already exists: {}",
            work.display()
        );
        fs::create_dir_all(&work).unwrap();
        let mut extracted = std::collections::HashSet::new();
        for model in MODELS
            .iter()
            .filter(|model| matches!(model.backend, TtsModelBackend::SherpaOnnx))
        {
            if archive_name
                .as_deref()
                .is_some_and(|name| name != model.directory_name)
            {
                continue;
            }
            // Historical mirror preparation includes only the original multilingual archive.
            if only_kokoro && model.directory_name != "kokoro-multi-lang-v1_0" {
                continue;
            }
            if extracted.insert(model.directory_name) {
                let archive = archive_dir.join(format!("{}.tar.bz2", model.directory_name));
                verify_model_archive(&archive, model).unwrap();
                extract_model_archive(&archive, &work).unwrap();
            }
            let model_dir = work.join(model.directory_name);
            assert!(
                model.has_required_files(&model_dir),
                "Missing files: {}",
                model.id
            );
            let engine = SherpaTtsEngine {
                phoneme_adapter: PhonemeAdapter::load(model.id, &model_dir).unwrap(),
                tts: create_engine(model, &model_dir, 2).unwrap(),
                model,
                model_dir,
                num_threads: 2,
            };
            for voice in model.voices {
                assert!(
                    voice.speaker_id >= 0 && voice.speaker_id < engine.tts.num_speakers(),
                    "Invalid speaker: {}",
                    voice.id
                );
            }
            let text = match model.language {
                "en-US" => "This is a short voice download test.",
                "zh-CN" => "你好，这是语音测试。",
                "es-ES" => "Hola, esta es una prueba de voz.",
                "fr-FR" => "Bonjour, ceci est un test de voix.",
                "hi-IN" => "नमस्ते, यह आवाज़ का परीक्षण है।",
                "it-IT" => "Ciao, questa è una prova della voce.",
                "pt-BR" => "Olá, este é um teste de voz.",
                "ar" | "ar-JO" => "مرحبا هذا اختبار للصوت",
                language => panic!("Add a smoke sample for {language}"),
            };
            let text = TextPreprocessor::create(model, model.default_text_preprocessor)
                .unwrap()
                .process(text)
                .unwrap();
            let audio = generate_audio(&engine, &text, model.default_voice, 1.0)
                .unwrap()
                .unwrap();
            assert!(
                audio.sample_rate() > 0 && audio.samples().len() > audio.sample_rate() as usize / 4
            );
            assert!(audio.samples().iter().all(|sample| sample.is_finite()));
            assert!(
                audio.samples().iter().any(|sample| sample.abs() > 0.001),
                "Silent audio: {}",
                model.id
            );
            // A section separator must not wedge a long save or require changing its source index.
            if engine.phoneme_adapter.is_some() {
                let separator_wav = work.join("separator.wav");
                let result =
                    synthesize_to_file(&engine, "* * *", model.default_voice, 1.0, &separator_wav)
                        .expect("Separator should use a silent placeholder");
                assert!(result.audio_duration_sec > 0.0);
                assert!(super::super::cache::wav_info(&separator_wav).is_some());
                assert!(generate_audio(&engine, "1", model.default_voice, 1.0)
                    .unwrap()
                    .is_some());
                let cases: Vec<FrontendCase> =
                    serde_json::from_str(include_str!("../../../tts/frontend-smoke.json")).unwrap();
                for case in cases.iter().filter(|case| case.model == model.id) {
                    let expected = case
                        .phones
                        .as_ref()
                        .expect("Reviewed candidate phone fixture");
                    assert_eq!(
                        engine
                            .phoneme_adapter
                            .as_ref()
                            .unwrap()
                            .phonemize(&case.text)
                            .unwrap()
                            .concat(),
                        expected.chars().map(|c| c as i32).collect::<Vec<_>>(),
                        "{}",
                        case.name
                    );
                }
            }
            if model.id == super::super::models::KOKORO_7M_MODEL_ID {
                let frontend = engine.phoneme_adapter.as_ref().unwrap();
                assert_eq!(
                    frontend.phonemize("Library > Section > Chapter").unwrap(),
                    frontend
                        .phonemize("Library greater than Section greater than Chapter")
                        .unwrap()
                );
                assert!(generate_audio(
                    &engine,
                    "Library > Section > Chapter",
                    model.default_voice,
                    1.0
                )
                .unwrap()
                .is_some());
            }
            let wav = work.join(format!("{}.wav", model.id.replace('/', "-")));
            assert!(audio.save(wav.to_str().unwrap()));
            assert!(super::super::cache::wav_info(&wav).is_some());
            exercised += 1;
            println!(
                "PASS {}: {} speakers, {} samples",
                model.id,
                engine.tts.num_speakers(),
                audio.samples().len()
            );
        }
        assert!(exercised > 0, "Archive filter did not exercise any model");
        fs::remove_dir_all(work).unwrap();
    }

    /// Replay a real save's exact chunks/settings into a separate directory, without touching its cache.
    /// This opt-in check catches long-document failures that short preview sentences cannot expose.
    #[test]
    #[ignore = "requires PAPERCUT_TTS_SMOKE_MANIFEST, PAPERCUT_TTS_MODEL_ROOT and PAPERCUT_TTS_SMOKE_OUTPUT_DIR"]
    fn cached_audiobook_smoke() {
        let manifest: NativeAudiobookManifest = serde_json::from_slice(
            &fs::read(env::var_os("PAPERCUT_TTS_SMOKE_MANIFEST").unwrap()).unwrap(),
        )
        .unwrap();
        let root = PathBuf::from(env::var_os("PAPERCUT_TTS_MODEL_ROOT").unwrap());
        let out = PathBuf::from(env::var_os("PAPERCUT_TTS_SMOKE_OUTPUT_DIR").unwrap());
        // A fresh evidence directory prevents accidental overwrites, including of the supplied save.
        fs::create_dir(&out).expect("Use a new smoke output directory");
        let model = model_definition(&manifest.model_id).unwrap();
        let model_dir = root.join(model.directory_name);
        let engine = SherpaTtsEngine {
            phoneme_adapter: PhonemeAdapter::load(model.id, &model_dir).unwrap(),
            tts: create_engine(model, &model_dir, manifest.thread_count).unwrap(),
            model,
            model_dir,
            num_threads: manifest.thread_count,
        };
        let preprocessor = TextPreprocessor::create(model, &manifest.text_preprocessor).unwrap();
        let mut failures = Vec::new();
        for (index, chunk) in manifest.chunks.iter().enumerate() {
            let output = out.join(format!("{:05}.wav", index + 1));
            let result = preprocessor.process(&chunk.text).and_then(|text| {
                synthesize_to_file(&engine, &text, &manifest.voice, manifest.speed, &output)
            });
            match result {
                Ok(_) => assert!(super::super::cache::wav_info(&output).is_some()),
                Err(error) => failures.push(format!("chunk {}: {error}", index + 1)),
            }
        }
        fs::write(out.join("failures.txt"), failures.join("\n")).unwrap();
        assert!(failures.is_empty(), "{}", failures.join("\n"));
        println!("PASS audiobook smoke: {} chunks", manifest.chunks.len());
    }

    /// One production-path corpus covers the current roster and qualified distills; returns to English after switching.
    /// Fixtures reuse the owner-reviewed Stage 2 passages; WAVs are evidence, not an automatic quality verdict.
    #[test]
    #[ignore = "requires cached roster models and PAPERCUT_TTS_MODEL_ROOT/PAPERCUT_TTS_SMOKE_OUTPUT_DIR"]
    fn cached_frontend_smoke() {
        let root = PathBuf::from(env::var_os("PAPERCUT_TTS_MODEL_ROOT").unwrap());
        let out = PathBuf::from(env::var_os("PAPERCUT_TTS_SMOKE_OUTPUT_DIR").unwrap());
        fs::create_dir_all(&out).unwrap();
        let cases: Vec<FrontendCase> =
            serde_json::from_str(include_str!("../../../tts/frontend-smoke.json")).unwrap();
        fs::write(
            out.join("catalog.json"),
            serde_json::to_vec_pretty(
                &super::super::models::visible_models()
                    .map(ModelDefinition::to_info)
                    .collect::<Vec<_>>(),
            )
            .unwrap(),
        )
        .unwrap();
        let mut first = None;
        for case in cases {
            let model = model_definition(&case.model).unwrap();
            let model_dir = root.join(model.directory_name);
            assert!(
                model.has_required_files(&model_dir),
                "Missing model: {}",
                case.model
            );
            let engine = SherpaTtsEngine {
                phoneme_adapter: PhonemeAdapter::load(model.id, &model_dir).unwrap(),
                tts: create_engine(model, &model_dir, 2).unwrap(),
                model,
                model_dir,
                num_threads: 2,
            };
            let voice = model
                .voices
                .iter()
                .find(|v| v.speaker_id == case.speaker)
                .unwrap();
            if let Some(expected) = &case.phones {
                let phones = engine
                    .phoneme_adapter
                    .as_ref()
                    .expect("Expected phoneme adapter")
                    .phonemize(&case.text)
                    .unwrap();
                let expected: Vec<i32> = expected.chars().map(|c| c as i32).collect();
                assert_eq!(phones.concat(), expected, "{}", case.name);
                assert_eq!(
                    generation_language(model, voice.id),
                    PhonemeAdapter::locale(model.id)
                );
                assert!(generate_audio(&engine, "Hello", "af_heart", 1.0).is_err());
                assert!(generate_audio(&engine, "\u{200b}", voice.id, 1.0)
                    .unwrap()
                    .is_none());
            }
            let audio = generate_audio(&engine, &case.text, voice.id, 1.0)
                .unwrap()
                .unwrap();
            assert!(audio.samples().len() > audio.sample_rate() as usize / 4);
            assert!(audio.samples().iter().all(|sample| sample.is_finite()));
            assert!(audio.samples().iter().any(|sample| sample.abs() > 0.001));
            if case.name == "english-us" {
                first = Some(
                    phonemize(
                        &engine.model_dir.join("espeak-ng-data"),
                        &case.text,
                        "en-us",
                    )
                    .unwrap(),
                );
            }
            if case.name == "english-return" {
                assert_eq!(
                    first,
                    Some(
                        phonemize(
                            &engine.model_dir.join("espeak-ng-data"),
                            &case.text,
                            "en-us"
                        )
                        .unwrap()
                    ),
                    "Language switching changed English phonemes"
                );
            }
            let wav = out.join(format!("{}.wav", case.name));
            assert!(audio.save(wav.to_str().unwrap()));
            assert!(super::super::cache::wav_info(&wav).is_some());
            println!("PASS {}: {} samples", case.name, audio.samples().len());
        }
    }

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

    #[test]
    fn kokoro_voice_prefix_selects_supported_frontend_mode() {
        let model = model_definition(DEFAULT_MODEL_ID).unwrap();
        assert_eq!(generation_language(model, "af_heart"), Some("en-us"));
        assert_eq!(generation_language(model, "bf_emma"), Some("en"));
        assert_eq!(
            generation_language(model_definition(KOKORO_ZH_MODEL_ID).unwrap(), "zf_xiaobei"),
            None
        );
        for (model_id, voice, language) in [
            ("sherpa-onnx/kokoro-multi-lang-v1_0-es", "ef_dora", "es"),
            ("sherpa-onnx/kokoro-multi-lang-v1_0-fr", "ff_siwis", "fr"),
            ("sherpa-onnx/kokoro-multi-lang-v1_0-hi", "hf_alpha", "hi"),
            ("sherpa-onnx/kokoro-multi-lang-v1_0-it", "if_sara", "it"),
            (
                "sherpa-onnx/kokoro-multi-lang-v1_0-pt-br",
                "pf_dora",
                "pt-br",
            ),
        ] {
            assert_eq!(
                generation_language(model_definition(model_id).unwrap(), voice),
                Some(language)
            );
        }
    }

    #[test]
    fn preview_text_is_short_and_speakable() {
        assert!(validate_preview_text("A short preview.").is_ok());
        assert!(validate_preview_text("   ").is_err());
        assert!(validate_preview_text(&"x".repeat(MAX_PREVIEW_TEXT_CHARS + 1)).is_err());
    }
}
