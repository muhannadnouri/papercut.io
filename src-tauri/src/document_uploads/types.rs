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
    pub(crate) format: String,
    pub(crate) imported_at_ms: u128,
    pub(crate) bytes: u64,
    pub(crate) sections: usize,
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
    pub(crate) failed: usize,
    pub(crate) file_name: Option<String>,
}

/// Final batch outcome, including successes retained alongside per-file failures.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadedDocumentBatchResult {
    pub(crate) selected: usize,
    pub(crate) processed: usize,
    pub(crate) imported: Vec<UploadedDocument>,
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

/// Request to read the stored source HTML of an uploaded document.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadedDocumentSourceRequest {
    pub(crate) document_url: String,
}

/// Request to run an FTS search over uploaded documents.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadedDocumentSearchRequest {
    pub(crate) query: String,
    pub(crate) limit: Option<usize>,
    pub(crate) document_urls: Option<Vec<String>>,
}

/// Request to delete one uploaded document by its URL.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UploadedDocumentDeleteRequest {
    pub(crate) document_url: String,
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
