use super::*;
use crate::document_uploads::{
    parsed::{ParsedDocument, ParsedSection},
    storage::StoredSourceKind,
    store::{
        delete_document_rows, open_db_in, update_document_title, upsert_document, PdfTextStatus,
    },
};
use std::{cell::Cell, path::PathBuf};

struct Library(PathBuf);

impl Library {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!(
                "papercut-semantic-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )))
    }
    fn db(&self) -> Connection {
        open_db_in(&self.0).unwrap()
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn identity() -> Identity {
    Identity::new("synthetic-encoder/tokenizer-v1", 2).unwrap()
}

fn put(db: &mut Connection, id: &str, title: &str, texts: &[&str]) {
    let parsed = ParsedDocument {
        title: title.into(),
        format: "html".into(),
        view_html: String::new(),
        sections: texts
            .iter()
            .enumerate()
            .map(|(index, text)| ParsedSection {
                heading: Some(format!("Section {index}")),
                text: (*text).into(),
                page_index: Some(index as u32),
            })
            .collect(),
        cover: None,
        assets: Vec::new(),
    };
    upsert_document(
        db,
        id,
        &format!("/uploads/{id}.html"),
        &parsed,
        None,
        StoredSourceKind::Html,
        1,
        1,
        PdfTextStatus::Ready,
    )
    .unwrap();
}

fn sync(db: &mut Connection, identity: &Identity, id: &str) -> (usize, usize) {
    let source = source(db, id).unwrap().unwrap();
    let mut embedded = 0;
    let mut reused = 0;
    synchronize(
        db,
        identity,
        &source,
        |_, _, text| Ok(vec![0..text.len()]),
        |_| Ok(vec![1.0, 0.0]),
        || Ok(()),
        |old| if old { reused += 1 } else { embedded += 1 },
    )
    .unwrap();
    (embedded, reused)
}

#[test]
fn optional_storage_reuses_passages_across_reimport_and_cascades_delete() {
    let library = Library::new();
    let mut db = library.db();
    put(
        &mut db,
        "abc123",
        "Transit guide",
        &["Electric buses", "😀 اتجاه المدينة", "城市交通"],
    );
    let identity = identity();
    assert!(
        !exists(&db).unwrap(),
        "normal import must not create semantic storage"
    );
    assert_eq!(
        coverage(&mut db, Some(&identity))
            .unwrap()
            .eligible_documents,
        1
    );
    assert!(
        !exists(&db).unwrap(),
        "coverage must not create semantic storage"
    );
    initialize(&mut db, &identity).unwrap();
    assert_eq!(sync(&mut db, &identity, "abc123"), (3, 0));
    assert_eq!(
        sync(&mut db, &identity, "abc123"),
        (0, 0),
        "fully indexed documents are skipped"
    );

    // Reopening through the normal store path retains triggers without any provider.
    drop(db);
    let mut db = library.db();
    put(
        &mut db,
        "abc123",
        "Transit guide",
        &["Electric buses", "😀 اتجاه المدينة", "城市交通"],
    );
    assert_eq!(
        coverage(&mut db, Some(&identity))
            .unwrap()
            .indexed_documents,
        0
    );
    assert_eq!(
        sync(&mut db, &identity, "abc123"),
        (0, 3),
        "replacement section row IDs must not force inference"
    );
    db.execute(
        "UPDATE uploaded_sections SET ordinal=ordinal+10 WHERE document_id='abc123'",
        [],
    )
    .unwrap();
    assert_eq!(
        sync(&mut db, &identity, "abc123"),
        (0, 3),
        "identical embedded content reuses vectors after locator shifts"
    );

    put(
        &mut db,
        "abc123",
        "Transit guide",
        &["Electric buses", "Updated street map"],
    );
    assert_eq!(sync(&mut db, &identity, "abc123"), (1, 1));
    assert_eq!(
        coverage(&mut db, Some(&identity)).unwrap().passages,
        2,
        "removed sections are pruned"
    );
    update_document_title(&mut db, "/uploads/abc123.html", "Travel guide").unwrap();
    assert_eq!(
        coverage(&mut db, Some(&identity))
            .unwrap()
            .indexed_documents,
        0
    );
    assert_eq!(
        sync(&mut db, &identity, "abc123"),
        (2, 0),
        "embedded metadata changes require fresh vectors"
    );

    delete_document_rows(&mut db, "abc123").unwrap();
    let coverage = coverage(&mut db, Some(&identity)).unwrap();
    assert_eq!(
        (
            coverage.indexed_documents,
            coverage.eligible_documents,
            coverage.passages
        ),
        (0, 0, 0)
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM upload_semantic_passages", [], |row| {
            row.get::<_, usize>(0)
        })
        .unwrap(),
        0
    );
}

#[test]
fn cancellation_hides_partial_documents_and_reuses_checkpoints_after_restart() {
    let library = Library::new();
    let mut db = library.db();
    put(
        &mut db,
        "abc123",
        "Guide",
        &["One passage", "Second passage"],
    );
    let identity = identity();
    initialize(&mut db, &identity).unwrap();
    let snapshot = source(&mut db, "abc123").unwrap().unwrap();
    let cancelled = Cell::new(false);
    let result = synchronize(
        &mut db,
        &identity,
        &snapshot,
        |_, _, text| Ok(vec![0..text.len()]),
        |_| Ok(vec![1.0, 0.0]),
        || {
            if cancelled.get() {
                Err("cancelled".into())
            } else {
                Ok(())
            }
        },
        |_| cancelled.set(true),
    );
    assert_eq!(result.unwrap_err(), "cancelled");
    assert_eq!(
        coverage(&mut db, Some(&identity))
            .unwrap()
            .indexed_documents,
        0
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM upload_semantic_passages", [], |row| {
            row.get::<_, usize>(0)
        })
        .unwrap(),
        1
    );
    drop(db);
    let mut db = library.db();
    assert_eq!(sync(&mut db, &identity, "abc123"), (1, 1));
    assert_eq!(
        coverage(&mut db, Some(&identity))
            .unwrap()
            .indexed_documents,
        1
    );
}

#[test]
fn concurrent_edits_deletion_and_identity_roundtrips_reject_stale_writers() {
    let library = Library::new();
    let mut db = library.db();
    let mut writer = library.db();
    let identity = identity();
    put(&mut db, "abc123", "Guide", &["Original directions"]);
    initialize(&mut db, &identity).unwrap();
    let snapshot = source(&mut db, "abc123").unwrap().unwrap();
    let result = synchronize(
        &mut db,
        &identity,
        &snapshot,
        |_, _, text| Ok(vec![0..text.len()]),
        |_| {
            put(&mut writer, "abc123", "Guide", &["Changed directions"]);
            Ok(vec![1.0, 0.0])
        },
        || Ok(()),
        |_| {},
    );
    assert!(result.unwrap_err().contains("source or model changed"));
    assert_eq!(coverage(&mut db, Some(&identity)).unwrap().passages, 0);

    let snapshot = source(&mut db, "abc123").unwrap().unwrap();
    delete_document_rows(&mut writer, "abc123").unwrap();
    put(&mut writer, "abc123", "Guide", &["Changed directions"]);
    assert!(
        check_current(&db, &snapshot, &identity).is_err(),
        "delete/recreate must not resurrect an old job"
    );

    let snapshot = source(&mut db, "abc123").unwrap().unwrap();
    remove(&mut writer).unwrap();
    initialize(&mut writer, &identity).unwrap();
    assert!(
        check_current(&db, &snapshot, &identity).is_err(),
        "remove/rebuild with the same key still invalidates a writer"
    );
    let snapshot = source(&mut db, "abc123").unwrap().unwrap();
    initialize(&mut writer, &Identity::new("other-model", 2).unwrap()).unwrap();
    initialize(&mut writer, &identity).unwrap();
    assert!(
        check_current(&db, &snapshot, &identity).is_err(),
        "switching away and back must not accept old work"
    );
}

#[test]
fn source_spans_and_vectors_are_validated_before_publishing() {
    let library = Library::new();
    let mut db = library.db();
    put(&mut db, "abc123", "Guide", &["😀 اتجاه"]);
    let identity = identity();
    initialize(&mut db, &identity).unwrap();
    let snapshot = source(&mut db, "abc123").unwrap().unwrap();
    assert!(synchronize(
        &mut db,
        &identity,
        &snapshot,
        |_, _, _| Ok(vec![1..4]),
        |_| panic!("UTF-8 boundary must fail before inference"),
        || Ok(()),
        |_| {},
    )
    .is_err());
    assert!(synchronize(
        &mut db,
        &identity,
        &snapshot,
        |_, _, _| Ok(vec![]),
        |_| panic!("empty spans must not publish coverage"),
        || Ok(()),
        |_| {},
    )
    .is_err());
    for vector in [
        vec![1.0],
        vec![f32::NAN, 0.0],
        vec![f32::INFINITY, 0.0],
        vec![0.0, 0.0],
        vec![2.0, 0.0],
    ] {
        assert!(synchronize(
            &mut db,
            &identity,
            &snapshot,
            |_, _, text| Ok(vec![0..text.len()]),
            |_| Ok(vector.clone()),
            || Ok(()),
            |_| {},
        )
        .is_err());
    }
    assert_eq!(coverage(&mut db, Some(&identity)).unwrap().passages, 0);
    assert_eq!(sync(&mut db, &identity, "abc123"), (1, 0));
    let snapshot = source(&mut db, "abc123").unwrap().unwrap();
    db.execute(
        "UPDATE uploaded_sections SET page_index=7 WHERE document_id='abc123'",
        [],
    )
    .unwrap();
    assert!(check_current(&db, &snapshot, &identity).is_err());
    assert_eq!(
        sync(&mut db, &identity, "abc123"),
        (0, 1),
        "locator changes reuse identical embedded text"
    );
    db.execute(
        "UPDATE uploaded_sections SET text='😀 اتجاه',page_index=8 WHERE document_id='abc123'",
        [],
    )
    .unwrap();
    let changed_vector = [0.0f32, 1.0]
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect::<Vec<_>>();
    db.execute(
        "UPDATE upload_semantic_passages SET vector=?1",
        [changed_vector],
    )
    .unwrap();
    let snapshot = source(&mut db, "abc123").unwrap().unwrap();
    assert!(
        synchronize(
            &mut db,
            &identity,
            &snapshot,
            |_, _, text| Ok(vec![0..text.len()]),
            |_| panic!("corrupt cache must fail explicitly"),
            || Ok(()),
            |_| {},
        )
        .is_err(),
        "even a changed unit-normalized vector fails its checksum"
    );
    assert_eq!(
        coverage(&mut db, Some(&identity))
            .unwrap()
            .indexed_documents,
        0
    );
}

#[test]
fn incompatible_identity_rebuilds_only_derived_data_and_unknown_schema_fails_closed() {
    let library = Library::new();
    let mut db = library.db();
    put(&mut db, "abc123", "Guide", &["Source body"]);
    let identity = identity();
    initialize(&mut db, &identity).unwrap();
    sync(&mut db, &identity, "abc123");
    let other = Identity::new("synthetic-encoder/tokenizer-v2", 2).unwrap();
    assert_eq!(
        coverage(&mut db, Some(&other)).unwrap().indexed_documents,
        0
    );
    initialize(&mut db, &other).unwrap();
    assert_eq!(sync(&mut db, &other, "abc123"), (1, 0));
    assert_eq!(
        db.query_row("SELECT text FROM uploaded_sections", [], |row| row
            .get::<_, String>(0))
            .unwrap(),
        "Source body"
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM uploaded_document_fts", [], |row| {
            row.get::<_, usize>(0)
        })
        .unwrap(),
        1
    );
    db.execute("UPDATE upload_semantic_metadata SET schema_version=99", [])
        .unwrap();
    assert!(initialize(&mut db, &identity).is_err());
    assert!(remove(&mut db).is_err());
    assert_eq!(
        coverage(&mut db, Some(&other)).unwrap().indexed_documents,
        0
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM upload_semantic_passages", [], |row| {
            row.get::<_, usize>(0)
        })
        .unwrap(),
        1,
        "unknown schema must not be destructively reset"
    );
}
