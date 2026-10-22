//! Real native command/storage regressions with synthetic source and existing-audio sentinels.
use super::{fixture, invoke};
use crate::document_uploads::{open_document_uploads_db, persist_derived_document};
use crate::translation::{
    formatting::FormattingPolicy,
    source::{load_translation_source_document, TranslationSourceDocument},
    storage::{
        persist_translated_document, PersistTranslationFragment, PersistTranslationRequest,
        PersistTranslationSection,
    },
};
use serde_json::{json, Value};
use std::{fs, path::Path};
use tauri::test::MockRuntime;

fn source(app: &tauri::AppHandle<MockRuntime>) -> TranslationSourceDocument {
    let mut db = open_document_uploads_db(app).unwrap();
    persist_derived_document(
        app,
        &mut db,
        "abc123",
        "/uploads/abc123.html",
        "Synthetic source",
        "html",
        "<p id='first'>Une <strong>maison</strong>.</p><p id='second'>Une <em>maison</em>.</p>"
            .into(),
        &[],
        1,
        |_| Ok(()),
    )
    .unwrap();
    load_translation_source_document(app, "/uploads/abc123.html").unwrap()
}

/// Use the real renderer and transaction; no engine, model files or private document participates.
fn request(source: &TranslationSourceDocument) -> PersistTranslationRequest {
    PersistTranslationRequest {
        formatting: FormattingPolicy::SentenceReview,
        source: source.clone(),
        source_language: "fr".into(),
        target_language: "en".into(),
        model_id: "opus-mt-fr-en-ctranslate2".into(),
        quality_mode: "balanced".into(),
        repair_mode: Default::default(),
        job_id: "synthetic-job".into(),
        glossary: vec![],
        translated_sections: source
            .blocks
            .iter()
            .map(|block| PersistTranslationSection {
                source_heading: block.heading.clone(),
                source_ordinal: block.ordinal,
                is_heading: block.is_heading,
                text: "A house.".into(),
                fragments: vec![PersistTranslationFragment {
                    needs_review: true,
                    source_start: 0,
                    source_end: block.text.chars().count(),
                    source_text: block.text.clone(),
                    text: "A house.".into(),
                    inline_phrases: vec![],
                    markup: None,
                }],
            })
            .collect(),
    }
}

fn load(window: &tauri::WebviewWindow<MockRuntime>, id: &str) -> Result<Value, Value> {
    invoke(window, "translation_review_load", json!({"id":id}))
}

fn edit(draft: &Value, block: usize, action: Value) -> Value {
    json!({"expected":draft["document"]["expected"], "block":draft["document"]["blocks"][block]["key"], "edit":action})
}

fn save(window: &tauri::WebviewWindow<MockRuntime>, edit: &Value) -> Result<Value, Value> {
    invoke(
        window,
        "translation_review_save",
        json!({"request":edit.to_string()}),
    )
}

/// Compare semantic index rows and actual files, rather than SQLite's changing WAL bytes.
fn published(
    app: &tauri::AppHandle<MockRuntime>,
    root: &Path,
    id: &str,
) -> (Vec<u8>, Vec<(String, String)>, String) {
    let db = open_document_uploads_db(app).unwrap();
    let rows = db
        .prepare(
            "SELECT text, body_ranges FROM uploaded_sections WHERE document_id=?1 ORDER BY ordinal",
        )
        .unwrap()
        .query_map([id], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let status = db
        .query_row(
            "SELECT status FROM translated_documents WHERE id=?1",
            [id],
            |row| row.get(0),
        )
        .unwrap();
    (
        fs::read(root.join(format!("document_uploads/{id}/source.html"))).unwrap(),
        rows,
        status,
    )
}

#[test]
fn real_commands_resume_drafts_without_source_models_or_published_audio_changes() {
    let (dir, app, window) = fixture();
    let source = source(app.handle());
    let info = persist_translated_document(app.handle(), request(&source), || Ok(())).unwrap();
    let before = published(app.handle(), &dir.0, &info.id);
    let audio = dir.0.join("audiobooks/existing");
    fs::create_dir_all(&audio).unwrap();
    fs::write(
        audio.join("manifest.json"),
        json!({"documentUrl":info.document_url}).to_string(),
    )
    .unwrap();
    fs::write(audio.join("audio.wav"), b"existing audio").unwrap();
    fs::write(audio.join("timings.json"), b"existing highlighting").unwrap();
    let draft = load(&window, &info.id).unwrap();
    assert_eq!(draft["document"]["blocks"].as_array().unwrap().len(), 2);
    assert_ne!(
        draft["document"]["blocks"][0]["key"],
        draft["document"]["blocks"][1]["key"]
    );
    let mutation = edit(
        &draft,
        1,
        json!({"action":"replaceMarks","marks":[{"range":{"start":2,"end":7},"kind":"bold"}]}),
    );
    let changed = save(&window, &mutation).unwrap();
    assert_eq!(changed["document"]["expected"]["revision"], 1);
    assert!(changed["document"]["blocks"][0]["editable"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        changed["document"]["blocks"][1]["editable"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(changed["previewHtml"]
        .as_str()
        .unwrap()
        .contains("<b>house</b>"));
    assert!(save(&window, &mutation).unwrap_err()["message"]
        .as_str()
        .unwrap()
        .contains("reload"));
    invoke(
        &window,
        "document_uploads_delete",
        json!({"request":{"documentUrl":source.document_url}}),
    )
    .unwrap();
    assert!(!dir.0.join("document_uploads/abc123").exists());
    assert!(
        !dir.0.join("translation-models").exists(),
        "review must not install a model"
    );
    assert_eq!(load(&window, &info.id).unwrap(), changed);
    assert_eq!(published(app.handle(), &dir.0, &info.id), before);
    assert_eq!(
        fs::read(audio.join("audio.wav")).unwrap(),
        b"existing audio"
    );
    assert_eq!(
        fs::read(audio.join("timings.json")).unwrap(),
        b"existing highlighting"
    );
    let approved = save(&window, &edit(&changed, 1, json!({"action":"approve"}))).unwrap();
    assert_eq!(approved["document"]["blocks"][1]["approved"], true);
    let reset = save(&window, &edit(&approved, 1, json!({"action":"reset"}))).unwrap();
    assert_eq!(reset["document"]["blocks"][1]["approved"], false);
    assert_eq!(published(app.handle(), &dir.0, &info.id), before);
}

#[test]
fn regeneration_retains_prior_variants_drafts_and_audio_and_deleted_drafts_cannot_revive() {
    let (dir, app, window) = fixture();
    let source = source(app.handle());
    let first = persist_translated_document(app.handle(), request(&source), || Ok(())).unwrap();
    let draft = load(&window, &first.id).unwrap();
    let changed = save(&window, &edit(&draft, 0, json!({"action":"approve"}))).unwrap();
    let audio = dir.0.join("audiobooks/old");
    fs::create_dir_all(&audio).unwrap();
    fs::write(
        audio.join("manifest.json"),
        json!({"documentUrl":first.document_url}).to_string(),
    )
    .unwrap();
    fs::write(audio.join("audio.wav"), b"old audio").unwrap();
    let first_published = published(app.handle(), &dir.0, &first.id);
    let second = persist_translated_document(app.handle(), request(&source), || Ok(())).unwrap();
    assert_ne!(first.id, second.id);
    let second_draft = load(&window, &second.id).unwrap();
    assert_ne!(
        changed["document"]["expected"]["generation"],
        second_draft["document"]["expected"]["generation"]
    );
    assert_eq!(load(&window, &first.id).unwrap(), changed);
    assert_eq!(published(app.handle(), &dir.0, &first.id), first_published);
    assert_eq!(fs::read(audio.join("audio.wav")).unwrap(), b"old audio");
    let mut stale_generation = edit(&second_draft, 0, json!({"action":"approve"}));
    stale_generation["expected"]["generation"] =
        changed["document"]["expected"]["generation"].clone();
    assert!(save(&window, &stale_generation).is_err());
    invoke(
        &window,
        "translation_delete_document",
        json!({"request":{"id":second.id}}),
    )
    .unwrap();
    assert!(load(&window, &second.id).unwrap().is_null());
    assert!(save(
        &window,
        &edit(&second_draft, 0, json!({"action":"approve"}))
    )
    .is_err());
    assert!(!dir
        .0
        .join(format!("document_uploads/{}", second.id))
        .exists());
    let db = open_document_uploads_db(app.handle()).unwrap();
    assert_eq!(
        db.query_row(
            "SELECT count(*) FROM translation_review WHERE variant_id=?1",
            [&second.id],
            |row| row.get::<_, usize>(0)
        )
        .unwrap(),
        0
    );
}

#[test]
fn command_admission_and_baseline_conflicts_never_change_the_draft() {
    let (dir, app, window) = fixture();
    assert!(load(&window, "../../elsewhere").is_err());
    for request in ["{}".into(), "x".repeat(16 * 1024 + 1)] {
        assert!(invoke(
            &window,
            "translation_review_save",
            json!({"request":request})
        )
        .is_err());
    }
    assert!(!dir.0.join("document_uploads").exists());
    let source = source(app.handle());
    let info = persist_translated_document(app.handle(), request(&source), || Ok(())).unwrap();
    assert!(
        load(&window, &source.document_id).unwrap().is_null(),
        "legacy uploads have no edit evidence"
    );
    let draft = load(&window, &info.id).unwrap();
    let action = json!({"action":"approve"});
    let mut injected = edit(&draft, 0, action.clone());
    injected["html"] = "<script>injected</script>".into();
    assert!(save(&window, &injected).is_err());
    let mut invalid = edit(
        &draft,
        0,
        json!({"action":"replaceMarks","marks":[{"range":{"start":0,"end":999},"kind":"italic"}]}),
    );
    assert!(save(&window, &invalid).is_err());
    invalid["block"]["sourceIdentity"] = "../../path".into();
    assert!(save(&window, &invalid).is_err());
    assert_eq!(load(&window, &info.id).unwrap(), draft);
    let path = dir
        .0
        .join(format!("document_uploads/{}/source.html", info.id));
    let original = fs::read(&path).unwrap();
    fs::write(&path, b"<p>Changed outside this review</p>").unwrap();
    assert!(save(&window, &edit(&draft, 0, action)).is_err());
    assert!(load(&window, &info.id).is_err());
    fs::write(path, original).unwrap();
    assert_eq!(load(&window, &info.id).unwrap(), draft);
}

#[test]
fn failed_native_save_leaves_no_partial_review_or_variant() {
    let (_dir, app, _window) = fixture();
    let source = source(app.handle());
    crate::translation::storage::list_translated_documents(app.handle()).unwrap();
    let db = open_document_uploads_db(app.handle()).unwrap();
    db.execute_batch("CREATE TRIGGER reject_review BEFORE INSERT ON translation_review BEGIN SELECT RAISE(ABORT,'injected review failure'); END;").unwrap();
    assert!(persist_translated_document(app.handle(), request(&source), || Ok(())).is_err());
    assert_eq!(
        db.query_row("SELECT count(*) FROM translated_documents", [], |r| r
            .get::<_, usize>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM translation_review", [], |r| r
            .get::<_, usize>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM uploaded_documents", [], |r| r
            .get::<_, usize>(0))
            .unwrap(),
        1
    );
}

#[cfg(unix)]
#[test]
fn asset_response_failure_preserves_revision_and_allows_the_same_edit_to_retry() {
    use std::os::unix::ffi::OsStringExt;
    let (dir, app, window) = fixture();
    let source = source(app.handle());
    let info = persist_translated_document(app.handle(), request(&source), || Ok(())).unwrap();
    let draft = load(&window, &info.id).unwrap();
    let edit = edit(&draft, 0, json!({"action":"approve"}));
    let assets = dir.0.join(format!("document_uploads/{}/assets", info.id));
    fs::create_dir_all(&assets).unwrap();
    let invalid = assets.join(std::ffi::OsString::from_vec(vec![0xff]));
    fs::write(&invalid, b"inaccessible response entry").unwrap();
    assert!(save(&window, &edit).is_err());
    let db = open_document_uploads_db(app.handle()).unwrap();
    assert_eq!(
        db.query_row(
            "SELECT revision FROM translation_review WHERE variant_id=?1",
            [&info.id],
            |r| r.get::<_, u64>(0)
        )
        .unwrap(),
        0
    );
    fs::remove_file(invalid).unwrap();
    assert_eq!(load(&window, &info.id).unwrap(), draft);
    assert_eq!(
        save(&window, &edit).unwrap()["document"]["expected"]["revision"],
        1
    );
}
