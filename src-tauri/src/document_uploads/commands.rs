//! The `#[tauri::command]` edge.
//!
//! Each command is a thin wrapper: it moves the blocking pipeline/store/search
//! work onto the blocking thread pool so the async runtime is never stalled by
//! filesystem or SQLite I/O, then maps a join error into a `String`. All real
//! logic lives in the modules these delegate to.

use tauri::Runtime;

use super::batch::{delete_batch, import_batch, import_folder, import_paths};
use super::organization::{
    create_folder, delete_folder, list_organization, move_documents, move_folder, rename_folder,
    reorder,
};
use super::pdf::{
    finalize_pdf_index, get_pdf_narration_segments, get_pdf_page_text_layer, get_pdf_source_bytes,
    get_pdf_source_path, pdf_has_ocr_text, store_pdf_page_text, PageTextLayer, PdfFinalizeRequest,
    PdfNarrationSegment, PdfPageTextReadRequest, PdfPageTextRequest,
};
use super::pipeline::{delete_upload, get_cover, get_source, import_pasted_text};
use super::search::{find_pdf_text, search_uploads};
use super::store::{list_uploads, open_db, update_document_title};
use super::types::{
    UploadedDocument, UploadedDocumentBatchResult, UploadedDocumentDeleteBatchRequest,
    UploadedDocumentDeleteBatchResult, UploadedDocumentDeleteRequest, UploadedDocumentDeleteResult,
    UploadedDocumentPastedTextRequest, UploadedDocumentPathImportRequest,
    UploadedDocumentSearchRequest, UploadedDocumentSearchResult, UploadedDocumentSource,
    UploadedDocumentSourceRequest, UploadedDocumentTitleUpdateRequest,
    UploadedLibraryCreateFolderRequest, UploadedLibraryDeleteFolderRequest,
    UploadedLibraryMoveDocumentsRequest, UploadedLibraryMoveFolderRequest,
    UploadedLibraryOrganization, UploadedLibraryRenameFolderRequest, UploadedLibraryReorderRequest,
    UploadedPdfFindRequest, UploadedPdfFindResult,
};
use super::DocumentUploadState;

/// Pick multiple HTML, EPUB, PDF, TXT, or Markdown files and import them as one cancellable batch.
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

/// Pick one desktop folder and preserve supported files through five visible levels.
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

/// Save user-authored plain text through the same parser and local index used
/// by imported TXT files; no clipboard or temporary-file access is required.
#[tauri::command]
pub async fn document_uploads_import_pasted_text<R: Runtime>(
    app: tauri::AppHandle<R>,
    request: UploadedDocumentPastedTextRequest,
) -> Result<UploadedDocument, String> {
    tauri::async_runtime::spawn_blocking(move || {
        import_pasted_text(&app, &request.title, &request.text)
    })
    .await
    .map_err(|err| format!("Pasted text import task failed: {err}"))?
}

/// Import files selected through a native desktop path entry point.
#[tauri::command]
pub async fn document_uploads_import_paths<R: Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, DocumentUploadState>,
    request: UploadedDocumentPathImportRequest,
) -> Result<UploadedDocumentBatchResult, String> {
    let control = state.begin_batch()?;
    tauri::async_runtime::spawn_blocking(move || import_paths(app, control, request.paths))
        .await
        .map_err(|err| format!("Dropped document import task failed: {err}"))?
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

/// Update an uploaded document's display title and FTS metadata atomically.
#[tauri::command]
pub async fn document_uploads_update_title<R: Runtime>(
    app: tauri::AppHandle<R>,
    request: UploadedDocumentTitleUpdateRequest,
) -> Result<UploadedDocument, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut db = open_db(&app)?;
        update_document_title(&mut db, &request.document_url, &request.title)
    })
    .await
    .map_err(|err| format!("Document title update task failed: {err}"))?
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

/// Find literal matches in one PDF without loading its page text into the WebView.
#[tauri::command]
pub async fn document_uploads_find_pdf_text<R: Runtime>(
    app: tauri::AppHandle<R>,
    request: UploadedPdfFindRequest,
) -> Result<UploadedPdfFindResult, String> {
    tauri::async_runtime::spawn_blocking(move || find_pdf_text(&app, request))
        .await
        .map_err(|err| format!("PDF Find task failed: {err}"))?
}

/// Read the stored sanitized source HTML for an uploaded document URL.
#[tauri::command]
pub async fn document_uploads_get_source<R: Runtime>(
    app: tauri::AppHandle<R>,
    request: UploadedDocumentSourceRequest,
) -> Result<UploadedDocumentSource, String> {
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

/// Return reconstructed narration text with compact page/block source runs.
#[tauri::command]
pub async fn document_uploads_get_pdf_narration_segments<R: Runtime>(
    app: tauri::AppHandle<R>,
    request: UploadedDocumentSourceRequest,
) -> Result<Vec<PdfNarrationSegment>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        get_pdf_narration_segments(&app, &request.document_url)
    })
    .await
    .map_err(|err| format!("PDF narration text task failed: {err}"))?
}

/// Read one validated derived PDF page layer for the currently rendered page.
#[tauri::command]
pub async fn document_uploads_get_pdf_page_text<R: Runtime>(
    app: tauri::AppHandle<R>,
    request: PdfPageTextReadRequest,
) -> Result<PageTextLayer, String> {
    tauri::async_runtime::spawn_blocking(move || get_pdf_page_text_layer(&app, request))
        .await
        .map_err(|err| format!("PDF page text read task failed: {err}"))?
}

/// Return the finalized document-level OCR signal used to select viewer Find.
#[tauri::command]
pub async fn document_uploads_pdf_has_ocr_text<R: Runtime>(
    app: tauri::AppHandle<R>,
    request: UploadedDocumentSourceRequest,
) -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(move || pdf_has_ocr_text(&app, &request.document_url))
        .await
        .map_err(|err| format!("PDF OCR marker task failed: {err}"))?
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
