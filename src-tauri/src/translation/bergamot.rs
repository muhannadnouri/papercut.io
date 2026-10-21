//! Bounded mailbox and lifetime for one WebView-owned Bergamot worker attempt.
//! The adapter child owns engine/markup conversion; transport never restores document structure.
//! Rust owns document planning, resume and persistence; the WebView owns inference.

use std::collections::HashSet;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use super::engine::TranslationSegmentOutput;
use serde::{Deserialize, Serialize};

mod adapter;
pub(super) mod assets;

const MAX_SEGMENTS: usize = 16;
const MAX_OUTPUT_BYTES: usize = 256 * 1024;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Segment {
    pub id: String,
    pub text: String,
    pub html: bool,
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
    Confirm {
        attempt_id: String,
        batch_id: u32,
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
    Confirmed {
        attempt_id: String,
        batch_id: u32,
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
    /// Admit one owner-bound attempt; the same mailbox serves sequential document batches.
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
            | WorkerReply::Confirmed {
                attempt_id,
                batch_id,
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
            if outputs.iter().any(|o| o.markup.is_some())
                || outputs.len() > MAX_SEGMENTS
                || size.is_none_or(|size| size > MAX_OUTPUT_BYTES)
            {
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

    /// One nonce/sequence mailbox and absolute deadline prevent stale replies extending an attempt.
    fn exchange(
        &mut self,
        request: impl FnOnce(String, u32) -> WorkerRequest,
    ) -> Result<WorkerReply, String> {
        if self.closed {
            return Err("Bergamot worker attempt is closed".into());
        }
        self.check_cancelled()?;
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
        (self.emit)(request(self.id.clone(), self.batch_id))?;
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
            WorkerReply::Failed { reason, .. } => Err(match reason {
                Failure::Cancelled => "Translation cancelled",
                Failure::WorkerLost => "Bergamot worker was lost",
                Failure::Unavailable => "Bergamot worker runtime is unavailable",
                Failure::EngineFailed => "Bergamot worker failed",
            }
            .into()),
            reply => Ok(reply),
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
impl Drop for WorkerEngine {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests;
