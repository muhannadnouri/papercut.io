//! Linux compatibility check, not an app integration or retrieval-quality test.
//! Uses official Libtashkeel's public engine interface; no patched package.

use fastembed::{
    InitOptionsUserDefined, Pooling, QuantizationMode, TextEmbedding, TokenizerFiles,
    UserDefinedEmbeddingModel,
};
use libtashkeel_base::{do_tashkeel, InferenceEngine, LibtashkeelError, LibtashkeelResult};
use ort::{session::builder::GraphOptimizationLevel, session::Session, value::Tensor};
use sha2::{Digest, Sha256};
use sherpa_onnx::{
    GenerationConfig, OfflineTts, OfflineTtsConfig, OfflineTtsModelConfig,
    OfflineTtsVitsModelConfig,
};
use std::{collections::BTreeSet, error::Error, fs, path::Path, sync::Mutex};

type CheckResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

// ponytail: serialize each engine's session; consider a pool only after measured contention.
struct TashkeelEngine(Mutex<Session>);

impl TashkeelEngine {
    fn load(path: &Path) -> CheckResult<Self> {
        Ok(Self(Mutex::new(
            Session::builder()?
                .with_optimization_level(GraphOptimizationLevel::Level3)
                .map_err(|error| error.to_string())?
                .with_intra_threads(2)
                .map_err(|error| error.to_string())?
                .with_inter_threads(2)
                .map_err(|error| error.to_string())?
                .with_parallel_execution(true)
                .map_err(|error| error.to_string())?
                .commit_from_file(path)?,
        )))
    }
}

impl InferenceEngine for TashkeelEngine {
    fn infer(
        &self,
        input_ids: Vec<i64>,
        diac_ids: Vec<i64>,
        seq_length: usize,
    ) -> LibtashkeelResult<(Vec<u8>, Vec<f32>)> {
        let infer = || -> CheckResult<_> {
            if input_ids.len() != seq_length || diac_ids.len() != seq_length {
                return Err("inconsistent Libtashkeel input lengths".into());
            }
            let length = i64::try_from(seq_length)?;
            let mut session = self.0.lock().map_err(|_| "ONNX session lock poisoned")?;
            let outputs = session.run(ort::inputs![
                Tensor::from_array(([1, seq_length], input_ids))?,
                Tensor::from_array(([1, seq_length], diac_ids))?,
                Tensor::from_array(([1], vec![length]))?,
            ])?;
            let targets = outputs[0].try_extract_tensor::<u8>()?.1.to_vec();
            let logits = outputs[1].try_extract_tensor::<f32>()?.1.to_vec();
            // Preserve upstream's flattened class logits, not one score per character.
            if targets.len() != seq_length
                || logits.len() < seq_length
                || logits.iter().any(|value| !value.is_finite())
            {
                return Err(format!(
                    "invalid Libtashkeel output: {} targets, {} logits, {seq_length} inputs",
                    targets.len(),
                    logits.len()
                )
                .into());
            }
            Ok((targets, logits))
        };
        infer().map_err(|error| LibtashkeelError::InferenceError(error.to_string()))
    }
}

fn embedding(dir: &Path, quantization: QuantizationMode) -> CheckResult<TextEmbedding> {
    let files = TokenizerFiles {
        tokenizer_file: fs::read(dir.join("tokenizer.json"))?,
        config_file: fs::read(dir.join("config.json"))?,
        special_tokens_map_file: fs::read(dir.join("special_tokens_map.json"))?,
        tokenizer_config_file: fs::read(dir.join("tokenizer_config.json"))?,
    };
    Ok(TextEmbedding::try_new_from_user_defined(
        UserDefinedEmbeddingModel::new(fs::read(dir.join("onnx/model.onnx"))?, files)
            .with_pooling(Pooling::Mean)
            .with_quantization(quantization),
        InitOptionsUserDefined::new()
            .with_max_length(512)
            .with_intra_threads(2),
    )?)
}

fn speech(dir: &Path) -> CheckResult<OfflineTts> {
    OfflineTts::create(&OfflineTtsConfig {
        model: OfflineTtsModelConfig {
            num_threads: 2,
            provider: Some("cpu".into()),
            vits: OfflineTtsVitsModelConfig {
                model: Some(dir.join("ar_JO-kareem-medium.onnx").display().to_string()),
                tokens: Some(dir.join("tokens.txt").display().to_string()),
                data_dir: Some(dir.join("espeak-ng-data").display().to_string()),
                ..Default::default()
            },
            ..Default::default()
        },
        max_num_sentences: 1,
        ..Default::default()
    })
    .ok_or_else(|| "failed to create cached Arabic speech engine".into())
}

fn vectors(model: &mut TextEmbedding) -> CheckResult<Vec<Vec<f32>>> {
    let texts = [
        "query: safe city transport",
        "passage: Residents reach the city center by frequent electric buses.",
        "query: النقل العام في المدينة",
    ];
    // Dynamic INT8 follows the retained experiment's per-text inference policy.
    let vectors = texts
        .into_iter()
        .map(|text| model.embed([text], None))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    assert_eq!(vectors.len(), 3);
    for vector in &vectors {
        assert_eq!(vector.len(), 384);
        assert!(vector.iter().all(|value| value.is_finite()));
        let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-4);
    }
    Ok(vectors)
}

fn main() -> CheckResult<()> {
    let args = std::env::args_os().collect::<Vec<_>>();
    if args.len() != 6 {
        return Err("usage: search-v2-runtime-compat speech-first|embedding-first none|dynamic-int8 TASHKEEL_ONNX E5_DIR ARABIC_TTS_DIR; set ORT_DYLIB_PATH to the cached shared runtime".into());
    }
    let order = args[1].to_str().ok_or("invalid order encoding")?;
    if !matches!(order, "speech-first" | "embedding-first") {
        return Err("unsupported initialization order".into());
    }
    let quantization = match args[2].to_str() {
        Some("none") => QuantizationMode::None,
        Some("dynamic-int8") => QuantizationMode::Dynamic,
        _ => return Err("unsupported quantization policy".into()),
    };
    let runtime = std::env::var_os("ORT_DYLIB_PATH").ok_or("ORT_DYLIB_PATH is required")?;
    let runtime = fs::canonicalize(runtime)?;
    ort::init_from(&runtime)?.commit();
    // ORT must return a recoverable error for an invalid ONNX model.
    assert!(Session::builder()?
        .commit_from_memory(b"invalid model")
        .is_err());
    let (tashkeel, mut model, tts) = if order == "speech-first" {
        let tashkeel = TashkeelEngine::load(Path::new(&args[3]))?;
        let tts = speech(Path::new(&args[5]))?;
        (tashkeel, embedding(Path::new(&args[4]), quantization)?, tts)
    } else {
        let model = embedding(Path::new(&args[4]), quantization)?;
        let tashkeel = TashkeelEngine::load(Path::new(&args[3]))?;
        (tashkeel, model, speech(Path::new(&args[5]))?)
    };
    assert!(tashkeel.infer(vec![1], vec![1], 2).is_err());
    let sources = [
        "كتب الطالب الدرس",
        "بسم الله الرحمن الرحيم",
        "قرأت الطالبة كتابا في المكتبة",
        "وصل القطار إلى المدينة صباحا",
        "بِسمِ اللّه الرّحمن الرّحيم ABC",
        "مّنْ يُقلِّب ABC",
    ];
    let first = sources
        .iter()
        .map(|source| do_tashkeel(&tashkeel, source, None, true))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(first[0], "كَتَبَ الطَّالِبُ الدَّرْس");
    assert_eq!(first[1], "بِسْمِ اللَّهِ الرَّحْمَنِ الرَّحِيم");
    let before = vectors(&mut model)?;
    let audio = tts
        .generate_with_config(
            &first[0],
            &GenerationConfig::default(),
            None::<fn(&[f32], f32) -> bool>,
        )
        .ok_or("failed to synthesize preprocessed Arabic")?;
    assert!(audio.sample_rate() > 0);
    assert!(audio.samples().len() > audio.sample_rate() as usize / 4);
    assert!(audio.samples().iter().all(|sample| sample.is_finite()));
    assert!(audio.samples().iter().any(|sample| sample.abs() > 0.001));
    assert_eq!(before, vectors(&mut model)?);
    for (source, expected) in sources.iter().zip(&first) {
        assert_eq!(do_tashkeel(&tashkeel, source, None, true)?, *expected);
    }
    std::thread::scope(|scope| {
        let workers = (0..4)
            .map(|_| scope.spawn(|| do_tashkeel(&tashkeel, sources[0], None, true)))
            .collect::<Vec<_>>();
        for worker in workers {
            assert_eq!(
                worker.join().expect("preprocessing worker panicked")?,
                first[0]
            );
        }
        Ok::<_, LibtashkeelError>(())
    })?;
    let maps = fs::read_to_string("/proc/self/maps")?;
    let libraries = maps
        .lines()
        .filter_map(|line| line.split_whitespace().last())
        .filter(|path| path.contains("libonnxruntime.so"))
        .collect::<BTreeSet<_>>();
    assert_eq!(libraries.len(), 1, "must share one native ORT image");
    assert_eq!(fs::canonicalize(libraries.first().unwrap())?, runtime);
    println!(
        "{}",
        serde_json::json!({
            "order": order, "quantization": args[2].to_str(), "arabic": first,
            "vectors": before, "sampleRate": audio.sample_rate(),
            "samples": audio.samples().len(), "onnxRuntimeMappings": libraries,
            "tashkeelModelSha256": format!("{:x}", Sha256::digest(fs::read(&args[3])?)),
        })
    );
    Ok(())
}
