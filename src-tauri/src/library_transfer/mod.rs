//! Portable, one-way Papercut library transfer.
//!
//! This module owns package export/import only. Same-network transport and
//! audiobook payloads are later layers over the same package contract.

mod package;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{Manager, Runtime};
use tauri_plugin_dialog::{DialogExt, FilePath};
use tauri_plugin_fs::{FsExt, OpenOptions};
use zip::ZipArchive;

use crate::document_uploads::{
    create_folder, list_organization, list_uploads, move_documents, now_ms,
    restore_transferred_document, upload_dir, UploadedDocument, UploadedLibraryCreateFolderRequest,
    UploadedLibraryFolder, UploadedLibraryMoveDocumentsRequest,
};
use package::{
    document_source_path, read_document_source, read_manifest, sha256_reader, write_package,
    TransferDocument, TransferDocumentLocation, TransferFolder, TransferManifest,
    TransferOrganization, MAX_PACKAGE_BYTES, PACKAGE_KIND, PACKAGE_VERSION,
};

const PACKAGE_EXTENSION: &str = "papercut-library";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryTransferExportResult {
    documents: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryTransferImportResult {
    selected: usize,
    imported: usize,
    skipped: usize,
    failed: usize,
    folders_created: usize,
    failures: Vec<LibraryTransferFailure>,
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
pub async fn library_transfer_export<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<LibraryTransferExportResult>, String> {
    tauri::async_runtime::spawn_blocking(move || export_library(app))
        .await
        .map_err(|err| format!("Library export task failed: {err}"))?
}

/// Pick a package, merge new documents, and rebuild their target-side FTS rows.
/// `None` is normal picker cancellation rather than an operation failure.
#[tauri::command]
pub async fn library_transfer_import<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<LibraryTransferImportResult>, String> {
    tauri::async_runtime::spawn_blocking(move || import_library(app))
        .await
        .map_err(|err| format!("Library import task failed: {err}"))?
}

fn export_library<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<LibraryTransferExportResult>, String> {
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
    let documents = list_uploads(&app)?;
    if documents.is_empty() {
        return Err("There are no uploaded documents to export".into());
    }
    let organization = list_organization(&app)?;
    let manifest = prepare_manifest(&app, &documents, organization)?;
    let temp_path = transfer_temp_path(&app, "export")?;
    let build_result: Result<(), String> = (|| {
        let writer = BufWriter::new(File::create(&temp_path).map_err(|err| {
            format!(
                "Failed to create temporary library package {}: {err}",
                temp_path.display()
            )
        })?);
        write_package(writer, &manifest, |document| {
            let path = upload_dir(&app, &document.id)?.join("source.html");
            let file = File::open(&path).map_err(|err| {
                format!(
                    "Failed to open transferred document {}: {err}",
                    path.display()
                )
            })?;
            Ok(Box::new(BufReader::new(file)))
        })?;
        copy_temp_to_destination(&app, &temp_path, destination)?;
        Ok(())
    })();
    let _ = fs::remove_file(&temp_path);
    build_result?;

    Ok(Some(LibraryTransferExportResult {
        documents: manifest.documents.len(),
    }))
}

/// Stage picker input into a seekable local file, validate the complete archive,
/// then merge each payload through the normal upload pipeline. The staging file
/// is removed whether validation, restoration, or organization fails.
fn import_library<R: Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Option<LibraryTransferImportResult>, String> {
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
        let file = File::open(&temp_path).map_err(|err| {
            format!(
                "Failed to open temporary library package {}: {err}",
                temp_path.display()
            )
        })?;
        let mut archive = ZipArchive::new(BufReader::new(file))
            .map_err(|err| format!("Selected file is not a valid Papercut library: {err}"))?;
        let manifest = read_manifest(&mut archive)?;
        restore_manifest(&app, &mut archive, manifest)
    })();
    let _ = fs::remove_file(&temp_path);
    import_result.map(Some)
}

/// Build the manifest without loading every source into memory. Sources are
/// opened again during ZIP writing and must retain the same size/checksum.
fn prepare_manifest<R: Runtime>(
    app: &tauri::AppHandle<R>,
    documents: &[UploadedDocument],
    organization: crate::document_uploads::UploadedLibraryOrganization,
) -> Result<TransferManifest, String> {
    let mut transfer_documents = Vec::with_capacity(documents.len());
    for document in documents {
        let path = upload_dir(app, &document.id)?.join("source.html");
        let mut reader = BufReader::new(File::open(&path).map_err(|err| {
            format!("Failed to open uploaded document {}: {err}", path.display())
        })?);
        let (source_sha256, source_bytes) = sha256_reader(&mut reader)?;
        transfer_documents.push(TransferDocument {
            id: document.id.clone(),
            title: document.title.clone(),
            format: document.format.clone(),
            imported_at_ms: u64::try_from(document.imported_at_ms)
                .map_err(|_| "Uploaded document timestamp is invalid".to_string())?,
            original_bytes: document.bytes,
            source_path: document_source_path(&document.id),
            source_bytes,
            source_sha256,
        });
    }

    Ok(TransferManifest {
        kind: PACKAGE_KIND.into(),
        schema_version: PACKAGE_VERSION,
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
    })
}

/// Import documents independently so one damaged payload does not discard valid
/// siblings. Organization is applied only to ids newly created by this import.
fn restore_manifest<R: Runtime, T: Read + std::io::Seek>(
    app: &tauri::AppHandle<R>,
    archive: &mut ZipArchive<T>,
    manifest: TransferManifest,
) -> Result<LibraryTransferImportResult, String> {
    let existing_ids: HashSet<String> = list_uploads(app)?
        .into_iter()
        .map(|document| document.id)
        .collect();
    let mut imported_ids = Vec::new();
    let mut skipped = 0;
    let mut failures = Vec::new();

    for document in &manifest.documents {
        if existing_ids.contains(&document.id) {
            skipped += 1;
            continue;
        }
        let result = read_document_source(archive, document).and_then(|source_html| {
            restore_transferred_document(
                app,
                document.id.clone(),
                source_html,
                document.format.clone(),
                document.imported_at_ms as u128,
                document.original_bytes,
            )
        });
        match result {
            Ok(_) => imported_ids.push(document.id.clone()),
            Err(error) => failures.push(LibraryTransferFailure {
                item: document.title.clone(),
                error,
            }),
        }
    }

    let (folders_created, mut organization_failures) =
        merge_organization(app, &manifest.organization, &imported_ids)?;
    failures.append(&mut organization_failures);
    Ok(LibraryTransferImportResult {
        selected: manifest.documents.len(),
        imported: imported_ids.len(),
        skipped,
        failed: failures.len(),
        folders_created,
        failures,
    })
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
    Ok(cache.join(format!(
        "library-transfer-{operation}-{}-{}.tmp",
        std::process::id(),
        now_ms()?
    )))
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
) -> Result<(), String> {
    let scoped_path = source.clone();
    let copy_result: Result<(), String> = (|| {
        let mut options = OpenOptions::new();
        options.read(true);
        let input = app
            .fs()
            .open(source, options)
            .map_err(|err| format!("Failed to open selected library package: {err}"))?;
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
    release_result
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
