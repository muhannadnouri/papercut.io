//! Runtime user-document upload feature.
//!
//! Splits the upload pipeline into focused submodules so each concern can grow
//! independently. Dependencies only point downward:
//!
//! ```text
//! commands -> { batch, pipeline, organization, search, store } -> { cover, epub, html, pdf, parsed, storage, types }
//! ```
//!
//! - [`commands`]: the thin `#[tauri::command]` edge exposed to the frontend.
//! - [`batch`]: sequential import/delete batches, progress, and import cancellation.
//! - [`cover`]: bounded gallery-thumbnail decoding and persistence.
//! - [`pipeline`]: orchestrates import / get-source / delete.
//! - [`html`]: HTML-specific parsing + sanitization.
//! - [`epub`]: EPUB-specific parsing, sanitization, and generated reading HTML.
//! - [`pdf`]: canonical PDF and bounded per-page text-layer storage.
//! - [`organization`]: folder and manual ordering metadata for uploaded docs.
//! - [`parsed`]: format-neutral parsed document shape.
//! - [`store`]: SQLite schema, persistence, and listing.
//! - [`search`]: FTS5 query building and execution.
//! - [`storage`]: filesystem paths, upload ids, size accounting, clock.
//! - [`types`]: serde DTOs shared across the boundary.

// `commands` is `pub(crate)` so `generate_handler!` in `lib.rs` can reach both
// each command and the hidden `__cmd__*` helper the macro generates beside it.
mod batch;
pub(crate) mod commands;
mod cover;
mod epub;
mod html;
mod organization;
mod parsed;
mod pdf;
mod pipeline;
mod search;
mod state;
mod storage;
mod store;
mod types;

pub(crate) use state::DocumentUploadState;

// Library transfer consumes this narrow storage API so its removable package
// module never duplicates document parsing, sanitization, indexing, or folder rules.
pub(crate) use organization::{create_folder, list_organization, move_documents};
pub(crate) use pdf::{get_pdf_source_path, restore_audiobook_pdf, restore_transferred_pdf};
#[cfg(feature = "native-tts-core")]
pub(crate) use pipeline::ensure_uploaded_source_exists;
pub(crate) use pipeline::restore_transferred_document;
pub(crate) use storage::{
    now_ms, upload_dir, upload_id_from_url, upload_source_path, StoredSourceKind,
};
pub(crate) use store::list_uploads;
pub(crate) use types::{
    UploadedDocument, UploadedLibraryCreateFolderRequest, UploadedLibraryFolder,
    UploadedLibraryMoveDocumentsRequest, UploadedLibraryOrganization,
};

pub(crate) use html::sanitize_html;
