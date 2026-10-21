//! Exercise mixed cache/fresh replies and the real checkpoint cutoff without model inference.
use super::*;
use crate::translation::{
    dom::DomPlan, engine::TranslationSegmentOutput, inline_markup::protected::ProtectedSource,
    source::TranslationSourceBlock,
};

struct Engine {
    state: TranslationState,
    seen: Vec<String>,
    rich: bool,
    corrupt_last: bool,
    cancel_after_call: bool,
}

impl TranslationEngine for Engine {
    fn supports_protected_markup(&self) -> bool {
        self.rich
    }

    fn translate_batch(
        &mut self,
        input: TranslationBatchInput,
    ) -> Result<Vec<TranslationSegmentOutput>, String> {
        self.seen
            .extend(input.segments.iter().map(|s| s.id.clone()));
        let mut outputs = input
            .segments
            .into_iter()
            .map(|s| TranslationSegmentOutput {
                id: s.id,
                source_text: s.text.clone(),
                text: s.text.to_uppercase(),
                // The synthetic source has no inline codes; protected fragments are still validated by the real contract.
                markup: s.markup.map(|p| p.html.to_uppercase()),
                needs_review: false,
            })
            .collect::<Vec<_>>();
        if self.corrupt_last {
            outputs.last_mut().unwrap().source_text = "wrong echo".into();
        }
        if self.cancel_after_call {
            self.state.cancel("document-job")?;
        }
        Ok(outputs)
    }
}

/// A cached middle fragment must not precede a fresh first fragment, nor certify part of a failed batch.
#[test]
fn mixed_provider_batches_keep_source_order_and_resume_only_after_whole_batch_admission() {
    for rich in [false, true] {
        for cancel in [false, true] {
            let (dir, app, mut source, old) = document_tests::fixture();
            let original =
                load_translation_source_document(app.handle(), &source.document_url).unwrap();
            source.view_html = "<html><body><p>Un. Deux. Trois.</p></body></html>".into();
            source.blocks = DomPlan::new(&source.view_html)
                .blocks
                .into_iter()
                .map(|b| TranslationSourceBlock {
                    ordinal: b.ordinal,
                    is_heading: b.is_heading,
                    heading: b.heading,
                    text: b.inline.text,
                })
                .collect();
            let mut request = old.request;
            request.model_id = if rich {
                "bergamot-fr-en-worker"
            } else {
                "hy-mt2-1.8b-q8"
            }
            .into();
            let plan = plan_translation_job(request, source.blocks.iter().map(|b| &b.text), 6, 16)
                .unwrap();
            let batch = &plan.batches[0];
            assert_eq!(batch.segments.len(), 3);
            let middle = &batch.segments[1];
            let mut cache = load_segment_cache(app.handle(), &plan).unwrap();
            if rich {
                let dom = DomPlan::new(&source.view_html);
                let mapping = ProtectedSource::new(&dom.blocks[0].node)
                    .unwrap()
                    .fragment(middle.source_start..middle.source_end, &middle.text)
                    .unwrap();
                cache.store_mapped_output(
                    middle,
                    &mapping,
                    &TranslationSegmentOutput {
                        id: middle.id.clone(),
                        source_text: middle.text.clone(),
                        text: "DEUX.".into(),
                        markup: Some("DEUX.".into()),
                        needs_review: false,
                    },
                );
            } else {
                cache.store_translation(middle, "DEUX.".into());
            }
            if !rich {
                // An empty text-memory entry is a cache miss, never a completed source fragment.
                let mut json = serde_json::to_value(&cache).unwrap();
                json["memory"]
                    [crate::translation::hash::stable_text_hash(&batch.segments[0].text)] =
                    "".into();
                cache = serde_json::from_value(json).unwrap();
            }
            save_segment_cache(app.handle(), &plan, &cache).unwrap();
            let before = document_tests::snapshot(&dir.0);
            let state = TranslationState::default();
            let mut failed = Engine {
                state: state.clone(),
                seen: vec![],
                rich,
                corrupt_last: !cancel,
                cancel_after_call: cancel,
            };
            let error = match run_translation_batches(
                app.handle(),
                &state,
                &mut failed,
                &plan,
                &source,
                "document-job",
                Instant::now(),
            ) {
                Err(error) => error,
                Ok(_) => panic!("invalid batch was admitted"),
            };
            assert_eq!(error.status, if cancel { "cancelled" } else { "failed" });
            assert_eq!(
                (
                    error.completed_segments,
                    error.cached_segments,
                    error.completed_batches
                ),
                (0, 0, 0)
            );
            assert_eq!(before, document_tests::snapshot(&dir.0));

            let state = TranslationState::default();
            let mut retry = Engine {
                state: state.clone(),
                seen: vec![],
                rich,
                corrupt_last: false,
                cancel_after_call: false,
            };
            let summary = run_translation_batches(
                app.handle(),
                &state,
                &mut retry,
                &plan,
                &source,
                "document-job",
                Instant::now(),
            )
            .unwrap_or_else(|e| panic!("{}", e.message));
            assert_eq!(summary.cached_segments, 1);
            assert_eq!(summary.sections[0].text, "UN. DEUX. TROIS.");
            assert_eq!(
                summary.sections[0]
                    .fragments
                    .iter()
                    .map(|f| f.source_start)
                    .collect::<Vec<_>>(),
                batch
                    .segments
                    .iter()
                    .map(|s| s.source_start)
                    .collect::<Vec<_>>()
            );
            let pending = vec![batch.segments[0].id.clone(), batch.segments[2].id.clone()];
            assert_eq!(retry.seen, pending);
            // Fully cached completion must still reach the shared writer and preserve one stable variant.
            for _ in 0..2 {
                finish_translation(
                    app.handle(),
                    &state,
                    &mut retry,
                    &plan,
                    &source,
                    "document-job",
                    Instant::now(),
                )
                .unwrap();
            }
            assert_eq!(retry.seen, pending);
            let variants = super::super::storage::list_translated_documents(app.handle()).unwrap();
            assert_eq!(variants.len(), 1);
            let saved =
                load_translation_source_document(app.handle(), &variants[0].document_url).unwrap();
            assert_eq!(saved.blocks[0].text, "UN. DEUX. TROIS.");
            assert_eq!(
                original,
                load_translation_source_document(app.handle(), &source.document_url).unwrap()
            );
        }
    }
}

/// Cancelled cache-only work and failed disk writes cannot announce a completed checkpoint.
#[test]
fn checkpoint_rejects_cancellation_and_write_failure_before_advancing_progress() {
    for (cancel, dirty) in [(true, true), (true, false), (false, true)] {
        let (dir, app, _source, plan) = document_tests::fixture();
        let batch = &plan.batches[0];
        let state = TranslationState::default();
        let mut cache = load_segment_cache(app.handle(), &plan).unwrap();
        cache.store_translation(&batch.segments[0], "Staged output".into());
        if cancel {
            state.cancel("document-job").unwrap();
        } else {
            std::fs::write(dir.0.join("translation"), b"blocked cache directory").unwrap();
        }
        let before = document_tests::snapshot(&dir.0);
        let mut run = BatchRunState {
            job_id: "document-job",
            plan: &plan,
            started: Instant::now(),
            completed_segments: 0,
            cached_segments: 0,
            completed_batches: 0,
            current_heading: None,
            preview: String::new(),
        };
        let error = match run.checkpoint(app.handle(), &state, batch, &cache, 1, dirty) {
            Err(error) => error,
            Ok(()) => panic!("checkpoint should fail"),
        };
        assert_eq!(error.status, if cancel { "cancelled" } else { "failed" });
        assert_eq!(
            (
                run.completed_segments,
                run.cached_segments,
                run.completed_batches
            ),
            (0, 0, 0)
        );
        assert_eq!(before, document_tests::snapshot(&dir.0));
    }
}
