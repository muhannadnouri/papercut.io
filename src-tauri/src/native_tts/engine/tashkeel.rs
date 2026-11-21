//! ORT adapter for the official Libtashkeel public inference interface.
//! Language processing and character maps remain in the registry package.

use libtashkeel_base::{InferenceEngine, LibtashkeelError, LibtashkeelResult};
use ort::{session::builder::GraphOptimizationLevel, session::Session, value::Tensor};
use std::{error::Error, sync::Mutex};

type CheckResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

// ponytail: serialize each engine's session; consider a pool only after measured contention.
pub(super) struct TashkeelEngine(Mutex<Session>);

impl TashkeelEngine {
    pub(super) fn from_bytes(bytes: &[u8]) -> CheckResult<Self> {
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
                .commit_from_memory(bytes)?,
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
