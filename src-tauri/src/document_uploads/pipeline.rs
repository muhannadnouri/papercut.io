//! Import / get-source / delete orchestration.
//!
//! These functions sequence the parser, storage, and SQLite store together but
//! contain no parsing, SQL, or path logic themselves. They run on the blocking
//! thread pool (see [`super::commands`]).

use std::fs;
use std::io::Read;
use std::path::Path;

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use tauri::Runtime;
use tauri_plugin_dialog::FilePath;
use tauri_plugin_fs::FsExt;

use super::cover::{
    backfill_thumbnail, write_thumbnail, THUMBNAIL_FILE_NAME, THUMBNAIL_MEDIA_TYPE,
};
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

const MAX_STORED_COVER_BYTES: u64 = 5 * 1024 * 1024;

/// Import one already-selected HTML source without coupling parsing to a picker.
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
    let cover_media_type = parsed
        .cover
        .as_ref()
        .map(|cover| cover.media_type.to_string());

    Ok(UploadedDocument {
        id,
        url,
        title: parsed.title,
        format: parsed.format,
        imported_at_ms,
        bytes,
        sections: parsed.sections.len(),
        cover_media_type,
    })
}

/// Restore normalized HTML from a library-transfer package while preserving
/// the source-derived id that existing document URLs and audiobook references
/// use. The transferred source still passes through the current sanitizer and
/// parser so search rows are rebuilt with this app version.
pub(crate) fn restore_transferred_document<R: Runtime>(
    app: &tauri::AppHandle<R>,
    id: String,
    source_html: String,
    format: String,
    imported_at_ms: u128,
    bytes: u64,
) -> Result<UploadedDocument, String> {
    let url = format!("{UPLOAD_URL_PREFIX}{id}.html");
    upload_id_from_url(&url)?;
    if imported_at_ms > i64::MAX as u128 {
        return Err("Transferred document timestamp is invalid".into());
    }
    if !matches!(format.as_str(), "html" | "epub") {
        return Err(format!(
            "Unsupported transferred document format {format:?}"
        ));
    }

    let mut parsed = parse_html_document(&source_html);
    parsed.format = format;
    if parsed.sections.is_empty() {
        return Err("Transferred document did not contain readable text".into());
    }
    let dir = upload_dir(app, &id)?;
    let mut db = open_db(app)?;
    write_and_index_document(&dir, &mut db, &id, &url, &parsed, imported_at_ms, bytes)?;
    let cover_media_type = parsed
        .cover
        .as_ref()
        .map(|cover| cover.media_type.to_string());

    Ok(UploadedDocument {
        id,
        url,
        title: parsed.title,
        format: parsed.format,
        imported_at_ms,
        bytes,
        sections: parsed.sections.len(),
        cover_media_type,
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
    let result = (|| {
        fs::write(dir.join("source.html"), parsed.view_html.as_bytes())
            .map_err(|err| format!("Failed to write imported document source: {err}"))?;
        if let Some(cover) = &parsed.cover {
            fs::write(dir.join(cover.file_name), &cover.bytes)
                .map_err(|err| format!("Failed to write imported document cover: {err}"))?;
            if let Err(error) = write_thumbnail(dir, &cover.bytes) {
                log::warn!("Skipping unusable imported document cover thumbnail: {error}");
            }
        }
        upsert_document(db, id, url, parsed, imported_at_ms, bytes)
    })();
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

/// Return only a display-sized cover associated with a validated upload URL.
///
/// The database MIME value selects a fixed filename, so neither the frontend nor
/// stored metadata can turn this command into arbitrary app-data file access.
/// Existing imports receive the same persisted thumbnail lazily on first access.
pub(crate) fn get_cover<R: Runtime>(
    app: &tauri::AppHandle<R>,
    request: UploadedDocumentSourceRequest,
) -> Result<Option<String>, String> {
    let id = upload_id_from_url(&request.document_url)?;
    let db = open_db(app)?;
    let Some(document) = find_upload_by_id(&db, &id)? else {
        return Ok(None);
    };
    let Some(media_type) = document.cover_media_type.as_deref() else {
        return Ok(None);
    };
    let Some(file_name) = stored_cover_file_name(media_type) else {
        return Ok(None);
    };
    let dir = upload_dir(app, &id)?;
    let source_path = dir.join(file_name);
    let thumbnail_path = dir.join(THUMBNAIL_FILE_NAME);
    if !thumbnail_path.is_file() {
        match backfill_thumbnail(&dir, &source_path) {
            Ok(()) => {}
            Err(_) if !source_path.is_file() => return Ok(None),
            Err(error) => {
                log::warn!("Skipping unusable uploaded document cover thumbnail: {error}");
                return Ok(None);
            }
        }
    }
    let mut file = match fs::File::open(&thumbnail_path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "Failed to open uploaded document cover thumbnail {}: {error}",
                thumbnail_path.display()
            ))
        }
    };
    let mut bytes = Vec::new();
    file.by_ref()
        .take(MAX_STORED_COVER_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Failed to read uploaded document cover: {error}"))?;
    if bytes.len() as u64 > MAX_STORED_COVER_BYTES {
        return Err("Uploaded document cover thumbnail exceeds the 5 MB limit".into());
    }
    Ok(Some(format!(
        "data:{THUMBNAIL_MEDIA_TYPE};base64,{}",
        BASE64_STANDARD.encode(bytes)
    )))
}

fn stored_cover_file_name(media_type: &str) -> Option<&'static str> {
    match media_type {
        "image/png" => Some("cover.png"),
        "image/jpeg" => Some("cover.jpg"),
        "image/gif" => Some("cover.gif"),
        "image/webp" => Some("cover.webp"),
        _ => None,
    }
}

/// Delete one upload through the same compensating filesystem/SQLite flow used
/// by both single and batch commands.
pub(crate) fn delete_upload<R: Runtime>(
    app: &tauri::AppHandle<R>,
    request: UploadedDocumentDeleteRequest,
) -> Result<UploadedDocumentDeleteResult, String> {
    let id = upload_id_from_url(&request.document_url)?;
    let dir = upload_dir(app, &id)?;
    let mut db = open_db(app)?;
    let bytes_freed = delete_stored_document(&dir, &id, || {
        // Store deletion is transactional, keeping metadata, sections, and FTS in sync.
        delete_document_rows(&mut db, &id)
    })?;

    Ok(UploadedDocumentDeleteResult {
        id,
        url: request.document_url,
        bytes_freed,
    })
}

/// Stage files beside their live directory so a failed SQLite transaction can
/// restore them. A leftover staging directory is restored first, making a retry
/// recover from an interruption on either side of the database commit.
fn delete_stored_document<F>(dir: &Path, id: &str, mut delete_rows: F) -> Result<u64, String>
where
    F: FnMut() -> Result<(), String>,
{
    let staged = dir.with_file_name(format!(".{id}.deleting"));
    if staged.exists() {
        if dir.exists() {
            return Err(format!(
                "Uploaded document has conflicting live and staged storage for {id}"
            ));
        }
        fs::rename(&staged, dir).map_err(|err| {
            format!(
                "Failed to recover interrupted document deletion {}: {err}",
                staged.display()
            )
        })?;
    }

    let bytes_freed = directory_size(dir)?;
    if !dir.exists() {
        delete_rows()?;
        return Ok(0);
    }
    fs::rename(dir, &staged).map_err(|err| {
        format!(
            "Failed to stage uploaded document files {}: {err}",
            dir.display()
        )
    })?;

    if let Err(error) = delete_rows() {
        return match fs::rename(&staged, dir) {
            Ok(()) => Err(error),
            Err(restore_error) => Err(format!(
                "{error}; failed to restore uploaded document files {}: {restore_error}",
                dir.display()
            )),
        };
    }

    fs::remove_dir_all(&staged).map_err(|err| {
        format!(
            "Document metadata was deleted, but staged files could not be removed {}: {err}",
            staged.display()
        )
    })?;
    Ok(bytes_freed)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use rusqlite::Connection;

    use super::{delete_stored_document, stored_cover_file_name, write_and_index_document};
    use crate::document_uploads::parsed::{ParsedDocument, ParsedSection};

    #[test]
    fn stored_cover_names_are_fixed_to_safe_raster_types() {
        assert_eq!(stored_cover_file_name("image/png"), Some("cover.png"));
        assert_eq!(stored_cover_file_name("image/jpeg"), Some("cover.jpg"));
        assert_eq!(stored_cover_file_name("image/svg+xml"), None);
        assert_eq!(stored_cover_file_name("../../source.html"), None);
    }

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
            cover: None,
        };
        let mut db = Connection::open_in_memory().expect("open database without upload schema");

        let error =
            write_and_index_document(&dir, &mut db, "abc", "/uploads/abc.html", &parsed, 1, 4)
                .expect_err("missing schema must fail");

        assert!(error.contains("Document upload database error"));
        assert!(!dir.exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn failed_delete_transaction_restores_staged_files() {
        let root = std::env::temp_dir().join(format!(
            "papercut-upload-delete-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let dir = root.join("abc");
        fs::create_dir_all(&dir).expect("create document directory");
        fs::write(dir.join("source.html"), b"test").expect("write document source");

        let error = delete_stored_document(&dir, "abc", || Err("database failed".into()))
            .expect_err("delete must fail");

        assert_eq!(error, "database failed");
        assert!(dir.join("source.html").is_file());
        assert!(!root.join(".abc.deleting").exists());
        fs::remove_dir_all(root).expect("remove test directory");
    }
}
