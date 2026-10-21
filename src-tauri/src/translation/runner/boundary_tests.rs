//! Boundary jobs exercise the actual cache, renderer and atomic writer with synthetic providers.
use super::*;
use crate::translation::engine::TranslationSegmentOutput;
use crate::translation::{
    dom::DomPlan, inline_markup::provider::pieces::ExactPieceTranslator,
    source::TranslationSourceBlock,
};

struct Engine {
    state: TranslationState,
    seen: Vec<String>,
    fail_on: Option<String>,
    cancel_in_call: bool,
    cancel_before_save: bool,
}
impl ExactPieceTranslator for Engine {
    fn source_fits(&self, text: &str) -> Result<bool, String> {
        Ok(text.chars().count() <= 900)
    }
    fn translate_exact(&mut self, sources: &[&str]) -> Result<Vec<String>, String> {
        self.seen.extend(sources.iter().map(|s| s.to_string()));
        if self.cancel_in_call {
            self.state.cancel("document-job")?;
        }
        Ok(sources
            .iter()
            .map(|s| {
                if self.fail_on.as_deref() == Some(*s) {
                    String::new()
                } else {
                    s.to_uppercase()
                }
            })
            .collect())
    }
}
impl TranslationEngine for Engine {
    fn exact_translator(&mut self) -> Option<&mut dyn ExactPieceTranslator> {
        Some(self)
    }
    fn translate_batch(
        &mut self,
        _: TranslationBatchInput,
    ) -> Result<Vec<TranslationSegmentOutput>, String> {
        panic!("boundary work must not use legacy phrase probes")
    }
    fn before_save(&mut self) -> Result<(), String> {
        if self.cancel_before_save {
            self.state.cancel("document-job")?;
        }
        Ok(())
    }
}

/// Model memory and lookup remain mocked; DOM, caches, validation and filesystem writes are real.
#[test]
fn html_epub_boundary_jobs_admit_whole_batches_resume_and_preserve_legacy_assets_and_audio() {
    for format in ["html", "epub"] {
        for language in ["fr", "es"] {
            let (dir, app, mut source, old_plan) = document_tests::fixture();
            let original = document_tests::snapshot(&dir.0);
            let asset = crate::document_uploads::ParsedDocumentAsset::new(
                "image/png",
                b"boundary image".to_vec(),
            )
            .unwrap();
            source.format = format.into();
            source.assets = Arc::new(vec![asset.clone()]);
            source.view_html = format!("<html><body><p><strong id='s'><a role='doc-noteref' href='#note'>1</a>Bonjour <em>été</em><img data-papercut-asset='{}'></strong> <a href='#note'>lien</a> <code>secret_key</code>.</p><p>Merci <i>encore</i>.</p><p id='note' translate='no'>Note.</p><table><tr><td><b>bleu</b></td><td>rouge</td></tr></table></body></html>", asset.file_name);
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
            let request = TranslationStartRequest {
                source_language: language.into(),
                model_id: format!("opus-mt-{language}-en-ctranslate2"),
                ..old_plan.request
            };
            let plan =
                plan_translation_job(request, source.blocks.iter().map(|b| &b.text), 900, 16)
                    .unwrap();
            let state = TranslationState::default();
            let mut engine = Engine {
                state: state.clone(),
                seen: vec![],
                fail_on: Some("encore".into()),
                cancel_in_call: false,
                cancel_before_save: false,
            };
            assert!(finish_translation(
                app.handle(),
                &state,
                &mut engine,
                &plan,
                &source,
                "document-job",
                Instant::now()
            )
            .is_err());
            let cache = load_segment_cache(app.handle(), &plan).unwrap();
            for segment in plan.batches.iter().flat_map(|b| &b.segments) {
                assert!(cache.translated_text_for(segment).is_none());
            }
            assert!(
                super::super::storage::list_translated_documents(app.handle())
                    .unwrap()
                    .is_empty()
            );
            engine.fail_on = None;
            let summary = run_translation_batches(
                app.handle(),
                &state,
                &mut engine,
                &plan,
                &source,
                "document-job",
                Instant::now(),
                None,
            )
            .unwrap_or_else(|e| panic!("{}", e.message));
            let make_request = |boundary, sections| PersistTranslationRequest {
                boundary,
                alignment: None,
                source: source.clone(),
                source_language: language.into(),
                target_language: "en".into(),
                model_id: plan.request.model_id.clone(),
                quality_mode: "balanced".into(),
                repair_mode: Default::default(),
                job_id: "document-job".into(),
                glossary: vec![],
                translated_sections: sections,
            };
            let mut missing = summary.sections.clone();
            for f in missing.iter_mut().flat_map(|s| &mut s.fragments) {
                f.markup = None;
            }
            assert!(persist_translated_document(
                app.handle(),
                make_request(true, missing.clone()),
                || Ok(())
            )
            .is_err());
            // A pre-existing legacy variant and its audio files must remain untouched by the new policy.
            let legacy =
                persist_translated_document(app.handle(), make_request(false, missing), || Ok(()))
                    .unwrap();
            let audio_dir = dir.0.join("audiobooks").join(&legacy.id);
            std::fs::create_dir_all(&audio_dir).unwrap();
            let audio = audio_dir.join("audio.wav");
            let timing = audio_dir.join("timings.json");
            std::fs::write(&audio, b"existing audiobook").unwrap();
            std::fs::write(&timing, b"existing highlighting").unwrap();
            let legacy_html = document_tests::snapshot(&dir.0);
            let calls = engine.seen.len();
            finish_translation(
                app.handle(),
                &state,
                &mut engine,
                &plan,
                &source,
                "document-job",
                Instant::now(),
            )
            .unwrap();
            assert_eq!(calls, engine.seen.len());
            assert!(engine
                .seen
                .iter()
                .all(|s| !s.contains("secret_key") && !s.contains("data-pc")));
            let variants = super::super::storage::list_translated_documents(app.handle()).unwrap();
            assert_eq!(variants.len(), 2);
            let variant = variants.iter().find(|v| v.id != legacy.id).unwrap();
            let saved =
                load_translation_source_document(app.handle(), &variant.document_url).unwrap();
            let dom = DomPlan::new(&saved.view_html);
            assert_eq!(
                dom.document
                    .select_first("strong#s > em")
                    .unwrap()
                    .text_contents(),
                "ÉTÉ"
            );
            assert!(dom.document.select_first("strong#s > img").is_ok());
            assert!(dom
                .document
                .select_first("strong#s > a[role=doc-noteref]")
                .is_ok());
            // Layout outside the reader's selected text blocks remains source-owned and unchanged.
            assert!(source.blocks.iter().all(|b| !b.text.contains("bleu")));
            assert_eq!(
                dom.document
                    .select_first("table td b")
                    .unwrap()
                    .text_contents(),
                "bleu"
            );
            assert!(saved.view_html.contains("<code>secret_key</code>"));
            assert_eq!(saved.assets.as_ref(), source.assets.as_ref());
            for (path, bytes) in original.iter().chain(&legacy_html) {
                assert_eq!(std::fs::read(path).unwrap(), *bytes);
            }
            assert_eq!(std::fs::read(&audio).unwrap(), b"existing audiobook");
            assert_eq!(std::fs::read(&timing).unwrap(), b"existing highlighting");
            let db = crate::document_uploads::open_document_uploads_db(app.handle()).unwrap();
            let settings: String = db
                .query_row(
                    "SELECT settings_json FROM translated_documents WHERE id=?1",
                    [&variant.id],
                    |r| r.get(0),
                )
                .unwrap();
            assert!(settings.contains(super::super::boundary::POLICY));
            // A cached job still reaches the cancellation check immediately before the shared writer.
            let before = document_tests::snapshot(&dir.0);
            engine.cancel_before_save = true;
            assert!(finish_translation(
                app.handle(),
                &state,
                &mut engine,
                &plan,
                &source,
                "document-job",
                Instant::now()
            )
            .is_err());
            assert_eq!(before, document_tests::snapshot(&dir.0));
        }
    }
}

#[test]
fn late_native_cancellation_commits_neither_partial_batch_nor_variant() {
    let (_dir, app, source, mut plan) = document_tests::fixture();
    plan.request.model_id = "opus-mt-fr-en-ctranslate2".into();
    let plan =
        plan_translation_job(plan.request, source.blocks.iter().map(|b| &b.text), 900, 16).unwrap();
    let state = TranslationState::default();
    let mut engine = Engine {
        state: state.clone(),
        seen: vec![],
        fail_on: None,
        cancel_in_call: true,
        cancel_before_save: false,
    };
    let error = finish_translation(
        app.handle(),
        &state,
        &mut engine,
        &plan,
        &source,
        "document-job",
        Instant::now(),
    )
    .unwrap_err();
    assert!(error.contains("cancelled"));
    let cache = load_segment_cache(app.handle(), &plan).unwrap();
    assert!(cache
        .translated_text_for(&plan.batches[0].segments[0])
        .is_none());
    assert!(
        super::super::storage::list_translated_documents(app.handle())
            .unwrap()
            .is_empty()
    );
}
