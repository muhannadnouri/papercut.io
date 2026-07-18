//! The `#[tauri::command]` edge.
//!
//! Each command is a thin wrapper: it moves the blocking pipeline/store/search
//! work onto the blocking thread pool so the async runtime is never stalled by
//! filesystem or SQLite I/O, then maps a join error into a `String`. All real
//! logic lives in the modules these delegate to.

use tauri::Runtime;

use super::batch::import_batch;
use super::organization::{
    create_folder, delete_folder, list_organization, move_documents, move_folder, rename_folder,
    reorder,
};
use super::pipeline::{delete_upload, get_source, import_epub, import_html};
use super::search::search_uploads;
use super::store::list_uploads;
use super::types::{
    UploadedDocument, UploadedDocumentBatchResult, UploadedDocumentDeleteRequest,
    UploadedDocumentDeleteResult, UploadedDocumentSearchRequest, UploadedDocumentSearchResult,
    UploadedDocumentSourceRequest, UploadedLibraryCreateFolderRequest,
    UploadedLibraryDeleteFolderRequest, UploadedLibraryMoveDocumentsRequest,
    UploadedLibraryMoveFolderRequest, UploadedLibraryOrganization,
    UploadedLibraryRenameFolderRequest, UploadedLibraryReorderRequest,
};
use super::DocumentUploadState;

/// Open the native picker, import the chosen HTML file, and return its metadata.
#[tauri::command]
pub async fn document_uploads_import_html<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<UploadedDocument, String> {
    tauri::async_runtime::spawn_blocking(move || import_html(app))
        .await
        .map_err(|err| format!("Document import task failed: {err}"))?
}

/// Open the native picker, import the chosen EPUB file, and return its metadata.
#[tauri::command]
pub async fn document_uploads_import_epub<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<UploadedDocument, String> {
    tauri::async_runtime::spawn_blocking(move || import_epub(app))
        .await
        .map_err(|err| format!("Document import task failed: {err}"))?
}

/// Pick multiple HTML/EPUB files and import them as one cancellable sequential batch.
#[tauri::command]
pub async fn document_uploads_import_batch<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, DocumentUploadState>,
) -> Result<UploadedDocumentBatchResult, String> {
    let control = state.begin_batch()?;
    tauri::async_runtime::spawn_blocking(move || import_batch(app, control))
        .await
        .map_err(|err| format!("Document batch import task failed: {err}"))?
}

/// Request cancellation after the currently importing file finishes.
#[tauri::command]
pub fn document_uploads_cancel_import_batch(
    state: tauri::State<'_, DocumentUploadState>,
) -> Result<bool, String> {
    state.cancel_batch()
}

/// List all stored uploads, newest first.
#[tauri::command]
pub async fn document_uploads_list<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<UploadedDocument>, String> {
    tauri::async_runtime::spawn_blocking(move || list_uploads(&app))
        .await
        .map_err(|err| format!("Document upload list task failed: {err}"))?
}

/// Run a full-text search across uploaded documents.
#[tauri::command]
pub async fn document_uploads_search<R: Runtime>(
    app: tauri::AppHandle<R>,
    request: UploadedDocumentSearchRequest,
) -> Result<Vec<UploadedDocumentSearchResult>, String> {
    tauri::async_runtime::spawn_blocking(move || search_uploads(&app, request))
        .await
        .map_err(|err| format!("Document upload search task failed: {err}"))?
}

/// Read the stored sanitized source HTML for an uploaded document URL.
#[tauri::command]
pub async fn document_uploads_get_source<R: Runtime>(
    app: tauri::AppHandle<R>,
    request: UploadedDocumentSourceRequest,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || get_source(&app, request))
        .await
        .map_err(|err| format!("Document upload source task failed: {err}"))?
}

/// Delete an uploaded document's rows and stored source directory.
#[tauri::command]
pub async fn document_uploads_delete<R: Runtime>(
    app: tauri::AppHandle<R>,
    request: UploadedDocumentDeleteRequest,
) -> Result<UploadedDocumentDeleteResult, String> {
    tauri::async_runtime::spawn_blocking(move || delete_upload(&app, request))
        .await
        .map_err(|err| format!("Document upload delete task failed: {err}"))?
}

/// Return uploaded-document folder and manual ordering metadata.
#[tauri::command]
pub async fn document_uploads_library_organization<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<UploadedLibraryOrganization, String> {
    tauri::async_runtime::spawn_blocking(move || list_organization(&app))
        .await
        .map_err(|err| format!("Document library organization task failed: {err}"))?
}

/// Create a user folder for uploaded documents.
#[tauri::command]
pub async fn document_uploads_create_folder<R: Runtime>(
    app: tauri::AppHandle<R>,
    request: UploadedLibraryCreateFolderRequest,
) -> Result<super::types::UploadedLibraryFolder, String> {
    tauri::async_runtime::spawn_blocking(move || create_folder(&app, request))
        .await
        .map_err(|err| format!("Document folder create task failed: {err}"))?
}

/// Rename a user folder without changing contained document URLs.
#[tauri::command]
pub async fn document_uploads_rename_folder<R: Runtime>(
    app: tauri::AppHandle<R>,
    request: UploadedLibraryRenameFolderRequest,
) -> Result<super::types::UploadedLibraryFolder, String> {
    tauri::async_runtime::spawn_blocking(move || rename_folder(&app, request))
        .await
        .map_err(|err| format!("Document folder rename task failed: {err}"))?
}

/// Delete an empty user folder.
#[tauri::command]
pub async fn document_uploads_delete_folder<R: Runtime>(
    app: tauri::AppHandle<R>,
    request: UploadedLibraryDeleteFolderRequest,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || delete_folder(&app, request))
        .await
        .map_err(|err| format!("Document folder delete task failed: {err}"))?
}

/// Move uploaded documents between folders by metadata only.
#[tauri::command]
pub async fn document_uploads_move_documents<R: Runtime>(
    app: tauri::AppHandle<R>,
    request: UploadedLibraryMoveDocumentsRequest,
) -> Result<UploadedLibraryOrganization, String> {
    tauri::async_runtime::spawn_blocking(move || move_documents(&app, request))
        .await
        .map_err(|err| format!("Document move task failed: {err}"))?
}

/// Move a folder while preserving document URLs and preventing cycles.
#[tauri::command]
pub async fn document_uploads_move_folder<R: Runtime>(
    app: tauri::AppHandle<R>,
    request: UploadedLibraryMoveFolderRequest,
) -> Result<UploadedLibraryOrganization, String> {
    tauri::async_runtime::spawn_blocking(move || move_folder(&app, request))
        .await
        .map_err(|err| format!("Document folder move task failed: {err}"))?
}

/// Persist manual sibling order for one uploaded-library folder/root.
#[tauri::command]
pub async fn document_uploads_reorder_library<R: Runtime>(
    app: tauri::AppHandle<R>,
    request: UploadedLibraryReorderRequest,
) -> Result<UploadedLibraryOrganization, String> {
    tauri::async_runtime::spawn_blocking(move || reorder(&app, request))
        .await
        .map_err(|err| format!("Document library reorder task failed: {err}"))?
}
