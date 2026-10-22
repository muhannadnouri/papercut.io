//! Synthetic durable-store checks use no source files, installed models or inference runtime.

use super::*;
use crate::translation::review::{Edit, Mark, MarkKind, ScalarRange};
use crate::translation::source::{TranslationSourceBlock, TranslationSourceDocument};
use crate::translation::storage::{
    ensure_translation_schema, PersistTranslationFragment, PersistTranslationRequest,
    PersistTranslationSection,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

struct TestDb(PathBuf);

impl TestDb {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Self(std::env::temp_dir().join(format!(
            "papercut-review-{}-{stamp}-{}.sqlite",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }

    fn open(&self) -> Connection {
        let db = Connection::open(&self.0).unwrap();
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS upload_schema_metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS uploaded_documents(id TEXT PRIMARY KEY);",
        )
        .unwrap();
        ensure_translation_schema(&db).unwrap();
        db
    }
}

impl Drop for TestDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn snapshot(id: &str, generation: &str) -> Snapshot {
    let source = crate::document_uploads::sanitize_html(
        "<article><p id='p'><strong>Bonjour</strong> monde.</p><p>Bonjour monde.</p></article>",
    );
    let request = PersistTranslationRequest {
        formatting: crate::translation::formatting::FormattingPolicy::SentenceReview,
        source: TranslationSourceDocument {
            document_id: "source1".into(),
            document_url: "/uploads/source1.html".into(),
            title: "Source".into(),
            format: "html".into(),
            view_html: source,
            source_formatting: None,
            assets: Default::default(),
            blocks: (0..2)
                .map(|ordinal| TranslationSourceBlock {
                    ordinal,
                    is_heading: false,
                    heading: None,
                    text: "Bonjour monde.".into(),
                })
                .collect(),
        },
        source_language: "fr".into(),
        target_language: "en".into(),
        model_id: "opus-mt-fr-en-ctranslate2".into(),
        quality_mode: "balanced".into(),
        repair_mode: Default::default(),
        job_id: "job1".into(),
        glossary: Vec::new(),
        translated_sections: (0..2)
            .map(|ordinal| PersistTranslationSection {
                source_heading: None,
                source_ordinal: ordinal,
                is_heading: false,
                text: "Hello world.".into(),
                fragments: vec![PersistTranslationFragment {
                    needs_review: true,
                    source_start: 0,
                    source_end: "Bonjour monde.".chars().count(),
                    source_text: "Bonjour monde.".into(),
                    text: "Hello world.".into(),
                    inline_phrases: Vec::new(),
                    markup: None,
                }],
            })
            .collect(),
    };
    let html = crate::document_uploads::sanitize_html(
        &crate::translation::render::render_translated_html("English", &request).unwrap(),
    );
    Snapshot::capture(&request, id.into(), generation.into(), &html).unwrap()
}

fn insert(db: &mut Connection, snapshot: &Snapshot) {
    let id = &snapshot.document.expected().variant_id;
    db.execute(
        "INSERT OR IGNORE INTO uploaded_documents(id) VALUES ('source1')",
        [],
    )
    .unwrap();
    let tx = db.transaction().unwrap();
    tx.execute(
        "INSERT INTO translated_documents(id,source_document_id,source_document_url,title,source_language,
          target_language,model_id,engine_id,quality_mode,settings_json,status,source_path,created_at_ms,updated_at_ms)
         VALUES (?1,'source1','/uploads/source1.html','English','fr','en','uninstalled-model','test',
                 'balanced','{}','needs-review',?2,1,1)",
        params![id, format!("/uploads/{id}.html")],
    )
    .unwrap();
    insert_in_transaction(&tx, snapshot).unwrap();
    tx.commit().unwrap();
}

fn edit(snapshot: &Snapshot, ordinal: usize) -> EditRequest {
    let json = serde_json::to_value(&snapshot.document).unwrap();
    EditRequest {
        expected: snapshot.document.expected().clone(),
        block: serde_json::from_value(json["blocks"][ordinal]["key"].clone()).unwrap(),
        edit: Edit::ReplaceMarks {
            marks: vec![Mark {
                range: ScalarRange { start: 0, end: 5 },
                kind: MarkKind::Bold,
            }],
        },
    }
}

#[test]
fn draft_reopens_without_source_or_installed_model_and_only_edits_the_selected_repeated_block() {
    let file = TestDb::new();
    let initial = snapshot("a1b2", &"12".repeat(16));
    let saved_preview;
    let immutable;
    {
        let mut db = file.open();
        insert(&mut db, &initial);
        immutable = db
            .query_row("SELECT snapshot_json FROM translation_review", [], |r| {
                r.get::<_, String>(0)
            })
            .unwrap();
        db.execute("DELETE FROM uploaded_documents WHERE id='source1'", [])
            .unwrap();
        let edited = apply_in(&mut db, &edit(&initial, 1)).unwrap();
        assert_eq!(edited.document.expected().revision, 1);
        saved_preview = edited.reconstruct().unwrap();
        assert_eq!(saved_preview.matches("<b>Hello</b>").count(), 1);
    }
    let db = file.open();
    let resumed = load_in(&db, "a1b2").unwrap().unwrap();
    assert_eq!(resumed.reconstruct().unwrap(), saved_preview);
    assert_eq!(resumed.document.expected().revision, 1);
    assert_eq!(
        db.query_row("SELECT snapshot_json FROM translation_review", [], |r| {
            r.get::<_, String>(0)
        })
        .unwrap(),
        immutable,
        "draft updates must never rewrite immutable source and baseline evidence"
    );
    assert!(resumed.source_preview().unwrap().contains("Bonjour"));
}

#[test]
fn stale_window_and_wrong_generation_reject_without_changing_the_draft() {
    let file = TestDb::new();
    let mut first_window = file.open();
    let initial = snapshot("a1b2", &"12".repeat(16));
    insert(&mut first_window, &initial);
    let mut second_window = file.open();
    let stale = edit(&load_in(&second_window, "a1b2").unwrap().unwrap(), 0);
    let saved = apply_in(&mut first_window, &edit(&initial, 1)).unwrap();
    assert!(apply_in(&mut second_window, &stale)
        .unwrap_err()
        .contains("changed"));
    let mut wrong_generation = edit(&saved, 0);
    wrong_generation.expected.generation = "34".repeat(16);
    assert!(apply_in(&mut second_window, &wrong_generation).is_err());
    assert_eq!(
        load_in(&first_window, "a1b2")
            .unwrap()
            .unwrap()
            .reconstruct()
            .unwrap(),
        saved.reconstruct().unwrap()
    );
}

#[test]
fn deleted_variant_cascades_review_and_stale_save_cannot_resurrect_it() {
    let file = TestDb::new();
    let mut db = file.open();
    let initial = snapshot("a1b2", &"12".repeat(16));
    insert(&mut db, &initial);
    let request = edit(&initial, 0);
    db.execute("DELETE FROM translated_documents WHERE id='a1b2'", [])
        .unwrap();
    assert!(load_in(&db, "a1b2").unwrap().is_none());
    assert!(apply_in(&mut db, &request).is_err());
    assert_eq!(
        db.query_row("SELECT count(*) FROM translation_review", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
}

#[test]
fn generations_coexist_and_old_generation_drafts_cannot_edit_new_variants() {
    let file = TestDb::new();
    let mut db = file.open();
    let old = snapshot("a1b2", &"12".repeat(16));
    let new = snapshot("a1b3", &"34".repeat(16));
    insert(&mut db, &old);
    insert(&mut db, &new);
    let mut wrong = edit(&old, 0);
    wrong.expected.variant_id = "a1b3".into();
    assert!(apply_in(&mut db, &wrong).is_err());
    assert_eq!(
        load_in(&db, "a1b2")
            .unwrap()
            .unwrap()
            .document
            .expected()
            .revision,
        0
    );
    assert_eq!(
        load_in(&db, "a1b3")
            .unwrap()
            .unwrap()
            .document
            .expected()
            .revision,
        0
    );
}

#[test]
fn failed_draft_update_rolls_back_without_mutating_baseline_metadata_or_search_content() {
    let file = TestDb::new();
    let mut db = file.open();
    let initial = snapshot("a1b2", &"12".repeat(16));
    insert(&mut db, &initial);
    db.execute_batch("CREATE TRIGGER fail_review_update BEFORE UPDATE ON translation_review BEGIN SELECT RAISE(ABORT, 'disk failure'); END;").unwrap();
    let before: (String, String) = db
        .query_row(
            "SELECT snapshot_json,document_json FROM translation_review",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert!(apply_in(&mut db, &edit(&initial, 0)).is_err());
    assert_eq!(
        db.query_row(
            "SELECT snapshot_json,document_json FROM translation_review",
            [],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        )
        .unwrap(),
        before
    );
    assert_eq!(
        db.query_row(
            "SELECT status,updated_at_ms FROM translated_documents",
            [],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        )
        .unwrap(),
        ("needs-review".into(), 1)
    );
}

#[test]
fn failed_response_preparation_leaves_the_draft_retryable() {
    let file = TestDb::new();
    let mut db = file.open();
    let initial = snapshot("a1b2", &"12".repeat(16));
    insert(&mut db, &initial);
    let stored = |db: &Connection| {
        db.query_row(
            "SELECT revision,snapshot_json,document_json FROM translation_review",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .unwrap()
    };
    let before = stored(&db);
    let request = edit(&initial, 0);
    let error = apply_prepared_in(&mut db, &request, |snapshot| {
        assert_eq!(snapshot.document.expected().revision, 1);
        assert!(snapshot.reconstruct().unwrap().contains("<b>Hello</b>"));
        Err::<(), _>("Cannot prepare native review response".to_string())
    })
    .unwrap_err();
    assert_eq!(error, "Cannot prepare native review response");
    assert_eq!(stored(&db), before);
    let saved = apply_in(&mut db, &request).unwrap();
    assert_eq!(saved.document.expected().revision, 1);
    assert_eq!(load_in(&db, "a1b2").unwrap().unwrap(), saved);
}

#[test]
fn immutable_document_corruption_rejects_even_when_the_saved_draft_is_valid() {
    let file = TestDb::new();
    let mut db = file.open();
    let initial = snapshot("a1b2", &"12".repeat(16));
    insert(&mut db, &initial);
    let immutable: String = db
        .query_row("SELECT snapshot_json FROM translation_review", [], |row| {
            row.get(0)
        })
        .unwrap();
    let mut corrupt: serde_json::Value = serde_json::from_str(&immutable).unwrap();
    corrupt["document"]["blocks"][0]["text"] = "Forged immutable text.".into();
    db.execute(
        "UPDATE translation_review SET snapshot_json=?1",
        [corrupt.to_string()],
    )
    .unwrap();
    assert!(load_in(&db, "a1b2").is_err());
    assert!(apply_in(&mut db, &edit(&initial, 0)).is_err());
    db.execute(
        "UPDATE translation_review SET snapshot_json=?1",
        [&immutable],
    )
    .unwrap();
    assert_eq!(load_in(&db, "a1b2").unwrap().unwrap(), initial);
}

#[test]
fn corrupted_evidence_or_forged_draft_text_is_rejected_on_reload() {
    let file = TestDb::new();
    let mut db = file.open();
    let initial = snapshot("a1b2", &"12".repeat(16));
    insert(&mut db, &initial);
    let draft: String = db
        .query_row("SELECT document_json FROM translation_review", [], |r| {
            r.get(0)
        })
        .unwrap();
    db.execute(
        "UPDATE translation_review SET document_json=?1",
        [draft.replace("Hello world.", "Forged text.")],
    )
    .unwrap();
    assert!(load_in(&db, "a1b2").is_err());
    db.execute("UPDATE translation_review SET document_json=?1", [&draft])
        .unwrap();
    db.execute(
        "UPDATE translation_review SET snapshot_json=replace(snapshot_json,'Bonjour','Corrupt')",
        [],
    )
    .unwrap();
    assert!(load_in(&db, "a1b2").is_err());
}

#[test]
fn legacy_variant_without_evidence_stays_read_only() {
    let file = TestDb::new();
    let mut db = file.open();
    let initial = snapshot("a1b2", &"12".repeat(16));
    insert(&mut db, &initial);
    db.execute("DELETE FROM translation_review", []).unwrap();
    assert!(load_in(&db, "a1b2").unwrap().is_none());
    assert!(apply_in(&mut db, &edit(&initial, 0)).is_err());
    assert_eq!(
        db.query_row("SELECT title FROM translated_documents", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "English"
    );
}
