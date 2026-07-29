//! Storage bridge for generated HTML documents such as translated variants.

use std::fs;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, Transaction};
use tauri::Runtime;

use super::parsed::{ParsedDocument, ParsedSection};
use super::pipeline;
use super::storage::{upload_dir, StoredSourceKind};
use super::store::{delete_document_rows_in_transaction, open_db, upsert_document_in_transaction};
use super::types::UploadedDocumentSourceRequest;

const DERIVED_GENERATION_FILE: &str = ".generation";

pub(crate) struct DerivedDocumentSection {
    pub(crate) heading: Option<String>,
    pub(crate) text: String,
}

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
) -> Result<String, String> {
    pipeline::get_source(
        app,
        UploadedDocumentSourceRequest {
            document_url: document_url.into(),
        },
    )
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
    sections: Vec<DerivedDocumentSection>,
    imported_at_ms: u128,
    bytes: u64,
    persist_metadata: F,
) -> Result<bool, String>
where
    R: Runtime,
    F: FnOnce(&Transaction<'_>) -> Result<(), String>,
{
    let dir = upload_dir(app, id)?;
    let staging_dir = dir.with_file_name(format!(".{id}.staging"));
    let backup_dir = dir.with_file_name(format!(".{id}.replacing"));
    recover_interrupted_replacement(db, id, &dir, &backup_dir)?;
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
        view_html.as_bytes(),
    ) {
        let _ = fs::remove_dir_all(&staging_dir);
        return Err(format!("Failed to write derived document source: {err}"));
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

    let parsed = ParsedDocument {
        title: title.into(),
        format: format.into(),
        view_html,
        sections: sections
            .into_iter()
            .map(|section| ParsedSection {
                heading: section.heading,
                text: section.text,
                page_index: None,
            })
            .collect(),
        cover: None,
    };
    let transaction_result = (|| {
        let tx = db.transaction().map_err(db_err)?;
        upsert_document_in_transaction(
            &tx,
            id,
            url,
            &parsed,
            StoredSourceKind::Html,
            imported_at_ms,
            bytes,
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

    use rusqlite::{params, Connection};

    use super::{recover_interrupted_replacement, DERIVED_GENERATION_FILE};

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
