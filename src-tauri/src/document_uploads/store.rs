//! SQLite persistence for uploaded documents.
//!
//! Owns the schema, the connection bootstrap, the index write path, listing,
//! and row deletion. Search-time reads live in [`super::search`]; this module
//! keeps everything that defines or mutates the database layout.

use std::time::Duration;

use rusqlite::{params, Connection, OptionalExtension, Row, TransactionBehavior};
use tauri::Runtime;

use super::parsed::ParsedDocument;
use super::storage::{upload_id_from_url, uploads_root, StoredSourceKind};
use super::types::UploadedDocument;

const MAX_TITLE_CHARS: usize = 512;

/// Apply the one title boundary shared by document creation and later edits.
/// Returning an owned trimmed value prevents callers from persisting whitespace
/// that the title editor would remove on its first save.
pub(crate) fn normalize_document_title(title: &str) -> Result<String, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("Document title cannot be empty".into());
    }
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(format!(
            "Document title cannot exceed {MAX_TITLE_CHARS} characters"
        ));
    }
    Ok(title.to_owned())
}

/// List all stored uploads as DTOs, newest import first.
pub(crate) fn list_uploads<R: Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<Vec<UploadedDocument>, String> {
    let db = open_db(app)?;
    let mut stmt = db
        .prepare(
            "SELECT id, url, title, original_file_name, format, source_kind, imported_at_ms, bytes, sections, cover_media_type, text_status \
             FROM uploaded_documents ORDER BY imported_at_ms DESC",
        )
        .map_err(db_err)?;
    let rows = stmt
        .query_map([], uploaded_document_from_row)
        .map_err(db_err)?;

    rows.collect::<Result<Vec<_>, _>>().map_err(db_err)
}

/// Look up an existing upload by its stable source-derived id.
pub(crate) fn find_upload_by_id(
    db: &Connection,
    id: &str,
) -> Result<Option<UploadedDocument>, String> {
    db.query_row(
        "SELECT id, url, title, original_file_name, format, source_kind, imported_at_ms, bytes, sections, cover_media_type, text_status \
         FROM uploaded_documents WHERE id = ?1",
        [id],
        uploaded_document_from_row,
    )
    .optional()
    .map_err(db_err)
}

fn uploaded_document_from_row(row: &Row<'_>) -> rusqlite::Result<UploadedDocument> {
    Ok(UploadedDocument {
        id: row.get(0)?,
        url: row.get(1)?,
        title: row.get(2)?,
        original_file_name: row.get(3)?,
        format: row.get(4)?,
        source_kind: row.get(5)?,
        imported_at_ms: row.get::<_, i64>(6)? as u128,
        bytes: row.get::<_, i64>(7)? as u64,
        sections: row.get::<_, i64>(8)? as usize,
        cover_media_type: row.get(9)?,
        text_status: row.get(10)?,
    })
}

/// Open (creating if needed) the search database and ensure the schema exists.
///
/// Idempotent: creates the storage dir, the metadata and section tables, and the
/// FTS5 virtual table on every call so callers never depend on prior setup.
pub(crate) fn open_db<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<Connection, String> {
    let root = uploads_root(app)?;
    std::fs::create_dir_all(&root)
        .map_err(|err| format!("Failed to create upload storage {}: {err}", root.display()))?;
    let mut db = Connection::open(root.join("search.sqlite3")).map_err(db_err)?;
    db.busy_timeout(Duration::from_secs(5)).map_err(db_err)?;
    db.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;",
    )
    .map_err(db_err)?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(db_err)?;
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS upload_schema_metadata (
           key TEXT PRIMARY KEY,
           value TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS uploaded_documents (
           id TEXT PRIMARY KEY,
           url TEXT NOT NULL UNIQUE,
           title TEXT NOT NULL,
           original_file_name TEXT,
           format TEXT NOT NULL,
           source_kind TEXT NOT NULL DEFAULT 'html',
           imported_at_ms INTEGER NOT NULL,
           bytes INTEGER NOT NULL,
           sections INTEGER NOT NULL,
           cover_media_type TEXT,
           text_status TEXT NOT NULL DEFAULT 'ready'
         );
         CREATE TABLE IF NOT EXISTS uploaded_sections (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           document_id TEXT NOT NULL,
           ordinal INTEGER NOT NULL,
           page_index INTEGER,
           heading TEXT,
           text TEXT NOT NULL,
           FOREIGN KEY(document_id) REFERENCES uploaded_documents(id) ON DELETE CASCADE
         );
         CREATE VIRTUAL TABLE IF NOT EXISTS uploaded_document_fts USING fts5(
           document_id UNINDEXED,
           section_id UNINDEXED,
           title,
           heading,
           text,
           tokenize = 'porter unicode61 remove_diacritics 1'
         );
         CREATE TABLE IF NOT EXISTS uploaded_folders (
           id TEXT PRIMARY KEY,
           parent_id TEXT,
           name TEXT NOT NULL,
           depth INTEGER NOT NULL,
           sort_order INTEGER NOT NULL,
           created_at_ms INTEGER NOT NULL,
           updated_at_ms INTEGER NOT NULL,
           FOREIGN KEY(parent_id) REFERENCES uploaded_folders(id) ON DELETE CASCADE,
           CHECK(depth >= 0 AND depth <= 4),
           CHECK(length(trim(name)) > 0)
         );
         CREATE INDEX IF NOT EXISTS uploaded_folders_parent_order_idx
           ON uploaded_folders(parent_id, sort_order, name);
         CREATE TABLE IF NOT EXISTS uploaded_document_locations (
           document_id TEXT PRIMARY KEY,
           folder_id TEXT,
           sort_order INTEGER NOT NULL,
           FOREIGN KEY(document_id) REFERENCES uploaded_documents(id) ON DELETE CASCADE,
           FOREIGN KEY(folder_id) REFERENCES uploaded_folders(id) ON DELETE SET NULL
         );
         CREATE INDEX IF NOT EXISTS uploaded_document_locations_folder_order_idx
           ON uploaded_document_locations(folder_id, sort_order);
         INSERT OR IGNORE INTO uploaded_document_locations (document_id, folder_id, sort_order)
           SELECT id, NULL, -imported_at_ms FROM uploaded_documents;",
    )
    .map_err(db_err)?;
    let previous_schema_version = tx
        .query_row(
            "SELECT value FROM upload_schema_metadata WHERE key = 'schema_version'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(db_err)?
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(0);
    ensure_schema_columns(&tx)?;
    if previous_schema_version < 6 {
        backfill_pdf_text_status(&tx)?;
    }
    tx.execute(
        "INSERT OR REPLACE INTO upload_schema_metadata (key, value) VALUES ('schema_version', '6')",
        [],
    )
    .map_err(db_err)?;
    tx.commit().map_err(db_err)?;
    Ok(db)
}

/// Add nullable metadata columns in place so existing documents and FTS rows
/// survive schema upgrades without a database rebuild.
fn ensure_schema_columns(db: &Connection) -> Result<(), String> {
    ensure_column(
        db,
        "uploaded_documents",
        "original_file_name",
        "ALTER TABLE uploaded_documents ADD COLUMN original_file_name TEXT",
    )?;
    ensure_column(
        db,
        "uploaded_documents",
        "cover_media_type",
        "ALTER TABLE uploaded_documents ADD COLUMN cover_media_type TEXT",
    )?;
    ensure_column(
        db,
        "uploaded_documents",
        "source_kind",
        "ALTER TABLE uploaded_documents ADD COLUMN source_kind TEXT NOT NULL DEFAULT 'html'",
    )?;
    ensure_column(
        db,
        "uploaded_documents",
        "text_status",
        "ALTER TABLE uploaded_documents ADD COLUMN text_status TEXT NOT NULL DEFAULT 'ready'",
    )?;
    ensure_column(
        db,
        "uploaded_sections",
        "page_index",
        "ALTER TABLE uploaded_sections ADD COLUMN page_index INTEGER",
    )?;
    Ok(())
}

/// Mark previously indexed PDFs with no extracted text so an upgrade does not
/// leave image-only documents looking searchable. Empty staged PDFs keep their
/// processing state because they have no committed page rows yet.
fn backfill_pdf_text_status(db: &Connection) -> Result<(), String> {
    db.execute(
        "UPDATE uploaded_documents SET text_status = 'recognition-required' \
         WHERE source_kind = 'pdf' AND sections > 0 \
           AND NOT EXISTS (SELECT 1 FROM uploaded_sections \
             WHERE document_id = uploaded_documents.id AND length(trim(text)) > 0)",
        [],
    )
    .map_err(db_err)?;
    Ok(())
}

/// Apply one additive migration while tolerating another connection winning
/// the check-then-ALTER race. Any other SQLite failure remains fatal.
fn ensure_column(
    db: &Connection,
    table: &str,
    column: &str,
    migration: &str,
) -> Result<(), String> {
    if has_column(db, table, column)? {
        return Ok(());
    }
    if let Err(error) = db.execute(migration, []) {
        // Another connection may have completed the same additive migration
        // after our check. Only suppress the error when the desired schema now exists.
        if !has_column(db, table, column)? {
            return Err(db_err(error));
        }
    }
    Ok(())
}

fn has_column(db: &Connection, table: &str, column: &str) -> Result<bool, String> {
    let mut stmt = db
        .prepare(&format!("PRAGMA table_info({table})"))
        .map_err(db_err)?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(db_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(db_err)?;
    Ok(columns.iter().any(|existing| existing == column))
}

/// Remove a document's metadata, section, and FTS rows in one transaction so
/// the index and metadata can never drift out of sync.
pub(crate) fn delete_document_rows(db: &mut Connection, id: &str) -> Result<(), String> {
    let tx = db.transaction().map_err(db_err)?;
    tx.execute(
        "DELETE FROM uploaded_document_fts WHERE document_id = ?1",
        [id],
    )
    .map_err(db_err)?;
    tx.execute("DELETE FROM uploaded_sections WHERE document_id = ?1", [id])
        .map_err(db_err)?;
    tx.execute("DELETE FROM uploaded_documents WHERE id = ?1", [id])
        .map_err(db_err)?;
    tx.commit().map_err(db_err)
}

/// PDF readiness persisted after the frontend has inspected every page.
#[derive(Clone, Copy, Debug, Default, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum PdfTextStatus {
    #[default]
    Ready,
    RecognitionAvailable,
    RecognitionRequired,
}

impl PdfTextStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::RecognitionAvailable => "recognition-available",
            Self::RecognitionRequired => "recognition-required",
        }
    }
}

/// Insert or update a parsed document and all of its sections atomically.
///
/// This deliberately avoids `INSERT OR REPLACE` because SQLite implements that
/// as delete-then-insert, which would cascade-delete the document's library
/// location. The document row updates in place while section and FTS rows are
/// rebuilt from the latest parsed content.
pub(crate) fn upsert_document(
    db: &mut Connection,
    id: &str,
    url: &str,
    parsed: &ParsedDocument,
    original_file_name: Option<&str>,
    source_kind: StoredSourceKind,
    imported_at_ms: u128,
    bytes: u64,
    pdf_text_status: PdfTextStatus,
) -> Result<(), String> {
    let tx = db.transaction().map_err(db_err)?;
    tx.execute(
        "DELETE FROM uploaded_document_fts WHERE document_id = ?1",
        [id],
    )
    .map_err(db_err)?;
    tx.execute("DELETE FROM uploaded_sections WHERE document_id = ?1", [id])
        .map_err(db_err)?;
    tx.execute(
        "INSERT INTO uploaded_documents \
         (id, url, title, original_file_name, format, source_kind, imported_at_ms, bytes, sections, cover_media_type, text_status) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11) \
         ON CONFLICT(id) DO UPDATE SET \
           url = excluded.url, \
           title = excluded.title, \
           original_file_name = COALESCE(excluded.original_file_name, uploaded_documents.original_file_name), \
           format = excluded.format, \
           source_kind = excluded.source_kind, \
           imported_at_ms = excluded.imported_at_ms, \
           bytes = excluded.bytes, \
           sections = excluded.sections, \
           cover_media_type = excluded.cover_media_type, \
           text_status = excluded.text_status",
        params![
            id,
            url,
            parsed.title,
            original_file_name,
            parsed.format,
            source_kind.as_str(),
            imported_at_ms as i64,
            bytes as i64,
            parsed.sections.len() as i64,
            parsed.cover.as_ref().map(|cover| cover.media_type),
            document_text_status(source_kind, parsed, pdf_text_status),
        ],
    )
    .map_err(db_err)?;
    tx.execute(
        "INSERT OR IGNORE INTO uploaded_document_locations (document_id, folder_id, sort_order) \
         VALUES (?1, NULL, ?2)",
        params![id, -(imported_at_ms as i64)],
    )
    .map_err(db_err)?;

    for (index, section) in parsed.sections.iter().enumerate() {
        tx.execute(
            "INSERT INTO uploaded_sections (document_id, ordinal, page_index, heading, text) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                id,
                index as i64,
                section.page_index.map(i64::from),
                section.heading,
                section.text
            ],
        )
        .map_err(db_err)?;
        let section_id = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO uploaded_document_fts (document_id, section_id, title, heading, text) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, section_id, parsed.title, section.heading, section.text],
        )
        .map_err(db_err)?;
    }

    tx.commit().map_err(db_err)
}

/// Store a canonical source whose derived sections are intentionally absent.
/// PDF transfer and staging use this until PDF.js rebuilds searchable page rows.
pub(crate) fn upsert_unindexed_document(
    db: &mut Connection,
    id: &str,
    url: &str,
    title: &str,
    original_file_name: Option<&str>,
    format: &str,
    source_kind: StoredSourceKind,
    imported_at_ms: u128,
    bytes: u64,
) -> Result<(), String> {
    let tx = db.transaction().map_err(db_err)?;
    tx.execute(
        "DELETE FROM uploaded_document_fts WHERE document_id = ?1",
        [id],
    )
    .map_err(db_err)?;
    tx.execute("DELETE FROM uploaded_sections WHERE document_id = ?1", [id])
        .map_err(db_err)?;
    tx.execute(
        "INSERT INTO uploaded_documents \
         (id, url, title, original_file_name, format, source_kind, imported_at_ms, bytes, sections, cover_media_type, text_status) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, NULL, 'processing') \
         ON CONFLICT(id) DO UPDATE SET \
           url = excluded.url, title = excluded.title, \
           original_file_name = COALESCE(excluded.original_file_name, uploaded_documents.original_file_name), \
           format = excluded.format, \
           source_kind = excluded.source_kind, imported_at_ms = excluded.imported_at_ms, \
           bytes = excluded.bytes, sections = 0, cover_media_type = NULL, \
           text_status = 'processing'",
        params![
            id,
            url,
            title,
            original_file_name,
            format,
            source_kind.as_str(),
            imported_at_ms as i64,
            bytes as i64,
        ],
    )
    .map_err(db_err)?;
    tx.execute(
        "INSERT OR IGNORE INTO uploaded_document_locations (document_id, folder_id, sort_order) \
         VALUES (?1, NULL, ?2)",
        params![id, -(imported_at_ms as i64)],
    )
    .map_err(db_err)?;
    tx.commit().map_err(db_err)
}

fn document_text_status(
    source_kind: StoredSourceKind,
    parsed: &ParsedDocument,
    pdf_text_status: PdfTextStatus,
) -> &'static str {
    if source_kind != StoredSourceKind::Pdf {
        return "ready";
    }
    if !parsed
        .sections
        .iter()
        .any(|section| !section.text.trim().is_empty())
    {
        return "recognition-required";
    }
    pdf_text_status.as_str()
}

/// Change display metadata and its duplicated FTS title in one transaction.
///
/// The stable document URL and source files remain untouched, so folders,
/// bookmarks, saved-audio ids, and reader links keep their existing identity.
pub(crate) fn update_document_title(
    db: &mut Connection,
    document_url: &str,
    title: &str,
) -> Result<UploadedDocument, String> {
    let id = upload_id_from_url(document_url)?;
    let title = normalize_document_title(title)?;

    let tx = db.transaction().map_err(db_err)?;
    let updated = tx
        .execute(
            "UPDATE uploaded_documents SET title = ?1 WHERE id = ?2 AND url = ?3",
            params![title, id, document_url],
        )
        .map_err(db_err)?;
    if updated == 0 {
        return Err("Uploaded document was not found".into());
    }
    tx.execute(
        "UPDATE uploaded_document_fts SET title = ?1 WHERE document_id = ?2",
        params![title, id],
    )
    .map_err(db_err)?;
    tx.commit().map_err(db_err)?;

    find_upload_by_id(db, &id)?.ok_or_else(|| "Updated document metadata is missing".to_string())
}

/// Format a rusqlite error into the feature's user-facing error string.
pub(crate) fn db_err(err: rusqlite::Error) -> String {
    format!("Document upload database error: {err}")
}

#[cfg(test)]
mod tests {
    use rusqlite::{params, Connection};

    use super::{
        backfill_pdf_text_status, ensure_schema_columns, find_upload_by_id, update_document_title,
        upsert_document, PdfTextStatus, MAX_TITLE_CHARS,
    };
    use crate::document_uploads::parsed::{ParsedDocument, ParsedSection};
    use crate::document_uploads::StoredSourceKind;

    /// Regression test for SQLite's `INSERT OR REPLACE` footgun:
    /// same-id document updates must not delete library placement metadata.
    #[test]
    fn upsert_document_preserves_existing_library_location() {
        let mut db = test_db();
        let first = parsed_document("First Title", &["Old body"]);
        upsert_document(
            &mut db,
            "abc123",
            "/uploads/abc123.html",
            &first,
            Some("original.html"),
            StoredSourceKind::Html,
            100,
            10,
            PdfTextStatus::Ready,
        )
        .expect("initial insert");
        let stored = find_upload_by_id(&db, "abc123")
            .expect("lookup existing upload")
            .expect("stored upload");
        assert_eq!(stored.id, "abc123");
        assert!(find_upload_by_id(&db, "missing")
            .expect("lookup missing upload")
            .is_none());
        db.execute(
            "INSERT INTO uploaded_folders \
             (id, parent_id, name, depth, sort_order, created_at_ms, updated_at_ms) \
             VALUES ('folder1', NULL, 'Reading', 0, 1000, 100, 100)",
            [],
        )
        .expect("insert folder");
        db.execute(
            "UPDATE uploaded_document_locations SET folder_id = 'folder1', sort_order = 42 \
             WHERE document_id = 'abc123'",
            [],
        )
        .expect("move document");

        let second = parsed_document("Second Title", &["New body", "Another section"]);
        upsert_document(
            &mut db,
            "abc123",
            "/uploads/abc123.html",
            &second,
            None,
            StoredSourceKind::Html,
            200,
            20,
            PdfTextStatus::Ready,
        )
        .expect("update existing document");

        let location: (Option<String>, i64) = db
            .query_row(
                "SELECT folder_id, sort_order FROM uploaded_document_locations WHERE document_id = 'abc123'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("document location");
        assert_eq!(location, (Some("folder1".to_string()), 42));

        let metadata: (String, i64, i64, i64) = db
            .query_row(
                "SELECT title, imported_at_ms, bytes, sections FROM uploaded_documents WHERE id = 'abc123'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("document metadata");
        assert_eq!(metadata, ("Second Title".to_string(), 200, 20, 2));

        let section_count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM uploaded_sections WHERE document_id = 'abc123'",
                [],
                |row| row.get(0),
            )
            .expect("section count");
        let fts_count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM uploaded_document_fts WHERE document_id = 'abc123'",
                [],
                |row| row.get(0),
            )
            .expect("fts count");
        assert_eq!(section_count, 2);
        assert_eq!(fts_count, 2);
        assert_eq!(
            find_upload_by_id(&db, "abc123")
                .expect("lookup updated upload")
                .expect("updated upload")
                .original_file_name
                .as_deref(),
            Some("original.html")
        );
    }

    #[test]
    fn title_update_keeps_document_and_fts_metadata_in_sync() {
        let mut db = test_db();
        upsert_document(
            &mut db,
            "abc123",
            "/uploads/abc123.html",
            &parsed_document("Old Title", &["One", "Two"]),
            Some("original.html"),
            StoredSourceKind::Html,
            100,
            10,
            PdfTextStatus::Ready,
        )
        .expect("insert document");

        let updated = update_document_title(&mut db, "/uploads/abc123.html", "  Better Title  ")
            .expect("update title");

        assert_eq!(updated.title, "Better Title");
        assert_eq!(updated.original_file_name.as_deref(), Some("original.html"));
        let fts_titles: Vec<String> = db
            .prepare(
                "SELECT title FROM uploaded_document_fts \
                 WHERE document_id = 'abc123' ORDER BY section_id",
            )
            .expect("prepare FTS title query")
            .query_map([], |row| row.get(0))
            .expect("read FTS titles")
            .collect::<Result<_, _>>()
            .expect("collect FTS titles");
        assert_eq!(fts_titles, vec!["Better Title", "Better Title"]);
        assert!(update_document_title(&mut db, "/uploads/abc123.html", " ").is_err());
        assert!(update_document_title(
            &mut db,
            "/uploads/abc123.html",
            &"a".repeat(MAX_TITLE_CHARS + 1),
        )
        .is_err());
    }

    #[test]
    fn upsert_pdf_document_persists_page_locator_and_source_kind() {
        let mut db = test_db();
        let mut parsed = parsed_document("PDF Title", &["Page text"]);
        parsed.format = "pdf".into();
        parsed.view_html.clear();
        parsed.sections[0].page_index = Some(7);

        upsert_document(
            &mut db,
            "pdf123",
            "/uploads/pdf123.pdf",
            &parsed,
            Some("source.pdf"),
            StoredSourceKind::Pdf,
            100,
            20,
            PdfTextStatus::Ready,
        )
        .expect("insert PDF");

        let stored: (String, Option<i64>, String) = db
            .query_row(
                "SELECT d.source_kind, s.page_index, d.text_status \
                 FROM uploaded_documents d \
                 JOIN uploaded_sections s ON s.document_id = d.id \
                 WHERE d.id = 'pdf123'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("stored PDF page");
        assert_eq!(stored, ("pdf".into(), Some(7), "ready".into()));

        upsert_document(
            &mut db,
            "pdf123",
            "/uploads/pdf123.pdf",
            &parsed,
            Some("source.pdf"),
            StoredSourceKind::Pdf,
            100,
            20,
            PdfTextStatus::RecognitionAvailable,
        )
        .expect("mark hybrid PDF for recognition");
        assert_eq!(
            find_upload_by_id(&db, "pdf123")
                .expect("lookup hybrid PDF")
                .expect("stored hybrid PDF")
                .text_status,
            "recognition-available"
        );

        parsed.sections[0].text = "  ".into();
        upsert_document(
            &mut db,
            "pdf123",
            "/uploads/pdf123.pdf",
            &parsed,
            Some("source.pdf"),
            StoredSourceKind::Pdf,
            100,
            20,
            PdfTextStatus::Ready,
        )
        .expect("replace PDF with image-only page");
        assert_eq!(
            find_upload_by_id(&db, "pdf123")
                .expect("lookup image-only PDF")
                .expect("stored image-only PDF")
                .text_status,
            "recognition-required"
        );
    }

    #[test]
    fn adds_source_and_locator_metadata_to_existing_upload_schema() {
        let db = Connection::open_in_memory().expect("open database");
        db.execute("CREATE TABLE uploaded_documents (id TEXT PRIMARY KEY)", [])
            .expect("create old schema");

        db.execute(
            "CREATE TABLE uploaded_sections (id INTEGER PRIMARY KEY)",
            [],
        )
        .expect("create old section schema");

        ensure_schema_columns(&db).expect("migrate upload metadata");
        ensure_schema_columns(&db).expect("migration remains idempotent");

        let has_cover_column = db
            .prepare("PRAGMA table_info(uploaded_documents)")
            .expect("inspect schema")
            .query_map([], |row| row.get::<_, String>(1))
            .expect("read columns")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect columns")
            .iter()
            .any(|column| column == "cover_media_type");
        assert!(has_cover_column);
        let has_original_file_name: bool = db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM pragma_table_info('uploaded_documents') \
                 WHERE name = 'original_file_name')",
                [],
                |row| row.get(0),
            )
            .expect("original filename column");
        assert!(has_original_file_name);

        let source_kind: String = db
            .query_row(
                "SELECT dflt_value FROM pragma_table_info('uploaded_documents') \
                 WHERE name = 'source_kind'",
                [],
                |row| row.get(0),
            )
            .expect("source kind default");
        assert_eq!(source_kind, "'html'");
        let text_status: String = db
            .query_row(
                "SELECT dflt_value FROM pragma_table_info('uploaded_documents') \
                 WHERE name = 'text_status'",
                [],
                |row| row.get(0),
            )
            .expect("text status default");
        assert_eq!(text_status, "'ready'");
        let has_page_index: bool = db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM pragma_table_info('uploaded_sections') \
                 WHERE name = 'page_index')",
                [],
                |row| row.get(0),
            )
            .expect("page locator column");
        assert!(has_page_index);
    }

    #[test]
    fn backfill_marks_only_finalized_textless_pdfs() {
        let db = test_db();
        for (id, sections, status) in [
            ("text-pdf", 1, "ready"),
            ("image-pdf", 1, "ready"),
            ("staged-pdf", 0, "processing"),
        ] {
            db.execute(
                "INSERT INTO uploaded_documents \
                 (id, url, title, format, source_kind, imported_at_ms, bytes, sections, text_status) \
                 VALUES (?1, ?2, ?1, 'pdf', 'pdf', 1, 1, ?3, ?4)",
                params![id, format!("/uploads/{id}.pdf"), sections, status],
            )
            .expect("insert legacy PDF");
        }
        db.execute(
            "INSERT INTO uploaded_sections (document_id, ordinal, page_index, text) \
             VALUES ('text-pdf', 0, 0, 'Readable text'), ('image-pdf', 0, 0, '  ')",
            [],
        )
        .expect("insert legacy PDF pages");

        backfill_pdf_text_status(&db).expect("backfill PDF text status");

        let statuses = db
            .prepare("SELECT id, text_status FROM uploaded_documents ORDER BY id")
            .expect("prepare status query")
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .expect("read statuses")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect statuses");
        assert_eq!(
            statuses,
            vec![
                ("image-pdf".into(), "recognition-required".into()),
                ("staged-pdf".into(), "processing".into()),
                ("text-pdf".into(), "ready".into()),
            ]
        );
    }

    /// Minimal in-memory schema for `upsert_document` without booting a Tauri app.
    fn test_db() -> Connection {
        let db = Connection::open_in_memory().expect("open test db");
        db.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE uploaded_documents (
               id TEXT PRIMARY KEY,
               url TEXT NOT NULL UNIQUE,
               title TEXT NOT NULL,
               original_file_name TEXT,
               format TEXT NOT NULL,
               source_kind TEXT NOT NULL DEFAULT 'html',
               imported_at_ms INTEGER NOT NULL,
               bytes INTEGER NOT NULL,
               sections INTEGER NOT NULL,
               cover_media_type TEXT,
               text_status TEXT NOT NULL DEFAULT 'ready'
             );
             CREATE TABLE uploaded_sections (
               id INTEGER PRIMARY KEY AUTOINCREMENT,
               document_id TEXT NOT NULL,
               ordinal INTEGER NOT NULL,
               page_index INTEGER,
               heading TEXT,
               text TEXT NOT NULL,
               FOREIGN KEY(document_id) REFERENCES uploaded_documents(id) ON DELETE CASCADE
             );
             CREATE VIRTUAL TABLE uploaded_document_fts USING fts5(
               document_id UNINDEXED,
               section_id UNINDEXED,
               title,
               heading,
               text
             );
             CREATE TABLE uploaded_folders (
               id TEXT PRIMARY KEY,
               parent_id TEXT,
               name TEXT NOT NULL,
               depth INTEGER NOT NULL,
               sort_order INTEGER NOT NULL,
               created_at_ms INTEGER NOT NULL,
               updated_at_ms INTEGER NOT NULL,
               FOREIGN KEY(parent_id) REFERENCES uploaded_folders(id) ON DELETE CASCADE
             );
             CREATE TABLE uploaded_document_locations (
               document_id TEXT PRIMARY KEY,
               folder_id TEXT,
               sort_order INTEGER NOT NULL,
               FOREIGN KEY(document_id) REFERENCES uploaded_documents(id) ON DELETE CASCADE,
               FOREIGN KEY(folder_id) REFERENCES uploaded_folders(id) ON DELETE SET NULL
             );",
        )
        .expect("create schema");
        db
    }

    /// Build small parsed documents whose section count/text can prove FTS rebuild behavior.
    fn parsed_document(title: &str, texts: &[&str]) -> ParsedDocument {
        ParsedDocument {
            title: title.to_string(),
            format: "html".to_string(),
            view_html: format!("<html><body><h1>{title}</h1></body></html>"),
            sections: texts
                .iter()
                .enumerate()
                .map(|(index, text)| ParsedSection {
                    heading: Some(format!("Section {}", index + 1)),
                    text: (*text).to_string(),
                    page_index: None,
                })
                .collect(),
            cover: None,
            assets: Vec::new(),
        }
    }
}
