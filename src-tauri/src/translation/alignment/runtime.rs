//! Bounded CPU embeddings and internal word projection; public jobs remain translator-only.
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use ort::{
    session::{RunOptions, Session},
    value::{Tensor, TensorElementType, ValueType},
};

use super::{assets, AdmissionError, CandidateTokenizer, EncodedText, MAX_TOKENS};
use crate::translation::{
    inline_markup::provider::{
        contract::{Rejection, ScopeIssue, Side},
        pieces::{Projection, Translation},
    },
    model_store::verified::AssetError,
    state::{TranslationModelOperation, TranslationState},
};

pub(super) const WIDTH: usize = 768;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SessionError {
    Admission(AdmissionError),
    Busy,
    Cancelled,
    RuntimeUnavailable,
    Graph,
    Inference,
    Output,
    Control,
    Mapping(Rejection),
}

impl From<Rejection> for SessionError {
    fn from(error: Rejection) -> Self {
        Self::Mapping(error)
    }
}

impl From<AdmissionError> for SessionError {
    fn from(error: AdmissionError) -> Self {
        match error {
            AdmissionError::Assets(AssetError::Cancelled) => Self::Cancelled,
            other => Self::Admission(other),
        }
    }
}

/// A job-scoped cancellation handle; native creation is checked before/after, runs use ORT termination.
#[derive(Default)]
pub(crate) struct Cancellation {
    cancelled: AtomicBool,
    active: Mutex<Option<Arc<RunOptions>>>,
}

impl Cancellation {
    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    /// Setting the flag first prevents a run from slipping between cancellation and registration.
    pub(crate) fn cancel(&self) -> Result<(), SessionError> {
        self.cancelled.store(true, Ordering::Release);
        if let Some(options) = self
            .active
            .lock()
            .map_err(|_| SessionError::Control)?
            .as_ref()
        {
            options.terminate().map_err(|_| SessionError::Control)?;
        }
        Ok(())
    }

    fn check(&self) -> Result<(), SessionError> {
        if self.is_cancelled() {
            Err(SessionError::Cancelled)
        } else {
            Ok(())
        }
    }

    /// One registered run, no background watcher or waiting queue; the lock closes the cancel/start race.
    fn register(&self, options: Arc<RunOptions>) -> Result<ActiveRun<'_>, SessionError> {
        let mut active = self.active.lock().map_err(|_| SessionError::Control)?;
        self.check()?;
        if active.is_some() {
            return Err(SessionError::Busy);
        }
        *active = Some(options.clone());
        Ok(ActiveRun {
            cancellation: self,
            options,
        })
    }
}

struct ActiveRun<'a> {
    cancellation: &'a Cancellation,
    options: Arc<RunOptions>,
}
impl Drop for ActiveRun<'_> {
    fn drop(&mut self) {
        // Even a poisoned caller lock must not retain native run options after the run ends.
        *self
            .cancellation
            .active
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = None;
    }
}

pub(crate) struct Embedding<'a> {
    pub(crate) encoding: EncodedText<'a>,
    pub(crate) values: Vec<f32>,
}
pub(crate) struct PairEmbeddings<'a> {
    pub(crate) source: Embedding<'a>,
    pub(crate) target: Embedding<'a>,
}

/// Field order releases the native session before its model-use lease; one lease admits one session.
pub(crate) struct AlignmentSession {
    session: Session,
    tokenizer: CandidateTokenizer,
    _lease: TranslationModelOperation,
}

/// Borrow the job-owned session; its existing registration terminates inference on cancellation.
pub(crate) struct NativeAlignment<'a> {
    pub(crate) session: &'a mut AlignmentSession,
    pub(crate) cancellation: std::sync::Arc<Cancellation>,
}

impl crate::translation::inline_markup::provider::pieces::PieceAligner for NativeAlignment<'_> {
    /// Runtime/model/tokenizer identities are independent from projection policy so MT text can be reused.
    fn versions(
        &self,
        model: &str,
    ) -> Result<crate::translation::inline_markup::provider::contract::Versions, String> {
        use crate::translation::{
            inline_markup::{protected, provider::contract},
            model_store,
        };
        if !matches!(
            model,
            "opus-mt-fr-en-ctranslate2" | "opus-mt-es-en-ctranslate2"
        ) {
            return Err("Strict alignment pair is not supported".into());
        }
        let manifest = model_store::manifest_by_id(model).ok_or("Missing translator pins")?;
        let files = manifest
            .files
            .iter()
            .map(|f| (f.path, f.sha256, f.bytes))
            .collect::<Vec<_>>();
        let pins = crate::translation::hash::stable_text_hash(
            &serde_json::to_string(&(manifest.revision, files))
                .map_err(|_| "Translator identity failed")?,
        );
        Ok(contract::Versions {
            translator: format!("{model}:{pins}:ct2rs-0.9.19:exact-v1"),
            aligner: format!(
                "{}:{}:ort-rc13-api17:cpu:{}",
                assets::MODEL_KEY,
                assets::REVISION,
                crate::translation::hash::stable_text_hash(ort::info())
            ),
            tokenizer: assets::TOKENIZER_ID.into(),
            normalization: format!("{}:exact-pieces-v1", protected::MAPPING_VERSION),
            policy: format!("{}:{}", super::extract::POLICY, contract::PROJECTION_POLICY),
        })
    }

    fn text_fits(&self, text: &str, side: Side) -> Result<bool, String> {
        self.cancellation
            .check()
            .map_err(|e| format!("Alignment rejected: {e:?}"))?;
        self.session
            .text_fits(text, side)
            .map_err(|e| format!("Alignment admission rejected: {e:?}"))
    }

    fn map(
        &mut self,
        translation: &Translation<'_>,
    ) -> Result<Vec<crate::translation::inline_markup::provider::contract::NodeMapping>, String>
    {
        self.session
            .project_translation(translation, &self.cancellation, None)
            .map(|p| p.nodes().to_vec())
            .map_err(|e| format!("Alignment mapping rejected: {e:?}"))
    }
}

impl AlignmentSession {
    /// Reuse the job's session for exact pieces, dropping each pair's tensors before the next pair.
    pub(crate) fn project_translation<'a, 'p>(
        &mut self,
        translation: &'a Translation<'p>,
        cancellation: &Cancellation,
        unresolved: Option<ScopeIssue>,
    ) -> Result<Projection<'a, 'p>, SessionError> {
        translation.project(
            |source, target| {
                let embeddings = self.infer_pair(source, target, cancellation)?;
                super::extract::word_links(&embeddings, &self.tokenizer.rules(), || {
                    cancellation.is_cancelled()
                })
            },
            unresolved,
            || cancellation.check(),
        )
    }
    /// Reuse the session's verified tokenizer for Stage 4 admission without running or duplicating the graph.
    pub(crate) fn text_fits(&self, text: &str, side: Side) -> Result<bool, SessionError> {
        self.tokenizer
            .text_fits(text, side)
            .map_err(SessionError::from)
    }
    /// Pin bytes under the existing state guard; never reopen a verified graph filename for ORT.
    pub(crate) fn load(
        state: &TranslationState,
        dir: &Path,
        cancellation: &Cancellation,
    ) -> Result<Self, SessionError> {
        Self::load_pack(state, dir, cancellation, false)
    }

    /// Application admission includes installed notices; raw developer probes retain their seven-file gate.
    pub(crate) fn load_installed(
        state: &TranslationState,
        dir: &Path,
        cancellation: &Cancellation,
    ) -> Result<Self, SessionError> {
        Self::load_pack(state, dir, cancellation, true)
    }

    /// Keep the lease through snapshot admission and native creation, including failed/cancelled loads.
    fn load_pack(
        state: &TranslationState,
        dir: &Path,
        cancellation: &Cancellation,
        installed: bool,
    ) -> Result<Self, SessionError> {
        cancellation.check()?;
        let lease = state
            .claim_model_read(assets::MODEL_KEY)
            .map_err(|_| SessionError::Busy)?;
        let cancelled = || cancellation.is_cancelled();
        let (tokenizer, weights) = if installed {
            assets::installed_runtime_snapshot(dir, &cancelled)?
        } else {
            assets::runtime_snapshot(dir, &cancelled)?
        };
        cancellation.check()?;
        let tokenizer = CandidateTokenizer::from_snapshot(&tokenizer)?;
        crate::onnx_runtime::initialize().map_err(|_| SessionError::RuntimeUnavailable)?;
        cancellation.check()?;
        let session = load_graph(&weights)?;
        drop(weights);
        // ORT has no cancellation hook for CreateSession; cancel discards the completed load here.
        cancellation.check()?;
        check_signature(
            &session
                .inputs()
                .iter()
                .map(|o| (o.name(), o.dtype()))
                .collect::<Vec<_>>(),
            &session
                .outputs()
                .iter()
                .map(|o| (o.name(), o.dtype()))
                .collect::<Vec<_>>(),
        )?;
        Ok(Self {
            session,
            tokenizer,
            _lease: lease,
        })
    }

    /// Reuse one session, infer sides sequentially and return no partial pair on cancellation/error.
    pub(crate) fn infer_pair<'a>(
        &mut self,
        source: &'a str,
        target: &'a str,
        cancellation: &Cancellation,
    ) -> Result<PairEmbeddings<'a>, SessionError> {
        cancellation.check()?;
        let source = self.tokenizer.encode(source, Side::Source)?;
        let target = self.tokenizer.encode(target, Side::Target)?;
        let source_values = infer(&mut self.session, &source, cancellation)?;
        let target_values = infer(&mut self.session, &target, cancellation)?;
        cancellation.check()?;
        Ok(PairEmbeddings {
            source: Embedding {
                encoding: source,
                values: source_values,
            },
            target: Embedding {
                encoding: target,
                values: target_values,
            },
        })
    }
}

/// Explicit CPU/sequential pools avoid inheriting another consumer's GPU or global thread settings.
fn load_graph(weights: &[u8]) -> Result<Session, SessionError> {
    (|| -> ort::Result<Session> {
        Session::builder()?
            .with_no_environment_execution_providers()?
            .with_execution_providers([ort::ep::CPU::default()
                .with_arena_allocator(false)
                .build()])?
            .with_independent_thread_pool()?
            .with_intra_threads(2)?
            .with_inter_threads(1)?
            .with_parallel_execution(false)?
            .with_memory_pattern(false)?
            .commit_from_memory(weights)
    })()
    .map_err(|_| SessionError::Graph)
}

/// Require the pinned export's exact signature, not metadata inferred from its misleading model config.
fn check_signature(
    inputs: &[(&str, &ValueType)],
    outputs: &[(&str, &ValueType)],
) -> Result<(), SessionError> {
    let tensor = |v: &ValueType, ty, shape: &[i64]| matches!(v, ValueType::Tensor{ty:t,shape:s,..} if *t==ty && s.as_ref()==shape);
    if inputs.len() != 2
        || outputs.len() != 1
        || inputs[0].0 != "input_ids"
        || inputs[1].0 != "attention_mask"
        || !inputs
            .iter()
            .all(|(_, v)| tensor(v, TensorElementType::Int64, &[-1, -1]))
        || outputs[0].0 != "last_hidden_state"
        || !tensor(
            outputs[0].1,
            TensorElementType::Float32,
            &[-1, -1, WIDTH as i64],
        )
    {
        return Err(SessionError::Graph);
    }
    Ok(())
}

/// Bound tensor construction to admitted tokens; cancellation discards even a completed native result.
fn infer(
    session: &mut Session,
    encoding: &EncodedText<'_>,
    cancellation: &Cancellation,
) -> Result<Vec<f32>, SessionError> {
    cancellation.check()?;
    let tokens = encoding.tokens.len();
    if !(3..=MAX_TOKENS).contains(&tokens) {
        return Err(SessionError::Output);
    }
    let ids = encoding
        .tokens
        .iter()
        .map(|t| i64::from(t.id))
        .collect::<Vec<_>>();
    let ids = Tensor::from_array(([1usize, tokens], ids)).map_err(|_| SessionError::Inference)?;
    let mask = Tensor::from_array(([1usize, tokens], vec![1i64; tokens]))
        .map_err(|_| SessionError::Inference)?;
    let active = cancellation.register(Arc::new(
        RunOptions::new().map_err(|_| SessionError::Inference)?,
    ))?;
    let result = session.run_with_options(
        ort::inputs!["input_ids"=>ids,"attention_mask"=>mask],
        &*active.options,
    );
    cancellation.check()?;
    let result = result.map_err(|_| SessionError::Inference)?;
    let (shape, values) = result
        .get("last_hidden_state")
        .ok_or(SessionError::Output)?
        .try_extract_tensor::<f32>()
        .map_err(|_| SessionError::Output)?;
    validated_values(shape.as_ref(), values, tokens)
}

/// Validate before copying a result; no non-finite or mis-sized embedding escapes to future extraction.
fn validated_values(
    shape: &[i64],
    values: &[f32],
    tokens: usize,
) -> Result<Vec<f32>, SessionError> {
    if !(3..=MAX_TOKENS).contains(&tokens)
        || shape != [1, tokens as i64, WIDTH as i64]
        || values.len() != tokens * WIDTH
        || values.iter().any(|v| !v.is_finite())
    {
        return Err(SessionError::Output);
    }
    Ok(values.to_vec())
}

#[cfg(test)]
mod tests;
