//! Serde data-transfer objects exchanged with the frontend.
//!
//! Fields are `pub(crate)` so sibling modules (pipeline, store, search) can
//! build and read them while the structs stay private to the upload feature.

use serde::{Deserialize, Serialize};

/// Metadata for one stored upload, returned by import and list.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadedDocument {
    pub(crate) id: String,
    pub(crate) url: String,
    pub(crate) title: String,
    pub(crate) original_file_name: Option<String>,
    pub(crate) format: String,
    pub(crate) source_kind: String,
    pub(crate) imported_at_ms: u128,
    pub(crate) bytes: u64,
    pub(crate) sections: usize,
    pub(crate) cover_media_type: Option<String>,
    pub(crate) text_status: String,
}

/// One file that could not be imported while the rest of its batch continued.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadedDocumentBatchFailure {
    pub(crate) file_name: String,
    pub(crate) error: String,
}

/// Count-based progress emitted while a sequential document batch runs.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadedDocumentBatchProgress {
    pub(crate) phase: String,
    pub(crate) processed: usize,
    pub(crate) total: usize,
    pub(crate) imported: usize,
    pub(crate) already_in_library: usize,
    pub(crate) failed: usize,
    pub(crate) file_name: Option<String>,
}

/// Final batch outcome. `imported` retains every successful document for
/// downstream indexing, while `already_in_library` identifies reused sources.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadedDocumentBatchResult {
    pub(crate) selected: usize,
    pub(crate) processed: usize,
    pub(crate) imported: Vec<UploadedDocument>,
    pub(crate) already_in_library: Vec<String>,
    pub(crate) failures: Vec<UploadedDocumentBatchFailure>,
    pub(crate) cancelled: bool,
}

/// One FTS hit: a matching section with a highlighted snippet.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadedDocumentSearchResult {
    pub(crate) id: String,
    pub(crate) document_id: String,
    pub(crate) url: String,
    pub(crate) title: String,
    pub(crate) excerpt: String,
    pub(crate) section_title: Option<String>,
    pub(crate) section_index: usize,
    pub(crate) page_index: Option<usize>,
    pub(crate) match_scope: String,
}

/// Outcome of a delete, including bytes reclaimed from app data.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadedDocumentDeleteResult {
    pub(crate) id: String,
    pub(crate) url: String,
    pub(crate) bytes_freed: u64,
}

/// One document that could not be deleted while the rest of its batch continued.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadedDocumentDeleteBatchFailure {
    pub(crate) document_url: String,
    pub(crate) error: String,
}

/// Count-based progress emitted while a sequential delete batch runs.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadedDocumentDeleteBatchProgress {
    pub(crate) phase: String,
    pub(crate) processed: usize,
    pub(crate) total: usize,
    pub(crate) deleted: usize,
    pub(crate) failed: usize,
    pub(crate) document_url: Option<String>,
}

/// Final delete-batch outcome, retaining successes alongside per-item failures.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadedDocumentDeleteBatchResult {
    pub(crate) selected: usize,
    pub(crate) processed: usize,
    pub(crate) deleted: Vec<UploadedDocumentDeleteResult>,
    pub(crate) failures: Vec<UploadedDocumentDeleteBatchFailure>,
    pub(crate) bytes_freed: u64,
}

/// Request identifying one stored upload at a validated source boundary.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadedDocumentSourceRequest {
    pub(crate) document_url: String,
}

/// Sanitized reader HTML plus validated local raster paths for the WebView.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadedDocumentSource {
    pub(crate) html: String,
    pub(crate) asset_paths: std::collections::HashMap<String, String>,
}

/// Request to run an FTS search over uploaded documents.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadedDocumentSearchRequest {
    pub(crate) query: String,
    pub(crate) limit: Option<usize>,
    pub(crate) document_urls: Option<Vec<String>>,
    pub(crate) exact_phrases: Option<Vec<String>>,
}

/// Request to find literal text within one indexed uploaded PDF.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadedPdfFindRequest {
    pub(crate) document_url: String,
    pub(crate) query: String,
}

/// Match count for one indexed PDF page.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadedPdfFindPage {
    pub(crate) page_index: usize,
    pub(crate) match_count: usize,
}

/// Compact whole-document Find result; geometry stays in page sidecars.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadedPdfFindResult {
    pub(crate) match_count: usize,
    pub(crate) pages: Vec<UploadedPdfFindPage>,
}

/// Request to delete one uploaded document by its URL.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadedDocumentDeleteRequest {
    pub(crate) document_url: String,
}

/// Request to change only Papercut's display title for one uploaded document.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadedDocumentTitleUpdateRequest {
    pub(crate) document_url: String,
    pub(crate) title: String,
}

/// Request to delete a bounded set of uploaded documents by URL.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadedDocumentDeleteBatchRequest {
    pub(crate) document_urls: Vec<String>,
}

/// A user-created library folder for organizing uploaded documents.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadedLibraryFolder {
    pub(crate) id: String,
    pub(crate) parent_id: Option<String>,
    pub(crate) name: String,
    pub(crate) depth: usize,
    pub(crate) sort_order: i64,
    pub(crate) created_at_ms: u128,
    pub(crate) updated_at_ms: u128,
}

/// Folder placement and manual order for one uploaded document.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadedDocumentLocation {
    pub(crate) document_id: String,
    pub(crate) folder_id: Option<String>,
    pub(crate) sort_order: i64,
}

/// Complete uploaded-library organization metadata.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadedLibraryOrganization {
    pub(crate) folders: Vec<UploadedLibraryFolder>,
    pub(crate) document_locations: Vec<UploadedDocumentLocation>,
}

/// Request to create a folder under a parent, or at the root when absent.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadedLibraryCreateFolderRequest {
    pub(crate) parent_id: Option<String>,
    pub(crate) name: String,
}

/// Request to rename a user-created library folder.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadedLibraryRenameFolderRequest {
    pub(crate) folder_id: String,
    pub(crate) name: String,
}

/// Request to delete an empty user-created library folder.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadedLibraryDeleteFolderRequest {
    pub(crate) folder_id: String,
}

/// Request to move uploaded documents into a target folder, or root when absent.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadedLibraryMoveDocumentsRequest {
    pub(crate) document_ids: Vec<String>,
    pub(crate) folder_id: Option<String>,
}

/// Request to move one folder under another folder, or root when absent.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadedLibraryMoveFolderRequest {
    pub(crate) folder_id: String,
    pub(crate) parent_id: Option<String>,
}

/// One item in a manual library ordering request.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadedLibraryOrderItem {
    pub(crate) item_type: String,
    pub(crate) id: String,
}

/// Request to assign sibling order for folders/documents in one folder.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadedLibraryReorderRequest {
    pub(crate) parent_id: Option<String>,
    pub(crate) items: Vec<UploadedLibraryOrderItem>,
}
