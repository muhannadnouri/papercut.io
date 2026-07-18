//! Import / get-source / delete orchestration.
//!
//! These functions sequence the parser, storage, and SQLite store together but
//! contain no parsing, SQL, or path logic themselves. They run on the blocking
//! thread pool (see [`super::commands`]).

use std::fs;
use std::io::Read;

use tauri::Runtime;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_fs::FsExt;

use super::epub::parse_epub_document;
use super::html::{decode_html_bytes, parse_html_document, sanitize_html};
use super::parsed::ParsedDocument;
use super::storage::directory_size;
use super::storage::{
    now_ms, upload_dir, upload_id, upload_id_from_url, MAX_EPUB_UPLOAD_BYTES, MAX_UPLOAD_BYTES,
    UPLOAD_URL_PREFIX,
};
use super::store::{delete_document_rows, open_db, upsert_document};
use super::types::{
    UploadedDocument, UploadedDocumentDeleteRequest, UploadedDocumentDeleteResult,
    UploadedDocumentSourceRequest,
};

/// Full import path: pick a file, enforce size limits, decode HTML bytes, parse and sanitize
/// it, store the sanitized source under app data, and index it into SQLite.
pub(crate) fn import_html<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<UploadedDocument, String> {
    let bytes = pick_and_read_file(
        &app,
        "Import HTML Document",
        "HTML Document",
        &["html", "htm"],
        MAX_UPLOAD_BYTES,
        "HTML document is larger than the 25 MB import limit",
        "Failed to open selected HTML document",
        "Failed to read selected HTML document",
    )?;
    let html = decode_html_bytes(&bytes)?;

    let parsed = parse_html_document(&html);
    if parsed.sections.is_empty() {
        return Err("HTML document did not contain readable text".into());
    }

    persist_document(&app, parsed, html.len() as u64)
}

/// Pick, parse, store, and index a local EPUB file.
pub(crate) fn import_epub<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<UploadedDocument, String> {
    let bytes = pick_and_read_file(
        &app,
        "Import EPUB Book",
        "EPUB Book",
        &["epub"],
        MAX_EPUB_UPLOAD_BYTES,
        "EPUB file is larger than the 100 MB import limit",
        "Failed to open selected EPUB file",
        "Failed to read selected EPUB file",
    )?;
    let parsed = parse_epub_document(&bytes, "Imported EPUB Book")?;
    if parsed.sections.is_empty() {
        return Err("EPUB did not contain readable text".into());
    }

    persist_document(&app, parsed, bytes.len() as u64)
}

fn pick_and_read_file<R: Runtime>(
    app: &tauri::AppHandle<R>,
    title: &str,
    filter_name: &str,
    extensions: &[&str],
    max_bytes: u64,
    too_large_message: &str,
    open_error_prefix: &str,
    read_error_prefix: &str,
) -> Result<Vec<u8>, String> {
    let source = app
        .dialog()
        .file()
        .set_title(title)
        .add_filter(filter_name, extensions)
        .blocking_pick_file()
        .ok_or_else(|| "Document import cancelled".to_string())?;

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

fn persist_document<R: Runtime>(
    app: &tauri::AppHandle<R>,
    parsed: ParsedDocument,
    bytes: u64,
) -> Result<UploadedDocument, String> {
    let imported_at_ms = now_ms()?;
    let id = upload_id(
        &parsed.format,
        &parsed.title,
        &parsed.sections,
        imported_at_ms,
    );
    let url = format!("{UPLOAD_URL_PREFIX}{id}.html");
    let dir = upload_dir(app, &id)?;
    fs::create_dir_all(&dir)
        .map_err(|err| format!("Failed to create upload directory {}: {err}", dir.display()))?;
    fs::write(dir.join("source.html"), parsed.view_html.as_bytes())
        .map_err(|err| format!("Failed to write imported document source: {err}"))?;

    let sections = parsed.sections.len();
    let title = parsed.title.clone();
    let format = parsed.format.clone();
    let mut db = open_db(app)?;
    upsert_document(&mut db, &id, &url, &parsed, imported_at_ms, bytes)?;

    Ok(UploadedDocument {
        id,
        url,
        title,
        format,
        imported_at_ms,
        bytes,
        sections,
    })
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
