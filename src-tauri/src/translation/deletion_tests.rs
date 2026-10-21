//! Synthetic real-IPC deletion checks; no model or private document is needed.
use super::{fixture, invoke};
use crate::document_uploads::{
    open_document_uploads_db, persist_derived_document, ParsedDocumentAsset,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path};
use tauri::test::MockRuntime;

#[path = "case_identity_tests.rs"]
mod case_identity_tests;
#[path = "mutation_tests.rs"]
mod mutation_tests;

const SOURCE: &str = "abc123";
const VARIANT: &str = "def456";

/// Exercise real PDF identity/storage without rendering or interpreting the sentinel bytes.
fn seed_pdf(app: &tauri::AppHandle<MockRuntime>) -> String {
    let pdf = b"%PDF-1.7\nSynthetic source sentinel\n%%EOF".to_vec();
    let id = format!("{:x}", Sha256::digest(&pdf));
    crate::document_uploads::restore_transferred_pdf(
        app,
        id.clone(),
        "PDF".into(),
        None,
        pdf.clone(),
        1,
        pdf.len() as u64,
    )
    .unwrap();
    id
}

/// Seed the real HTML/asset/index writer and the existing v1 provenance schema.
fn seed(app: &tauri::AppHandle<MockRuntime>, id: &str, source: Option<&str>, format: &str) {
    crate::translation::storage::list_translated_documents(app).unwrap();
    let mut db = open_document_uploads_db(app).unwrap();
    let asset = ParsedDocumentAsset::new("image/png", b"synthetic image".to_vec()).unwrap();
    persist_derived_document(app, &mut db, id, &format!("/uploads/{id}.html"), id,
        format, format!("<p>Stored words <strong>with emphasis</strong><img data-papercut-asset='{}'></p>", asset.file_name),
        &[asset], 1, |tx| {
            if let Some(source) = source {
                tx.execute("INSERT INTO translated_documents (id, source_document_id,
                    source_document_url, title, source_language, target_language, model_id,
                    engine_id, quality_mode, settings_json, status, source_path, created_at_ms, updated_at_ms)
                    VALUES (?1, ?2, ?3, 'English', 'fr', 'en', 'opus-mt-fr-en-ctranslate2',
                    'ctranslate2', 'balanced', '{}', 'ready', ?4, 1, 1)",
                    rusqlite::params![id, source, format!("/uploads/{source}.html"), format!("/uploads/{id}.html")])
                    .unwrap();
            }
            Ok(())
        }).unwrap();
}

/// Compare reader/assets and queryable rows, avoiding SQLite's changing WAL bytes.
fn snapshot(
    root: &Path,
    app: &tauri::AppHandle<MockRuntime>,
) -> (BTreeMap<String, Vec<u8>>, Vec<Vec<String>>) {
    fn files(root: &Path, at: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(at).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                files(root, &path, out);
            } else if !path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("search.sqlite3")
            {
                out.insert(
                    path.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                    fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut contents = BTreeMap::new();
    files(root, root, &mut contents);
    let db = open_document_uploads_db(app).unwrap();
    let rows = [
        "uploaded_documents",
        "uploaded_sections",
        "uploaded_document_fts",
        "uploaded_document_search_fts",
        "translated_documents",
    ]
    .iter()
    .map(|table| {
        let mut stmt = db
            .prepare(&format!("SELECT * FROM {table} ORDER BY 1, 2"))
            .unwrap();
        let cols = stmt.column_count();
        stmt.query_map([], |row| {
            Ok((0..cols)
                .map(|i| format!("{:?}", row.get_ref(i).unwrap()))
                .collect::<Vec<_>>()
                .join("|"))
        })
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
    })
    .collect();
    (contents, rows)
}

fn delete(window: &tauri::WebviewWindow<MockRuntime>, id: &str) -> Result<Value, Value> {
    invoke(
        window,
        "translation_delete_document",
        json!({"request":{"id":id}}),
    )
}

#[test]
fn translation_delete_rejects_malformed_ids_before_creating_storage() {
    let (dir, _app, window) = fixture();
    for id in [
        "",
        ".",
        "..",
        "../abc123",
        "/tmp",
        "abc/123",
        "abc\\123",
        "abc?123",
        "abc#123",
        "abc%2f123",
        "abc\0",
        "é",
        "abc.html",
    ] {
        assert!(delete(&window, id).is_err(), "accepted {id:?}");
        assert!(
            !dir.0.join("document_uploads").exists(),
            "invalid identity reached storage"
        );
    }
}

#[test]
fn translation_delete_cannot_remove_original_html_epub_or_pdf_or_unknown_storage() {
    let (dir, app, window) = fixture();
    for (id, format) in [(SOURCE, "html"), ("abc124", "epub")] {
        seed(app.handle(), id, None, format);
    }
    let pdf_id = seed_pdf(app.handle());
    seed(app.handle(), VARIANT, Some(SOURCE), "html");
    // Even unindexed live/staged files do not prove translation ownership.
    let root = dir.0.join("document_uploads");
    for name in ["feed", ".feed.deleting", ".face.deleting"] {
        fs::create_dir(root.join(name)).unwrap();
        fs::write(root.join(name).join("sentinel"), b"unowned").unwrap();
    }
    let before = snapshot(&dir.0, app.handle());
    for id in [
        SOURCE,
        "abc124",
        pdf_id.as_str(),
        "feed",
        "face",
        "deadbeef",
    ] {
        let result = delete(&window, id).unwrap();
        assert_eq!(result["deleted"], false);
        assert_eq!(result["bytesFreed"], 0);
        assert_eq!(snapshot(&dir.0, app.handle()), before, "modified {id}");
    }
    for id in ["", "../abc123", "abc123#suffix"] {
        assert!(delete(&window, id).is_err());
        assert_eq!(snapshot(&dir.0, app.handle()), before);
    }
}

#[test]
fn translation_delete_serializes_with_pending_audiobook_reference_publication() {
    use std::sync::mpsc;
    use std::time::Duration;
    let (dir, app, _window) = fixture();
    seed(app.handle(), SOURCE, None, "html");
    seed(app.handle(), VARIANT, Some(SOURCE), "html");
    let (started_tx, started_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let handle = app.handle().clone();
    let join = crate::native_tts::with_audiobook_reference_lock(|| {
        let join = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            done_tx
                .send(crate::translation::storage::delete_translated_document(
                    &handle, VARIANT,
                ))
                .unwrap();
        });
        started_rx.recv().unwrap();
        // The publisher owns the same short lock used by real audiobook saves.
        let premature = done_rx.recv_timeout(Duration::from_millis(100));
        let audio = dir.0.join("audiobooks/pending");
        fs::create_dir_all(&audio).unwrap();
        fs::write(
            audio.join("manifest.json"),
            json!({"documentUrl":"/uploads/def456.html"}).to_string(),
        )
        .unwrap();
        Ok((join, premature))
    })
    .unwrap();
    join.0.join().unwrap();
    assert!(
        matches!(join.1, Err(mpsc::RecvTimeoutError::Timeout)),
        "deletion bypassed the publisher lock"
    );
    assert!(done_rx
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .unwrap_err()
        .contains("saved audio"));
    assert!(dir.0.join("document_uploads/def456/source.html").is_file());
}

#[test]
fn owned_metadata_without_files_is_removable_and_conflicting_staging_rolls_back() {
    let (dir, app, window) = fixture();
    seed(app.handle(), SOURCE, None, "html");
    seed(app.handle(), VARIANT, Some(SOURCE), "html");
    let staged = dir.0.join("document_uploads/.def456.deleting");
    fs::create_dir(&staged).unwrap();
    fs::write(staged.join("sentinel"), b"conflict").unwrap();
    let before = snapshot(&dir.0, app.handle());
    assert!(delete(&window, VARIANT).is_err());
    assert_eq!(snapshot(&dir.0, app.handle()), before);
    fs::remove_dir_all(staged).unwrap();
    fs::remove_dir_all(dir.0.join("document_uploads/def456")).unwrap();
    let result = delete(&window, VARIANT).unwrap();
    assert_eq!(result["deleted"], true);
    assert_eq!(result["bytesFreed"], 0);
    assert_eq!(
        invoke(&window, "translation_list_documents", json!({})).unwrap(),
        json!([])
    );
}

#[test]
fn translation_and_library_deletion_remove_only_the_owned_variant_including_batch_and_folder() {
    for route in ["translation", "single", "batch", "folder"] {
        let (dir, app, window) = fixture();
        seed(app.handle(), SOURCE, None, "epub");
        seed(app.handle(), VARIANT, Some(SOURCE), "html");
        let source_dir = dir.0.join("document_uploads/abc123");
        let source_files = snapshot(&source_dir, app.handle()).0;
        let result = match route {
            "translation" => delete(&window, VARIANT),
            "single" => invoke(
                &window,
                "document_uploads_delete",
                json!({"request":{"documentUrl":"/uploads/def456.html"}}),
            ),
            "batch" => invoke(
                &window,
                "document_uploads_delete_batch",
                json!({"request":{"documentUrls":["/uploads/def456.html"]}}),
            ),
            _ => {
                let folder = crate::document_uploads::create_folder(
                    app.handle(),
                    crate::document_uploads::UploadedLibraryCreateFolderRequest {
                        parent_id: None,
                        name: "Translations".into(),
                    },
                )
                .unwrap();
                crate::document_uploads::move_documents(
                    app.handle(),
                    crate::document_uploads::UploadedLibraryMoveDocumentsRequest {
                        document_ids: vec![VARIANT.into()],
                        folder_id: Some(folder.id.clone()),
                    },
                )
                .unwrap();
                invoke(
                    &window,
                    "document_uploads_delete_folder",
                    json!({"request":{"folderId":folder.id}}),
                )
            }
        }
        .unwrap();
        assert!(
            result["bytesFreed"].as_u64().unwrap() > 0,
            "{route}: {result}"
        );
        assert!(!dir.0.join("document_uploads/def456").exists());
        assert!(!dir.0.join("document_uploads/.def456.deleting").exists());
        assert_eq!(snapshot(&source_dir, app.handle()).0, source_files);
        assert_eq!(
            invoke(&window, "translation_list_documents", json!({})).unwrap(),
            json!([])
        );
        let db = open_document_uploads_db(app.handle()).unwrap();
        for (table, column) in [
            ("uploaded_documents", "id"),
            ("uploaded_sections", "document_id"),
            ("uploaded_document_fts", "document_id"),
            ("uploaded_document_search_fts", "document_id"),
        ] {
            let count: usize = db
                .query_row(
                    &format!("SELECT count(*) FROM {table} WHERE {column}=?1"),
                    [VARIANT],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(count, 0, "{route}: {table}");
            let count: usize = db
                .query_row(
                    &format!("SELECT count(*) FROM {table} WHERE {column}=?1"),
                    [SOURCE],
                    |r| r.get(0),
                )
                .unwrap();
            assert!(count > 0, "{route}: source {table}");
        }
        assert_eq!(
            delete(&window, VARIANT).unwrap()["deleted"],
            false,
            "safe repeat"
        );
    }
}

#[test]
fn translation_delete_preserves_pending_and_completed_audiobook_sources() {
    for status in ["generating", "completed"] {
        let (dir, app, window) = fixture();
        seed(app.handle(), SOURCE, None, "html");
        seed(app.handle(), VARIANT, Some(SOURCE), "html");
        let audio = dir.0.join("audiobooks/example");
        fs::create_dir_all(&audio).unwrap();
        fs::write(
            audio.join("manifest.json"),
            json!({"documentUrl":"/uploads/def456.html", "status":status}).to_string(),
        )
        .unwrap();
        fs::write(audio.join("audio.wav"), b"existing audio sentinel").unwrap();
        fs::write(audio.join("timings.json"), b"existing highlight sentinel").unwrap();
        let before = snapshot(&dir.0, app.handle());
        let error = delete(&window, VARIANT).unwrap_err();
        assert!(
            error["message"].as_str().unwrap().contains("saved audio"),
            "{error}"
        );
        assert_eq!(snapshot(&dir.0, app.handle()), before);
        // An unrelated reference must not block this variant once its own audio is removed.
        fs::write(
            audio.join("manifest.json"),
            json!({"documentUrl":"/uploads/abc123.html"}).to_string(),
        )
        .unwrap();
        assert_eq!(delete(&window, VARIANT).unwrap()["deleted"], true);
        assert_eq!(
            fs::read(audio.join("audio.wav")).unwrap(),
            b"existing audio sentinel"
        );
    }
}

#[test]
fn deleting_original_retains_independent_translation_and_clears_source_fk() {
    let (dir, app, window) = fixture();
    seed(app.handle(), SOURCE, None, "html");
    seed(app.handle(), VARIANT, Some(SOURCE), "html");
    let variant_dir = dir.0.join("document_uploads/def456");
    let before = snapshot(&variant_dir, app.handle()).0;
    invoke(
        &window,
        "document_uploads_delete",
        json!({"request":{"documentUrl":"/uploads/abc123.html"}}),
    )
    .unwrap();
    assert_eq!(snapshot(&variant_dir, app.handle()).0, before);
    let db = open_document_uploads_db(app.handle()).unwrap();
    let source: Option<String> = db
        .query_row(
            "SELECT source_document_id FROM translated_documents WHERE id=?1",
            [VARIANT],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(source, None);
    assert_eq!(
        invoke(&window, "translation_list_documents", json!({}))
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn failed_delete_rolls_back_files_provenance_and_index_then_recovers_and_retries() {
    for route in ["translation", "library"] {
        let (dir, app, window) = fixture();
        seed(app.handle(), SOURCE, None, "html");
        seed(app.handle(), VARIANT, Some(SOURCE), "html");
        let db = open_document_uploads_db(app.handle()).unwrap();
        db.execute_batch(
            "CREATE TRIGGER reject_delete BEFORE DELETE ON uploaded_documents WHEN OLD.id='def456'
            BEGIN SELECT RAISE(ABORT, 'injected deletion failure'); END;",
        )
        .unwrap();
        let before = snapshot(&dir.0, app.handle());
        let attempt = || {
            if route == "translation" {
                delete(&window, VARIANT)
            } else {
                invoke(
                    &window,
                    "document_uploads_delete",
                    json!({"request":{"documentUrl":"/uploads/def456.html"}}),
                )
            }
        };
        assert!(attempt().is_err());
        assert_eq!(snapshot(&dir.0, app.handle()), before, "{route}");
        // Simulate interruption after files were staged but before the DB committed.
        fs::rename(
            dir.0.join("document_uploads/def456"),
            dir.0.join("document_uploads/.def456.deleting"),
        )
        .unwrap();
        assert!(attempt().is_err());
        assert_eq!(
            snapshot(&dir.0, app.handle()),
            before,
            "{route} staged rollback"
        );
        db.execute_batch("DROP TRIGGER reject_delete").unwrap();
        attempt().unwrap();
        assert_eq!(
            invoke(&window, "translation_list_documents", json!({})).unwrap(),
            json!([])
        );
        assert!(!dir.0.join("document_uploads/def456").exists());
    }
}

#[test]
fn library_deletion_uses_stored_identity_for_url_aliases_and_rejects_wrong_kind() {
    for kind in ["html", "pdf"] {
        for reference_suffix in ["", "?audio=1", "#chapter"] {
            for request_suffix in ["?view=reader", "#section", "wrong-kind", ""] {
                for route in ["single", "batch"] {
                    let (dir, app, window) = fixture();
                    seed(app.handle(), SOURCE, None, "html");
                    let id = if kind == "html" {
                        seed(app.handle(), VARIANT, Some(SOURCE), "html");
                        VARIANT.to_string()
                    } else {
                        seed_pdf(app.handle())
                    };
                    let canonical = format!("/uploads/{id}.{kind}");
                    let requested = if request_suffix == "wrong-kind" {
                        format!(
                            "/uploads/{id}.{}",
                            if kind == "html" { "pdf" } else { "html" }
                        )
                    } else {
                        format!("{canonical}{request_suffix}")
                    };
                    let audio = dir.0.join("audiobooks/example");
                    fs::create_dir_all(&audio).unwrap();
                    fs::write(
                        audio.join("manifest.json"),
                        json!({"documentUrl":format!("{canonical}{reference_suffix}")}).to_string(),
                    )
                    .unwrap();
                    fs::write(audio.join("audio.wav"), b"audio sentinel").unwrap();
                    fs::write(audio.join("timings.json"), b"timing sentinel").unwrap();
                    let before = snapshot(&dir.0, app.handle());
                    let attempt = |url: &str| {
                        if route == "single" {
                            invoke(
                                &window,
                                "document_uploads_delete",
                                json!({"request":{"documentUrl":url}}),
                            )
                        } else {
                            invoke(
                                &window,
                                "document_uploads_delete_batch",
                                json!({"request":{"documentUrls":[url]}}),
                            )
                        }
                    };
                    let result = attempt(&requested);
                    assert!(
                        result.is_err()
                            || result.as_ref().unwrap()["failures"]
                                .as_array()
                                .is_some_and(|v| v.len() == 1),
                        "{kind} {route} {reference_suffix} {request_suffix}: {result:?}"
                    );
                    assert_eq!(
                        snapshot(&dir.0, app.handle()),
                        before,
                        "referenced source changed"
                    );
                    fs::remove_file(audio.join("manifest.json")).unwrap();
                    if request_suffix == "wrong-kind" {
                        let before = snapshot(&dir.0, app.handle());
                        let result = attempt(&requested);
                        assert!(
                            result.is_err()
                                || result.unwrap()["failures"]
                                    .as_array()
                                    .is_some_and(|v| v.len() == 1)
                        );
                        assert_eq!(
                            snapshot(&dir.0, app.handle()),
                            before,
                            "wrong-kind request changed files"
                        );
                    }
                    let result = attempt(&format!("{canonical}?view=reader#section")).unwrap();
                    let deleted = if route == "single" {
                        &result
                    } else {
                        &result["deleted"][0]
                    };
                    assert_eq!(
                        deleted["url"], canonical,
                        "return the stored identity for refresh"
                    );
                    assert!(!dir.0.join("document_uploads").join(&id).exists());
                    assert_eq!(
                        fs::read(audio.join("audio.wav")).unwrap(),
                        b"audio sentinel"
                    );
                    assert_eq!(
                        fs::read(audio.join("timings.json")).unwrap(),
                        b"timing sentinel"
                    );
                }
            }
        }
    }
}

#[test]
fn library_retry_without_metadata_keeps_audio_protection_and_recovers_staging() {
    for kind in ["html", "pdf"] {
        let (dir, app, window) = fixture();
        seed(app.handle(), SOURCE, None, "html");
        // Simulate committed DB deletion with filesystem cleanup interrupted.
        let staged = dir.0.join("document_uploads/.feed.deleting");
        fs::create_dir(&staged).unwrap();
        fs::write(staged.join(format!("source.{kind}")), b"staged source").unwrap();
        let audio = dir.0.join("audiobooks/example");
        fs::create_dir_all(&audio).unwrap();
        fs::write(
            audio.join("manifest.json"),
            json!({"documentUrl":format!("/uploads/feed.{kind}?reader=1#part")}).to_string(),
        )
        .unwrap();
        let request = json!({"request":{"documentUrl":format!("/uploads/feed.{kind}?retry=1")}});
        let before = snapshot(&dir.0, app.handle());
        assert!(invoke(&window, "document_uploads_delete", request.clone()).is_err());
        assert_eq!(snapshot(&dir.0, app.handle()), before);
        fs::remove_file(audio.join("manifest.json")).unwrap();
        let result = invoke(&window, "document_uploads_delete", request).unwrap();
        assert_eq!(result["url"], format!("/uploads/feed.{kind}"));
        assert_eq!(result["bytesFreed"], b"staged source".len());
        assert!(!staged.exists());
        assert!(!dir.0.join("document_uploads/feed").exists());
        assert!(dir.0.join("document_uploads/abc123/source.html").is_file());
    }
}
