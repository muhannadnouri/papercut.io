//! Bounded text-only bridge to one WebView-owned Bergamot worker attempt.
//! This prototype does not admit catalog models or write document/cache data.

use std::collections::HashSet;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use super::engine::{
    validate_outputs, TranslationBatchInput, TranslationEngine, TranslationSegmentOutput,
};
use serde::{Deserialize, Serialize};

const MAX_SEGMENTS: usize = 16;
const MAX_SOURCE_CHARS: usize = 900;
const MAX_OUTPUT_BYTES: usize = 256 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Segment {
    pub id: String,
    pub text: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct BatchRequest {
    pub job_id: String,
    pub source_language: String,
    pub target_language: String,
    pub segments: Vec<Segment>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum WorkerRequest {
    Batch {
        attempt_id: String,
        batch_id: u32,
        source_language: String,
        target_language: String,
        segments: Vec<Segment>,
    },
    Stop {
        attempt_id: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(crate) enum WorkerReply {
    Result {
        attempt_id: String,
        batch_id: u32,
        outputs: Vec<TranslationSegmentOutput>,
    },
    Failed {
        attempt_id: String,
        batch_id: u32,
        reason: Failure,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Failure {
    Cancelled,
    WorkerLost,
    Unavailable,
    EngineFailed,
}

struct Active {
    id: String,
    owner: String,
    pending: Option<(u32, mpsc::SyncSender<WorkerReply>)>,
}

#[derive(Default)]
pub(crate) struct WorkerBridge(Mutex<Option<Active>>);

impl WorkerBridge {
    pub fn open(
        self: &Arc<Self>,
        owner: String,
        job_id: String,
        cancelled: Arc<Mutex<HashSet<String>>>,
        emit: impl Fn(WorkerRequest) -> Result<(), String> + Send + 'static,
    ) -> Result<WorkerEngine, String> {
        if job_id.is_empty() || job_id.len() > 128 {
            return Err("Invalid Bergamot worker job id".into());
        }
        let mut active = self.0.lock().map_err(|_| "Bergamot worker lock poisoned")?;
        if active.is_some() {
            return Err("A Bergamot worker attempt is already in progress".into());
        }
        let mut nonce = [0u8; 16];
        getrandom::fill(&mut nonce).map_err(|_| "Could not create Bergamot worker attempt")?;
        let id = nonce
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        *active = Some(Active {
            id: id.clone(),
            owner,
            pending: None,
        });
        Ok(WorkerEngine {
            bridge: Arc::clone(self),
            id,
            job_id,
            cancelled,
            emit: Box::new(emit),
            batch_id: 0,
            timeout: Duration::from_secs(120),
            closed: false,
        })
    }

    /// A reply consumes its one-slot mailbox. Duplicates and stale attempts cannot wake another batch.
    pub fn reply(&self, owner: &str, mut reply: WorkerReply) -> Result<bool, String> {
        let (id, batch) = match &reply {
            WorkerReply::Result {
                attempt_id,
                batch_id,
                ..
            }
            | WorkerReply::Failed {
                attempt_id,
                batch_id,
                ..
            } => (attempt_id, *batch_id),
        };
        let mut active = self.0.lock().map_err(|_| "Bergamot worker lock poisoned")?;
        let Some(active) = active.as_mut().filter(|a| a.id == *id && a.owner == owner) else {
            return Ok(false);
        };
        if active.pending.as_ref().map(|p| p.0) != Some(batch) {
            return Ok(false);
        }
        if let WorkerReply::Result { outputs, .. } = &reply {
            let size = outputs.iter().try_fold(0usize, |n, o| {
                n.checked_add(o.id.len())?
                    .checked_add(o.source_text.len())?
                    .checked_add(o.text.len())
            });
            if outputs.len() > MAX_SEGMENTS || size.is_none_or(|size| size > MAX_OUTPUT_BYTES) {
                reply = WorkerReply::Failed {
                    attempt_id: active.id.clone(),
                    batch_id: batch,
                    reason: Failure::EngineFailed,
                };
            }
        }
        let (_, sender) = active
            .pending
            .take()
            .ok_or("Bergamot worker mailbox missing")?;
        Ok(sender.try_send(reply).is_ok())
    }
}

pub(crate) struct WorkerEngine {
    bridge: Arc<WorkerBridge>,
    id: String,
    job_id: String,
    cancelled: Arc<Mutex<HashSet<String>>>,
    emit: Box<dyn Fn(WorkerRequest) -> Result<(), String> + Send>,
    batch_id: u32,
    timeout: Duration,
    closed: bool,
}

impl WorkerEngine {
    fn check_cancelled(&self) -> Result<(), String> {
        if self
            .cancelled
            .lock()
            .map_err(|_| "Translation cancellation lock poisoned")?
            .contains(&self.job_id)
        {
            return Err("Translation cancelled".into());
        }
        Ok(())
    }

    fn batch(
        &mut self,
        input: TranslationBatchInput,
    ) -> Result<Vec<TranslationSegmentOutput>, String> {
        if self.closed {
            return Err("Bergamot worker attempt is closed".into());
        }
        self.check_cancelled()?;
        if !matches!(input.source_language.as_str(), "fr" | "es")
            || input.target_language != "en"
            || input.segments.is_empty()
            || input.segments.len() > MAX_SEGMENTS
            || input.segments.iter().any(|s| {
                s.id.is_empty()
                    || s.id.len() > 128
                    || s.text.trim().is_empty()
                    || s.text.chars().take(MAX_SOURCE_CHARS + 1).count() > MAX_SOURCE_CHARS
            })
        {
            return Err(
                "Bergamot worker requires a bounded French/Spanish to English text batch".into(),
            );
        }
        let expected = input
            .segments
            .iter()
            .map(|s| (s.id.as_str(), s.text.as_str()))
            .collect::<Vec<_>>();
        let mut ids = HashSet::new();
        if expected.iter().any(|(id, _)| !ids.insert(*id)) {
            return Err("Duplicate Bergamot segment identities".into());
        }
        self.batch_id = self
            .batch_id
            .checked_add(1)
            .ok_or("Bergamot batch counter exhausted")?;
        let deadline = Instant::now() + self.timeout;
        let (sender, receiver) = mpsc::sync_channel(1);
        {
            let mut active = self
                .bridge
                .0
                .lock()
                .map_err(|_| "Bergamot worker lock poisoned")?;
            let active = active
                .as_mut()
                .filter(|a| a.id == self.id)
                .ok_or("Bergamot worker attempt is closed")?;
            active.pending = Some((self.batch_id, sender));
        }
        (self.emit)(WorkerRequest::Batch {
            attempt_id: self.id.clone(),
            batch_id: self.batch_id,
            source_language: input.source_language.clone(),
            target_language: input.target_language.clone(),
            segments: input
                .segments
                .iter()
                .map(|s| Segment {
                    id: s.id.clone(),
                    text: s.text.clone(),
                })
                .collect(),
        })?;
        let reply = loop {
            self.check_cancelled()?;
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or("Bergamot worker timed out")?;
            match receiver.recv_timeout(remaining.min(Duration::from_millis(25))) {
                Ok(reply) => break reply,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("Bergamot worker was lost".into())
                }
            }
        };
        self.check_cancelled()?;
        if Instant::now() >= deadline {
            return Err("Bergamot worker timed out".into());
        }
        match reply {
            WorkerReply::Result { outputs, .. } => {
                validate_outputs(&expected, &outputs)?;
                Ok(outputs)
            }
            WorkerReply::Failed { reason, .. } => Err(match reason {
                Failure::Cancelled => "Translation cancelled",
                Failure::WorkerLost => "Bergamot worker was lost",
                Failure::Unavailable => "Bergamot worker runtime is unavailable",
                Failure::EngineFailed => "Bergamot worker failed",
            }
            .into()),
        }
    }

    fn close(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;
        if let Ok(mut active) = self.bridge.0.lock() {
            if active.as_ref().is_some_and(|a| a.id == self.id) {
                *active = None;
            }
        }
        let _ = (self.emit)(WorkerRequest::Stop {
            attempt_id: self.id.clone(),
        });
    }
}
impl TranslationEngine for WorkerEngine {
    fn translate_batch(
        &mut self,
        input: TranslationBatchInput,
    ) -> Result<Vec<TranslationSegmentOutput>, String> {
        let result = self.batch(input);
        if result.is_err() {
            self.close();
        }
        result
    }
}
impl Drop for WorkerEngine {
    fn drop(&mut self) {
        self.close();
    }
}

/// Reuse translation job admission and cancellation without creating a parallel document writer.
pub(super) fn run_batch(
    state: &super::state::TranslationState,
    owner: String,
    request: BatchRequest,
    emit: impl Fn(WorkerRequest) -> Result<(), String> + Send + 'static,
) -> Result<Vec<TranslationSegmentOutput>, String> {
    let _job = state.claim_job("bergamot-worker-prototype", "bergamot-worker-prototype")?;
    let job_id = request.job_id;
    state
        .cancelled_jobs
        .lock()
        .map_err(|_| "Translation cancellation lock poisoned")?
        .remove(&job_id);
    let result = (|| {
        let mut engine =
            state
                .bergamot
                .open(owner, job_id.clone(), state.cancelled_jobs.clone(), emit)?;
        engine.translate_batch(TranslationBatchInput {
            model_id: "bergamot-worker-prototype".into(),
            source_language: request.source_language,
            target_language: request.target_language,
            quality_mode: "balanced".into(),
            repair_mode: Default::default(),
            glossary: Vec::new(),
            segments: request
                .segments
                .into_iter()
                .map(|s| super::engine::TranslationSegmentInput {
                    id: s.id,
                    text: s.text,
                    context: Default::default(),
                })
                .collect(),
        })
    })();
    if let Ok(mut cancelled) = state.cancelled_jobs.lock() {
        cancelled.remove(&job_id);
    }
    result
}

#[cfg(test)]
mod tests;
