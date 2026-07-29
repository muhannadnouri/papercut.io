//! Storage bridge for generated HTML documents such as translated variants.

use std::fs;

use tauri::Runtime;

use super::parsed::{ParsedDocument, ParsedSection};
use super::pipeline;
use super::storage::{directory_size, upload_dir, StoredSourceKind};
use super::store::{delete_document_rows, open_db, upsert_document};
use super::types::UploadedDocumentSourceRequest;

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
/// Files are promoted before their database rows become visible. A failed
/// database write removes the promoted directory so a partial variant cannot
/// appear in the Library.
#[allow(clippy::too_many_arguments)]
pub(crate) fn persist_derived_document<R: Runtime>(
    app: &tauri::AppHandle<R>,
    id: &str,
    url: &str,
    title: &str,
    format: &str,
    view_html: String,
    sections: Vec<DerivedDocumentSection>,
    imported_at_ms: u128,
    bytes: u64,
) -> Result<(), String> {
    let dir = upload_dir(app, id)?;
    let staging_dir = upload_dir(app, &format!("{id}.staging"))?;
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
        staging_dir.join(StoredSourceKind::Html.file_name()),
        view_html.as_bytes(),
    ) {
        let _ = fs::remove_dir_all(&staging_dir);
        return Err(format!("Failed to write derived document source: {err}"));
    }
    if dir.exists() {
        let _ = fs::remove_dir_all(&staging_dir);
        return Err(format!(
            "Derived document directory already exists: {}",
            dir.display()
        ));
    }
    if let Err(err) = fs::rename(&staging_dir, &dir) {
        let _ = fs::remove_dir_all(&staging_dir);
        return Err(format!(
            "Failed to promote derived document directory {}: {err}",
            dir.display()
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
    let mut db = open_db(app)?;
    if let Err(err) = upsert_document(
        &mut db,
        id,
        url,
        &parsed,
        StoredSourceKind::Html,
        imported_at_ms,
        bytes,
    ) {
        let _ = fs::remove_dir_all(&dir);
        return Err(err);
    }
    Ok(())
}

/// Delete only one generated variant and its search rows.
pub(crate) fn delete_derived_document<R: Runtime>(
    app: &tauri::AppHandle<R>,
    id: &str,
) -> Result<u64, String> {
    let dir = upload_dir(app, id)?;
    let bytes_freed = directory_size(&dir)?;
    let mut db = open_db(app)?;
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|err| {
            format!(
                "Failed to delete derived document directory {}: {err}",
                dir.display()
            )
        })?;
    }
    delete_document_rows(&mut db, id)?;
    Ok(bytes_freed)
}
