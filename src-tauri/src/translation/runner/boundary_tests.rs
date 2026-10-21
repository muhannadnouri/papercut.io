//! Boundary jobs exercise the actual cache, renderer and atomic writer with synthetic providers.
use super::*;
use crate::translation::engine::TranslationSegmentOutput;
use crate::translation::{
    dom::DomPlan, inline_markup::pieces::ExactPieceTranslator, source::TranslationSourceBlock,
};
use tauri::Manager;

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
        assert!(sources.len() <= 16);
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

/// Dense nested source identities survive checkpointing and retries without retranslation or duplication.
#[test]
fn dense_boundary_markup_saves_after_partial_resume_then_reuses_the_complete_cache() {
    for format in ["html", "epub"] {
        for language in ["fr", "es"] {
            let (dir, app, mut source, old) = document_tests::fixture();
            let dense = (0..48)
                .map(|i| format!("Before <strong id='s{i}'><em>term</em></strong> after. "))
                .collect::<String>();
            source.format = format.into();
            source.view_html = format!("<html><body><p><span id='whole'><a id='ref' role='doc-noteref' href='#note'>1</a>{dense}</span></p><p>Merci.</p><p id='note' translate='no'>Note.</p></body></html>");
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
            let plan = plan_translation_job(
                TranslationStartRequest {
                    source_language: language.into(),
                    model_id: format!("opus-mt-{language}-en-ctranslate2"),
                    ..old.request
                },
                source.blocks.iter().map(|b| &b.text),
                900,
                1,
            )
            .unwrap();
            let state = TranslationState::default();
            let mut engine = Engine {
                state: state.clone(),
                seen: vec![],
                fail_on: Some("Merci.".into()),
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
            assert_eq!(engine.seen.len(), 49);
            assert_eq!(engine.seen[0], "Before term after.");
            let cache = load_segment_cache(app.handle(), &plan).unwrap();
            assert_eq!(
                cache
                    .translated_text_for(&plan.batches[0].segments[0])
                    .unwrap(),
                source.blocks[0].text.to_uppercase()
            );
            assert!(cache
                .translated_text_for(&plan.batches[1].segments[0])
                .is_none());
            assert!(
                super::super::storage::list_translated_documents(app.handle())
                    .unwrap()
                    .is_empty()
            );
            engine.fail_on = None;
            engine.seen.clear();
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
            assert_eq!(engine.seen, ["Merci."]);
            let variants = super::super::storage::list_translated_documents(app.handle()).unwrap();
            assert_eq!(variants.len(), 1);
            assert_eq!(variants[0].status, "needs-review");
            let saved =
                load_translation_source_document(app.handle(), &variants[0].document_url).unwrap();
            assert_eq!(saved.blocks[0].text, source.blocks[0].text.to_uppercase());
            let dom = DomPlan::new(&saved.view_html);
            assert_eq!(dom.document.select("span#whole").unwrap().count(), 1);
            assert_eq!(
                dom.document
                    .select("span#whole > strong > em")
                    .unwrap()
                    .count(),
                48
            );
            assert_eq!(
                dom.document.select("span#whole > a#ref").unwrap().count(),
                1
            );
            for i in 0..48 {
                assert_eq!(
                    dom.document
                        .select(&format!("strong#s{i}"))
                        .unwrap()
                        .count(),
                    1
                );
                assert_eq!(
                    dom.document
                        .select_first(&format!("strong#s{i} > em"))
                        .unwrap()
                        .text_contents(),
                    "term"
                );
            }
            let before = document_tests::snapshot(&dir.0);
            engine.seen.clear();
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
            assert!(engine.seen.is_empty());
            assert_eq!(before, document_tests::snapshot(&dir.0));
        }
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
                fail_on: Some("Merci encore.".into()),
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
            )
            .unwrap_or_else(|e| panic!("{}", e.message));
            let make_request = |boundary, sections| PersistTranslationRequest {
                formatting: if boundary {
                    FormattingPolicy::SentenceReview
                } else {
                    FormattingPolicy::Provider
                },

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
                f.needs_review = false;
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
            let response = finish_translation(
                app.handle(),
                &state,
                &mut engine,
                &plan,
                &source,
                "document-job",
                Instant::now(),
            )
            .unwrap();
            assert_eq!(response.status, "needs-review");
            assert_eq!(calls, engine.seen.len());
            assert!(engine
                .seen
                .iter()
                .all(|s| !s.contains("secret_key") && !s.contains("data-pc")));
            let variants = super::super::storage::list_translated_documents(app.handle()).unwrap();
            assert_eq!(variants.len(), 2);
            let variant = variants.iter().find(|v| v.id != legacy.id).unwrap();
            assert_eq!(variant.status, "needs-review");
            let saved =
                load_translation_source_document(app.handle(), &variant.document_url).unwrap();
            let dom = DomPlan::new(&saved.view_html);
            assert_eq!(
                dom.document
                    .select_first("strong#s > em")
                    .unwrap()
                    .text_contents(),
                "été"
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
            assert!(settings.contains(super::super::formatting::SENTENCE_REVIEW_POLICY));
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

/// Retired whole-fragment caches must not resume into sentence-first jobs; only complete batches checkpoint.
#[test]
fn sentence_cache_policy_retires_fragment_policies_and_resumes_review_batches() {
    use crate::translation::{hash::StableHasher, inline_markup::protected::ProtectedSource};
    for format in ["html", "epub"] {
        for (language, retired_policy) in [
            ("fr", "ctranslate2-source-boundaries-v1"),
            ("es", "ctranslate2-source-boundaries-v1"),
            ("fr", "ctranslate2-source-boundaries-v2"),
            ("es", "ctranslate2-source-boundaries-v2"),
            ("fr", "ctranslate2-source-boundaries-v3"),
            ("es", "ctranslate2-source-boundaries-v3"),
        ] {
            let (_dir, app, mut source, old) = document_tests::fixture();
            source.format = format.into();
            source.view_html = "<html><body><p>«<strong>Bonjour. Merci.</strong>» !</p><p><em>Encore. Demain.</em> ?</p></body></html>".into();
            let dom = DomPlan::new(&source.view_html);
            source.blocks = dom
                .blocks
                .iter()
                .map(|b| TranslationSourceBlock {
                    ordinal: b.ordinal,
                    is_heading: b.is_heading,
                    heading: b.heading.clone(),
                    text: b.inline.text.clone(),
                })
                .collect();
            let plan = plan_translation_job(
                TranslationStartRequest {
                    source_language: language.into(),
                    model_id: format!("opus-mt-{language}-en-ctranslate2"),
                    ..old.request
                },
                source.blocks.iter().map(|b| &b.text),
                900,
                1,
            )
            .unwrap();
            let segment = &plan.batches[0].segments[0];
            let mapping = ProtectedSource::for_boundaries(&dom.blocks[0].node)
                .unwrap()
                .fragment(segment.source_start..segment.source_end, &segment.text)
                .unwrap();
            // Freeze the retired key, rather than deriving it from the new policy constant under test.
            let mut key = StableHasher::new();
            for field in [
                plan.request.document_url.as_str(),
                language,
                "en",
                plan.request.model_id.as_str(),
                "balanced",
                plan.request.repair_mode.label(),
                retired_policy,
            ] {
                key.write_str(field);
            }
            let mut retired_plan = plan.clone();
            retired_plan.cache_key = key.finish_hex();
            assert_ne!(retired_plan.cache_key, plan.cache_key);
            let mut retired_cache = load_segment_cache(app.handle(), &retired_plan).unwrap();
            retired_cache.store_mapped_output(segment, &mapping, &TranslationSegmentOutput { needs_review: false,
                id: segment.id.clone(), source_text: segment.text.clone(),
                text: "\"OLD\" - I'm sorry, I'm sorry.".into(),
                markup: Some("&quot;<span data-pc='0'>OLD</span>&quot; - I&apos;m sorry, I&apos;m sorry.".into())
            });
            assert!(retired_cache.mapped_output_for(segment, &mapping).is_some());
            save_segment_cache(app.handle(), &retired_plan, &retired_cache).unwrap();
            let state = TranslationState::default();
            let mut engine = Engine {
                state: state.clone(),
                seen: vec![],
                fail_on: Some("Demain. ?".into()),
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
            assert_eq!(
                engine.seen,
                ["«Bonjour.", "Merci.» !", "Encore.", "Demain. ?"]
            );
            let cache = load_segment_cache(app.handle(), &plan).unwrap();
            assert_eq!(
                cache
                    .sentence_output_for(segment, &mapping, true)
                    .unwrap()
                    .text,
                "«BONJOUR. MERCI.» !"
            );
            assert!(cache
                .translated_text_for(&plan.batches[1].segments[0])
                .is_none());
            assert!(
                super::super::storage::list_translated_documents(app.handle())
                    .unwrap()
                    .is_empty()
            );
            engine.seen.clear();
            engine.fail_on = None;
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
            assert_eq!(engine.seen, ["Encore.", "Demain. ?"]);
            let variants = super::super::storage::list_translated_documents(app.handle()).unwrap();
            assert_eq!(variants.len(), 1);
            assert_eq!(variants[0].status, "needs-review");
            let saved =
                load_translation_source_document(app.handle(), &variants[0].document_url).unwrap();
            assert_eq!(
                saved
                    .blocks
                    .iter()
                    .map(|b| b.text.as_str())
                    .collect::<Vec<_>>(),
                ["«BONJOUR. MERCI.» !", "ENCORE. DEMAIN. ?"]
            );
            let parsed = DomPlan::new(&saved.view_html);
            assert_eq!(
                parsed
                    .document
                    .select_first("p[data-papercut-translation-review=original] > strong")
                    .unwrap()
                    .text_contents(),
                "Bonjour. Merci."
            );
            assert_eq!(
                parsed.document.select_first("em").unwrap().text_contents(),
                "Encore. Demain."
            );
            let before = document_tests::snapshot(&_dir.0);
            engine.seen.clear();
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
            assert!(engine.seen.is_empty());
            assert_eq!(before, document_tests::snapshot(&_dir.0));
            assert_eq!(
                load_segment_cache(app.handle(), &retired_plan)
                    .unwrap()
                    .mapped_output_for(segment, &mapping)
                    .unwrap()
                    .text,
                "\"OLD\" - I'm sorry, I'm sorry."
            );
        }
    }
}

/// Exercise the actual disk loader: policy comes from the admitted plan, never a JSON field or markup presence.
#[test]
fn sentence_review_disk_cache_rebinds_policy_without_changing_schema_or_strict_admission() {
    use crate::translation::{dom::DomPlan, inline_markup::protected::ProtectedSource};
    let (_dir, app, _source, old) = document_tests::fixture();
    for (model, language) in [
        ("opus-mt-fr-en-ctranslate2", "fr"),
        ("opus-mt-es-en-ctranslate2", "es"),
    ] {
        let mut request = old.request.clone();
        request.model_id = model.into();
        request.source_language = language.into();
        let plan = plan_translation_job(request.clone(), ["Un mot."], 900, 16).unwrap();
        let segment = &plan.batches[0].segments[0];
        let dom = DomPlan::new("<p>Un <em>mot</em>.</p>");
        let mapping = ProtectedSource::for_boundaries(&dom.blocks[0].node)
            .unwrap()
            .fragment(segment.source_start..segment.source_end, &segment.text)
            .unwrap();
        let output = TranslationSegmentOutput {
            id: segment.id.clone(),
            source_text: segment.text.clone(),
            text: "A word.".into(),
            markup: None,
            needs_review: true,
        };
        let mut cache = load_segment_cache(app.handle(), &plan).unwrap();
        cache.store_mapped_output(segment, &mapping, &output);
        let mut json = serde_json::to_value(&cache).unwrap();
        assert!(json.get("formatting").is_none());
        // Retired receipts, including unparseable old shapes, must neither block normal resume nor supply prose.
        json["aligned"] = serde_json::json!({ segment.id.clone(): { "binding": { "target": "Untrusted retired target" } } });
        save_segment_cache(app.handle(), &plan, &cache).unwrap();
        let cache_path = app
            .path()
            .app_data_dir()
            .unwrap()
            .join("translation/segment-cache")
            .join(&plan.cache_key)
            .join("segments.json");
        std::fs::write(&cache_path, serde_json::to_vec(&json).unwrap()).unwrap();
        let resumed = load_segment_cache(app.handle(), &plan).unwrap();
        assert!(
            resumed
                .sentence_output_for(segment, &mapping, true)
                .unwrap()
                .needs_review
        );
        let mut retired_only = json.clone();
        retired_only["segments"] = serde_json::json!({});
        retired_only["memory"] = serde_json::json!({});
        std::fs::write(&cache_path, serde_json::to_vec(&retired_only).unwrap()).unwrap();
        let retired_only = load_segment_cache(app.handle(), &plan).unwrap();
        assert!(retired_only
            .sentence_output_for(segment, &mapping, true)
            .is_none());
        assert!(retired_only.translated_text_for_source(segment).is_none());
        std::fs::write(&cache_path, serde_json::to_vec(&json).unwrap()).unwrap();
        request.preserve_formatting = true;
        let strict = plan_translation_job(request, ["Un mot."], 900, 16).unwrap();
        assert_ne!(strict.cache_key, plan.cache_key);
        let strict_cache = load_segment_cache(app.handle(), &strict).unwrap();
        assert!(strict_cache
            .sentence_output_for(segment, &mapping, true)
            .is_none());
        let resumed = load_segment_cache(app.handle(), &plan).unwrap();
        assert_eq!(
            resumed
                .sentence_output_for(segment, &mapping, true)
                .unwrap()
                .text,
            "A word."
        );
    }
}
