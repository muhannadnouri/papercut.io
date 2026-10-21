//! Synthetic documents exercise the shared loader/runner/writer; external assets stay opt-in.
use super::*;
use crate::translation::{
    engine::{TranslationBatchInput, TranslationEngine, TranslationSegmentOutput},
    model_store::{TranslationModelFile, TranslationModelManifest},
    runner::{self, document_tests::fixture},
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

const TRANSLATOR: &str = "opus-mt-fr-en-ctranslate2";
const AUXILIARY: &str = "alignment-awesome-align-int8";

/// A tiny native manifest reaches real model/source admission without downloading a translator.
fn translator(app: &tauri::AppHandle<tauri::test::MockRuntime>) -> TranslationModelManifest {
    let manifest = TranslationModelManifest {
        model_id: TRANSLATOR,
        directory_name: TRANSLATOR,
        source_label: "synthetic",
        source_url: "",
        revision: "",
        installable: true,
        files: &[TranslationModelFile {
            path: "model.bin",
            download_url: None,
            bytes: 3,
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        }],
    };
    let dir =
        crate::translation::model_store::installed_translation_model_dir(app, manifest).unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("model.bin"), b"abc").unwrap();
    manifest
}

/// Abort the real write transaction after save admission; no filesystem permissions or timing tricks.
fn reject_saves(app: &tauri::AppHandle<tauri::test::MockRuntime>) {
    crate::translation::storage::list_translated_documents(app).unwrap();
    let db = crate::document_uploads::open_document_uploads_db(app).unwrap();
    db.execute_batch("CREATE TRIGGER fail_translation BEFORE INSERT ON translated_documents BEGIN SELECT RAISE(ABORT, 'synthetic write failure'); END;").unwrap();
}

struct Engine {
    state: TranslationState,
    mode: &'static str,
    calls: Arc<AtomicUsize>,
    dropped: Arc<AtomicUsize>,
    auxiliary_expected: bool,
}
impl TranslationEngine for Engine {
    fn exact_translator(
        &mut self,
    ) -> Option<&mut dyn crate::translation::inline_markup::pieces::ExactPieceTranslator> {
        Some(self)
    }
    fn translate_batch(
        &mut self,
        input: TranslationBatchInput,
    ) -> Result<Vec<TranslationSegmentOutput>, String> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        assert!(self.state.claim_model_install(TRANSLATOR).is_err());
        if self.auxiliary_expected {
            assert!(self.state.claim_model_install(AUXILIARY).is_err());
        }
        if self.mode == "batch-error" {
            return Err("synthetic inference failure".into());
        }
        if self.mode == "batch-cancel" {
            self.state.cancel("document-job").unwrap();
        }
        Ok(input
            .segments
            .into_iter()
            .map(|s| TranslationSegmentOutput {
                needs_review: false,
                id: s.id,
                source_text: s.text.clone(),
                text: format!("Translated {}", s.text),
                markup: None,
            })
            .collect())
    }
    fn before_save(&mut self) -> Result<(), String> {
        assert!(self.state.claim_model_install(TRANSLATOR).is_err());
        if self.auxiliary_expected {
            assert!(self.state.claim_model_install(AUXILIARY).is_err());
        }
        if self.mode == "save-cancel" {
            self.state.cancel("document-job").unwrap();
        }
        if self.mode == "save-error" {
            return Err("synthetic pre-save failure".into());
        }
        Ok(())
    }
}

impl crate::translation::inline_markup::pieces::ExactPieceTranslator for Engine {
    fn source_fits(&self, _: &str) -> Result<bool, String> {
        Ok(true)
    }

    /// Identity decoding isolates lifecycle from translation quality while retaining all existing failure hooks.
    fn translate_exact(&mut self, sources: &[&str]) -> Result<Vec<String>, String> {
        let input = TranslationBatchInput {
            model_id: TRANSLATOR.into(),
            source_language: "fr".into(),
            target_language: "en".into(),
            quality_mode: "balanced".into(),
            repair_mode: Default::default(),
            glossary: vec![],
            segments: sources
                .iter()
                .enumerate()
                .map(
                    |(i, s)| crate::translation::engine::TranslationSegmentInput {
                        id: i.to_string(),
                        text: (*s).into(),
                        markup: None,
                        context: Default::default(),
                    },
                )
                .collect(),
        };
        self.translate_batch(input)
            .map(|outputs| outputs.into_iter().map(|o| o.source_text).collect())
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        // Native translator destruction must finish before its files become replaceable.
        assert!(self.state.model_operation_active(TRANSLATOR));
        if self.auxiliary_expected {
            assert!(self.state.model_operation_active(AUXILIARY));
        }
        self.dropped.fetch_add(1, Ordering::Relaxed);
    }
}

#[test]
fn runner_releases_ownership_after_load_inference_cancellation_save_and_success() {
    for mode in [
        "load-error",
        "load-cancel",
        "batch-error",
        "batch-cancel",
        "save-error",
        "save-cancel",
        "write-error",
        "ok",
    ] {
        let (_root, app, _, plan) = fixture();
        let state = TranslationState::default();
        let manifest = translator(app.handle());
        if mode == "write-error" {
            reject_saves(app.handle());
        }
        let mut request = plan.request.clone();
        request.model_id = TRANSLATOR.into();
        let calls = Arc::new(AtomicUsize::new(0));
        let dropped = Arc::new(AtomicUsize::new(0));
        let result =
            runner::start_with_engine(app.handle(), &state, request, manifest, |_, _, job| {
                assert_eq!(job.id(), "document-job");
                if mode == "load-error" {
                    return Err("synthetic load failure".into());
                }
                let engine = Engine {
                    state: state.clone(),
                    mode,
                    calls: calls.clone(),
                    dropped: dropped.clone(),
                    auxiliary_expected: false,
                };
                if mode == "load-cancel" {
                    state.cancel(job.id()).unwrap();
                }
                Ok(Box::new(engine))
            });
        assert_eq!(result.is_ok(), mode == "ok", "{mode}: {result:?}");
        if mode == "load-cancel" {
            assert_eq!(calls.load(Ordering::Relaxed), 0);
        }
        assert_eq!(
            dropped.load(Ordering::Relaxed),
            usize::from(mode != "load-error")
        );
        assert!(!state
            .cancelled_jobs
            .lock()
            .unwrap()
            .contains("document-job"));
        assert!(state.claim_model_install(TRANSLATOR).is_ok());
        assert!(state.claim_model_removal(AUXILIARY).is_ok());
        let variants =
            crate::translation::storage::list_translated_documents(app.handle()).unwrap();
        assert_eq!(variants.len(), usize::from(mode == "ok"));
        if mode == "batch-cancel" {
            let plan = crate::translation::job::plan_translation_job(
                crate::translation::types::TranslationStartRequest {
                    model_id: TRANSLATOR.into(),
                    ..plan.request
                },
                ["Bonjour le monde.", "Merci beaucoup.", "Au revoir."],
                900,
                16,
            )
            .unwrap();
            let cache = crate::translation::cache::load_segment_cache(app.handle(), &plan).unwrap();
            assert!(
                cache
                    .translated_text_for(&plan.batches[0].segments[0])
                    .is_none(),
                "late native results must not be reusable"
            );
        }
    }
}
