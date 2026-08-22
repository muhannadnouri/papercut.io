//! Portable, one-way Papercut library transfer.
//!
//! Package export/import remains the storage boundary. The `network` module
//! transports that same document-and-optional-audiobook package without
//! introducing a second serialization or restore path.

pub(crate) mod network;
mod package;

pub use network::LibraryTransferState;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use percent_encoding::percent_decode_str;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager, Runtime};
use tauri_plugin_dialog::{DialogExt, FilePath};
use tauri_plugin_fs::{FsExt, OpenOptions};
use zip::ZipArchive;

use crate::document_uploads::{
    create_folder, list_organization, list_uploads, move_documents, now_ms,
    restore_transferred_document, restore_transferred_pdf, stored_reader_asset_paths, upload_dir,
    upload_id_from_url, upload_source_path, ParsedDocumentAsset, StoredSourceKind,
    UploadedDocument, UploadedLibraryCreateFolderRequest, UploadedLibraryFolder,
    UploadedLibraryMoveDocumentsRequest,
};
use package::{
    audiobook_file_path, copy_audiobook_file, document_asset_path, document_source_path,
    read_document_asset, read_document_source, read_manifest, sha256_reader, write_package,
    TransferAudiobook, TransferAudiobookFile, TransferDocument, TransferDocumentAsset,
    TransferDocumentLocation, TransferFolder, TransferManifest, TransferOrganization,
    MAX_AUDIOBOOKS, MAX_PACKAGE_BYTES, PACKAGE_KIND, PACKAGE_VERSION,
};

const PACKAGE_EXTENSION: &str = "papercut-library";
pub(crate) const LIBRARY_TRANSFER_PROGRESS_EVENT: &str = "library-transfer-progress";
const STORAGE_RESERVE_BYTES: u64 = 64 * 1024 * 1024;
const DOCUMENT_STORAGE_MULTIPLIER: u64 = 3;
const STALE_TRANSFER_FILE_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const TRANSFER_FAILED_CODE: &str = "library-transfer-failed";
const INSUFFICIENT_SPACE_CODE: &str = "insufficient-space";

/// Stable failure facts shared by Tauri command rejections and authenticated
/// receiver status frames; localized wording remains a frontend concern.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryTransferError {
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    required_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    available_bytes: Option<u64>,
}

impl LibraryTransferError {
    fn message(message: impl Into<String>) -> Self {
        Self {
            code: TRANSFER_FAILED_CODE.into(),
            message: message.into(),
            required_bytes: None,
            available_bytes: None,
        }
    }

    fn insufficient_space(required_bytes: u64, available_bytes: u64) -> Self {
        Self {
            code: INSUFFICIENT_SPACE_CODE.into(),
            message: format!(
                "Not enough storage for library transfer: {required_bytes} bytes required, {available_bytes} available"
            ),
            required_bytes: Some(required_bytes),
            available_bytes: Some(available_bytes),
        }
    }

    /// Only storage pressure can succeed by retrying the same verified package
    /// later; malformed or incompatible packages will fail identically.
    pub(crate) fn is_retryable(&self) -> bool {
        self.code == INSUFFICIENT_SPACE_CODE
    }
}

impl std::fmt::Display for LibraryTransferError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for LibraryTransferError {}

impl From<String> for LibraryTransferError {
    fn from(message: String) -> Self {
        Self::message(message)
    }
}

impl From<&str> for LibraryTransferError {
    fn from(message: &str) -> Self {
        Self::message(message)
    }
}

pub(crate) type LibraryTransferResult<T> = Result<T, LibraryTransferError>;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryTransferExportRequest {
    #[serde(default)]
    audiobook_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryTransferExportResult {
    documents: usize,
    audiobooks: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryTransferImportResult {
    selected: usize,
    imported: usize,
    skipped: usize,
    failed: usize,
    folders_created: usize,
    audiobooks_selected: usize,
    audiobooks_imported: usize,
    audiobooks_skipped: usize,
    audiobooks_failed: usize,
    imported_audiobooks: Vec<crate::native_tts::NativeSavedAudiobookRecord>,
    failures: Vec<LibraryTransferFailure>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LibraryTransferProgress {
    operation: String,
    phase: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    bytes_processed: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bytes_total: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    items_processed: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    items_total: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    item: Option<String>,
}

/// Emit locale-neutral transfer facts; the WebView owns wording and number
/// formatting while Rust remains the authority for bytes and restored items.
pub(crate) fn emit_transfer_progress(
    app: &tauri::AppHandle,
    operation: &str,
    phase: &str,
    bytes: Option<(u64, u64)>,
    items: Option<(usize, usize)>,
    item: Option<String>,
) -> LibraryTransferProgress {
    let progress = LibraryTransferProgress {
        operation: operation.into(),
        phase: phase.into(),
        bytes_processed: bytes.map(|value| value.0),
        bytes_total: bytes.map(|value| value.1),
        items_processed: items.map(|value| value.0),
        items_total: items.map(|value| value.1),
        item,
    };
    let _ = app.emit(LIBRARY_TRANSFER_PROGRESS_EVENT, progress.clone());
    progress
}

type TransferProgressForwarder<'a> = Option<&'a mut dyn FnMut(&LibraryTransferProgress)>;

/// Publish one canonical progress value locally and, for LAN receives, forward
/// that exact value to the authenticated sender over the existing connection.
fn report_transfer_progress(
    app: &tauri::AppHandle,
    operation: &str,
    phase: &str,
    bytes: Option<(u64, u64)>,
    items: Option<(usize, usize)>,
    item: Option<String>,
    forwarder: &mut TransferProgressForwarder<'_>,
) {
    let progress = emit_transfer_progress(app, operation, phase, bytes, items, item);
    if let Some(forward) = forwarder.as_deref_mut() {
        forward(&progress);
    }
}

struct PreparedPackage {
    manifest: TransferManifest,
    payloads: HashMap<String, PathBuf>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LibraryTransferFailure {
    item: String,
    error: String,
}

/// Pick an OS-owned destination and create a documents/folders package.
/// `None` is normal picker cancellation rather than an operation failure.
#[tauri::command]
pub async fn library_transfer_export(
    app: tauri::AppHandle,
    request: Option<LibraryTransferExportRequest>,
) -> LibraryTransferResult<Option<LibraryTransferExportResult>> {
    tauri::async_runtime::spawn_blocking(move || {
        export_library(app, request.unwrap_or_default().audiobook_ids)
    })
    .await
    .map_err(|err| format!("Library export task failed: {err}"))?
}

/// Pick a package, merge new documents, and rebuild their target-side FTS rows.
/// `None` is normal picker cancellation rather than an operation failure.
#[tauri::command]
pub async fn library_transfer_import(
    app: tauri::AppHandle,
) -> LibraryTransferResult<Option<LibraryTransferImportResult>> {
    tauri::async_runtime::spawn_blocking(move || import_library(app))
        .await
        .map_err(|err| format!("Library import task failed: {err}"))?
}

fn export_library(
    app: tauri::AppHandle,
    audiobook_ids: Vec<String>,
) -> LibraryTransferResult<Option<LibraryTransferExportResult>> {
    let destination = app
        .dialog()
        .file()
        .set_title("Export Papercut Library")
        .set_file_name(format!("papercut-library.{PACKAGE_EXTENSION}"))
        .add_filter("Papercut Library", &[PACKAGE_EXTENSION])
        .blocking_save_file();
    let Some(destination) = destination else {
        return Ok(None);
    };
    let temp_path = transfer_temp_path(&app, "export")?;
    let build_result: LibraryTransferResult<LibraryTransferExportResult> = (|| {
        let result = build_library_package(&app, &temp_path, &audiobook_ids)?;
        copy_temp_to_destination(&app, &temp_path, destination)?;
        Ok(result)
    })();
    let _ = fs::remove_file(&temp_path);
    build_result.map(Some)
}

/// Stage picker input into a seekable local file, validate the complete archive,
/// then merge each payload through the normal upload pipeline. The staging file
/// is removed whether validation, restoration, or organization fails.
fn import_library(
    app: tauri::AppHandle,
) -> LibraryTransferResult<Option<LibraryTransferImportResult>> {
    let source = app
        .dialog()
        .file()
        .set_title("Import Papercut Library")
        .add_filter("Papercut Library", &[PACKAGE_EXTENSION])
        .blocking_pick_file();
    let Some(source) = source else {
        return Ok(None);
    };

    let temp_path = transfer_temp_path(&app, "import")?;
    let import_result = (|| {
        copy_source_to_temp(&app, source, &temp_path)?;
        import_library_package(&app, &temp_path, "import", None)
    })();
    let _ = fs::remove_file(&temp_path);
    import_result.map(Some)
}

/// Build the canonical package at an app-owned path so file export and LAN
/// transfer cannot drift into separate serialization or validation paths.
fn build_library_package(
    app: &tauri::AppHandle,
    path: &Path,
    audiobook_ids: &[String],
) -> LibraryTransferResult<LibraryTransferExportResult> {
    let documents = list_uploads(app)?;
    let organization = list_organization(app)?;
    let prepared = prepare_manifest(app, &documents, organization, audiobook_ids)?;
    if prepared.manifest.documents.is_empty() && prepared.manifest.audiobooks.is_empty() {
        return Err("There are no uploaded documents or saved audiobooks to export".into());
    }
    let result = LibraryTransferExportResult {
        documents: prepared.manifest.documents.len(),
        audiobooks: prepared.manifest.audiobooks.len(),
    };
    ensure_available_space(
        path.parent()
            .ok_or_else(|| "Temporary library package path has no parent".to_string())?,
        manifest_payload_bytes(&prepared.manifest)?,
    )?;
    let writer = BufWriter::new(File::create(path).map_err(|err| {
        format!(
            "Failed to create temporary library package {}: {err}",
            path.display()
        )
    })?);
    write_package(writer, &prepared.manifest, |archive_path| {
        let payload_path = prepared
            .payloads
            .get(archive_path)
            .ok_or_else(|| format!("Missing prepared transfer payload: {archive_path}"))?;
        let file = File::open(payload_path).map_err(|err| {
            format!(
                "Failed to open transferred payload {}: {err}",
                payload_path.display()
            )
        })?;
        Ok(Box::new(BufReader::new(file)))
    })?;
    Ok(result)
}

/// Open and restore one already-staged package. Network receive deliberately
/// enters through the same archive checks and merge logic as picker import.
fn import_library_package(
    app: &tauri::AppHandle,
    path: &Path,
    operation: &str,
    mut forwarder: TransferProgressForwarder<'_>,
) -> LibraryTransferResult<LibraryTransferImportResult> {
    report_transfer_progress(
        app,
        operation,
        "verifying",
        None,
        None,
        None,
        &mut forwarder,
    );
    let file = File::open(path).map_err(|err| {
        format!(
            "Failed to open temporary library package {}: {err}",
            path.display()
        )
    })?;
    let mut archive = ZipArchive::new(BufReader::new(file))
        .map_err(|err| format!("Selected file is not a valid Papercut library: {err}"))?;
    let manifest = read_manifest(&mut archive)?;
    restore_manifest(app, &mut archive, manifest, operation, &mut forwarder)
}

/// Build the manifest without loading every source into memory. Sources are
/// opened again during ZIP writing and must retain the same size/checksum.
fn prepare_manifest(
    app: &tauri::AppHandle,
    documents: &[UploadedDocument],
    organization: crate::document_uploads::UploadedLibraryOrganization,
    audiobook_ids: &[String],
) -> Result<PreparedPackage, String> {
    let mut transfer_documents = Vec::with_capacity(documents.len());
    let mut payloads = HashMap::new();
    for document in documents {
        let source_kind = StoredSourceKind::from_str(&document.source_kind)?;
        let path = upload_source_path(app, &document.id, source_kind)?;
        let mut reader = BufReader::new(File::open(&path).map_err(|err| {
            format!("Failed to open uploaded document {}: {err}", path.display())
        })?);
        let (source_sha256, source_bytes) = sha256_reader(&mut reader)?;
        let source_path = document_source_path(&document.id, source_kind.as_str());
        let mut assets = Vec::new();
        if document.format == "epub" {
            let mut stored_assets: Vec<_> =
                stored_reader_asset_paths(&upload_dir(app, &document.id)?)?
                    .into_iter()
                    .collect();
            stored_assets.sort_unstable_by(|left, right| left.0.cmp(&right.0));
            for (file_name, stored_path) in stored_assets {
                let stored_path = PathBuf::from(stored_path);
                let mut reader = BufReader::new(File::open(&stored_path).map_err(|err| {
                    format!(
                        "Failed to open uploaded document reader image {}: {err}",
                        stored_path.display()
                    )
                })?);
                let (sha256, bytes) = sha256_reader(&mut reader)?;
                let path = document_asset_path(&document.id, &file_name);
                payloads.insert(path.clone(), stored_path);
                assets.push(TransferDocumentAsset {
                    file_name,
                    path,
                    bytes,
                    sha256,
                });
            }
        }
        transfer_documents.push(TransferDocument {
            id: document.id.clone(),
            title: document.title.clone(),
            original_file_name: document.original_file_name.clone(),
            format: document.format.clone(),
            source_kind: source_kind.as_str().into(),
            imported_at_ms: u64::try_from(document.imported_at_ms)
                .map_err(|_| "Uploaded document timestamp is invalid".to_string())?,
            original_bytes: document.bytes,
            source_path: source_path.clone(),
            source_bytes,
            source_sha256,
            assets,
        });
        payloads.insert(source_path, path);
    }

    let selected_audiobook_ids = validate_audiobook_selection(audiobook_ids)?;
    let mut transfer_audiobooks = Vec::with_capacity(selected_audiobook_ids.len());
    if !selected_audiobook_ids.is_empty() {
        for audiobook in crate::native_tts::list_audiobook_transfer_payloads(app)? {
            if !selected_audiobook_ids.contains(audiobook.record.id.as_str()) {
                continue;
            }
            let mut files = Vec::with_capacity(audiobook.files.len());
            for file in audiobook.files {
                let mut reader = BufReader::new(
                    File::open(&file.source_path)
                        .map_err(|err| format!("Failed to open saved audiobook payload: {err}"))?,
                );
                let (sha256, bytes) = sha256_reader(&mut reader)?;
                let path = audiobook_file_path(&audiobook.storage_key, &file.relative_path);
                payloads.insert(path.clone(), file.source_path);
                files.push(TransferAudiobookFile {
                    relative_path: file.relative_path,
                    path,
                    bytes,
                    sha256,
                });
            }
            transfer_audiobooks.push(TransferAudiobook {
                id: audiobook.record.id,
                title: audiobook.record.title,
                storage_key: audiobook.storage_key,
                files,
            });
        }
    }

    let manifest = TransferManifest {
        kind: PACKAGE_KIND.into(),
        schema_version: if transfer_audiobooks.is_empty()
            && transfer_documents
                .iter()
                .all(|document| document.source_kind == "html" && document.assets.is_empty())
        {
            1
        } else {
            PACKAGE_VERSION
        },
        created_at_ms: u64::try_from(now_ms()?)
            .map_err(|_| "System timestamp is invalid".to_string())?,
        documents: transfer_documents,
        organization: TransferOrganization {
            folders: organization
                .folders
                .into_iter()
                .map(|folder| TransferFolder {
                    id: folder.id,
                    parent_id: folder.parent_id,
                    name: folder.name,
                    depth: folder.depth,
                    sort_order: folder.sort_order,
                })
                .collect(),
            document_locations: organization
                .document_locations
                .into_iter()
                .map(|location| TransferDocumentLocation {
                    document_id: location.document_id,
                    folder_id: location.folder_id,
                    sort_order: location.sort_order,
                })
                .collect(),
        },
        audiobooks: transfer_audiobooks,
    };
    Ok(PreparedPackage { manifest, payloads })
}

/// Deduplicate UI selections and apply the package limit before touching large
/// audiobook payloads; IDs that disappeared since the dialog opened are simply
/// absent from the authoritative native registry.
fn validate_audiobook_selection(audiobook_ids: &[String]) -> Result<HashSet<&str>, String> {
    let selected: HashSet<_> = audiobook_ids.iter().map(String::as_str).collect();
    if selected.len() > MAX_AUDIOBOOKS {
        return Err(format!(
            "No more than {MAX_AUDIOBOOKS} saved audiobooks can be transferred"
        ));
    }
    Ok(selected)
}

/// Import documents independently so one damaged payload does not discard valid
/// siblings. Organization is applied only to ids newly created by this import.
fn restore_manifest<T: Read + std::io::Seek>(
    app: &tauri::AppHandle,
    archive: &mut ZipArchive<T>,
    manifest: TransferManifest,
    operation: &str,
    forwarder: &mut TransferProgressForwarder<'_>,
) -> LibraryTransferResult<LibraryTransferImportResult> {
    let existing_ids: HashSet<String> = list_uploads(app)?
        .into_iter()
        .map(|document| document.id)
        .collect();
    ensure_restore_space(app, &manifest, &existing_ids)?;
    let mut imported_ids = Vec::new();
    let mut skipped = 0;
    let mut failures = Vec::new();

    let document_total = manifest.documents.len();
    if document_total > 0 {
        report_transfer_progress(
            app,
            operation,
            "importingDocuments",
            None,
            Some((0, document_total)),
            None,
            forwarder,
        );
    }
    for (index, document) in manifest.documents.iter().enumerate() {
        report_transfer_progress(
            app,
            operation,
            "importingDocuments",
            None,
            Some((index, document_total)),
            Some(document.title.clone()),
            forwarder,
        );
        if existing_ids.contains(&document.id) {
            skipped += 1;
            report_transfer_progress(
                app,
                operation,
                "importingDocuments",
                None,
                Some((index + 1, document_total)),
                None,
                forwarder,
            );
            continue;
        }
        let result = (|| {
            let source = read_document_source(archive, document)?;
            let assets = document
                .assets
                .iter()
                .map(|asset| {
                    Ok(ParsedDocumentAsset {
                        file_name: asset.file_name.clone(),
                        bytes: read_document_asset(archive, asset)?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            match StoredSourceKind::from_str(&document.source_kind)? {
                StoredSourceKind::Html => {
                    let source_html = String::from_utf8(source).map_err(|_| {
                        format!(
                            "Library-transfer document is not UTF-8: {}",
                            document.source_path
                        )
                    })?;
                    restore_transferred_document(
                        app,
                        document.id.clone(),
                        source_html,
                        document.title.clone(),
                        document.original_file_name.clone(),
                        document.format.clone(),
                        document.imported_at_ms as u128,
                        document.original_bytes,
                        assets,
                    )
                }
                StoredSourceKind::Pdf => restore_transferred_pdf(
                    app,
                    document.id.clone(),
                    document.title.clone(),
                    document.original_file_name.clone(),
                    source,
                    document.imported_at_ms as u128,
                    document.original_bytes,
                ),
            }
        })();
        match result {
            Ok(_) => imported_ids.push(document.id.clone()),
            Err(error) => failures.push(LibraryTransferFailure {
                item: document.title.clone(),
                error,
            }),
        }
        report_transfer_progress(
            app,
            operation,
            "importingDocuments",
            None,
            Some((index + 1, document_total)),
            None,
            forwarder,
        );
    }

    let (folders_created, mut organization_failures) =
        merge_organization(app, &manifest.organization, &imported_ids)?;
    failures.append(&mut organization_failures);
    let document_failures = failures.len();
    let mut audiobooks_imported = Vec::new();
    let mut audiobooks_skipped = 0;
    let mut audiobooks_failed = 0;
    let audiobook_total = manifest.audiobooks.len();
    if audiobook_total > 0 {
        report_transfer_progress(
            app,
            operation,
            "restoringAudiobooks",
            None,
            Some((0, audiobook_total)),
            None,
            forwarder,
        );
    }
    for (index, audiobook) in manifest.audiobooks.iter().enumerate() {
        report_transfer_progress(
            app,
            operation,
            "restoringAudiobooks",
            None,
            Some((index, audiobook_total)),
            Some(audiobook.title.clone()),
            forwarder,
        );
        match restore_audiobook(app, archive, audiobook) {
            Ok(Some(record)) => audiobooks_imported.push(record),
            Ok(None) => audiobooks_skipped += 1,
            Err(error) => {
                audiobooks_failed += 1;
                failures.push(LibraryTransferFailure {
                    item: audiobook.title.clone(),
                    error,
                });
            }
        }
        report_transfer_progress(
            app,
            operation,
            "restoringAudiobooks",
            None,
            Some((index + 1, audiobook_total)),
            None,
            forwarder,
        );
    }
    Ok(LibraryTransferImportResult {
        selected: manifest.documents.len(),
        imported: imported_ids.len(),
        skipped,
        failed: document_failures,
        folders_created,
        audiobooks_selected: manifest.audiobooks.len(),
        audiobooks_imported: audiobooks_imported.len(),
        audiobooks_skipped,
        audiobooks_failed,
        imported_audiobooks: audiobooks_imported,
        failures,
    })
}

/// Restore one audiobook through a same-filesystem staging directory, then ask
/// the native registry to validate identity, manifest totals, and every chunk
/// before atomically promoting it into the live audiobook directory.
fn restore_audiobook<T: Read + std::io::Seek>(
    app: &tauri::AppHandle,
    archive: &mut ZipArchive<T>,
    audiobook: &TransferAudiobook,
) -> Result<Option<crate::native_tts::NativeSavedAudiobookRecord>, String> {
    let root = crate::native_tts::audiobooks_dir(app)?;
    fs::create_dir_all(&root)
        .map_err(|err| format!("Failed to create native audiobook directory: {err}"))?;
    let destination = root.join(&audiobook.storage_key);
    if destination.is_dir() {
        crate::native_tts::validate_transferred_audiobook(&destination, &audiobook.id)?;
        return Ok(None);
    }

    let staging_root = root.join(format!(".transfer-{}-{}", audiobook.storage_key, now_ms()?));
    let staging_audio = staging_root.join(&audiobook.storage_key);
    let staging_source = staging_root.join("source");
    let result = (|| {
        fs::create_dir_all(staging_audio.join("chunks"))
            .map_err(|err| format!("Failed to stage transferred audiobook: {err}"))?;
        for file in &audiobook.files {
            let target = if let Some(name) = file.relative_path.strip_prefix("source/") {
                fs::create_dir_all(&staging_source)
                    .map_err(|err| format!("Failed to stage imported source: {err}"))?;
                staging_source.join(name)
            } else {
                staging_audio.join(&file.relative_path)
            };
            let mut writer = BufWriter::new(
                File::create(&target)
                    .map_err(|err| format!("Failed to create staged audiobook payload: {err}"))?,
            );
            copy_audiobook_file(archive, file, &mut writer)?;
            writer
                .flush()
                .map_err(|err| format!("Failed to finish staged audiobook payload: {err}"))?;
        }

        let record =
            crate::native_tts::validate_transferred_audiobook(&staging_audio, &audiobook.id)?;
        validate_transferred_audiobook_document(app, &record, &staging_source)?;
        let installed_source = restore_imported_audiobook_source(app, &record, &staging_source)?;
        if let Err(err) = fs::rename(&staging_audio, &destination) {
            if let Some(path) = installed_source {
                let _ = fs::remove_dir_all(path);
            }
            return Err(format!("Failed to install transferred audiobook: {err}"));
        }
        Ok(record)
    })();
    let _ = fs::remove_dir_all(&staging_root);
    result.map(Some)
}

/// Ensure a transferred audiobook can only reopen an app-owned document.
/// Imported bundle sources must travel beside the audio, generic uploads must
/// already have been restored, and bundled documents stay below `/documents/`.
fn validate_transferred_audiobook_document(
    app: &tauri::AppHandle,
    record: &crate::native_tts::NativeSavedAudiobookRecord,
    staging_source: &Path,
) -> Result<(), String> {
    if crate::native_tts::imported_upload_id_from_document_url(&record.document_url).is_ok() {
        return staging_source
            .join("source.html")
            .is_file()
            .then_some(())
            .ok_or_else(|| "Transferred imported audiobook is missing its source document".into());
    }
    if let Ok(upload_id) = upload_id_from_url(&record.document_url) {
        let canonical_url = format!("/uploads/{upload_id}.html");
        if record.document_url == canonical_url
            && upload_dir(app, &upload_id)?.join("source.html").is_file()
        {
            return Ok(());
        }
        return Err("Transferred audiobook references a missing uploaded document".into());
    }
    if is_bundled_document_url(&record.document_url) {
        return Ok(());
    }
    Err("Transferred audiobook document URL is not app-owned".into())
}

/// Decode the relative bundled path before checking components so encoded dot
/// segments cannot escape `/documents/` when the WebView resolves the URL.
fn is_bundled_document_url(document_url: &str) -> bool {
    let Some(encoded_path) = document_url.strip_prefix("/documents/") else {
        return false;
    };
    if encoded_path.is_empty()
        || document_url.contains(['?', '#', '\\'])
        || document_url.chars().any(char::is_control)
    {
        return false;
    }
    let Ok(decoded_path) = percent_decode_str(encoded_path).decode_utf8() else {
        return false;
    };
    decoded_path.ends_with(".html")
        && !decoded_path.contains('\\')
        && !decoded_path.chars().any(char::is_control)
        && Path::new(decoded_path.as_ref())
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
}

/// Imported audiobook bundles own their reading HTML outside the audio cache.
/// Re-sanitize it at this trust boundary and never overwrite an existing upload.
fn restore_imported_audiobook_source(
    app: &tauri::AppHandle,
    record: &crate::native_tts::NativeSavedAudiobookRecord,
    staging_source: &Path,
) -> Result<Option<PathBuf>, String> {
    let Ok(upload_id) =
        crate::native_tts::imported_upload_id_from_document_url(&record.document_url)
    else {
        return Ok(None);
    };
    let source_path = staging_source.join("source.html");
    if !source_path.is_file() {
        return Err("Transferred imported audiobook is missing its source document".into());
    }
    let source_html = fs::read_to_string(&source_path)
        .map_err(|err| format!("Failed to read transferred audiobook source: {err}"))?;
    fs::write(
        &source_path,
        crate::document_uploads::sanitize_html(&source_html),
    )
    .map_err(|err| format!("Failed to sanitize transferred audiobook source: {err}"))?;

    let destination = crate::native_tts::imported_upload_dir(app, &upload_id)?;
    if destination.exists() {
        return Ok(None);
    }
    let parent = destination
        .parent()
        .ok_or_else(|| "Imported audiobook destination is invalid".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|err| format!("Failed to create imported audiobook directory: {err}"))?;
    fs::rename(staging_source, &destination)
        .map_err(|err| format!("Failed to install imported audiobook source: {err}"))?;
    Ok(Some(destination))
}

/// Map source folders onto target siblings by parent/name, creating only missing
/// folders. This preserves the target library and avoids duplicate folder trees.
fn merge_organization<R: Runtime>(
    app: &tauri::AppHandle<R>,
    source: &TransferOrganization,
    imported_ids: &[String],
) -> Result<(usize, Vec<LibraryTransferFailure>), String> {
    let imported: HashSet<&str> = imported_ids.iter().map(String::as_str).collect();
    let mut target_folders = list_organization(app)?.folders;
    let mut source_to_target: HashMap<String, String> = HashMap::new();
    let mut folders: Vec<_> = source.folders.iter().collect();
    folders.sort_by_key(|folder| (folder.depth, folder.sort_order));
    let mut created = 0;
    let mut failures = Vec::new();

    for folder in folders {
        let target_parent = match folder.parent_id.as_deref() {
            Some(parent_id) => match source_to_target.get(parent_id) {
                Some(mapped) => Some(mapped.clone()),
                None => {
                    failures.push(LibraryTransferFailure {
                        item: folder.name.clone(),
                        error: "Parent folder could not be restored".into(),
                    });
                    continue;
                }
            },
            None => None,
        };
        let matching =
            matching_target_folder(&target_folders, target_parent.as_deref(), &folder.name);
        let target_id = if let Some(existing) = matching {
            existing.id.clone()
        } else {
            match create_folder(
                app,
                UploadedLibraryCreateFolderRequest {
                    parent_id: target_parent.clone(),
                    name: folder.name.clone(),
                },
            ) {
                Ok(new_folder) => {
                    let id = new_folder.id.clone();
                    target_folders.push(new_folder);
                    created += 1;
                    id
                }
                Err(error) => {
                    failures.push(LibraryTransferFailure {
                        item: folder.name.clone(),
                        error,
                    });
                    continue;
                }
            }
        };
        source_to_target.insert(folder.id.clone(), target_id);
    }

    let mut locations: Vec<_> = source
        .document_locations
        .iter()
        .filter(|location| imported.contains(location.document_id.as_str()))
        .collect();
    locations.sort_by_key(|location| location.sort_order);
    let mut groups: BTreeMap<Option<String>, Vec<String>> = BTreeMap::new();
    let mut placed = HashSet::new();
    for location in locations {
        let folder_id = location
            .folder_id
            .as_deref()
            .and_then(|id| source_to_target.get(id))
            .cloned();
        groups
            .entry(folder_id)
            .or_default()
            .push(location.document_id.clone());
        placed.insert(location.document_id.as_str());
    }
    for document_id in imported_ids {
        if !placed.contains(document_id.as_str()) {
            groups.entry(None).or_default().push(document_id.clone());
        }
    }

    for (folder_id, document_ids) in groups {
        if let Err(error) = move_documents(
            app,
            UploadedLibraryMoveDocumentsRequest {
                document_ids,
                folder_id,
            },
        ) {
            failures.push(LibraryTransferFailure {
                item: "Library organization".into(),
                error,
            });
        }
    }
    Ok((created, failures))
}

/// Match only siblings under the mapped parent; identical names in separate
/// branches are intentionally distinct folders.
fn matching_target_folder<'a>(
    folders: &'a [UploadedLibraryFolder],
    parent_id: Option<&str>,
    name: &str,
) -> Option<&'a UploadedLibraryFolder> {
    let normalized_name = name.to_lowercase();
    folders.iter().find(|folder| {
        folder.parent_id.as_deref() == parent_id && folder.name.to_lowercase() == normalized_name
    })
}

fn transfer_temp_path<R: Runtime>(
    app: &tauri::AppHandle<R>,
    operation: &str,
) -> Result<PathBuf, String> {
    let cache = transfer_cache_dir(app)?;
    Ok(cache.join(format!(
        "library-transfer-{operation}-{}-{}.tmp",
        std::process::id(),
        now_ms()?
    )))
}

pub(crate) fn transfer_cache_dir<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf, String> {
    let cache = app
        .path()
        .app_cache_dir()
        .map_err(|err| format!("Failed to resolve app cache directory: {err}"))?;
    fs::create_dir_all(&cache).map_err(|err| {
        format!(
            "Failed to create app cache directory {}: {err}",
            cache.display()
        )
    })?;
    cleanup_stale_transfer_files(&cache);
    Ok(cache)
}

/// Best-effort cleanup bounds disk use after crashes without delaying or
/// failing a new transfer when cache metadata cannot be read or removed.
fn cleanup_stale_transfer_files(cache: &Path) {
    let Ok(entries) = fs::read_dir(cache) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(age) = entry
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| modified.elapsed().ok())
        else {
            continue;
        };
        if is_stale_transfer_file(name, age) {
            let _ = fs::remove_file(entry.path());
        }
    }
}

fn is_stale_transfer_file(name: &str, age: Duration) -> bool {
    name.starts_with("library-transfer-")
        && (name.ends_with(".tmp") || name.ends_with(".part"))
        && age >= STALE_TRANSFER_FILE_AGE
}

/// Reject a transfer before it starts writing when the target filesystem cannot
/// hold the payload plus a small reserve for metadata and concurrent app writes.
pub(crate) fn ensure_available_space(path: &Path, payload_bytes: u64) -> LibraryTransferResult<()> {
    if payload_bytes == 0 {
        return Ok(());
    }
    let required = required_space_bytes(payload_bytes)?;
    let available = fs4::available_space(path).map_err(|err| {
        format!(
            "Failed to inspect available storage at {}: {err}",
            path.display()
        )
    })?;
    if available < required {
        return Err(LibraryTransferError::insufficient_space(
            required, available,
        ));
    }
    Ok(())
}

fn required_space_bytes(payload_bytes: u64) -> Result<u64, String> {
    payload_bytes
        .checked_add(STORAGE_RESERVE_BYTES)
        .ok_or_else(|| "Library transfer storage estimate is too large".to_string())
}

fn manifest_payload_bytes(manifest: &TransferManifest) -> Result<u64, String> {
    let mut bytes = 0u64;
    for document in &manifest.documents {
        bytes = bytes
            .checked_add(document.source_bytes)
            .ok_or_else(|| "Library transfer storage estimate is too large".to_string())?;
    }
    for audiobook in &manifest.audiobooks {
        for file in &audiobook.files {
            bytes = bytes
                .checked_add(file.bytes)
                .ok_or_else(|| "Library transfer storage estimate is too large".to_string())?;
        }
    }
    Ok(bytes)
}

/// Estimate only data absent from the target. Document source bytes are tripled
/// to cover source HTML, SQLite section text, and its FTS index; audiobook files
/// already carry their final uncompressed sizes in the manifest.
fn ensure_restore_space(
    app: &tauri::AppHandle,
    manifest: &TransferManifest,
    existing_document_ids: &HashSet<String>,
) -> LibraryTransferResult<()> {
    let mut required = 0u64;
    for document in &manifest.documents {
        if existing_document_ids.contains(&document.id) {
            continue;
        }
        let document_bytes = document
            .source_bytes
            .checked_mul(DOCUMENT_STORAGE_MULTIPLIER)
            .ok_or_else(|| "Library transfer storage estimate is too large".to_string())?;
        required = required
            .checked_add(document_bytes)
            .ok_or_else(|| "Library transfer storage estimate is too large".to_string())?;
    }

    if !manifest.audiobooks.is_empty() {
        let audiobook_root = crate::native_tts::audiobooks_dir(app)?;
        for audiobook in &manifest.audiobooks {
            if audiobook_root.join(&audiobook.storage_key).is_dir() {
                continue;
            }
            for file in &audiobook.files {
                required = required
                    .checked_add(file.bytes)
                    .ok_or_else(|| "Library transfer storage estimate is too large".to_string())?;
            }
        }
    }

    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|err| format!("Failed to resolve app data directory: {err}"))?;
    fs::create_dir_all(&app_data).map_err(|err| {
        format!(
            "Failed to create app data directory {}: {err}",
            app_data.display()
        )
    })?;
    ensure_available_space(&app_data, required)
}

/// Copy a completed local package through Tauri's filesystem handle so desktop
/// paths and mobile document-provider URIs share one destination path.
fn copy_temp_to_destination<R: Runtime>(
    app: &tauri::AppHandle<R>,
    temp_path: &Path,
    destination: FilePath,
) -> Result<(), String> {
    let scoped_path = destination.clone();
    let copy_result: Result<(), String> = (|| {
        let mut input = BufReader::new(
            File::open(temp_path)
                .map_err(|err| format!("Failed to open temporary package: {err}"))?,
        );
        let mut options = OpenOptions::new();
        options.write(true).create(true).truncate(true);
        let output = app
            .fs()
            .open(destination, options)
            .map_err(|err| format!("Failed to open selected library export destination: {err}"))?;
        let mut output = BufWriter::new(output);
        std::io::copy(&mut input, &mut output)
            .map_err(|err| format!("Failed to write selected library export: {err}"))?;
        output
            .flush()
            .map_err(|err| format!("Failed to finish selected library export: {err}"))
    })();
    let release_result = release_picker_access(app, scoped_path);
    copy_result?;
    release_result
}

/// Stage a picker-owned source into app cache because ZIP needs seekable input
/// while Android and iOS pickers may return provider-backed handles.
fn copy_source_to_temp<R: Runtime>(
    app: &tauri::AppHandle<R>,
    source: FilePath,
    temp_path: &Path,
) -> LibraryTransferResult<()> {
    let scoped_path = source.clone();
    let copy_result: LibraryTransferResult<()> = (|| {
        let mut options = OpenOptions::new();
        options.read(true);
        let input = app
            .fs()
            .open(source, options)
            .map_err(|err| format!("Failed to open selected library package: {err}"))?;
        if let Ok(source_bytes) = input.metadata().map(|metadata| metadata.len()) {
            if source_bytes > MAX_PACKAGE_BYTES {
                return Err("Selected library package is larger than the supported size".into());
            }
            if source_bytes > 0 {
                ensure_available_space(
                    temp_path.parent().ok_or_else(|| {
                        "Temporary library package path has no parent".to_string()
                    })?,
                    source_bytes,
                )?;
            }
        }
        let mut input = BufReader::new(input).take(MAX_PACKAGE_BYTES + 1);
        let mut output = BufWriter::new(File::create(temp_path).map_err(|err| {
            format!(
                "Failed to create temporary library package {}: {err}",
                temp_path.display()
            )
        })?);
        let copied = std::io::copy(&mut input, &mut output)
            .map_err(|err| format!("Failed to read selected library package: {err}"))?;
        output
            .flush()
            .map_err(|err| format!("Failed to stage selected library package: {err}"))?;
        if copied > MAX_PACKAGE_BYTES {
            return Err("Selected library package is larger than the supported size".into());
        }
        Ok(())
    })();
    let release_result = release_picker_access(app, scoped_path);
    copy_result?;
    Ok(release_result?)
}

/// Tauri starts iOS security-scoped access when a picker URL is opened; release
/// it after staging so repeated transfers do not exhaust the process allowance.
#[cfg(target_os = "ios")]
fn release_picker_access<R: Runtime>(
    app: &tauri::AppHandle<R>,
    path: FilePath,
) -> Result<(), String> {
    app.fs()
        .stop_accessing_security_scoped_resource(path)
        .map_err(|err| format!("Failed to release selected library file: {err}"))
}

#[cfg(not(target_os = "ios"))]
fn release_picker_access<R: Runtime>(
    _app: &tauri::AppHandle<R>,
    _path: FilePath,
) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_matching_is_case_insensitive_and_scoped_to_the_parent() {
        let folders = vec![
            test_folder("root", None, "Politics"),
            test_folder("nested", Some("root"), "History"),
            test_folder("other", None, "History"),
        ];

        assert_eq!(
            matching_target_folder(&folders, Some("root"), "HISTORY")
                .map(|folder| folder.id.as_str()),
            Some("nested")
        );
        assert_eq!(
            matching_target_folder(&folders, None, "history").map(|folder| folder.id.as_str()),
            Some("other")
        );
    }

    #[test]
    fn storage_estimate_keeps_headroom_and_rejects_overflow() {
        assert_eq!(
            required_space_bytes(1024).unwrap(),
            STORAGE_RESERVE_BYTES + 1024
        );
        assert!(required_space_bytes(u64::MAX).is_err());
    }

    #[test]
    fn insufficient_space_error_keeps_retry_and_ipc_details_structured() {
        let error = LibraryTransferError::insufficient_space(100, 50);
        let payload = serde_json::to_value(&error).unwrap();

        assert!(error.is_retryable());
        assert_eq!(payload["code"], INSUFFICIENT_SPACE_CODE);
        assert_eq!(payload["requiredBytes"], 100);
        assert_eq!(payload["availableBytes"], 50);
        assert!(!LibraryTransferError::message("invalid package").is_retryable());
    }

    #[test]
    fn bundled_document_urls_stay_below_the_documents_root() {
        assert!(is_bundled_document_url(
            "/documents/Linea%20Proletaria/Article.html"
        ));
        assert!(!is_bundled_document_url("data:text/html,<script></script>"));
        assert!(!is_bundled_document_url("/documents/%2e%2e/index.html"));
        assert!(!is_bundled_document_url("/documents/..\\index.html"));
    }

    #[test]
    fn stale_cleanup_only_targets_transfer_temporaries() {
        let stale = STALE_TRANSFER_FILE_AGE;
        assert!(is_stale_transfer_file(
            "library-transfer-export-1-2.tmp",
            stale
        ));
        assert!(is_stale_transfer_file(
            "library-transfer-lan-receive-session.part",
            stale
        ));
        assert!(!is_stale_transfer_file("other-cache.tmp", stale));
        assert!(!is_stale_transfer_file(
            "library-transfer-active.tmp",
            stale - Duration::from_secs(1)
        ));
    }

    #[test]
    fn audiobook_selection_deduplicates_ids_and_enforces_the_package_limit() {
        let ids = ["one".into(), "one".into(), "two".into()];
        let selected = validate_audiobook_selection(&ids).expect("valid selection");
        assert_eq!(selected.len(), 2);

        let too_many = (0..=MAX_AUDIOBOOKS)
            .map(|index| format!("audiobook-{index}"))
            .collect::<Vec<_>>();
        assert!(validate_audiobook_selection(&too_many).is_err());
    }

    fn test_folder(id: &str, parent_id: Option<&str>, name: &str) -> UploadedLibraryFolder {
        UploadedLibraryFolder {
            id: id.into(),
            parent_id: parent_id.map(str::to_string),
            name: name.into(),
            depth: usize::from(parent_id.is_some()),
            sort_order: 0,
            created_at_ms: 1,
            updated_at_ms: 1,
        }
    }
}
