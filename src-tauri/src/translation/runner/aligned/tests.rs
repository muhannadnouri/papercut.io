//! Synthetic gold links exercise lifecycle/storage, not neural alignment quality or private documents.
use super::*;
use crate::translation::{
    dom::DomPlan,
    inline_markup::provider::{
        contract::{
            tests::{rules, tokens},
            NodeMapping, Rejection, Side, TokenizedText, Versions, WordLinks, PROJECTION_POLICY,
        },
        pieces::{ExactPieceTranslator, Translation},
    },
    source::TranslationSourceBlock,
};

struct Engine {
    calls: usize,
    state: TranslationState,
    cancel_after_translation: bool,
    cancel_before_save: bool,
}
impl ExactPieceTranslator for Engine {
    fn source_fits(&self, source: &str) -> Result<bool, String> {
        Ok(source.split_whitespace().count() <= 2)
    }
    fn translate_exact(&mut self, source: &[&str]) -> Result<Vec<String>, String> {
        self.calls += source.len();
        if self.cancel_after_translation {
            self.state.cancel("document-job")?;
        }
        Ok(source.iter().map(|s| s.to_uppercase()).collect())
    }
}
impl TranslationEngine for Engine {
    fn exact_translator(&mut self) -> Option<&mut dyn ExactPieceTranslator> {
        Some(self)
    }
    fn translate_batch(
        &mut self,
        _: TranslationBatchInput,
    ) -> Result<Vec<super::super::super::engine::TranslationSegmentOutput>, String> {
        panic!("strict work entered the legacy translator/probe path")
    }
    fn before_save(&mut self) -> Result<(), String> {
        if self.cancel_before_save {
            self.state.cancel("document-job")?;
        }
        Ok(())
    }
}
struct Aligner {
    versions: Versions,
    calls: usize,
    fail_at: usize,
    cancel_at: usize,
    state: TranslationState,
}
impl PieceAligner for Aligner {
    fn versions(&self, _: &str) -> Result<Versions, String> {
        Ok(self.versions.clone())
    }
    fn text_fits(&self, _: &str, _: Side) -> Result<bool, String> {
        Ok(true)
    }
    fn map(&mut self, translation: &Translation<'_>) -> Result<Vec<NodeMapping>, String> {
        self.calls += 1;
        if self.calls == self.fail_at {
            return Err("Synthetic unresolved scope".into());
        }
        let projection = translation
            .project(
                |s, t| {
                    let st = tokens(s);
                    let tt = tokens(t);
                    WordLinks::new(
                        s,
                        t,
                        &rules(),
                        &TokenizedText {
                            normalized: s,
                            tokens: &st,
                        },
                        &TokenizedText {
                            normalized: t,
                            tokens: &tt,
                        },
                        (0..st.len() - 2).map(|i| (i, i)).collect(),
                    )
                },
                None,
                || Ok::<_, Rejection>(()),
            )
            .map_err(|e| format!("{e:?}"))?;
        if self.calls == self.cancel_at {
            self.state.cancel("document-job")?;
        }
        Ok(projection.nodes().to_vec())
    }
}

/// Reuse the isolated real app-data/DB fixture; only deterministic model calls are substituted.
fn fixture(
    html: &str,
    format: &str,
    batch_size: usize,
) -> (
    super::super::document_tests::Fixture,
    tauri::App<tauri::test::MockRuntime>,
    TranslationSourceDocument,
    TranslationJobPlan,
) {
    let (dir, app, mut source, old) = super::super::document_tests::fixture();
    source.format = format.into();
    source.view_html = html.into();
    source.blocks = DomPlan::new(html)
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
    request.model_id = "opus-mt-fr-en-ctranslate2".into();
    let plan = plan_translation_job(
        request,
        source.blocks.iter().map(|b| &b.text),
        900,
        batch_size,
    )
    .unwrap();
    (dir, app, source, plan)
}

fn engines(state: &TranslationState) -> (Engine, Aligner) {
    (
        Engine {
            calls: 0,
            state: state.clone(),
            cancel_after_translation: false,
            cancel_before_save: false,
        },
        Aligner {
            calls: 0,
            fail_at: usize::MAX,
            cancel_at: usize::MAX,
            state: state.clone(),
            versions: Versions {
                translator: "synthetic-translator-v1".into(),
                aligner: "synthetic-aligner-v1".into(),
                tokenizer: "tokens@one".into(),
                normalization: "reader-pieces-v1".into(),
                policy: format!("synthetic-extraction-v1:{PROJECTION_POLICY}"),
            },
        },
    )
}

/// Use the same foreground ownership and atomic save admission as public jobs.
fn run(
    app: &tauri::AppHandle<tauri::test::MockRuntime>,
    state: &TranslationState,
    source: &TranslationSourceDocument,
    plan: &TranslationJobPlan,
    engine: &mut Engine,
    aligner: &mut Aligner,
) -> Result<TranslationStartResponse, String> {
    let _job = lifecycle::Job::new(
        state,
        &plan.cache_key,
        "document-job",
        "translator-test-pack",
    )?;
    let _auxiliary = state.claim_model_read("aligner-test-pack")?;
    finish_with_alignment(
        app,
        state,
        engine,
        plan,
        source,
        "document-job",
        Instant::now(),
        Some(aligner),
    )
}

fn stored(app: &tauri::AppHandle<tauri::test::MockRuntime>) -> TranslationSourceDocument {
    let variants = super::super::super::storage::list_translated_documents(app).unwrap();
    assert_eq!(variants.len(), 1);
    load_translation_source_document(app, &variants[0].document_url).unwrap()
}

#[test]
fn html_epub_strict_jobs_save_and_resume_source_owned_hierarchy_assets_and_list_context() {
    for format in ["html", "epub"] {
        let (dir, app, mut source, plan) = fixture("<html><body><p>Un <span id='phrase'><a role='doc-noteref' href='#note'><sup>1</sup></a><strong>livre</strong></span> ici <code>Literal🙂</code> fin.</p><ul><li>Un <em>mot</em> ici</li></ul><table><tbody><tr><td>Une <strong>cellule</strong> ici</td></tr></tbody></table><p>Une <a id='link' href='#note'><em><img id='picture'></em>image</a> ici</p><p id='note' translate='no'>Note originale.</p></body></html>", format, 2);
        let asset = crate::document_uploads::ParsedDocumentAsset::new(
            "image/png",
            b"synthetic owned image".to_vec(),
        )
        .unwrap();
        source.view_html = source.view_html.replace(
            "id='picture'",
            &format!("id='picture' data-papercut-asset='{}'", asset.file_name),
        );
        source.assets = Arc::new(vec![asset.clone()]);
        let originals = super::super::document_tests::snapshot(&dir.0);
        let audiobook = dir.0.join("audiobooks/existing");
        std::fs::create_dir_all(&audiobook).unwrap();
        std::fs::write(audiobook.join("audio.wav"), b"existing audio").unwrap();
        std::fs::write(audiobook.join("timings.json"), b"existing read-along").unwrap();
        let state = TranslationState::default();
        let (mut engine, mut aligner) = engines(&state);
        run(
            app.handle(),
            &state,
            &source,
            &plan,
            &mut engine,
            &mut aligner,
        )
        .unwrap();
        assert!(
            engine.calls > plan.total_segments,
            "expected actual exact-piece subdivisions"
        );
        let saved = stored(app.handle());
        let dom = super::super::super::html::parse_html_document(&saved.view_html);
        assert_eq!(
            dom.select("#phrase > a[role=doc-noteref] > sup")
                .unwrap()
                .count(),
            1
        );
        assert_eq!(
            dom.select_first("#phrase > strong")
                .unwrap()
                .text_contents(),
            "LIVRE"
        );
        assert_eq!(dom.select_first("li > em").unwrap().text_contents(), "MOT");
        assert_eq!(
            dom.select_first("td > strong").unwrap().text_contents(),
            "cellule"
        );
        assert_eq!(dom.select("#link > em > #picture").unwrap().count(), 1);
        assert_eq!(
            dom.select_first("code").unwrap().text_contents(),
            "Literal🙂"
        );
        for id in ["phrase", "link", "picture", "note"] {
            assert_eq!(dom.select(&format!("#{id}")).unwrap().count(), 1);
        }
        assert_eq!(saved.assets.as_ref(), &vec![asset]);
        assert!(saved.view_html.contains("Note originale."));
        assert!(!saved.view_html.contains("data-pc="));
        let before = (engine.calls, aligner.calls);
        run(
            app.handle(),
            &state,
            &source,
            &plan,
            &mut engine,
            &mut aligner,
        )
        .unwrap();
        assert_eq!((engine.calls, aligner.calls), before);
        assert_eq!(stored(app.handle()).view_html, saved.view_html);
        let db = crate::document_uploads::open_document_uploads_db(app.handle()).unwrap();
        let settings: String = db
            .query_row("SELECT settings_json FROM translated_documents", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(settings.contains(PROJECTION_POLICY));
        for (path, bytes) in originals {
            assert_eq!(std::fs::read(path).unwrap(), bytes);
        }
        assert_eq!(
            std::fs::read(audiobook.join("audio.wav")).unwrap(),
            b"existing audio"
        );
        assert_eq!(
            std::fs::read(audiobook.join("timings.json")).unwrap(),
            b"existing read-along"
        );
    }
}

#[test]
fn failed_batch_is_not_checkpointed_and_retry_skips_only_wholly_validated_batches() {
    let (_dir, app, source, plan) = fixture(
        "<p>Un <em>livre</em></p><p>Une <strong>lettre</strong></p><p>Un mot</p>",
        "html",
        2,
    );
    let state = TranslationState::default();
    let (mut engine, mut aligner) = engines(&state);
    aligner.fail_at = 2;
    assert!(run(
        app.handle(),
        &state,
        &source,
        &plan,
        &mut engine,
        &mut aligner
    )
    .is_err());
    let cache = load_segment_cache(app.handle(), &plan).unwrap();
    assert!(cache.aligned_for("b0:s0").is_none());
    assert!(
        super::super::super::storage::list_translated_documents(app.handle())
            .unwrap()
            .is_empty()
    );
    aligner.calls = 0;
    aligner.fail_at = 3;
    assert!(run(
        app.handle(),
        &state,
        &source,
        &plan,
        &mut engine,
        &mut aligner
    )
    .is_err());
    let cache = load_segment_cache(app.handle(), &plan).unwrap();
    assert!(cache.aligned_for("b0:s0").is_some());
    assert!(cache.aligned_for("b1:s0").is_some());
    assert!(cache.aligned_for("b2:s0").is_none());
    aligner.fail_at = usize::MAX;
    engine.calls = 0;
    aligner.calls = 0;
    run(
        app.handle(),
        &state,
        &source,
        &plan,
        &mut engine,
        &mut aligner,
    )
    .unwrap();
    assert_eq!((engine.calls, aligner.calls), (1, 1));
    stored(app.handle());
}

#[test]
fn resume_realigns_policy_tokenizer_and_model_changes_but_never_admits_legacy_hints() {
    let (_dir, app, source, plan) = fixture("<p>Un <em>livre</em></p>", "html", 2);
    let state = TranslationState::default();
    let (mut engine, mut aligner) = engines(&state);
    let mut cache = load_segment_cache(app.handle(), &plan).unwrap();
    cache.store_translation(&plan.batches[0].segments[0], "untrusted legacy text".into());
    cache.store_probe_translation("livre", "untrusted legacy hint".into());
    save_segment_cache(app.handle(), &plan, &cache).unwrap();
    run(
        app.handle(),
        &state,
        &source,
        &plan,
        &mut engine,
        &mut aligner,
    )
    .unwrap();
    assert_eq!((engine.calls, aligner.calls), (1, 1));
    for field in ["policy", "tokenizer", "aligner"] {
        match field {
            "policy" => aligner.versions.policy = format!("changed-extraction:{PROJECTION_POLICY}"),
            "tokenizer" => aligner.versions.tokenizer.push_str("-changed"),
            _ => aligner.versions.aligner.push_str("-changed"),
        }
        let before = aligner.calls;
        run(
            app.handle(),
            &state,
            &source,
            &plan,
            &mut engine,
            &mut aligner,
        )
        .unwrap();
        assert_eq!(engine.calls, 1);
        assert_eq!(aligner.calls, before + 1);
    }
    aligner.versions.translator.push_str("-new-revision");
    run(
        app.handle(),
        &state,
        &source,
        &plan,
        &mut engine,
        &mut aligner,
    )
    .unwrap();
    assert_eq!(engine.calls, 2);
    let mut changed = source.clone();
    changed.view_html = changed
        .view_html
        .replace("<em>", "<strong>")
        .replace("</em>", "</strong>");
    run(
        app.handle(),
        &state,
        &changed,
        &plan,
        &mut engine,
        &mut aligner,
    )
    .unwrap();
    assert_eq!(engine.calls, 3);
    let mut moved = changed.clone();
    moved.view_html = moved
        .view_html
        .replace("<p>", "<li>")
        .replace("</p>", "</li>");
    run(
        app.handle(),
        &state,
        &moved,
        &plan,
        &mut engine,
        &mut aligner,
    )
    .unwrap();
    assert_eq!(
        engine.calls, 4,
        "different block context must not reuse the earlier receipt"
    );
    let variants = super::super::super::storage::list_translated_documents(app.handle()).unwrap();
    assert_eq!(variants.len(), 5, "new profiles preserve earlier derived variants; same-profile retries replace only their own variant");
    assert!(variants.iter().any(|v| load_translation_source_document(
        app.handle(),
        &v.document_url
    )
    .unwrap()
    .view_html
    .contains("<li")));
}

#[test]
fn corrupted_target_nodes_and_piece_boundaries_are_discarded_before_cache_resume() {
    let (dir, app, source, plan) = fixture("<p>Un <em>livre</em></p>", "html", 2);
    let state = TranslationState::default();
    let (mut engine, mut aligner) = engines(&state);
    run(
        app.handle(),
        &state,
        &source,
        &plan,
        &mut engine,
        &mut aligner,
    )
    .unwrap();
    let cache_path = dir
        .0
        .join("translation/segment-cache")
        .join(&plan.cache_key)
        .join("segments.json");
    for kind in ["target", "nodes", "pieces", "source"] {
        let mut json: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&cache_path).unwrap()).unwrap();
        let record = &mut json["aligned"]["b0:s0"];
        match kind {
            "target" => record["binding"]["target"] = "WRONG WORDS".into(),
            "nodes" => record["nodes"][0]["targets"][0]["end"] = usize::MAX.into(),
            "pieces" => record["targets"][0]["end"] = usize::MAX.into(),
            _ => record["binding"]["source_plan"] = "changed-dom".into(),
        }
        std::fs::write(&cache_path, serde_json::to_vec(&json).unwrap()).unwrap();
        let before = engine.calls;
        run(
            app.handle(),
            &state,
            &source,
            &plan,
            &mut engine,
            &mut aligner,
        )
        .unwrap();
        assert_eq!(engine.calls, before + 1);
        assert!(stored(app.handle()).view_html.contains("<em>LIVRE</em>"));
    }
}

#[test]
fn cancellation_during_native_work_and_before_save_preserves_existing_variant_and_source() {
    for mode in [
        "translation",
        "alignment",
        "last-batch",
        "save",
        "translating-batch",
        "aligning-batch",
        "reconstructing-batch",
    ] {
        let (dir, app, source, plan) = fixture("<p>Un <em>livre</em></p>", "html", 2);
        let state = TranslationState::default();
        let (mut engine, mut aligner) = engines(&state);
        run(
            app.handle(),
            &state,
            &source,
            &plan,
            &mut engine,
            &mut aligner,
        )
        .unwrap();
        let before = super::super::document_tests::snapshot(&dir.0);
        let listener = if mode == "last-batch" || mode.ends_with("-batch") {
            use tauri::Listener;
            let host = state.clone();
            let phase = if mode == "last-batch" {
                "batch-complete"
            } else {
                mode
            };
            Some(app.listen(TRANSLATION_JOB_PROGRESS_EVENT, move |event| {
                if event.payload().contains(phase) {
                    host.cancel("document-job").unwrap();
                }
            }))
        } else {
            None
        };
        engine.cancel_after_translation = mode == "translation";
        engine.cancel_before_save = mode == "save";
        if mode != "save" && mode != "last-batch" {
            aligner.versions.translator.push_str("-new");
        }
        if mode == "alignment" {
            aligner.cancel_at = aligner.calls + 1;
        }
        let result = run(
            app.handle(),
            &state,
            &source,
            &plan,
            &mut engine,
            &mut aligner,
        );
        assert!(result.unwrap_err().contains("cancel"), "{mode}");
        if let Some(id) = listener {
            use tauri::Listener;
            app.unlisten(id);
        }
        assert_eq!(super::super::document_tests::snapshot(&dir.0), before);
        let cache = load_segment_cache(app.handle(), &plan).unwrap();
        if mode != "save" && mode != "last-batch" {
            assert_eq!(
                cache
                    .aligned_for("b0:s0")
                    .unwrap()
                    .binding
                    .versions
                    .translator,
                "synthetic-translator-v1"
            );
        }
        assert!(state.claim_model_removal("aligner-test-pack").is_ok());
    }
}

#[test]
fn sqlite_failure_rolls_back_existing_variant_and_retry_reuses_validated_cache() {
    let (dir, app, source, plan) = fixture("<p>Un <em>livre</em></p>", "html", 2);
    let state = TranslationState::default();
    let (mut engine, mut aligner) = engines(&state);
    run(
        app.handle(),
        &state,
        &source,
        &plan,
        &mut engine,
        &mut aligner,
    )
    .unwrap();
    let before = super::super::document_tests::snapshot(&dir.0);
    let db = crate::document_uploads::open_document_uploads_db(app.handle()).unwrap();
    db.execute_batch("CREATE TRIGGER fail_translation BEFORE INSERT ON translated_documents BEGIN SELECT RAISE(ABORT, 'synthetic storage failure'); END;").unwrap();
    assert!(run(
        app.handle(),
        &state,
        &source,
        &plan,
        &mut engine,
        &mut aligner
    )
    .unwrap_err()
    .contains("synthetic storage failure"));
    assert_eq!(super::super::document_tests::snapshot(&dir.0), before);
    let counts = (engine.calls, aligner.calls);
    db.execute_batch("DROP TRIGGER fail_translation").unwrap();
    run(
        app.handle(),
        &state,
        &source,
        &plan,
        &mut engine,
        &mut aligner,
    )
    .unwrap();
    assert_eq!((engine.calls, aligner.calls), counts);
    stored(app.handle());
}

#[test]
fn missing_mixed_or_changed_strict_receipts_never_reach_save_admission_or_legacy_fallback() {
    let (_dir, app, source, plan) = fixture(
        "<p>Un <em>livre</em></p><p>Un <strong>mot</strong></p>",
        "html",
        2,
    );
    let state = TranslationState::default();
    let (mut engine, mut aligner) = engines(&state);
    for defect in [
        "missing",
        "all-missing",
        "worker",
        "versions",
        "text",
        "location",
    ] {
        let mut summary = run_translation_batches(
            app.handle(),
            &state,
            &mut engine,
            &plan,
            &source,
            "document-job",
            Instant::now(),
            Some(&mut aligner),
        )
        .unwrap_or_else(|e| panic!("{}", e.message));
        if defect == "all-missing" {
            for fragment in summary.sections.iter_mut().flat_map(|s| &mut s.fragments) {
                fragment.aligned = None;
            }
        }
        let first = &mut summary.sections[0].fragments[0];
        match defect {
            "missing" | "all-missing" => first.aligned = None,
            "worker" => first.markup = Some("legacy transport".into()),
            "versions" => first
                .aligned
                .as_mut()
                .unwrap()
                .binding
                .versions
                .tokenizer
                .push_str("-stale"),
            "text" => first.text = "OTHER TEXT".into(),
            _ => first.source_start = 1,
        }
        let admitted = std::cell::Cell::new(false);
        let result = persist_translated_document(
            app.handle(),
            PersistTranslationRequest {
                alignment: Some(aligner.versions.clone()),
                source: source.clone(),
                source_language: "fr".into(),
                target_language: "en".into(),
                model_id: plan.request.model_id.clone(),
                quality_mode: "balanced".into(),
                repair_mode: Default::default(),
                job_id: "document-job".into(),
                glossary: vec![],
                translated_sections: summary.sections,
            },
            || {
                admitted.set(true);
                Ok(())
            },
        );
        assert!(result.is_err(), "{defect}");
        assert!(!admitted.get(), "{defect}");
        assert!(
            super::super::super::storage::list_translated_documents(app.handle())
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn strict_result_preserves_legacy_variant_identity_files_and_search_rows() {
    let (dir, app, source, plan) = fixture("<p>Un <em>livre</em></p>", "html", 2);
    let state = TranslationState::default();
    let (mut engine, mut aligner) = engines(&state);
    let mut summary = run_translation_batches(
        app.handle(),
        &state,
        &mut engine,
        &plan,
        &source,
        "document-job",
        Instant::now(),
        Some(&mut aligner),
    )
    .unwrap_or_else(|e| panic!("{}", e.message));
    for fragment in summary.sections.iter_mut().flat_map(|s| &mut s.fragments) {
        fragment.aligned = None;
    }
    let legacy = persist_translated_document(
        app.handle(),
        PersistTranslationRequest {
            alignment: None,
            source: source.clone(),
            source_language: "fr".into(),
            target_language: "en".into(),
            model_id: plan.request.model_id.clone(),
            quality_mode: "balanced".into(),
            repair_mode: Default::default(),
            job_id: "legacy-job".into(),
            glossary: vec![],
            translated_sections: summary.sections,
        },
        || Ok(()),
    )
    .unwrap();
    let before = super::super::document_tests::snapshot(&dir.0);
    for _ in 0..2 {
        run(
            app.handle(),
            &state,
            &source,
            &plan,
            &mut engine,
            &mut aligner,
        )
        .unwrap();
    }
    let variants = super::super::super::storage::list_translated_documents(app.handle()).unwrap();
    assert_eq!(variants.len(), 2);
    assert!(variants.iter().any(|v| v.id == legacy.id));
    for (path, bytes) in before {
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
    let db = crate::document_uploads::open_document_uploads_db(app.handle()).unwrap();
    let count: usize = db
        .query_row(
            "SELECT count(*) FROM uploaded_documents WHERE id = ?1",
            [&legacy.id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
    // The normal FTS path remains readable for the original derived variant after strict storage.
    let sections: usize = db
        .query_row(
            "SELECT count(*) FROM uploaded_sections WHERE document_id = ?1",
            [&legacy.id],
            |r| r.get(0),
        )
        .unwrap();
    assert!(sections > 0);
    let searchable: usize = db.query_row("SELECT count(*) FROM uploaded_document_search_fts WHERE document_id = ?1 AND uploaded_document_search_fts MATCH 'LIVRE'", [&legacy.id], |r| r.get(0)).unwrap();
    assert!(searchable > 0);
}

#[cfg(all(
    feature = "native-translation-ctranslate2",
    feature = "native-translation-alignment-runtime"
))]
#[test]
#[ignore = "requires external pinned CT2/INT8 assets and official ORT library; no downloads"]
fn installed_native_french_spanish_jobs_save_and_resume_through_the_shared_runner() {
    use crate::translation::model_store::{installed_translation_model_dir, manifest_by_id};
    let translators =
        std::env::var_os("PAPERCUT_CT2_MODELS_DIR").expect("Set PAPERCUT_CT2_MODELS_DIR");
    let auxiliary = std::env::var_os("PAPERCUT_ALIGNMENT_ASSETS_DIR")
        .expect("Set PAPERCUT_ALIGNMENT_ASSETS_DIR");
    for (model, language, html, expected) in [
        (
            "opus-mt-fr-en-ctranslate2",
            "fr",
            "<p>Un <em>livre</em></p>",
            "Book",
        ),
        (
            "opus-mt-es-en-ctranslate2",
            "es",
            "<p>Un <em>libro</em></p>",
            "book",
        ),
    ] {
        let (_dir, app, source, mut plan) = fixture(html, "html", 2);
        plan.request.model_id = model.into();
        plan.request.source_language = language.into();
        plan.request.preserve_formatting = true;
        let manifest = manifest_by_id(model).unwrap();
        for (pack, origin) in [
            (manifest, std::path::Path::new(&translators).join(model)),
            (
                manifest_by_id("alignment-awesome-align-int8").unwrap(),
                std::path::PathBuf::from(&auxiliary),
            ),
        ] {
            let dir = installed_translation_model_dir(app.handle(), pack).unwrap();
            std::fs::create_dir_all(&dir).unwrap();
            for file in pack.files {
                if let Some(bytes) = pack.bundled_file(file.path) {
                    std::fs::write(dir.join(file.path), bytes).unwrap();
                } else {
                    std::fs::copy(origin.join(file.path), dir.join(file.path)).unwrap();
                }
            }
        }
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
        let state = TranslationState::default();
        for _ in 0..2 {
            start_with_engine(
                app.handle(),
                &state,
                plan.request.clone(),
                manifest,
                |dir, _, _job| {
                    Ok(Box::new(CTranslate2Engine::for_installed_model(
                        model,
                        dir.to_path_buf(),
                    )?))
                },
            )
            .unwrap();
            let saved = stored(app.handle());
            let dom = super::super::super::html::parse_html_document(&saved.view_html);
            assert_eq!(dom.select_first("em").unwrap().text_contents(), expected);
            assert!(state
                .claim_model_removal("alignment-awesome-align-int8")
                .is_ok());
            assert!(state.claim_model_install(model).is_ok());
        }
        let settings: String = db
            .query_row("SELECT settings_json FROM translated_documents", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(settings.contains("ort-rc13-api17"));
        assert!(settings.contains(PROJECTION_POLICY));
        // Compiling/installing the auxiliary does not opt ordinary command starts into strict alignment.
        assert!(
            start_translation(app.handle(), &state, plan.request.clone())
                .unwrap_err()
                .contains("not available")
        );
        plan.request.preserve_formatting = false;
        start_translation(app.handle(), &state, plan.request.clone()).unwrap();
        let variants =
            super::super::super::storage::list_translated_documents(app.handle()).unwrap();
        assert_eq!(variants.len(), 2);
        let profiles: Vec<String> = db
            .prepare("SELECT settings_json FROM translated_documents")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            profiles
                .iter()
                .filter(|p| p.contains("\"alignment\""))
                .count(),
            1
        );
    }
}

/// Progress names actual phases; a resumed receipt does not pretend to run inference again.
#[test]
fn strict_progress_reports_alignment_reconstruction_and_cache_resume() {
    use tauri::Listener;
    let (_dir, app, source, plan) = fixture("<p>Un <em>livre</em></p>", "html", 2);
    let state = TranslationState::default();
    let (mut engine, mut aligner) = engines(&state);
    let stages = Arc::new(Mutex::new(Vec::new()));
    let captured = stages.clone();
    let listener = app.listen(TRANSLATION_JOB_PROGRESS_EVENT, move |event| {
        let payload: serde_json::Value = serde_json::from_str(event.payload()).unwrap();
        captured
            .lock()
            .unwrap()
            .push(payload["status"].as_str().unwrap().to_string());
    });
    run(
        app.handle(),
        &state,
        &source,
        &plan,
        &mut engine,
        &mut aligner,
    )
    .unwrap();
    let first = stages.lock().unwrap().clone();
    let align = first.iter().position(|s| s == "aligning-batch").unwrap();
    let reconstruct = first
        .iter()
        .position(|s| s == "reconstructing-batch")
        .unwrap();
    let complete = first.iter().position(|s| s == "batch-complete").unwrap();
    assert!(align < reconstruct && reconstruct < complete);
    stages.lock().unwrap().clear();
    run(
        app.handle(),
        &state,
        &source,
        &plan,
        &mut engine,
        &mut aligner,
    )
    .unwrap();
    let resumed = stages.lock().unwrap();
    assert!(!resumed.iter().any(|s| s == "aligning-batch"));
    assert!(resumed.iter().any(|s| s == "reconstructing-batch"));
    assert_eq!(aligner.calls, 1);
    app.unlisten(listener);
}
