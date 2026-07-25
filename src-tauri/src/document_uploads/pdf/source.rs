//! Canonical PDF source persistence.

use std::fs;

use tauri::Runtime;
use tauri_plugin_dialog::FilePath;

use super::super::storage::{
    now_ms, read_source_bytes, source_upload_id, upload_dir, upload_reference_from_url,
    upload_source_path, upload_url, StoredSourceKind, MAX_PDF_UPLOAD_BYTES,
};
use super::super::store::{find_upload_by_id, open_db, upsert_unindexed_document};
use super::super::types::UploadedDocument;

pub(crate) const SOURCE_FILE_NAME: &str = "source.pdf";

/// Store one selected PDF before PDF.js derives its rebuildable page text.
/// The source is not considered searchable until the separate finalization
/// command atomically replaces its page and FTS rows.
pub(crate) fn import_pdf_source<R: Runtime>(
    app: &tauri::AppHandle<R>,
    source: FilePath,
    fallback_title: String,
) -> Result<UploadedDocument, String> {
    let bytes = read_source_bytes(
        app,
        source,
        MAX_PDF_UPLOAD_BYTES,
        "PDF file is larger than the 250 MB import limit",
        "Failed to open selected PDF",
        "Failed to read selected PDF",
    )?;
    if !bytes.starts_with(b"%PDF-") {
        return Err("Selected file does not have a valid PDF header".into());
    }
    let id = source_upload_id(&bytes);
    if let Some(existing) = existing_pdf(app, &id)? {
        return Ok(existing);
    }

    let imported_at_ms = now_ms()?;
    let source_kind = StoredSourceKind::Pdf;
    let url = upload_url(&id, source_kind);
    persist_unindexed_pdf(app, id, url, fallback_title, bytes, imported_at_ms)
}

/// Return raw bytes through Tauri's binary IPC response rather than expanding a
/// potentially large PDF into a JSON number array or base64 string.
pub(crate) fn get_pdf_source_bytes<R: Runtime>(
    app: &tauri::AppHandle<R>,
    document_url: &str,
) -> Result<Vec<u8>, String> {
    let (id, source_kind) = upload_reference_from_url(document_url)?;
    if source_kind != StoredSourceKind::Pdf {
        return Err("Document is not an uploaded PDF".into());
    }
    let db = open_db(app)?;
    let document =
        find_upload_by_id(&db, &id)?.ok_or_else(|| "PDF upload metadata is missing".to_string())?;
    if StoredSourceKind::from_str(&document.source_kind)? != source_kind {
        return Err("Uploaded PDF source metadata does not match its URL".into());
    }
    let path = upload_source_path(app, &id, source_kind)?;
    let metadata =
        fs::metadata(&path).map_err(|err| format!("Failed to inspect PDF source: {err}"))?;
    if metadata.len() > MAX_PDF_UPLOAD_BYTES {
        return Err("Stored PDF exceeds the 250 MB import limit".into());
    }
    fs::read(&path).map_err(|err| format!("Failed to read PDF source: {err}"))
}

/// Restore a checksum-verified transfer payload without trying to parse or
/// index it. The PDF import/index path rebuilds page text from this source.
///
/// Refusing an existing directory is important: failure cleanup removes the
/// whole directory and must never erase a previously imported document.
pub(crate) fn restore_transferred_pdf<R: Runtime>(
    app: &tauri::AppHandle<R>,
    id: String,
    title: String,
    source: Vec<u8>,
    imported_at_ms: u128,
    original_bytes: u64,
) -> Result<UploadedDocument, String> {
    if !source.starts_with(b"%PDF-") {
        return Err("Transferred PDF source has an invalid header".into());
    }
    if imported_at_ms > i64::MAX as u128 {
        return Err("Transferred document timestamp is invalid".into());
    }

    let source_kind = StoredSourceKind::Pdf;
    let url = upload_url(&id, source_kind);
    let dir = upload_dir(app, &id)?;
    if dir.exists() {
        return Err("Transferred PDF upload already exists".into());
    }
    fs::create_dir_all(&dir)
        .map_err(|err| format!("Failed to create PDF upload directory: {err}"))?;
    let result = (|| {
        fs::write(dir.join(SOURCE_FILE_NAME), source)
            .map_err(|err| format!("Failed to store PDF source: {err}"))?;
        let mut db = open_db(app)?;
        upsert_unindexed_document(
            &mut db,
            &id,
            &url,
            &title,
            "pdf",
            source_kind,
            imported_at_ms,
            original_bytes,
        )
    })();
    if let Err(error) = result {
        fs::remove_dir_all(&dir).map_err(|cleanup_error| {
            format!(
                "{error}; failed to clean up incomplete PDF upload {}: {cleanup_error}",
                dir.display()
            )
        })?;
        return Err(error);
    }

    Ok(UploadedDocument {
        id,
        url,
        title,
        format: "pdf".into(),
        source_kind: source_kind.as_str().into(),
        imported_at_ms,
        bytes: original_bytes,
        sections: 0,
        cover_media_type: None,
    })
}

fn existing_pdf<R: Runtime>(
    app: &tauri::AppHandle<R>,
    id: &str,
) -> Result<Option<UploadedDocument>, String> {
    let db = open_db(app)?;
    let Some(existing) = find_upload_by_id(&db, id)? else {
        return Ok(None);
    };
    let source_kind = StoredSourceKind::from_str(&existing.source_kind)?;
    let source_exists = upload_source_path(app, id, source_kind)?.is_file();
    Ok((source_kind == StoredSourceKind::Pdf && source_exists).then_some(existing))
}

fn persist_unindexed_pdf<R: Runtime>(
    app: &tauri::AppHandle<R>,
    id: String,
    url: String,
    title: String,
    source: Vec<u8>,
    imported_at_ms: u128,
) -> Result<UploadedDocument, String> {
    let bytes = source.len() as u64;
    let source_kind = StoredSourceKind::Pdf;
    let dir = upload_dir(app, &id)?;
    fs::create_dir_all(&dir)
        .map_err(|err| format!("Failed to create PDF upload directory: {err}"))?;
    let result = (|| {
        fs::write(dir.join(SOURCE_FILE_NAME), source)
            .map_err(|err| format!("Failed to store PDF source: {err}"))?;
        let mut db = open_db(app)?;
        upsert_unindexed_document(
            &mut db,
            &id,
            &url,
            &title,
            "pdf",
            source_kind,
            imported_at_ms,
            bytes,
        )
    })();
    if let Err(error) = result {
        fs::remove_dir_all(&dir).map_err(|cleanup_error| {
            format!(
                "{error}; failed to clean up incomplete PDF upload {}: {cleanup_error}",
                dir.display()
            )
        })?;
        return Err(error);
    }

    Ok(UploadedDocument {
        id,
        url,
        title,
        format: "pdf".into(),
        source_kind: source_kind.as_str().into(),
        imported_at_ms,
        bytes,
        sections: 0,
        cover_media_type: None,
    })
}
