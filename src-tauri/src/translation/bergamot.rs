//! Bounded text/protected-code bridge to one WebView-owned Bergamot worker attempt.
//! Rust owns document planning, resume and persistence; the WebView owns inference.

use std::collections::HashSet;
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use super::engine::{
    validate_outputs, TranslationBatchInput, TranslationEngine, TranslationSegmentOutput,
};
use serde::{Deserialize, Serialize};

pub(super) mod assets;

const MAX_SEGMENTS: usize = 16;
const MAX_SOURCE_CHARS: usize = 900;
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

    /// Convert protected plans into opaque HTML, then validate and restore replies before the shared runner.
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
        let reply = self.exchange(|attempt_id, batch_id| WorkerRequest::Batch {
            attempt_id,
            batch_id,
            source_language: input.source_language.clone(),
            target_language: input.target_language.clone(),
            segments: input
                .segments
                .iter()
                .map(|s| Segment {
                    id: s.id.clone(),
                    text: s
                        .markup
                        .as_ref()
                        .map(|p| p.html.clone())
                        .unwrap_or_else(|| s.text.clone()),
                    html: s.markup.is_some(),
                })
                .collect(),
        })?;
        match reply {
            WorkerReply::Result { mut outputs, .. } => {
                let transport = input
                    .segments
                    .iter()
                    .map(|s| {
                        (
                            s.id.as_str(),
                            s.markup
                                .as_ref()
                                .map(|p| p.html.as_str())
                                .unwrap_or(&s.text),
                        )
                    })
                    .collect::<Vec<_>>();
                validate_outputs(&transport, &outputs)?;
                for (segment, output) in input.segments.iter().zip(&mut outputs) {
                    if output.markup.is_some() {
                        return Err("Worker cannot supply trusted mappings".into());
                    }
                    if let Some(plan) = &segment.markup {
                        let restored = plan.restore(&output.text)?;
                        output.markup = Some(std::mem::replace(&mut output.text, restored.text));
                        output.source_text = segment.text.clone();
                    }
                }
                validate_outputs(&expected, &outputs)?;
                Ok(outputs)
            }
            _ => Err("Unexpected Bergamot worker reply".into()),
        }
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
impl TranslationEngine for WorkerEngine {
    fn supports_protected_markup(&self) -> bool {
        true
    }

    fn before_save(&mut self) -> Result<(), String> {
        let result = self
            .exchange(|attempt_id, batch_id| WorkerRequest::Confirm {
                attempt_id,
                batch_id,
            })
            .and_then(|reply| match reply {
                WorkerReply::Confirmed { .. } => Ok(()),
                _ => Err("Bergamot save confirmation missing".into()),
            });
        if result.is_err() {
            self.close();
        }
        result
    }

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

#[cfg(test)]
mod tests;
