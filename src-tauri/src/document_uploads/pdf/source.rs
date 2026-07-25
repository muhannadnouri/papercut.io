//! Canonical PDF source persistence.

use std::fs;

use tauri::Runtime;

use super::super::storage::{upload_dir, upload_url, StoredSourceKind};
use super::super::store::{open_db, upsert_unindexed_document};
use super::super::types::UploadedDocument;

pub(crate) const SOURCE_FILE_NAME: &str = "source.pdf";

/// Restore a checksum-verified transfer payload without trying to parse or
/// index it. Stage 3 rebuilds page text from this canonical source.
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
