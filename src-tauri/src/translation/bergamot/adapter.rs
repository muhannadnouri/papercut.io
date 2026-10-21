//! Bergamot engine admission and protected-markup conversion over the existing worker mailbox.
//! Wire replies echo transport HTML; admitted outputs are rebound to source prose only after restoration.
use std::collections::HashSet;

use super::{Segment, WorkerEngine, WorkerReply, WorkerRequest, MAX_SEGMENTS};
use crate::translation::engine::{
    validate_outputs, TranslationBatchInput, TranslationEngine, TranslationSegmentOutput,
};

const MAX_SOURCE_CHARS: usize = 900;

impl WorkerEngine {
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
                // The worker saw opaque HTML, so a prose echo cannot authenticate its reply.
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
                // Only restored results may rejoin the runner's original-prose contract.
                validate_outputs(&expected, &outputs)?;
                Ok(outputs)
            }
            _ => Err("Unexpected Bergamot worker reply".into()),
        }
    }
}

impl TranslationEngine for WorkerEngine {
    fn supports_protected_markup(&self) -> bool {
        true
    }

    /// Even an all-cache job must confirm its live owner before the writer claims the save.
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

    /// Any adapter rejection closes the attempt; no partly restored batch can be retried on its mailbox.
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
