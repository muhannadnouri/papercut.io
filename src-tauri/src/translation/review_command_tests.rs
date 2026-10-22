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

pub(super) fn source(app: &tauri::AppHandle<MockRuntime>) -> TranslationSourceDocument {
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
pub(super) fn request(source: &TranslationSourceDocument) -> PersistTranslationRequest {
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

pub(super) fn load(window: &tauri::WebviewWindow<MockRuntime>, id: &str) -> Result<Value, Value> {
    invoke(window, "translation_review_load", json!({"id":id}))
}

pub(super) fn edit(draft: &Value, block: usize, action: Value) -> Value {
    json!({"expected":draft["document"]["expected"], "block":draft["document"]["blocks"][block]["key"], "edit":action})
}

pub(super) fn save(
    window: &tauri::WebviewWindow<MockRuntime>,
    edit: &Value,
) -> Result<Value, Value> {
    invoke(
        window,
        "translation_review_save",
        json!({"request":edit.to_string()}),
    )
}

/// Compare semantic index rows and actual files, rather than SQLite's changing WAL bytes.
pub(super) fn published(
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

/// Compare the exact saved evidence independently of reader whitespace normalization.
fn review_evidence(app: &tauri::AppHandle<MockRuntime>, id: &str) -> Value {
    let db = open_document_uploads_db(app).unwrap();
    let snapshot: String = db
        .query_row(
            "SELECT snapshot_json FROM translation_review WHERE variant_id=?1",
            [id],
            |row| row.get(0),
        )
        .unwrap();
    serde_json::from_str(&snapshot).unwrap()
}

#[test]
fn reader_whitespace_equivalence_saves_and_resumes_without_rewriting_english() {
    use sha2::{Digest, Sha256};
    for separator in [" ", "  ", "\n", "\t", "\u{a0}"] {
        let (dir, app, window) = fixture();
        let source = source(app.handle());
        let english = format!("A{separator}house <safe> & exact.");
        let mut request = request(&source);
        request.translated_sections[0].text = english.clone();
        request.translated_sections[0].fragments[0].text = english.clone();
        let info = persist_translated_document(app.handle(), request, || Ok(()))
            .unwrap_or_else(|error| panic!("separator {separator:?}: {error}"));
        let before = published(app.handle(), &dir.0, &info.id);
        let immutable = review_evidence(app.handle(), &info.id);
        assert_eq!(
            immutable["baselineHtml"].as_str().unwrap().as_bytes(),
            before.0
        );
        let draft = load(&window, &info.id).unwrap();
        let block = &draft["document"]["blocks"][0];
        assert_eq!(block["text"], english);
        assert_eq!(
            block["textHash"],
            format!("{:x}", Sha256::digest(english.as_bytes()))
        );
        assert_eq!(
            draft["document"]["publishedHash"],
            format!("{:x}", Sha256::digest(&before.0))
        );
        let start = english.find("house").unwrap();
        let start = english[..start].chars().count();
        let changed = save(
            &window,
            &edit(
                &draft,
                0,
                json!({"action":"replaceMarks","marks":[{"range":{"start":start,"end":start+5},"kind":"bold"}]}),
            ),
        )
        .unwrap();
        assert_eq!(changed["document"]["blocks"][0]["text"], english);
        assert_eq!(
            changed["document"]["blocks"][0]["textHash"],
            block["textHash"]
        );
        let preview = changed["previewHtml"].as_str().unwrap();
        assert!(preview.contains("<b>house</b> &lt;safe&gt; &amp; exact."));
        let target = crate::translation::dom::DomPlan::new(preview);
        assert_eq!(target.blocks[0].node.text_contents(), english);
        assert!(crate::translation::html::parse_html_document(preview)
            .select("safe")
            .unwrap()
            .next()
            .is_none());
        assert_eq!(load(&window, &info.id).unwrap(), changed);
        assert_eq!(review_evidence(app.handle(), &info.id), immutable);
        assert_eq!(published(app.handle(), &dir.0, &info.id), before);
    }
}

#[test]
fn whitespace_review_keeps_protected_literal_note_and_attachment_bytes_and_locks() {
    use crate::translation::inline_markup::protected::ProtectedSource;
    let (dir, app, window) = fixture();
    let literal = "API  \n\tvalue\u{a0}x";
    let asset = crate::document_uploads::ParsedDocumentAsset::new(
        "image/png",
        b"owned review image".to_vec(),
    )
    .unwrap();
    let mut db = open_document_uploads_db(app.handle()).unwrap();
    persist_derived_document(
        app.handle(),
        &mut db,
        "abc123",
        "/uploads/abc123.html",
        "Synthetic protected source",
        "html",
        format!("<p id='ref'>Gardez <code>{literal}</code><a role='doc-noteref' href='#n'><sup>1</sup></a><img data-papercut-asset='{}'> fin.</p><p id='n' role='doc-footnote'>Note<a role='doc-backlink' href='#ref'>Retour</a>.</p>", asset.file_name),
        std::slice::from_ref(&asset),
        1,
        |_| Ok(()),
    )
    .unwrap();
    let source = load_translation_source_document(app.handle(), "/uploads/abc123.html").unwrap();
    let plan = crate::translation::dom::DomPlan::new(&source.view_html);
    let first = &plan.blocks[0];
    let protected = ProtectedSource::for_boundaries(&first.node)
        .unwrap()
        .fragment(0..first.inline.text.chars().count(), &first.inline.text)
        .unwrap();
    let transport = protected
        .html
        .replacen("Gardez ", "Keep  \n\t", 1)
        .replacen(" fin.", " end.", 1);
    let restored = protected.restore_boundaries(&transport).unwrap();
    let mut request = request(&source);
    request.translated_sections[0].text = restored.text.clone();
    request.translated_sections[0].fragments[0].text = restored.text;
    request.translated_sections[0].fragments[0].needs_review = false;
    request.translated_sections[0].fragments[0].markup = Some(transport);
    let info = persist_translated_document(app.handle(), request, || Ok(())).unwrap();
    let before = published(app.handle(), &dir.0, &info.id);
    let immutable = review_evidence(app.handle(), &info.id);
    let draft = load(&window, &info.id).unwrap();
    let english = draft["document"]["blocks"][0]["text"].as_str().unwrap();
    assert!(english.starts_with("Keep  \n\t"));
    assert!(english.contains(literal));
    let start = english[..english.find("API").unwrap()].chars().count();
    let end = start + literal.chars().count();
    assert!(draft["document"]["blocks"][0]["locked"]
        .as_array()
        .unwrap()
        .contains(&json!({"start":start,"end":end})));
    assert!(save(
        &window,
        &edit(
            &draft,
            0,
            json!({"action":"replaceMarks","marks":[{"range":{"start":start+1,"end":start+2},"kind":"bold"}]}),
        ),
    )
    .is_err());
    assert_eq!(load(&window, &info.id).unwrap(), draft);
    let changed = save(
        &window,
        &edit(
            &draft,
            0,
            json!({"action":"replaceMarks","marks":[{"range":{"start":0,"end":4},"kind":"italic"}]}),
        ),
    )
    .unwrap();
    let original = crate::translation::dom::DomPlan::new(std::str::from_utf8(&before.0).unwrap());
    let preview = crate::translation::dom::DomPlan::new(changed["previewHtml"].as_str().unwrap());
    assert_eq!(preview.blocks[0].node.text_contents(), english);
    assert_eq!(
        preview.blocks[0]
            .node
            .select_first("i")
            .unwrap()
            .text_contents(),
        "Keep"
    );
    for selector in ["code", "a[role='doc-noteref']", "img"] {
        assert_eq!(
            preview.blocks[0]
                .node
                .select_first(selector)
                .unwrap()
                .as_node()
                .to_string(),
            original.blocks[0]
                .node
                .select_first(selector)
                .unwrap()
                .as_node()
                .to_string(),
            "{selector} must remain byte-identical"
        );
    }
    assert_eq!(load(&window, &info.id).unwrap(), changed);
    assert_eq!(review_evidence(app.handle(), &info.id), immutable);
    assert_eq!(published(app.handle(), &dir.0, &info.id), before);
    assert_eq!(
        fs::read(dir.0.join(format!(
            "document_uploads/{}/assets/{}",
            info.id, asset.file_name
        )))
        .unwrap(),
        asset.bytes
    );
}

#[test]
fn whitespace_coverage_check_still_rejects_changed_words_and_interpreted_decoder_markup() {
    let (_dir, app, _window) = fixture();
    let source = source(app.handle());
    let english = "A  house <b>safe</b>.";
    let mut request = request(&source);
    request.translated_sections[0].text = english.into();
    request.translated_sections[0].fragments[0].text = english.into();
    let html = crate::document_uploads::sanitize_html(
        &crate::translation::render::render_translated_html("English", &request).unwrap(),
    );
    let capture = |html: &str| {
        crate::translation::review::Snapshot::capture(
            &request,
            "abc124".into(),
            "12".repeat(16),
            html,
        )
    };
    assert!(capture(&html).is_ok());
    for changed in [
        html.replacen("A  house", "A  home", 1),
        html.replacen("&lt;b&gt;safe&lt;/b&gt;", "<b>safe</b>", 1),
    ] {
        assert_ne!(changed, html);
        assert!(capture(&changed).is_err());
    }
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
