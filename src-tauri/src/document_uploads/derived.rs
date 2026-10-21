//! Storage bridge for generated HTML documents such as translated variants.

use std::fs;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, Transaction};
use tauri::Runtime;

use super::html::{parsed_html_document, sanitize_html};
use super::parsed::{ParsedDocument, ParsedDocumentAsset};
use super::pipeline;
use super::reader_assets::{owned_assets, snapshot_assets};
use super::storage::{upload_dir, StoredSourceKind};
use super::store::{
    delete_document_rows_in_transaction, open_db, upsert_document_in_transaction, PdfTextStatus,
};
use super::types::UploadedDocumentSourceRequest;

const DERIVED_GENERATION_FILE: &str = ".generation";

/// Open the shared upload database without exposing the store module.
pub(crate) fn open_document_uploads_db<R: Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<rusqlite::Connection, String> {
    open_db(app)
}

/// Read generated reader HTML through the same current sanitizer as the viewer.
pub(crate) fn read_uploaded_document_source<R: Runtime>(
    app: &tauri::AppHandle<R>,
    document_url: &str,
) -> Result<(String, Vec<ParsedDocumentAsset>), String> {
    let source = pipeline::get_source(
        app,
        UploadedDocumentSourceRequest {
            document_url: document_url.into(),
        },
    )?;
    let assets = snapshot_assets(&source.html, &source.asset_paths)?;
    Ok((source.html, assets))
}

/// Stage and index a generated HTML document without exposing parser internals.
///
/// Existing files stay staged as a backup until upload/index rows and feature
/// metadata commit together. A failed replacement therefore restores the
/// previous usable variant instead of leaving a partial Library entry.
#[allow(clippy::too_many_arguments)]
pub(crate) fn persist_derived_document<R, F>(
    app: &tauri::AppHandle<R>,
    db: &mut Connection,
    id: &str,
    url: &str,
    title: &str,
    format: &str,
    view_html: String,
    assets: &[ParsedDocumentAsset],
    imported_at_ms: u128,
    persist_metadata: F,
) -> Result<bool, String>
where
    R: Runtime,
    F: FnOnce(&Transaction<'_>) -> Result<(), String>,
{
    let dir = upload_dir(app, id)?;
    let parsed = prepare_derived_document(title, format, &view_html, assets)?;
    persist_derived_document_at(&dir, db, id, url, parsed, imported_at_ms, persist_metadata)
}

fn prepare_derived_document(
    title: &str,
    format: &str,
    html: &str,
    assets: &[ParsedDocumentAsset],
) -> Result<ParsedDocument, String> {
    let mut parsed = parsed_html_document(title.into(), format, sanitize_html(html));
    parsed.assets = owned_assets(&parsed.view_html, assets)?;
    Ok(parsed)
}

fn persist_derived_document_at<F>(
    dir: &Path,
    db: &mut Connection,
    id: &str,
    url: &str,
    parsed: ParsedDocument,
    imported_at_ms: u128,
    persist_metadata: F,
) -> Result<bool, String>
where
    F: FnOnce(&Transaction<'_>) -> Result<(), String>,
{
    let bytes = parsed.view_html.len() as u64
        + parsed
            .assets
            .iter()
            .map(|asset| asset.bytes.len() as u64)
            .sum::<u64>();
    let staging_dir = dir.with_file_name(format!(".{id}.staging"));
    let backup_dir = dir.with_file_name(format!(".{id}.replacing"));
    recover_interrupted_replacement(db, id, dir, &backup_dir)?;
    if staging_dir.exists() {
        fs::remove_dir_all(&staging_dir).map_err(|err| {
            format!(
                "Failed to clear stale derived document staging directory {}: {err}",
                staging_dir.display()
            )
        })?;
    }
    fs::create_dir_all(&staging_dir).map_err(|err| {
        format!(
            "Failed to create derived document staging directory {}: {err}",
            staging_dir.display()
        )
    })?;
    if let Err(err) = fs::write(
        staging_dir.join(DERIVED_GENERATION_FILE),
        format!("{imported_at_ms}:{bytes}"),
    ) {
        let _ = fs::remove_dir_all(&staging_dir);
        return Err(format!(
            "Failed to write derived document generation marker: {err}"
        ));
    }
    if let Err(err) = fs::write(
        staging_dir.join(StoredSourceKind::Html.file_name()),
        parsed.view_html.as_bytes(),
    ) {
        let _ = fs::remove_dir_all(&staging_dir);
        return Err(format!("Failed to write derived document source: {err}"));
    }
    if let Err(error) = pipeline::write_reader_assets(&staging_dir, &parsed.assets) {
        let _ = fs::remove_dir_all(&staging_dir);
        return Err(error);
    }
    let replaced = dir.exists();
    if replaced {
        fs::rename(&dir, &backup_dir).map_err(|err| {
            let _ = fs::remove_dir_all(&staging_dir);
            format!(
                "Failed to stage existing derived document {}: {err}",
                dir.display()
            )
        })?;
    }
    if let Err(err) = fs::rename(&staging_dir, &dir) {
        let _ = fs::remove_dir_all(&staging_dir);
        return Err(restore_replaced_directory(
            &dir,
            &backup_dir,
            format!(
                "Failed to promote derived document directory {}: {err}",
                dir.display()
            ),
        ));
    }

    let transaction_result = (|| {
        let tx = db.transaction().map_err(db_err)?;
        upsert_document_in_transaction(
            &tx,
            id,
            url,
            &parsed,
            None,
            StoredSourceKind::Html,
            imported_at_ms,
            bytes,
            PdfTextStatus::Ready,
        )?;
        persist_metadata(&tx)?;
        tx.commit().map_err(db_err)
    })();
    if let Err(error) = transaction_result {
        return Err(restore_replaced_directory(&dir, &backup_dir, error));
    }

    if backup_dir.exists() {
        if let Err(error) = fs::remove_dir_all(&backup_dir) {
            log::warn!(
                "Derived document was stored, but its replacement backup could not be removed {}: {error}",
                backup_dir.display()
            );
        }
    }
    Ok(replaced)
}

/// Delete one generated variant, feature metadata, and search rows together.
pub(crate) fn delete_derived_document<R, F>(
    app: &tauri::AppHandle<R>,
    id: &str,
    mut delete_metadata: F,
) -> Result<u64, String>
where
    R: Runtime,
    F: FnMut(&Transaction<'_>) -> Result<(), String>,
{
    let dir = upload_dir(app, id)?;
    let mut db = open_db(app)?;
    pipeline::delete_stored_document(&dir, id, || {
        let tx = db.transaction().map_err(db_err)?;
        delete_metadata(&tx)?;
        delete_document_rows_in_transaction(&tx, id)?;
        tx.commit().map_err(db_err)
    })
}

/// Recover a replacement interrupted before or after its SQLite commit.
///
/// The live generation marker mirrors the upload row timestamp and size. A
/// match means the replacement committed; otherwise the previous files win.
fn recover_interrupted_replacement(
    db: &Connection,
    id: &str,
    dir: &Path,
    backup_dir: &Path,
) -> Result<(), String> {
    if !backup_dir.exists() {
        return Ok(());
    }
    if !dir.exists() {
        return fs::rename(backup_dir, dir).map_err(|err| {
            format!(
                "Failed to recover interrupted derived document replacement {}: {err}",
                backup_dir.display()
            )
        });
    }

    let stored_generation = db
        .query_row(
            "SELECT imported_at_ms, bytes FROM uploaded_documents WHERE id = ?1",
            [id],
            |row| {
                Ok(format!(
                    "{}:{}",
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?
                ))
            },
        )
        .optional()
        .map_err(db_err)?;
    let live_generation = fs::read_to_string(dir.join(DERIVED_GENERATION_FILE)).ok();
    if stored_generation.is_some() && live_generation == stored_generation {
        fs::remove_dir_all(backup_dir).map_err(|err| {
            format!(
                "Failed to clear committed derived document backup {}: {err}",
                backup_dir.display()
            )
        })
    } else {
        fs::remove_dir_all(dir).map_err(|err| {
            format!(
                "Failed to discard interrupted derived document replacement {}: {err}",
                dir.display()
            )
        })?;
        fs::rename(backup_dir, dir).map_err(|err| {
            format!(
                "Failed to restore derived document backup {}: {err}",
                backup_dir.display()
            )
        })
    }
}

fn restore_replaced_directory(dir: &Path, backup_dir: &Path, error: String) -> String {
    let remove_error = dir
        .exists()
        .then(|| fs::remove_dir_all(dir).err())
        .flatten();
    let restore_error = backup_dir
        .exists()
        .then(|| fs::rename(backup_dir, dir).err())
        .flatten();
    match (remove_error, restore_error) {
        (None, None) => error,
        (remove_error, restore_error) => format!(
            "{error}; failed to restore previous derived document (remove: {}; restore: {})",
            remove_error
                .map(|value| value.to_string())
                .unwrap_or_else(|| "ok".into()),
            restore_error
                .map(|value| value.to_string())
                .unwrap_or_else(|| "ok".into())
        ),
    }
}

fn db_err(err: rusqlite::Error) -> String {
    format!("Document database error: {err}")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use kuchikiki::traits::TendrilSink;
    use rusqlite::{params, Connection};

    use super::{recover_interrupted_replacement, DERIVED_GENERATION_FILE};

    #[test]
    fn review_original_survives_reopen_but_only_translation_enters_the_search_index() {
        use super::{
            persist_derived_document_at, prepare_derived_document, sanitize_html,
            ParsedDocumentAsset,
        };
        use crate::document_uploads::store::open_db_in;
        let root = test_root("translation-review");
        let mut db = open_db_in(&root).unwrap();
        let asset =
            ParsedDocumentAsset::new("image/png", b"retained original image".to_vec()).unwrap();
        let html = format!("<article><h2 data-papercut-translation-review='original' id='heading'>Titre original</h2><h2 data-papercut-translation-review='label'>Translation — formatting needs review</h2><h2>English heading</h2><p id='ref' data-papercut-translation-review='original'><strong>Original <em>emphasis</em></strong><a href='#note' role='doc-noteref'>1</a><img data-papercut-asset='{}'></p><p data-papercut-translation-review='label'>Translation</p><p>Translated meaning.</p><p id='note' data-papercut-translation-review='original'>Original note<a href='#ref' role='doc-backlink'>Back</a></p><p>Translated note.</p></article>", asset.file_name);
        let parsed = prepare_derived_document("English", "html", &html, &[asset.clone()]).unwrap();
        assert_eq!(
            parsed
                .sections
                .iter()
                .map(|s| s.text.as_str())
                .collect::<Vec<_>>(),
            ["English heading", "Translated meaning.", "Translated note."]
        );
        assert!(parsed
            .sections
            .iter()
            .all(|s| s.heading.as_deref() == Some("English heading")));
        let live = root.join("variant");
        persist_derived_document_at(
            &live,
            &mut db,
            "variant",
            "/uploads/variant.html",
            parsed,
            1,
            |_| Ok(()),
        )
        .unwrap();
        let saved = fs::read_to_string(live.join("source.html")).unwrap();
        let reopened =
            prepare_derived_document("English", "html", &saved, &[asset.clone()]).unwrap();
        assert_eq!(
            reopened
                .sections
                .iter()
                .map(|s| s.text.as_str())
                .collect::<Vec<_>>(),
            ["English heading", "Translated meaning.", "Translated note."]
        );
        let dom = kuchikiki::parse_html()
            .one(sanitize_html(&saved))
            .document_node;
        for id in ["heading", "ref", "note"] {
            assert_eq!(dom.select(&format!("#{id}")).unwrap().count(), 1);
        }
        assert_eq!(
            dom.select("#ref > strong > em")
                .unwrap()
                .next()
                .unwrap()
                .text_contents(),
            "emphasis"
        );
        assert_eq!(
            dom.select("[data-papercut-translation-review='original'][data-papercut-section]")
                .unwrap()
                .count(),
            0
        );
        assert_eq!(dom.select("[data-papercut-section]").unwrap().count(), 3);
        let rows = db
            .prepare(
                "SELECT text FROM uploaded_sections WHERE document_id='variant' ORDER BY ordinal",
            )
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            rows,
            ["English heading", "Translated meaning.", "Translated note."]
        );
        assert_eq!(
            fs::read(live.join("assets").join(&asset.file_name)).unwrap(),
            asset.bytes
        );
        drop(db);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn saved_reader_retains_protection_markers_and_formatting_through_the_real_preparation_path() {
        use super::{persist_derived_document_at, prepare_derived_document, sanitize_html};
        use crate::document_uploads::store::open_db_in;
        let root = test_root("metadata");
        let mut db = open_db_in(&root).unwrap();
        let html = r#"<p translate="no">user_id</p><p id="r"><strong>Translated<a role="doc-noteref" href='#n'>1</a>.</strong></p><p id="n" role="doc-endnote">Note<a role="doc-backlink" href='#r'>Back</a></p>"#;
        let parsed = prepare_derived_document("English", "html", html, &[]).unwrap();
        let expected = parsed.view_html.clone();
        let live = root.join("variant");
        persist_derived_document_at(
            &live,
            &mut db,
            "variant",
            "/uploads/variant.html",
            parsed,
            1,
            |_| Ok(()),
        )
        .unwrap();
        let saved = fs::read_to_string(live.join("source.html")).unwrap();
        assert_eq!(saved, expected);
        let reader = sanitize_html(&saved);
        assert!(reader.contains("translate=\"no\""));
        assert!(reader.contains("role=\"doc-noteref\""));
        assert!(reader.contains("role=\"doc-backlink\""));
        assert!(reader.contains("<strong>Translated"));
        let rows: Vec<(String, String)> = db.prepare("SELECT text,body_ranges FROM uploaded_sections WHERE document_id='variant' ORDER BY ordinal").unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?))).unwrap().collect::<Result<_,_>>().unwrap();
        assert_eq!(
            rows.iter().map(|row| row.0.as_str()).collect::<Vec<_>>(),
            ["user_id", "Translated1.", "NoteBack"]
        );
        assert_eq!(rows[2].1, "[]");
        drop(db);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn derived_assets_survive_source_deletion_and_failed_replacement_rolls_back() {
        use super::{
            parsed_html_document, persist_derived_document_at, prepare_derived_document,
            snapshot_assets, ParsedDocumentAsset,
        };
        use crate::document_uploads::{pipeline::stored_reader_asset_paths, store::open_db_in};
        let root = test_root("asset-rollback");
        let mut db = open_db_in(&root).unwrap();
        let source = root.join("source");
        fs::create_dir_all(source.join("assets")).unwrap();
        let asset =
            ParsedDocumentAsset::new("image/png", b"original image bytes".to_vec()).unwrap();
        fs::write(source.join("assets").join(&asset.file_name), &asset.bytes).unwrap();
        let html = format!("<article><h1>Chapter</h1><div>Hello <a href='#note'>note</a><img data-papercut-asset='{}'></div><aside role=doc-endnotes><p id=note>Note body</p></aside></article>", asset.file_name);
        let paths = stored_reader_asset_paths(&source).unwrap();
        let assets = snapshot_assets(&html, &paths).unwrap();
        let parsed = prepare_derived_document("English", "html", &html, &assets).unwrap();
        let expected_html = parsed.view_html.clone();
        let expected_bytes = expected_html.len() + asset.bytes.len();
        let live = root.join("variant");
        assert!(!persist_derived_document_at(
            &live,
            &mut db,
            "variant",
            "/uploads/variant.html",
            parsed,
            1,
            |_| Ok(())
        )
        .unwrap());
        fs::remove_dir_all(source).unwrap();
        assert_eq!(
            fs::read(live.join("assets").join(&asset.file_name)).unwrap(),
            asset.bytes
        );
        assert_eq!(
            fs::read_to_string(live.join("source.html")).unwrap(),
            expected_html
        );
        let sections: Vec<(String, String)> = db.prepare("SELECT text, body_ranges FROM uploaded_sections WHERE document_id='variant' ORDER BY ordinal").unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?))).unwrap().collect::<Result<_,_>>().unwrap();
        assert_eq!(sections.len(), 3);
        assert_eq!(sections[0], ("Chapter".into(), "[]".into()));
        assert_eq!(sections[1].0, "Hello note");
        assert_ne!(sections[1].1, "[]");
        assert_eq!(sections[2], ("Note body".into(), "[]".into()));
        let bytes: i64 = db
            .query_row(
                "SELECT bytes FROM uploaded_documents WHERE id='variant'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(bytes as usize, expected_bytes);
        let replacement = parsed_html_document(
            "Replacement".into(),
            "html",
            "<p>Replacement text</p>".into(),
        );
        assert!(persist_derived_document_at(
            &live,
            &mut db,
            "variant",
            "/uploads/variant.html",
            replacement,
            2,
            |_| Err("metadata failure".into())
        )
        .is_err());
        assert_eq!(
            fs::read_to_string(live.join("source.html")).unwrap(),
            expected_html
        );
        assert_eq!(
            fs::read(live.join("assets").join(&asset.file_name)).unwrap(),
            asset.bytes
        );
        let indexed: String = db
            .query_row(
                "SELECT text FROM uploaded_sections WHERE document_id='variant' AND ordinal=1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(indexed, "Hello note");
        assert!(!root.join(".variant.staging").exists());
        assert!(!root.join(".variant.replacing").exists());
        let replacement = parsed_html_document(
            "New edition".into(),
            "html",
            "<h1>New chapter</h1><p>New body</p>".into(),
        );
        assert!(persist_derived_document_at(
            &live,
            &mut db,
            "variant",
            "/uploads/variant.html",
            replacement,
            3,
            |_| Ok(())
        )
        .unwrap());
        assert!(!live.join("assets").exists());
        let indexed: String = db
            .query_row(
                "SELECT text FROM uploaded_sections WHERE document_id='variant' AND ordinal=1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(indexed, "New body");
        drop(db);
        fs::remove_dir_all(root).unwrap();
    }

    fn test_root(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "papercut-derived-{name}-{}-{nonce}",
            std::process::id()
        ))
    }

    fn database(generation: &str) -> Connection {
        let db = Connection::open_in_memory().expect("database");
        db.execute_batch(
            "CREATE TABLE uploaded_documents (
                id TEXT PRIMARY KEY,
                imported_at_ms INTEGER NOT NULL,
                bytes INTEGER NOT NULL
            );",
        )
        .expect("schema");
        let (timestamp, bytes) = generation.split_once(':').expect("generation");
        db.execute(
            "INSERT INTO uploaded_documents (id, imported_at_ms, bytes) VALUES ('variant', ?1, ?2)",
            params![
                timestamp.parse::<i64>().expect("timestamp"),
                bytes.parse::<i64>().expect("bytes")
            ],
        )
        .expect("row");
        db
    }

    fn write_generation(dir: &std::path::Path, generation: &str, content: &str) {
        fs::create_dir_all(dir).expect("directory");
        fs::write(dir.join(DERIVED_GENERATION_FILE), generation).expect("marker");
        fs::write(dir.join("source.html"), content).expect("content");
    }

    #[test]
    fn replacement_recovery_follows_the_committed_database_generation() {
        let root = test_root("recovery");
        let live = root.join("variant");
        let backup = root.join(".variant.replacing");

        write_generation(&live, "2:20", "new");
        write_generation(&backup, "1:10", "old");
        recover_interrupted_replacement(&database("1:10"), "variant", &live, &backup)
            .expect("restore pre-commit backup");
        assert_eq!(
            fs::read_to_string(live.join("source.html")).expect("restored content"),
            "old"
        );
        assert!(!backup.exists());

        write_generation(&backup, "1:10", "old");
        write_generation(&live, "2:20", "new");
        recover_interrupted_replacement(&database("2:20"), "variant", &live, &backup)
            .expect("keep committed replacement");
        assert_eq!(
            fs::read_to_string(live.join("source.html")).expect("committed content"),
            "new"
        );
        assert!(!backup.exists());

        fs::remove_dir_all(root).expect("cleanup");
    }
}
