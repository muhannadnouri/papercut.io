//! Exercise the real batch cache, renderer and SQLite/file writer with a deterministic worker.
use super::*;
use crate::translation::bergamot::{Failure, WorkerReply, WorkerRequest};
use crate::translation::engine::TranslationSegmentOutput;
use crate::translation::source::TranslationSourceBlock;
use tauri::Manager;

pub(super) struct Fixture(pub(super) std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        let mut nonce = [0u8; 16];
        getrandom::fill(&mut nonce).unwrap();
        let path = std::env::temp_dir().join(format!("papercut-bergamot-document-{nonce:x?}"));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(super) fn fixture() -> (
    Fixture,
    tauri::App<tauri::test::MockRuntime>,
    TranslationSourceDocument,
    TranslationJobPlan,
) {
    let dir = Fixture::new();
    let mut context = tauri::test::mock_context(tauri::test::noop_assets());
    // An absolute identifier keeps every real cache/database/file write under this fixture.
    context.config_mut().identifier = dir.0.as_path().to_str().unwrap().into();
    let app = tauri::test::mock_builder().build(context).unwrap();
    assert_eq!(app.path().app_data_dir().unwrap(), dir.0.as_path());
    let html =
        "<html><body><p>Bonjour le monde.</p><p>Merci beaucoup.</p><p>Au revoir.</p></body></html>";
    let source = TranslationSourceDocument {
        document_id: "abc123".into(),
        document_url: "/uploads/abc123.html".into(),
        title: "Example".into(),
        format: "html".into(),
        view_html: html.into(),
        assets: Default::default(),
        blocks: ["Bonjour le monde.", "Merci beaucoup.", "Au revoir."]
            .iter()
            .enumerate()
            .map(|(ordinal, text)| TranslationSourceBlock {
                ordinal,
                is_heading: false,
                heading: None,
                text: (*text).into(),
            })
            .collect(),
    };
    let request = TranslationStartRequest {
        job_id: Some("document-job".into()),
        document_url: source.document_url.clone(),
        source_language: "fr".into(),
        target_language: "en".into(),
        model_id: "bergamot-fr-en-worker".into(),
        quality_mode: "balanced".into(),
        repair_mode: TranslationRepairMode::Off,
        use_hardware_acceleration: false,
        preserve_formatting: false,
        glossary: vec![],
    };
    let plan = plan_translation_job(
        request,
        source.blocks.iter().map(|b| b.text.as_str()),
        900,
        1,
    )
    .unwrap();
    let mut db = crate::document_uploads::open_document_uploads_db(app.handle()).unwrap();
    crate::document_uploads::persist_derived_document(
        app.handle(),
        &mut db,
        &source.document_id,
        &source.document_url,
        &source.title,
        "html",
        source.view_html.clone(),
        &source.assets,
        1,
        |_| Ok(()),
    )
    .unwrap();
    (dir, app, source, plan)
}

fn worker(
    state: &TranslationState,
    fail_batch: usize,
    at_save: &'static str,
    seen: Arc<Mutex<Vec<String>>>,
) -> super::super::bergamot::WorkerEngine {
    let host = state.clone();
    // The transport callback is Fn; use a counter shared with the deterministic host.
    let counter = Arc::new(Mutex::new(0usize));
    state
        .bergamot
        .open(
            "main".into(),
            "document-job".into(),
            state.cancelled_jobs.clone(),
            move |message| {
                let reply = match message {
                    WorkerRequest::Batch {
                        attempt_id,
                        batch_id,
                        segments,
                        ..
                    } => {
                        let mut count = counter.lock().unwrap();
                        *count += 1;
                        seen.lock()
                            .unwrap()
                            .extend(segments.iter().map(|s| s.id.clone()));
                        if *count == fail_batch {
                            WorkerReply::Failed {
                                attempt_id,
                                batch_id,
                                reason: Failure::WorkerLost,
                            }
                        } else {
                            WorkerReply::Result {
                                attempt_id,
                                batch_id,
                                outputs: segments
                                    .into_iter()
                                    .map(|s| TranslationSegmentOutput {
                                        needs_review: false,
                                        markup: None,
                                        id: s.id,
                                        text: match s.text.as_str() {
                                            "Bonjour le monde." => "Hello world.",
                                            "Merci beaucoup." => "Thank you very much.",
                                            _ => "Goodbye.",
                                        }
                                        .into(),
                                        source_text: s.text,
                                    })
                                    .collect(),
                            }
                        }
                    }
                    WorkerRequest::Confirm {
                        attempt_id,
                        batch_id,
                    } => {
                        seen.lock().unwrap().push("confirm".into());
                        if at_save == "cancel" {
                            host.cancel("document-job").unwrap();
                        }
                        if at_save == "lost" {
                            WorkerReply::Failed {
                                attempt_id,
                                batch_id,
                                reason: Failure::WorkerLost,
                            }
                        } else {
                            WorkerReply::Confirmed {
                                attempt_id,
                                batch_id,
                            }
                        }
                    }
                    WorkerRequest::Stop { .. } => return Ok(()),
                };
                host.bergamot.reply("main", reply).map(|_| ())
            },
        )
        .unwrap()
}

#[test]
fn failed_later_batch_resumes_only_completed_work_and_stores_one_provenanced_variant() {
    let (_dir, app, source, plan) = fixture();
    let state = TranslationState::default();
    let seen = Arc::new(Mutex::new(vec![]));
    let mut engine = worker(&state, 2, "ok", seen.clone());
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
    drop(engine);
    let cache = load_segment_cache(app.handle(), &plan).unwrap();
    assert!(cache
        .translated_text_for(&plan.batches[0].segments[0])
        .is_some());
    assert!(cache
        .translated_text_for(&plan.batches[1].segments[0])
        .is_none());
    assert!(
        super::super::storage::list_translated_documents(app.handle())
            .unwrap()
            .is_empty()
    );
    seen.lock().unwrap().clear();
    let mut engine = worker(&state, usize::MAX, "ok", seen.clone());
    let stored = finish_translation(
        app.handle(),
        &state,
        &mut engine,
        &plan,
        &source,
        "document-job",
        Instant::now(),
    )
    .unwrap();
    assert_eq!(stored.status, "stored");
    assert_eq!(
        *seen.lock().unwrap(),
        vec![
            plan.batches[1].segments[0].id.clone(),
            plan.batches[2].segments[0].id.clone(),
            "confirm".into()
        ]
    );
    let db = crate::document_uploads::open_document_uploads_db(app.handle()).unwrap();
    let (engine_id, settings): (String, String) = db
        .query_row(
            "SELECT engine_id, settings_json FROM translated_documents",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(engine_id, "bergamot");
    assert!(settings.contains("bergamot-protected-codes-v6:"));
    assert_eq!(
        super::super::storage::list_translated_documents(app.handle())
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn fully_cached_jobs_require_live_confirmation_and_cancel_before_save_preserves_existing_variant() {
    let (_dir, app, source, plan) = fixture();
    let state = TranslationState::default();
    let seen = Arc::new(Mutex::new(vec![]));
    let mut engine = worker(&state, usize::MAX, "ok", seen.clone());
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
    drop(engine);
    let before = super::super::storage::list_translated_documents(app.handle()).unwrap();
    let root = app.path().app_data_dir().unwrap().join("document_uploads");
    let files_before = snapshot(&root);
    for mode in ["lost", "cancel"] {
        seen.lock().unwrap().clear();
        let mut engine = worker(&state, usize::MAX, mode, seen.clone());
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
        assert!(
            error.contains(if mode == "lost" { "lost" } else { "cancelled" }),
            "{error}"
        );
        assert_eq!(*seen.lock().unwrap(), vec!["confirm"]);
        assert_eq!(
            super::super::storage::list_translated_documents(app.handle()).unwrap()[0].id,
            before[0].id
        );
        assert_eq!(snapshot(&root), files_before);
        drop(engine);
        state.cancelled_jobs.lock().unwrap().clear();
    }
}

pub(super) fn snapshot(root: &std::path::Path) -> BTreeMap<std::path::PathBuf, Vec<u8>> {
    let mut entries = BTreeMap::new();
    for entry in std::fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            entries.extend(snapshot(&path));
        } else if path.extension().is_some_and(|ext| ext == "html") {
            entries.insert(path.clone(), std::fs::read(path).unwrap());
        }
    }
    entries
}

#[test]
fn inline_probes_use_bounded_requests_and_resume_without_more_inference() {
    struct Echo {
        sizes: Vec<usize>,
    }
    impl TranslationEngine for Echo {
        fn translate_batch(
            &mut self,
            input: TranslationBatchInput,
        ) -> Result<Vec<TranslationSegmentOutput>, String> {
            assert!(input.segments.len() <= 16);
            self.sizes.push(input.segments.len());
            Ok(input
                .segments
                .into_iter()
                .map(|s| TranslationSegmentOutput {
                    needs_review: false,
                    markup: None,
                    id: s.id,
                    source_text: s.text.clone(),
                    text: format!("Translated {}", s.text),
                })
                .collect())
        }
    }
    let (_dir, app, source, mut plan) = fixture();
    let texts = (0..16).map(|n| format!("Les mots importants du paragraphe {n} doivent rester lisibles et correctement traduits.")).collect::<Vec<_>>();
    plan = plan_translation_job(plan.request, texts.iter().map(String::as_str), 900, 16).unwrap();
    let probes = texts
        .iter()
        .enumerate()
        .map(|(block, _)| {
            (0..8)
                .map(|n| InlinePhraseProbe {
                    source_start: n,
                    source_end: n + 1,
                    text: format!("mot-{block}-{n}"),
                })
                .collect()
        })
        .collect::<Vec<_>>();
    let mut cache = load_segment_cache(app.handle(), &plan).unwrap();
    let mut engine = Echo { sizes: vec![] };
    let (hints, stored) =
        translate_inline_phrase_hints(&mut engine, &plan, &plan.batches[0], &probes, &mut cache);
    assert!(stored);
    assert_eq!(engine.sizes, vec![16; 8]);
    assert_eq!(hints.len(), 16);
    assert!(hints.values().all(|hints| hints.len() == 8));
    save_segment_cache(app.handle(), &plan, &cache).unwrap();
    let mut resumed = load_segment_cache(app.handle(), &plan).unwrap();
    let (again, stored) =
        translate_inline_phrase_hints(&mut engine, &plan, &plan.batches[0], &probes, &mut resumed);
    assert!(!stored);
    assert_eq!(engine.sizes.len(), 8);
    assert_eq!(again.len(), hints.len());
    // A native adapter has no remote save checkpoint and still uses the shared writer.
    let native = plan_translation_job(
        TranslationStartRequest {
            model_id: "hy-mt2-1.8b-q8".into(),
            ..plan.request
        },
        source.blocks.iter().map(|b| b.text.as_str()),
        900,
        16,
    )
    .unwrap();
    finish_translation(
        app.handle(),
        &TranslationState::default(),
        &mut engine,
        &native,
        &source,
        "native-job",
        Instant::now(),
    )
    .unwrap();
    let db = crate::document_uploads::open_document_uploads_db(app.handle()).unwrap();
    let engine: String = db
        .query_row("SELECT engine_id FROM translated_documents", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(engine, "llama.cpp");
}

#[test]
fn cancellation_after_last_batch_survives_validation_and_never_creates_a_variant() {
    use tauri::Listener;
    let (_dir, app, source, plan) = fixture();
    let state = TranslationState::default();
    let cancel = state.clone();
    let listener = app.listen(TRANSLATION_JOB_PROGRESS_EVENT, move |event| {
        let progress: serde_json::Value = serde_json::from_str(event.payload()).unwrap();
        if progress["status"] == "validating" {
            cancel.cancel("document-job").unwrap();
        }
    });
    let seen = Arc::new(Mutex::new(vec![]));
    let mut engine = worker(&state, usize::MAX, "ok", seen.clone());
    assert!(finish_translation(
        app.handle(),
        &state,
        &mut engine,
        &plan,
        &source,
        "document-job",
        Instant::now()
    )
    .unwrap_err()
    .contains("cancelled"));
    assert!(!seen.lock().unwrap().contains(&"confirm".into()));
    let cache = load_segment_cache(app.handle(), &plan).unwrap();
    assert!(cache
        .translated_text_for(&plan.batches[2].segments[0])
        .is_some());
    assert!(
        super::super::storage::list_translated_documents(app.handle())
            .unwrap()
            .is_empty()
    );
    app.unlisten(listener);
}

#[test]
fn worker_document_preflight_rejects_options_it_cannot_honor_before_opening_an_engine() {
    let (_dir, app, _source, plan) = fixture();
    let state = TranslationState::default();
    for option in [
        "model", "source", "target", "quality", "repair", "hardware", "glossary",
    ] {
        let mut request = plan.request.clone();
        match option {
            "model" => request.model_id = "opus-mt-fr-en-ctranslate2".into(),
            "source" => request.source_language = "es".into(),
            "target" => request.target_language = "ar".into(),
            "quality" => request.quality_mode = "quality".into(),
            "repair" => request.repair_mode = TranslationRepairMode::Chapter,
            "hardware" => request.use_hardware_acceleration = true,
            _ => request.glossary.push(TranslationGlossaryEntry {
                source: "bonjour".into(),
                target: "hello".into(),
                note: None,
            }),
        }
        let error =
            start_bergamot_translation(app.handle(), &state, request, "main".into(), |_| {
                panic!("invalid request reached worker")
            })
            .unwrap_err();
        assert_eq!(
            super::super::types::TranslationCommandError::new(error).code,
            "unsupported-translation-option"
        );
    }
}

/// Compiling a worker cannot override product admission on either public job path.
#[test]
fn worker_catalog_requires_product_acceptance_on_both_job_paths() {
    let (_dir, app, _source, plan) = fixture();
    let state = TranslationState::default();
    let error = start_translation(app.handle(), &state, plan.request.clone()).unwrap_err();
    assert!(error.contains("provider acceptance pending"));
    let error =
        start_bergamot_translation(app.handle(), &state, plan.request, "main".into(), |_| {
            panic!("disabled worker reached inference")
        })
        .unwrap_err();
    assert!(error.contains("provider acceptance pending"));
}

/// Exercise the real bridge and writer with deterministic protected-code replies, including a bad later result.
#[test]
fn rich_document_validates_entire_batch_before_cache_then_resumes_source_owned_markup() {
    for format in ["html", "epub"] {
        let (_dir, app, mut source, old_plan) = fixture();
        let original =
            load_translation_source_document(app.handle(), &source.document_url).unwrap();
        let asset = crate::document_uploads::ParsedDocumentAsset::new(
            "image/png",
            b"rich job image bytes".to_vec(),
        )
        .unwrap();
        source.assets = Arc::new(vec![asset.clone()]);
        source.format = format.into();
        source.view_html = "<html><body><p>La bague en <strong>or</strong> est perdue.</p><p>Gardez <code>private_id</code> et <span translate='no'>42</span><a role='doc-noteref' href='#note'><sup>1</sup></a>.</p><p><strong>Le chien</strong> bleu.</p><p id='note' translate='no'>Note originale.</p></body></html>".into();
        source.view_html = source.view_html.replace(
            "</a>.</p>",
            &format!("</a><img data-papercut-asset='{}'>.</p>", asset.file_name),
        );
        source.blocks = super::super::dom::DomPlan::new(&source.view_html)
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
            old_plan.request,
            source.blocks.iter().map(|b| &b.text),
            900,
            16,
        )
        .unwrap();
        let state = TranslationState::default();
        let seen = Arc::new(Mutex::new(Vec::new()));
        #[derive(Clone, Copy)]
        enum ReplyMode {
            Valid,
            MissingCode,
            Expansion,
        }
        let make_worker = |mode: ReplyMode| {
            let host = state.clone();
            let seen = seen.clone();
            state.bergamot.open("main".into(), "document-job".into(), state.cancelled_jobs.clone(), move |request| {
            let reply = match request {
                WorkerRequest::Batch { attempt_id, batch_id, segments, .. } => {
                    seen.lock().unwrap().extend(segments.iter().map(|s| s.id.clone()));
                    let outputs = segments.into_iter().map(|s| {
                        assert!(s.html);
                        for secret in ["private_id", "42", "#note", "data-papercut-asset"] { assert!(!s.text.contains(secret)); }
                        let target = if s.id == "b0:s0" && matches!(mode, ReplyMode::Expansion) {
                            "<span data-pc='0'>x</span>".repeat(1000)
                        } else { (if s.id == "b0:s0" { "The <span data-pc='0'>gold</span> ring is lost." }
                            else if s.id == "b2:s0" { "<span data-pc='0'>The</span> blue <span data-pc='0'>dog</span>." }
                            else if matches!(mode, ReplyMode::MissingCode) { "Keep a private identifier and the number." }
                            else { "<span data-pc='2' translate='no'></span>Keep <span data-pc='0' translate='no'>PC0</span> and <span data-pc='1' translate='no'>PC1</span><span data-pc='3' translate='no'></span>." }).to_string() };
                        TranslationSegmentOutput { needs_review: false, id: s.id, source_text: s.text, text: target.into(), markup: None }
                    }).collect();
                    WorkerReply::Result { attempt_id, batch_id, outputs }
                },
                WorkerRequest::Confirm { attempt_id, batch_id } => WorkerReply::Confirmed { attempt_id, batch_id },
                WorkerRequest::Stop { .. } => return Ok(()),
            };
            host.bergamot.reply("main", reply).map(|_| ())
        }).unwrap()
        };
        let mut invalid = make_worker(ReplyMode::MissingCode);
        assert!(finish_translation(
            app.handle(),
            &state,
            &mut invalid,
            &plan,
            &source,
            "document-job",
            Instant::now()
        )
        .is_err());
        drop(invalid);
        let cache = load_segment_cache(app.handle(), &plan).unwrap();
        assert!(cache
            .translated_text_for(&plan.batches[0].segments[0])
            .is_none());
        assert!(
            super::super::storage::list_translated_documents(app.handle())
                .unwrap()
                .is_empty()
        );
        let mut valid = make_worker(ReplyMode::Valid);
        finish_translation(
            app.handle(),
            &state,
            &mut valid,
            &plan,
            &source,
            "document-job",
            Instant::now(),
        )
        .unwrap();
        drop(valid);
        let variants = super::super::storage::list_translated_documents(app.handle()).unwrap();
        let saved =
            load_translation_source_document(app.handle(), &variants[0].document_url).unwrap();
        assert!(saved.view_html.contains("<strong>gold</strong>"));
        // Disjoint formatting survives adapter validation, cache replay and source-owned rendering.
        assert!(saved
            .view_html
            .contains("<strong>The</strong> blue <strong>dog</strong>."));
        assert!(saved.view_html.contains("<code>private_id</code>"));
        assert!(saved.view_html.contains("href=\"#note\""));
        assert!(saved.view_html.contains("Note originale."));
        let saved_dom = super::super::html::parse_html_document(&saved.view_html);
        assert_eq!(
            saved_dom
                .select("p > span[translate='no'] + a[role='doc-noteref'] + img")
                .unwrap()
                .count(),
            1
        );
        assert!(!saved.view_html.contains("data-pc"));
        assert!(!saved.view_html.contains("PC0"));
        assert_eq!(saved.assets.as_ref(), &vec![asset]);
        seen.lock().unwrap().clear();
        let mut resumed = make_worker(ReplyMode::Valid);
        finish_translation(
            app.handle(),
            &state,
            &mut resumed,
            &plan,
            &source,
            "document-job",
            Instant::now(),
        )
        .unwrap();
        drop(resumed);
        assert!(seen.lock().unwrap().is_empty());
        let replayed =
            load_translation_source_document(app.handle(), &variants[0].document_url).unwrap();
        assert_eq!(saved.view_html, replayed.view_html);
        assert_eq!(
            super::super::storage::list_translated_documents(app.handle())
                .unwrap()
                .len(),
            1
        );
        // A new malformed attempt must leave the previous completed variant intact.
        let mut changed = source.clone();
        changed.view_html = source
            .view_html
            .replace("<strong>or</strong>", "<em>or</em>");
        let mut invalid = make_worker(ReplyMode::MissingCode);
        // The invalid second block is cached, so change both source-owned structures to force both results.
        changed.view_html = changed.view_html.replace(
            "<code>private_id</code>",
            "<code id='literal'>private_id</code>",
        );
        assert!(finish_translation(
            app.handle(),
            &state,
            &mut invalid,
            &plan,
            &changed,
            "document-job",
            Instant::now()
        )
        .is_err());
        drop(invalid);
        let unchanged =
            load_translation_source_document(app.handle(), &variants[0].document_url).unwrap();
        assert_eq!(saved.view_html, unchanged.view_html);
        // A source-owned long attribute must not amplify a small reply or replace an existing variant/cache.
        let before =
            serde_json::to_value(load_segment_cache(app.handle(), &plan).unwrap()).unwrap();
        let mut expanded = source.clone();
        expanded.view_html = source.view_html.replace(
            "<strong>or</strong>",
            &format!("<strong title='{}'>or</strong>", "x".repeat(8192)),
        );
        let mut worker = make_worker(ReplyMode::Expansion);
        let error = finish_translation(
            app.handle(),
            &state,
            &mut worker,
            &plan,
            &expanded,
            "document-job",
            Instant::now(),
        )
        .unwrap_err();
        assert!(error.contains("reconstruction"), "{error}");
        drop(worker);
        assert_eq!(
            before,
            serde_json::to_value(load_segment_cache(app.handle(), &plan).unwrap()).unwrap()
        );
        assert_eq!(
            saved.view_html,
            load_translation_source_document(app.handle(), &variants[0].document_url)
                .unwrap()
                .view_html
        );
        let unchanged_original =
            load_translation_source_document(app.handle(), &source.document_url).unwrap();
        assert_eq!(original, unchanged_original);
    }
}

/// Exercise provider planning through the real protected DOM, writer and cache for both source formats.
#[test]
fn segmented_worker_document_keeps_nested_markup_assets_quote_notes_and_cache_resume() {
    for format in ["html", "epub"] {
        let (_dir, app, mut source, old_plan) = fixture();
        let original =
            load_translation_source_document(app.handle(), &source.document_url).unwrap();
        let asset = crate::document_uploads::ParsedDocumentAsset::new(
            "image/png",
            b"segmented quote image".to_vec(),
        )
        .unwrap();
        source.assets = Arc::new(vec![asset.clone()]);
        source.format = format.into();
        let prose = format!(
            "{}: {}; {}. Fin.",
            "Une introduction avec café 🙂 ".repeat(12),
            "Un développement avec des archives ".repeat(12),
            "Une conclusion sur ces archives ".repeat(12)
        );
        source.view_html = format!("<html><body><p><strong><em>{prose}</em></strong> «Bonjour»<a id='ref' role='doc-noteref' href='#note'>1</a><img data-papercut-asset='{}'>.</p><p id='note' translate='no'>Note originale.</p></body></html>", asset.file_name);
        source.blocks = super::super::dom::DomPlan::new(&source.view_html)
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
            old_plan.request,
            source.blocks.iter().map(|b| &b.text),
            900,
            2,
        )
        .unwrap();
        assert_eq!(plan.max_segment_chars, 320);
        assert!(plan.total_segments > 3);
        let state = TranslationState::default();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let make_worker = |cancel: bool| {
            let host = state.clone();
            let seen = seen.clone();
            state
                .bergamot
                .open(
                    "main".into(),
                    "document-job".into(),
                    state.cancelled_jobs.clone(),
                    move |request| {
                        let reply = match request {
                            WorkerRequest::Batch {
                                attempt_id,
                                batch_id,
                                segments,
                                ..
                            } => {
                                seen.lock()
                                    .unwrap()
                                    .extend(segments.iter().map(|s| s.id.clone()));
                                let outputs = segments
                                    .into_iter()
                                    .map(|s| {
                                        assert!(s.html);
                                        let text = if s.text.contains("«Bonjour»") {
                                            let marker =
                                                "<span data-pc=\"2\" translate=\"no\"></span>";
                                            assert!(s.text.contains(marker));
                                            let moved = s.text.replace("«Bonjour»", "“Hello.”");
                                            format!(
                                                "{marker}{}",
                                                moved
                                                    .strip_suffix('.')
                                                    .unwrap()
                                                    .replace(marker, "")
                                            )
                                        } else {
                                            s.text.clone()
                                        };
                                        TranslationSegmentOutput {
                                            needs_review: false,
                                            id: s.id,
                                            source_text: s.text,
                                            text,
                                            markup: None,
                                        }
                                    })
                                    .collect();
                                WorkerReply::Result {
                                    attempt_id,
                                    batch_id,
                                    outputs,
                                }
                            }
                            WorkerRequest::Confirm {
                                attempt_id,
                                batch_id,
                            } => {
                                if cancel {
                                    host.cancel("document-job").unwrap();
                                }
                                WorkerReply::Confirmed {
                                    attempt_id,
                                    batch_id,
                                }
                            }
                            WorkerRequest::Stop { .. } => return Ok(()),
                        };
                        host.bergamot.reply("main", reply).map(|_| ())
                    },
                )
                .unwrap()
        };
        let mut worker = make_worker(false);
        finish_translation(
            app.handle(),
            &state,
            &mut worker,
            &plan,
            &source,
            "document-job",
            Instant::now(),
        )
        .unwrap();
        drop(worker);
        assert_eq!(seen.lock().unwrap().len(), plan.total_segments);
        let variants = super::super::storage::list_translated_documents(app.handle()).unwrap();
        let saved =
            load_translation_source_document(app.handle(), &variants[0].document_url).unwrap();
        assert!(
            saved.view_html.contains("“Hello.”<a"),
            "{}",
            saved.view_html
        );
        assert_eq!(saved.assets.as_ref(), &vec![asset]);
        let dom = super::super::html::parse_html_document(&saved.view_html);
        assert_eq!(dom.select("a#ref[href='#note'] + img").unwrap().count(), 1);
        assert!(dom.select("strong > em").unwrap().count() >= 1);
        let styled = dom
            .select("strong > em")
            .unwrap()
            .map(|node| node.text_contents())
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(
            styled.split_whitespace().collect::<Vec<_>>().join(" "),
            prose.split_whitespace().collect::<Vec<_>>().join(" ")
        );
        assert!(!saved.view_html.contains("data-pc"));
        seen.lock().unwrap().clear();
        let mut resumed = make_worker(false);
        let result = finish_translation(
            app.handle(),
            &state,
            &mut resumed,
            &plan,
            &source,
            "document-job",
            Instant::now(),
        )
        .unwrap();
        drop(resumed);
        assert_eq!(result.status, "stored");
        assert!(seen.lock().unwrap().is_empty());
        assert_eq!(
            saved.view_html,
            load_translation_source_document(app.handle(), &variants[0].document_url)
                .unwrap()
                .view_html
        );
        let mut cancelled = make_worker(true);
        assert!(finish_translation(
            app.handle(),
            &state,
            &mut cancelled,
            &plan,
            &source,
            "document-job",
            Instant::now()
        )
        .unwrap_err()
        .contains("cancel"));
        drop(cancelled);
        assert_eq!(
            saved.view_html,
            load_translation_source_document(app.handle(), &variants[0].document_url)
                .unwrap()
                .view_html
        );
        assert_eq!(
            original,
            load_translation_source_document(app.handle(), &source.document_url).unwrap()
        );
    }
}

/// A short authored ID and reverse quote order must survive the real HTML/EPUB writer and cache-only resume.
#[test]
fn compact_ids_and_reverse_quote_notes_save_and_resume_without_identity_loss() {
    for format in ["html", "epub"] {
        let (_dir, app, mut source, old_plan) = fixture();
        let original =
            load_translation_source_document(app.handle(), &source.document_url).unwrap();
        source.format = format.into();
        source.view_html = "<html><body><p><span id='authored'>Bonjour. Merci.</span> «Salut.<a id='ref' role='doc-noteref' href='#note'>1</a>»</p><p id='note' translate='no'>Note originale.</p></body></html>".into();
        source.blocks = super::super::dom::DomPlan::new(&source.view_html)
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
            old_plan.request,
            source.blocks.iter().map(|b| &b.text),
            900,
            16,
        )
        .unwrap();
        assert_eq!(plan.total_segments, 1);
        let state = TranslationState::default();
        let calls = Arc::new(Mutex::new(0));
        let make_worker = || {
            let host = state.clone();
            let calls = calls.clone();
            state.bergamot.open("main".into(), "document-job".into(), state.cancelled_jobs.clone(), move |request| {
                let reply = match request {
                    WorkerRequest::Batch { attempt_id, batch_id, segments, .. } => {
                        *calls.lock().unwrap() += 1;
                        let outputs = segments.into_iter().map(|s| {
                            assert!(s.html);
                            assert!(s.text.contains("Bonjour. Merci."));
                            TranslationSegmentOutput { needs_review: false, id:s.id, source_text:s.text, text:"<span data-pc='0'>Hello. Thanks.</span> “Hi”.<span data-pc='1' translate='no'></span>".into(), markup:None }
                        }).collect();
                        WorkerReply::Result { attempt_id, batch_id, outputs }
                    }
                    WorkerRequest::Confirm { attempt_id, batch_id } => WorkerReply::Confirmed { attempt_id, batch_id },
                    WorkerRequest::Stop { .. } => return Ok(()),
                };
                host.bergamot.reply("main", reply).map(|_| ())
            }).unwrap()
        };
        let mut worker = make_worker();
        finish_translation(
            app.handle(),
            &state,
            &mut worker,
            &plan,
            &source,
            "document-job",
            Instant::now(),
        )
        .unwrap();
        drop(worker);
        let variants = super::super::storage::list_translated_documents(app.handle()).unwrap();
        let saved =
            load_translation_source_document(app.handle(), &variants[0].document_url).unwrap();
        let dom = super::super::html::parse_html_document(&saved.view_html);
        assert_eq!(dom.select("span#authored").unwrap().count(), 1);
        assert_eq!(
            dom.select_first("span#authored").unwrap().text_contents(),
            "Hello. Thanks."
        );
        assert_eq!(dom.select("a#ref[href='#note']").unwrap().count(), 1);
        assert!(saved.view_html.contains("“Hi<a"), "{}", saved.view_html);
        assert!(saved.view_html.contains("</a>”."), "{}", saved.view_html);
        assert!(saved.view_html.contains("Note originale."));
        let mut resumed = make_worker();
        finish_translation(
            app.handle(),
            &state,
            &mut resumed,
            &plan,
            &source,
            "document-job",
            Instant::now(),
        )
        .unwrap();
        drop(resumed);
        assert_eq!(*calls.lock().unwrap(), 1);
        assert_eq!(
            super::super::storage::list_translated_documents(app.handle())
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            saved.view_html,
            load_translation_source_document(app.handle(), &variants[0].document_url)
                .unwrap()
                .view_html
        );
        assert_eq!(
            original,
            load_translation_source_document(app.handle(), &source.document_url).unwrap()
        );
    }
}

/// Direct commands reject strict admission before touching the source/model filesystem or worker.
#[test]
fn public_commands_cannot_enable_strict_formatting_by_forging_request_flags() {
    let (dir, app, _source, mut plan) = fixture();
    let before = snapshot(&dir.0);
    let state = TranslationState::default();
    plan.request.preserve_formatting = true;
    plan.request.model_id = "opus-mt-fr-en-ctranslate2".into();
    assert!(
        start_translation(app.handle(), &state, plan.request.clone())
            .unwrap_err()
            .contains("not available")
    );
    plan.request.model_id = "bergamot-fr-en-worker".into();
    let error = start_bergamot_translation(
        app.handle(),
        &state,
        plan.request,
        "test-owner".into(),
        |_| panic!("must not open the worker"),
    )
    .unwrap_err();
    assert!(error.contains("does not support"));
    assert_eq!(before, snapshot(&dir.0));
}

/// Saved provenance is historical metadata, never a requirement to reload the retired aligner or receipts.
#[test]
fn saved_alignment_provenance_remains_readable_without_a_runtime_or_file_rewrites() {
    let (dir, app, source, plan) = fixture();
    let state = TranslationState::default();
    let mut engine = worker(&state, usize::MAX, "ok", Arc::default());
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
    let db = crate::document_uploads::open_document_uploads_db(app.handle()).unwrap();
    db.execute(
        "UPDATE translated_documents SET settings_json = ?1",
        [r#"{"alignment":{"policy":"retired","tokenizer":"historical"}}"#],
    )
    .unwrap();
    for name in ["audio.wav", "timings.json"] {
        std::fs::write(dir.0.join(name), b"existing recording").unwrap();
    }
    let before = snapshot(&dir.0);
    let rows = super::super::storage::list_translated_documents(app.handle()).unwrap();
    assert_eq!(rows.len(), 1);
    let saved = load_translation_source_document(app.handle(), &rows[0].document_url).unwrap();
    assert!(!saved.view_html.trim().is_empty());
    assert_eq!(before, snapshot(&dir.0));
}
