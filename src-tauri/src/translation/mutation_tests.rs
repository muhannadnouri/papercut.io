//! Force both save/delete orders through real storage and IPC without model inference.
use super::{fixture, invoke, seed, snapshot, SOURCE, VARIANT};
use crate::document_uploads::{
    open_document_uploads_db, persist_derived_document, ParsedDocumentAsset,
};
use rusqlite::{Connection, Transaction};
use serde_json::{json, Value};
use std::{fs, sync::mpsc, time::Duration};
use tauri::test::MockRuntime;

/// Replace the same variant with distinct HTML/assets so a mixed generation is detectable.
fn replace(
    app: &tauri::AppHandle<MockRuntime>,
    db: &mut Connection,
    at_metadata: impl FnOnce(&Transaction<'_>) -> Result<(), String>,
) -> Result<bool, String> {
    let asset = ParsedDocumentAsset::new("image/png", b"replacement image".to_vec()).unwrap();
    persist_derived_document(
        app,
        db,
        VARIANT,
        "/uploads/def456.html",
        "Replacement",
        "html",
        format!(
            "<p>Replacement words<img data-papercut-asset='{}'></p>",
            asset.file_name
        ),
        &[asset],
        2,
        |tx| {
            tx.execute("INSERT OR REPLACE INTO translated_documents (id,source_document_id,source_document_url,title,source_language,target_language,model_id,engine_id,quality_mode,settings_json,status,source_path,created_at_ms,updated_at_ms)
                VALUES ('def456','abc123','/uploads/abc123.html','Replacement','fr','en','opus-mt-fr-en-ctranslate2','ctranslate2','balanced','{}','ready','/uploads/def456.html',2,2)", []).map_err(|e|e.to_string())?;
            at_metadata(tx)
        },
    )
}

fn remove(window: &tauri::WebviewWindow<MockRuntime>, route: &str) -> Result<Value, Value> {
    match route {
        "translation" => invoke(
            window,
            "translation_delete_document",
            json!({"request":{"id":VARIANT}}),
        ),
        "single" => invoke(
            window,
            "document_uploads_delete",
            json!({"request":{"documentUrl":"/uploads/def456.html"}}),
        ),
        _ => invoke(
            window,
            "document_uploads_delete_batch",
            json!({"request":{"documentUrls":["/uploads/def456.html"]}}),
        ),
    }
}

#[test]
fn save_waits_for_delete_and_publishes_complete_files_rows_and_assets_after_success_or_rollback() {
    for fail_delete in [false, true] {
        let (dir, app, _window) = fixture();
        seed(app.handle(), SOURCE, None, "html");
        seed(app.handle(), VARIANT, Some(SOURCE), "html");
        let mut writer_db = open_document_uploads_db(app.handle()).unwrap();
        if fail_delete {
            writer_db.execute_batch("CREATE TRIGGER reject_delete BEFORE DELETE ON uploaded_documents WHEN OLD.id='def456'
                BEGIN SELECT RAISE(ABORT, 'injected delete failure'); END;").unwrap();
        }
        let original = snapshot(&dir.0.join("document_uploads/abc123"), app.handle()).0;
        let (start_tx, start_rx) = mpsc::channel();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let handle = app.handle().clone();
        let writer = std::thread::spawn(move || {
            start_rx.recv().unwrap();
            entered_tx.send(()).unwrap();
            let result = replace(&handle, &mut writer_db, |_| Ok(()));
            done_tx.send(result).unwrap();
        });
        let marker = dir.0.join("document_uploads/def456/.generation");
        let mut before_commit = None;
        let result =
            crate::document_uploads::delete_derived_document(app.handle(), VARIANT, |tx| {
                let owned = tx
                    .execute("DELETE FROM translated_documents WHERE id=?1", [VARIANT])
                    .unwrap()
                    > 0;
                start_tx.send(()).unwrap();
                entered_rx.recv().unwrap();
                // The writer must wait before promotion, not just at SQLite's write lock.
                let premature = done_rx.recv_timeout(Duration::from_millis(100));
                before_commit = Some((premature, fs::read_to_string(&marker).unwrap()));
                Ok(owned)
            });
        writer.join().unwrap();
        assert_eq!(result.is_err(), fail_delete);
        let (premature, generation) = before_commit.unwrap();
        assert!(matches!(premature, Err(mpsc::RecvTimeoutError::Timeout)));
        assert!(
            generation.starts_with("1:"),
            "writer promoted while deletion still owned the files: {generation}"
        );
        assert!(done_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .is_ok());
        let live = dir.0.join("document_uploads/def456");
        assert!(fs::read_to_string(live.join("source.html"))
            .unwrap()
            .contains("Replacement words"));
        let asset = ParsedDocumentAsset::new("image/png", b"replacement image".to_vec()).unwrap();
        assert_eq!(
            fs::read(live.join("assets").join(asset.file_name)).unwrap(),
            asset.bytes
        );
        let db = open_document_uploads_db(app.handle()).unwrap();
        assert_eq!(
            db.query_row(
                "SELECT title FROM uploaded_documents WHERE id=?1",
                [VARIANT],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "Replacement"
        );
        assert_eq!(
            db.query_row(
                "SELECT title FROM translated_documents WHERE id=?1",
                [VARIANT],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "Replacement"
        );
        assert!(!dir.0.join("document_uploads/.def456.replacing").exists());
        assert!(!dir.0.join("document_uploads/.def456.deleting").exists());
        assert_eq!(
            snapshot(&dir.0.join("document_uploads/abc123"), app.handle()).0,
            original
        );
    }
}

#[test]
fn all_deletion_routes_wait_for_save_commit_or_rollback_before_removing_the_generation() {
    for route in ["translation", "single", "batch"] {
        for fail_save in [false, true] {
            let (dir, app, window) = fixture();
            seed(app.handle(), SOURCE, None, "html");
            seed(app.handle(), VARIANT, Some(SOURCE), "html");
            let original = snapshot(&dir.0.join("document_uploads/abc123"), app.handle()).0;
            let mut db = open_document_uploads_db(app.handle()).unwrap();
            let (ready_tx, ready_rx) = mpsc::channel();
            let (release_tx, release_rx) = mpsc::channel();
            let handle = app.handle().clone();
            let writer = std::thread::spawn(move || {
                replace(&handle, &mut db, |_| {
                    ready_tx.send(()).unwrap();
                    release_rx.recv().unwrap();
                    if fail_save {
                        Err("injected save failure".into())
                    } else {
                        Ok(())
                    }
                })
            });
            ready_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            let (entered_tx, entered_rx) = mpsc::channel();
            let (done_tx, done_rx) = mpsc::channel();
            let deletion = std::thread::spawn(move || {
                entered_tx.send(()).unwrap();
                done_tx.send(remove(&window, route)).unwrap();
            });
            entered_rx.recv().unwrap();
            let premature = done_rx.recv_timeout(Duration::from_millis(100));
            let promoted_exists = dir.0.join("document_uploads/def456/source.html").is_file();
            release_tx.send(()).unwrap();
            assert_eq!(writer.join().unwrap().is_err(), fail_save);
            deletion.join().unwrap();
            assert!(matches!(premature, Err(mpsc::RecvTimeoutError::Timeout)));
            assert!(
                promoted_exists,
                "{route} removed files during the save transaction"
            );
            let result = done_rx
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .unwrap();
            if route == "batch" {
                assert!(result["failures"].as_array().unwrap().is_empty());
            }
            assert!(!dir.0.join("document_uploads/def456").exists());
            assert!(!dir.0.join("document_uploads/.def456.replacing").exists());
            assert!(!dir.0.join("document_uploads/.def456.deleting").exists());
            let db = open_document_uploads_db(app.handle()).unwrap();
            for (table, column) in [
                ("uploaded_documents", "id"),
                ("translated_documents", "id"),
                ("uploaded_sections", "document_id"),
                ("uploaded_document_fts", "document_id"),
                ("uploaded_document_search_fts", "document_id"),
            ] {
                assert_eq!(
                    db.query_row(
                        &format!("SELECT count(*) FROM {table} WHERE {column}=?1"),
                        [VARIANT],
                        |r| r.get::<_, usize>(0)
                    )
                    .unwrap(),
                    0,
                    "{route}: {table}"
                );
            }
            assert_eq!(
                snapshot(&dir.0.join("document_uploads/abc123"), app.handle()).0,
                original
            );
        }
    }
}
