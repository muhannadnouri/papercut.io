//! Library labels remain independent of immutable translation provenance during publication.
use super::{
    fixture, invoke,
    review_command_tests::{edit, load, published, request, save, source},
};
use crate::{
    document_uploads::open_document_uploads_db,
    translation::{
        review, review_store,
        storage::{persist_translated_document, prepare_publication, publish_prepared},
    },
};
use serde_json::{json, Value};
use tauri::test::MockRuntime;

fn finish(window: &tauri::WebviewWindow<MockRuntime>, draft: &Value) -> Result<Value, Value> {
    invoke(
        window,
        "translation_review_finish",
        json!({"request":draft["document"]["expected"].to_string()}),
    )
}

fn approve_all(window: &tauri::WebviewWindow<MockRuntime>, mut draft: Value) -> Value {
    for block in 0..draft["document"]["blocks"].as_array().unwrap().len() {
        draft = save(window, &edit(&draft, block, json!({"action":"approve"}))).unwrap();
    }
    draft
}

/// Exercise the real Library rename command, including FTS/spelling changes, rather than SQL alone.
fn rename(window: &tauri::WebviewWindow<MockRuntime>, url: &str, title: &str) {
    let result = invoke(
        window,
        "document_uploads_update_title",
        json!({"request":{"documentUrl":url,"title":title}}),
    )
    .unwrap();
    assert_eq!(result["title"], title);
}

fn evidence(
    app: &tauri::AppHandle<MockRuntime>,
    id: &str,
) -> (String, i64, Option<i64>, Option<String>) {
    let db = open_document_uploads_db(app).unwrap();
    db.query_row("SELECT document_json,revision,published_revision,published_hash FROM translation_review WHERE variant_id=?1", [id], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).unwrap()
}

/// Compare stable section/FTS/spelling identities; unchanged inputs must not rebuild any projection.
fn projection(
    app: &tauri::AppHandle<MockRuntime>,
    id: &str,
) -> Vec<Vec<Vec<rusqlite::types::Value>>> {
    let db = open_document_uploads_db(app).unwrap();
    [
        "SELECT * FROM uploaded_sections WHERE document_id=?1 ORDER BY id",
        "SELECT rowid,* FROM uploaded_document_fts WHERE document_id=?1 ORDER BY rowid",
        "SELECT rowid,* FROM uploaded_document_search_fts WHERE document_id=?1 ORDER BY rowid",
        "SELECT * FROM uploaded_spelling_terms WHERE document_id=?1 ORDER BY term,source",
        "SELECT * FROM uploaded_spelling_indexed WHERE document_id=?1",
    ]
    .iter()
    .map(|query| {
        let mut statement = db.prepare(query).unwrap();
        let columns = statement.column_count();
        statement
            .query_map([id], |row| {
                (0..columns).map(|column| row.get(column)).collect()
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    })
    .collect()
}

fn assert_title(app: &tauri::AppHandle<MockRuntime>, id: &str, title: &str, provenance: &str) {
    let db = open_document_uploads_db(app).unwrap();
    let labels: (String, String) = db.query_row("SELECT u.title,t.title FROM uploaded_documents u JOIN translated_documents t ON t.id=u.id WHERE u.id=?1", [id], |row| Ok((row.get(0)?,row.get(1)?))).unwrap();
    assert_eq!(labels, (title.into(), provenance.into()));
}

#[test]
fn finish_preserves_library_renames_before_publication_repeat_and_later_draft_edits() {
    let (dir, app, window) = fixture();
    let source = source(app.handle());
    let info = persist_translated_document(app.handle(), request(&source), || Ok(())).unwrap();
    let draft = load(&window, &info.id).unwrap();
    let changed = save(
        &window,
        &edit(
            &draft,
            0,
            json!({"action":"replaceMarks","marks":[{"range":{"start":2,"end":7},"kind":"bold"}]}),
        ),
    )
    .unwrap();
    let ready = approve_all(&window, changed);
    rename(&window, &info.document_url, "My saved Library label");
    let indexed = projection(app.handle(), &info.id);
    // The same unchanged-title condition is what keeps conceptual-search coverage ready.
    let db = open_document_uploads_db(app.handle()).unwrap();
    db.execute_batch("CREATE TRIGGER retain_library_title BEFORE UPDATE OF title ON uploaded_documents WHEN old.title != new.title BEGIN SELECT RAISE(ABORT,'publication changed user title'); END;").unwrap();
    let first = finish(&window, &ready).unwrap();
    assert_eq!(first["title"], "My saved Library label");
    assert_title(
        app.handle(),
        &info.id,
        "My saved Library label",
        &info.title,
    );
    assert_eq!(projection(app.handle(), &info.id), indexed);
    assert!(
        std::str::from_utf8(&published(app.handle(), &dir.0, &info.id).0)
            .unwrap()
            .contains("<b>house</b>")
    );
    let first_files = published(app.handle(), &dir.0, &info.id);
    let receipt = evidence(app.handle(), &info.id);
    db.execute_batch("DROP TRIGGER retain_library_title")
        .unwrap();
    rename(&window, &info.document_url, "Renamed after Finish");
    let indexed = projection(app.handle(), &info.id);
    let repeated = finish(&window, &ready).unwrap();
    assert_eq!(repeated["title"], "Renamed after Finish");
    assert_eq!(repeated["updatedAtMs"], first["updatedAtMs"]);
    assert_eq!(evidence(app.handle(), &info.id), receipt);
    assert_eq!(published(app.handle(), &dir.0, &info.id), first_files);
    assert_eq!(projection(app.handle(), &info.id), indexed);
    let reset = save(&window, &edit(&ready, 0, json!({"action":"reset"}))).unwrap();
    let approved = save(&window, &edit(&reset, 0, json!({"action":"approve"}))).unwrap();
    let next = finish(&window, &approved).unwrap();
    assert_eq!(next["title"], "Renamed after Finish");
    assert_title(app.handle(), &info.id, "Renamed after Finish", &info.title);
    assert_eq!(projection(app.handle(), &info.id), indexed);
    assert!(
        !std::str::from_utf8(&published(app.handle(), &dir.0, &info.id).0)
            .unwrap()
            .contains("<b>house</b>")
    );
    assert_eq!(finish(&window, &approved).unwrap(), next);
}

#[test]
fn rename_between_preparation_and_promotion_rejects_without_writes_and_retries() {
    let (dir, app, window) = fixture();
    let source = source(app.handle());
    let info = persist_translated_document(app.handle(), request(&source), || Ok(())).unwrap();
    let ready = approve_all(&window, load(&window, &info.id).unwrap());
    let expected: review::Expected =
        serde_json::from_value(ready["document"]["expected"].clone()).unwrap();
    // This is the production prepare/publish boundary, with no sleeps or process-wide test hook.
    let prepared = prepare_publication(app.handle(), &expected)
        .unwrap()
        .unwrap();
    rename(&window, &info.document_url, "Concurrent Library rename");
    let before = published(app.handle(), &dir.0, &info.id);
    let receipt = evidence(app.handle(), &info.id);
    let indexed = projection(app.handle(), &info.id);
    let error = publish_prepared(app.handle(), &expected, prepared).unwrap_err();
    assert_eq!(error, review_store::review_error(review::Error::Stale));
    assert_eq!(published(app.handle(), &dir.0, &info.id), before);
    assert_eq!(evidence(app.handle(), &info.id), receipt);
    assert_eq!(load(&window, &info.id).unwrap(), ready);
    assert_eq!(projection(app.handle(), &info.id), indexed);
    assert_title(
        app.handle(),
        &info.id,
        "Concurrent Library rename",
        &info.title,
    );
    for suffix in ["staging", "replacing"] {
        assert!(!dir
            .0
            .join(format!("document_uploads/.{}.{suffix}", info.id))
            .exists());
    }
    let finished = finish(&window, &ready).unwrap();
    assert_eq!(finished["title"], "Concurrent Library rename");
    assert_eq!(finish(&window, &ready).unwrap(), finished);
    assert_title(
        app.handle(),
        &info.id,
        "Concurrent Library rename",
        &info.title,
    );
    assert_eq!(projection(app.handle(), &info.id), indexed);
}
