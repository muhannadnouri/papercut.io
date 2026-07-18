//! Import / get-source / delete orchestration.
//!
//! These functions sequence the parser, storage, and SQLite store together but
//! contain no parsing, SQL, or path logic themselves. They run on the blocking
//! thread pool (see [`super::commands`]).

use std::fs;
use std::io::Read;
use std::path::Path;

use tauri::Runtime;
use tauri_plugin_dialog::{DialogExt, FilePath};
use tauri_plugin_fs::FsExt;

use super::epub::parse_epub_document;
use super::html::{decode_html_bytes, parse_html_document, sanitize_html};
use super::parsed::ParsedDocument;
use super::storage::directory_size;
use super::storage::{
    now_ms, source_upload_id, upload_dir, upload_id_from_url, MAX_EPUB_UPLOAD_BYTES,
    MAX_UPLOAD_BYTES, UPLOAD_URL_PREFIX,
};
use super::store::{delete_document_rows, find_upload_by_id, open_db, upsert_document};
use super::types::{
    UploadedDocument, UploadedDocumentDeleteRequest, UploadedDocumentDeleteResult,
    UploadedDocumentSourceRequest,
};

/// Full import path: pick a file, enforce size limits, decode HTML bytes, parse and sanitize
/// it, store the sanitized source under app data, and index it into SQLite.
pub(crate) fn import_html<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<UploadedDocument, String> {
    let source = pick_file(
        &app,
        "Import HTML Document",
        "HTML Document",
        &["html", "htm"],
    )?;
    import_html_source(&app, source)
}

/// Import one already-selected HTML source; batch import can reuse this path
/// without opening another native dialog.
pub(crate) fn import_html_source<R: Runtime>(
    app: &tauri::AppHandle<R>,
    source: FilePath,
) -> Result<UploadedDocument, String> {
    let bytes = read_file(
        app,
        source,
        MAX_UPLOAD_BYTES,
        "HTML document is larger than the 25 MB import limit",
        "Failed to open selected HTML document",
        "Failed to read selected HTML document",
    )?;
    let id = source_upload_id(&bytes);
    if let Some(existing) = existing_upload(app, &id)? {
        return Ok(existing);
    }
    let html = decode_html_bytes(&bytes)?;

    let parsed = parse_html_document(&html);
    if parsed.sections.is_empty() {
        return Err("HTML document did not contain readable text".into());
    }

    persist_document(app, id, parsed, bytes.len() as u64)
}

/// Pick, parse, store, and index a local EPUB file.
pub(crate) fn import_epub<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<UploadedDocument, String> {
    let source = pick_file(&app, "Import EPUB Book", "EPUB Book", &["epub"])?;
    import_epub_source(&app, source)
}

/// Import one already-selected EPUB source without coupling parsing to a picker.
pub(crate) fn import_epub_source<R: Runtime>(
    app: &tauri::AppHandle<R>,
    source: FilePath,
) -> Result<UploadedDocument, String> {
    let bytes = read_file(
        app,
        source,
        MAX_EPUB_UPLOAD_BYTES,
        "EPUB file is larger than the 100 MB import limit",
        "Failed to open selected EPUB file",
        "Failed to read selected EPUB file",
    )?;
    let id = source_upload_id(&bytes);
    if let Some(existing) = existing_upload(app, &id)? {
        return Ok(existing);
    }
    let parsed = parse_epub_document(&bytes, "Imported EPUB Book")?;
    if parsed.sections.is_empty() {
        return Err("EPUB did not contain readable text".into());
    }

    persist_document(app, id, parsed, bytes.len() as u64)
}

fn pick_file<R: Runtime>(
    app: &tauri::AppHandle<R>,
    title: &str,
    filter_name: &str,
    extensions: &[&str],
) -> Result<FilePath, String> {
    app.dialog()
        .file()
        .set_title(title)
        .add_filter(filter_name, extensions)
        .blocking_pick_file()
        .ok_or_else(|| "Document import cancelled".to_string())
}

fn read_file<R: Runtime>(
    app: &tauri::AppHandle<R>,
    source: FilePath,
    max_bytes: u64,
    too_large_message: &str,
    open_error_prefix: &str,
    read_error_prefix: &str,
) -> Result<Vec<u8>, String> {
    let mut options = tauri_plugin_fs::OpenOptions::new();
    options.read(true);
    let mut file = app
        .fs()
        .open(source, options)
        .map_err(|err| format!("{open_error_prefix}: {err}"))?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|err| format!("{read_error_prefix}: {err}"))?;
    if bytes.len() as u64 > max_bytes {
        return Err(too_large_message.into());
    }
    Ok(bytes)
}

/// Return an exact previously imported file only when its stored reader source
/// still exists; a missing file is repaired by continuing through persistence.
fn existing_upload<R: Runtime>(
    app: &tauri::AppHandle<R>,
    id: &str,
) -> Result<Option<UploadedDocument>, String> {
    let db = open_db(app)?;
    let Some(existing) = find_upload_by_id(&db, id)? else {
        return Ok(None);
    };
    let source_exists = upload_dir(app, id)?.join("source.html").is_file();
    Ok(source_exists.then_some(existing))
}

fn persist_document<R: Runtime>(
    app: &tauri::AppHandle<R>,
    id: String,
    parsed: ParsedDocument,
    bytes: u64,
) -> Result<UploadedDocument, String> {
    let imported_at_ms = now_ms()?;
    let url = format!("{UPLOAD_URL_PREFIX}{id}.html");
    let dir = upload_dir(app, &id)?;
    let mut db = open_db(app)?;
    write_and_index_document(&dir, &mut db, &id, &url, &parsed, imported_at_ms, bytes)?;

    Ok(UploadedDocument {
        id,
        url,
        title: parsed.title,
        format: parsed.format,
        imported_at_ms,
        bytes,
        sections: parsed.sections.len(),
    })
}

/// Keep filesystem and SQLite failures from leaving a newly created upload
/// directory behind. A process crash can still interrupt the two storage systems.
fn write_and_index_document(
    dir: &Path,
    db: &mut rusqlite::Connection,
    id: &str,
    url: &str,
    parsed: &ParsedDocument,
    imported_at_ms: u128,
    bytes: u64,
) -> Result<(), String> {
    fs::create_dir_all(&dir)
        .map_err(|err| format!("Failed to create upload directory {}: {err}", dir.display()))?;
    let result = fs::write(dir.join("source.html"), parsed.view_html.as_bytes())
        .map_err(|err| format!("Failed to write imported document source: {err}"))
        .and_then(|_| upsert_document(db, id, url, parsed, imported_at_ms, bytes));
    if let Err(error) = result {
        fs::remove_dir_all(dir).map_err(|cleanup_error| {
            format!(
                "{error}; failed to clean up incomplete upload {}: {cleanup_error}",
                dir.display()
            )
        })?;
        return Err(error);
    }
    Ok(())
}

/// Resolve an uploaded document URL to its stored source file and return its HTML.
pub(crate) fn get_source<R: Runtime>(
    app: &tauri::AppHandle<R>,
    request: UploadedDocumentSourceRequest,
) -> Result<String, String> {
    let id = upload_id_from_url(&request.document_url)?;
    let path = upload_dir(app, &id)?.join("source.html");
    let source = fs::read_to_string(&path)
        .map_err(|err| format!("Failed to read uploaded document {}: {err}", path.display()))?;
    // Re-sanitize on read so documents imported by older app versions cannot
    // bypass a newer security policy merely because their stored file persists.
    Ok(sanitize_html(&source))
}

/// Delete one upload: remove its stored directory and all of its SQLite rows,
/// reporting how many bytes the directory freed.
pub(crate) fn delete_upload<R: Runtime>(
    app: &tauri::AppHandle<R>,
    request: UploadedDocumentDeleteRequest,
) -> Result<UploadedDocumentDeleteResult, String> {
    let id = upload_id_from_url(&request.document_url)?;
    let dir = upload_dir(app, &id)?;
    let bytes_freed = directory_size(&dir)?;
    let mut db = open_db(app)?;

    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|err| {
            format!(
                "Failed to delete uploaded document files {}: {err}",
                dir.display()
            )
        })?;
    }

    // Delete search rows in one transaction so the FTS table and metadata cannot drift apart.
    delete_document_rows(&mut db, &id)?;

    Ok(UploadedDocumentDeleteResult {
        id,
        url: request.document_url,
        bytes_freed,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use rusqlite::Connection;

    use super::write_and_index_document;
    use crate::document_uploads::parsed::{ParsedDocument, ParsedSection};

    #[test]
    fn failed_index_write_removes_incomplete_upload_directory() {
        let dir = std::env::temp_dir().join(format!(
            "papercut-upload-cleanup-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let parsed = ParsedDocument {
            title: "Test".into(),
            format: "html".into(),
            view_html: "<html><body>Test</body></html>".into(),
            sections: vec![ParsedSection {
                heading: None,
                text: "Test".into(),
            }],
        };
        let mut db = Connection::open_in_memory().expect("open database without upload schema");

        let error =
            write_and_index_document(&dir, &mut db, "abc", "/uploads/abc.html", &parsed, 1, 4)
                .expect_err("missing schema must fail");

        assert!(error.contains("Document upload database error"));
        assert!(!dir.exists());
        let _ = fs::remove_dir_all(dir);
    }
}
