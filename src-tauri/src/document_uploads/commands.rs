//! The `#[tauri::command]` edge.
//!
//! Each command is a thin wrapper: it moves the blocking pipeline/store/search
//! work onto the blocking thread pool so the async runtime is never stalled by
//! filesystem or SQLite I/O, then maps a join error into a `String`. All real
//! logic lives in the modules these delegate to.

use tauri::Runtime;

use super::batch::{delete_batch, import_batch, import_folder};
use super::organization::{
    create_folder, delete_folder, list_organization, move_documents, move_folder, rename_folder,
    reorder,
};
use super::pdf::{
    finalize_pdf_index, get_pdf_readable_blocks, get_pdf_source_bytes, get_pdf_source_path,
    store_pdf_page_text, PdfFinalizeRequest, PdfPageTextRequest,
};
use super::pipeline::{delete_upload, get_cover, get_source};
use super::search::search_uploads;
use super::store::list_uploads;
use super::types::{
    UploadedDocument, UploadedDocumentBatchResult, UploadedDocumentDeleteBatchRequest,
    UploadedDocumentDeleteBatchResult, UploadedDocumentDeleteRequest, UploadedDocumentDeleteResult,
    UploadedDocumentSearchRequest, UploadedDocumentSearchResult, UploadedDocumentSourceRequest,
    UploadedLibraryCreateFolderRequest, UploadedLibraryDeleteFolderRequest,
    UploadedLibraryMoveDocumentsRequest, UploadedLibraryMoveFolderRequest,
    UploadedLibraryOrganization, UploadedLibraryRenameFolderRequest, UploadedLibraryReorderRequest,
};
use super::DocumentUploadState;

/// Pick multiple HTML, EPUB, or PDF files and import them as one cancellable batch.
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

/// Pick one desktop folder and import its direct supported children as a batch.
#[tauri::command]
pub async fn document_uploads_import_folder<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, DocumentUploadState>,
) -> Result<UploadedDocumentBatchResult, String> {
    let control = state.begin_batch()?;
    tauri::async_runtime::spawn_blocking(move || import_folder(app, control))
        .await
        .map_err(|err| format!("Document folder import task failed: {err}"))?
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

/// Read one retained EPUB cover through the validated upload boundary.
#[tauri::command]
pub async fn document_uploads_get_cover<R: Runtime>(
    app: tauri::AppHandle<R>,
    request: UploadedDocumentSourceRequest,
) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || get_cover(&app, request))
        .await
        .map_err(|err| format!("Document upload cover task failed: {err}"))?
}

/// Stream one canonical PDF source to PDF.js as binary IPC, not JSON/base64.
#[tauri::command]
pub async fn document_uploads_get_pdf_source<R: Runtime>(
    app: tauri::AppHandle<R>,
    request: UploadedDocumentSourceRequest,
) -> Result<tauri::ipc::Response, String> {
    let bytes = tauri::async_runtime::spawn_blocking(move || {
        get_pdf_source_bytes(&app, &request.document_url)
    })
    .await
    .map_err(|err| format!("PDF source task failed: {err}"))??;
    Ok(tauri::ipc::Response::new(bytes))
}

/// Return a validated app-data path for Tauri's scoped, range-capable asset protocol.
#[tauri::command]
pub async fn document_uploads_get_pdf_asset_path<R: Runtime>(
    app: tauri::AppHandle<R>,
    request: UploadedDocumentSourceRequest,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = get_pdf_source_path(&app, &request.document_url)?;
        path.into_os_string()
            .into_string()
            .map_err(|_| "PDF source path is not valid UTF-8".to_string())
    })
    .await
    .map_err(|err| format!("PDF asset path task failed: {err}"))?
}

/// Return validated, ordered block text for narration without PDF geometry.
#[tauri::command]
pub async fn document_uploads_get_pdf_readable_blocks<R: Runtime>(
    app: tauri::AppHandle<R>,
    request: UploadedDocumentSourceRequest,
) -> Result<Vec<Vec<String>>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        get_pdf_readable_blocks(&app, &request.document_url)
    })
    .await
    .map_err(|err| format!("PDF readable text task failed: {err}"))?
}

/// Persist one bounded page text layer emitted by PDF.js.
#[tauri::command]
pub async fn document_uploads_store_pdf_page_text<R: Runtime>(
    app: tauri::AppHandle<R>,
    request: PdfPageTextRequest,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || store_pdf_page_text(&app, request))
        .await
        .map_err(|err| format!("PDF page text task failed: {err}"))?
}

/// Replace a PDF's page and FTS rows after all sidecars are durable.
#[tauri::command]
pub async fn document_uploads_finalize_pdf<R: Runtime>(
    app: tauri::AppHandle<R>,
    request: PdfFinalizeRequest,
) -> Result<UploadedDocument, String> {
    tauri::async_runtime::spawn_blocking(move || finalize_pdf_index(&app, request))
        .await
        .map_err(|err| format!("PDF index task failed: {err}"))?
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

/// Delete selected uploads sequentially and retain per-document failures.
#[tauri::command]
pub async fn document_uploads_delete_batch<R: Runtime>(
    app: tauri::AppHandle<R>,
    request: UploadedDocumentDeleteBatchRequest,
) -> Result<UploadedDocumentDeleteBatchResult, String> {
    tauri::async_runtime::spawn_blocking(move || delete_batch(app, request))
        .await
        .map_err(|err| format!("Document upload delete batch task failed: {err}"))?
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
