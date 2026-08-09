//! Sequential document import/delete batches with progress and partial results.

use std::collections::HashMap;
use std::collections::HashSet;
#[cfg(desktop)]
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use percent_encoding::percent_decode_str;
use tauri::{Emitter, Runtime};
use tauri_plugin_dialog::{DialogExt, FilePath};
use tauri_plugin_fs::FsExt;

#[cfg(desktop)]
use super::organization::{organize_folder_import, MAX_FOLDER_DEPTH};
use super::pdf::import_pdf_source;
use super::pipeline::{delete_upload, import_epub_source, import_html_source};
use super::state::DocumentBatchControl;
use super::storage::upload_id_from_url;
use super::store::list_uploads;
use super::types::{
    UploadedDocument, UploadedDocumentBatchFailure, UploadedDocumentBatchProgress,
    UploadedDocumentBatchResult, UploadedDocumentDeleteBatchFailure,
    UploadedDocumentDeleteBatchProgress, UploadedDocumentDeleteBatchRequest,
    UploadedDocumentDeleteBatchResult, UploadedDocumentDeleteRequest,
};

pub(crate) const DOCUMENT_IMPORT_PROGRESS_EVENT: &str = "document-uploads-import-progress";
pub(crate) const DOCUMENT_DELETE_PROGRESS_EVENT: &str = "document-uploads-delete-progress";
const MAX_BATCH_DOCUMENTS: usize = 500;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DocumentFormat {
    Html,
    Epub,
    Pdf,
}

struct BatchRun<T> {
    selected: usize,
    processed: usize,
    imported: Vec<T>,
    already_in_library: Vec<String>,
    failures: Vec<UploadedDocumentBatchFailure>,
    cancelled: bool,
}

struct DeleteBatchRun<T> {
    selected: usize,
    processed: usize,
    deleted: Vec<T>,
    failures: Vec<UploadedDocumentDeleteBatchFailure>,
}

#[cfg(desktop)]
struct FolderSource {
    source: FilePath,
    relative_folder: Vec<String>,
}

#[cfg(desktop)]
struct FolderSelection {
    root_name: String,
    sources: Vec<FolderSource>,
}

#[cfg_attr(not(desktop), allow(dead_code))]
struct FolderOrganizationPlan {
    root_name: String,
    folders_by_source: HashMap<PathBuf, Vec<String>>,
    placed_document_ids: HashSet<String>,
    placements: Vec<(String, Vec<String>)>,
}

/// Open one multi-file picker and process every selected document in sequence.
/// A bad file is reported in the result and does not discard successful imports.
pub(crate) fn import_batch<R: Runtime>(
    app: tauri::AppHandle<R>,
    control: DocumentBatchControl,
) -> Result<UploadedDocumentBatchResult, String> {
    let sources = app
        .dialog()
        .file()
        .set_title("Import Documents")
        .add_filter("Documents", &["html", "htm", "epub", "pdf"])
        .blocking_pick_files()
        .ok_or_else(|| "Document import cancelled".to_string())?;
    import_sources(app, control, sources, None, None)
}

/// Pick one desktop folder and preserve its supported files and subfolders.
///
/// The selected folder becomes a new top-level Library folder. Because that
/// consumes depth zero, filesystem descendants are read through depth four to
/// match the Library's existing five-visible-level limit. Symlinks are skipped.
#[cfg(desktop)]
pub(crate) fn import_folder<R: Runtime>(
    app: tauri::AppHandle<R>,
    control: DocumentBatchControl,
) -> Result<UploadedDocumentBatchResult, String> {
    let folder = app
        .dialog()
        .file()
        .set_title("Import Documents from Folder")
        .blocking_pick_folder()
        .ok_or_else(|| "Document import cancelled".to_string())?;
    let selection = folder_sources(folder)?;
    if selection.sources.is_empty() {
        return Err("The selected folder has no HTML, EPUB, or PDF files".into());
    }
    let folders_by_source = selection
        .sources
        .iter()
        .filter_map(|item| match &item.source {
            FilePath::Path(path) => Some((path.clone(), item.relative_folder.clone())),
            FilePath::Url(_) => None,
        })
        .collect();
    let sources = selection
        .sources
        .into_iter()
        .map(|item| item.source)
        .collect();
    let plan = FolderOrganizationPlan {
        root_name: selection.root_name,
        folders_by_source,
        placed_document_ids: HashSet::new(),
        placements: Vec::new(),
    };
    import_sources(app, control, sources, None, Some(plan))
}

/// Keep command registration uniform across targets without compiling the
/// desktop-only folder dialog API into mobile builds.
#[cfg(mobile)]
pub(crate) fn import_folder<R: Runtime>(
    _app: tauri::AppHandle<R>,
    _control: DocumentBatchControl,
) -> Result<UploadedDocumentBatchResult, String> {
    Err("Folder import is available on desktop only".into())
}

/// Run picker-produced sources through one progress/cancellation path so file
/// and folder selection cannot drift in import behavior.
fn import_sources<R: Runtime>(
    app: tauri::AppHandle<R>,
    control: DocumentBatchControl,
    sources: Vec<FilePath>,
    pdf_title: Option<&str>,
    #[cfg_attr(not(desktop), allow(unused_variables, unused_mut))] mut folder_plan: Option<
        FolderOrganizationPlan,
    >,
) -> Result<UploadedDocumentBatchResult, String> {
    if sources.len() > MAX_BATCH_DOCUMENTS {
        return Err(format!(
            "Select at most {MAX_BATCH_DOCUMENTS} documents in one import"
        ));
    }

    let mut known_document_ids: HashSet<_> = list_uploads(&app)?
        .into_iter()
        .map(|document| document.id)
        .collect();

    #[allow(unused_mut)]
    let mut run = process_sources(
        sources,
        || control.is_cancelled(),
        |source| {
            #[cfg(desktop)]
            let relative_folder = match (&folder_plan, &source) {
                (Some(plan), FilePath::Path(path)) => plan.folders_by_source.get(path).cloned(),
                _ => None,
            };
            let document = import_source(&app, source, pdf_title)?;
            let already_in_library = !known_document_ids.insert(document.id.clone());
            #[cfg(desktop)]
            if let (Some(plan), Some(relative_folder)) = (&mut folder_plan, relative_folder) {
                if !already_in_library && plan.placed_document_ids.insert(document.id.clone()) {
                    plan.placements.push((document.id.clone(), relative_folder));
                }
            }
            Ok((document, already_in_library))
        },
        |progress| {
            let _ = app.emit(DOCUMENT_IMPORT_PROGRESS_EVENT, progress);
        },
    )?;

    #[cfg(desktop)]
    if let Some(plan) = folder_plan {
        if !plan.placements.is_empty() {
            if let Err(error) = organize_folder_import(&app, &plan.root_name, &plan.placements) {
                run.failures.push(UploadedDocumentBatchFailure {
                    file_name: plan.root_name,
                    error: format!(
                        "Documents were imported, but their folder structure could not be saved: {error}"
                    ),
                });
            }
        }
    }
    let phase = if run.cancelled {
        "cancelled"
    } else {
        "completed"
    };
    let added = run.imported.len() - run.already_in_library.len();
    let _ = app.emit(
        DOCUMENT_IMPORT_PROGRESS_EVENT,
        UploadedDocumentBatchProgress {
            phase: phase.into(),
            processed: run.processed,
            total: run.selected,
            imported: added,
            already_in_library: run.already_in_library.len(),
            failed: run.failures.len(),
            file_name: None,
        },
    );

    Ok(UploadedDocumentBatchResult {
        selected: run.selected,
        processed: run.processed,
        imported: run.imported,
        already_in_library: run.already_in_library,
        failures: run.failures,
        cancelled: run.cancelled,
    })
}

/// Import one app-owned scanner PDF through the same bounded runner used by
/// picker imports, preserving validation, progress, and partial-result rules.
pub(crate) fn import_scanner_source<R: Runtime>(
    app: tauri::AppHandle<R>,
    control: DocumentBatchControl,
    source: PathBuf,
    title: &str,
) -> Result<UploadedDocumentBatchResult, String> {
    import_sources(
        app,
        control,
        vec![FilePath::Path(source)],
        Some(title),
        None,
    )
}

/// Delete a bounded, deduplicated URL list sequentially so one bad document
/// cannot discard successful siblings or produce hundreds of concurrent DB writes.
pub(crate) fn delete_batch<R: Runtime>(
    app: tauri::AppHandle<R>,
    request: UploadedDocumentDeleteBatchRequest,
) -> Result<UploadedDocumentDeleteBatchResult, String> {
    if request.document_urls.is_empty() {
        return Err("Select at least one document to delete".into());
    }
    if request.document_urls.len() > MAX_BATCH_DOCUMENTS {
        return Err(format!(
            "Select at most {MAX_BATCH_DOCUMENTS} documents to delete at once"
        ));
    }

    let mut seen = HashSet::new();
    let document_urls: Vec<_> = request
        .document_urls
        .into_iter()
        .filter(|url| seen.insert(url.clone()))
        .collect();
    for document_url in &document_urls {
        upload_id_from_url(document_url)?;
    }

    let run = process_deletions(
        document_urls,
        |document_url| delete_upload(&app, UploadedDocumentDeleteRequest { document_url }),
        |progress| {
            let _ = app.emit(DOCUMENT_DELETE_PROGRESS_EVENT, progress);
        },
    );
    let bytes_freed = run.deleted.iter().map(|result| result.bytes_freed).sum();
    let _ = app.emit(
        DOCUMENT_DELETE_PROGRESS_EVENT,
        UploadedDocumentDeleteBatchProgress {
            phase: "completed".into(),
            processed: run.processed,
            total: run.selected,
            deleted: run.deleted.len(),
            failed: run.failures.len(),
            document_url: None,
        },
    );

    Ok(UploadedDocumentDeleteBatchResult {
        selected: run.selected,
        processed: run.processed,
        deleted: run.deleted,
        failures: run.failures,
        bytes_freed,
    })
}

/// Keep partial-result accounting independent from Tauri so one focused unit
/// test covers continuation and progress behavior.
fn process_deletions<T, D, P>(
    document_urls: Vec<String>,
    mut delete: D,
    mut progress: P,
) -> DeleteBatchRun<T>
where
    D: FnMut(String) -> Result<T, String>,
    P: FnMut(UploadedDocumentDeleteBatchProgress),
{
    let selected = document_urls.len();
    let mut run = DeleteBatchRun {
        selected,
        processed: 0,
        deleted: Vec::new(),
        failures: Vec::new(),
    };

    for document_url in document_urls {
        progress(UploadedDocumentDeleteBatchProgress {
            phase: "deleting".into(),
            processed: run.processed,
            total: selected,
            deleted: run.deleted.len(),
            failed: run.failures.len(),
            document_url: Some(document_url.clone()),
        });
        match delete(document_url.clone()) {
            Ok(result) => run.deleted.push(result),
            Err(error) => run.failures.push(UploadedDocumentDeleteBatchFailure {
                document_url,
                error,
            }),
        }
        run.processed += 1;
        progress(UploadedDocumentDeleteBatchProgress {
            phase: "deleting".into(),
            processed: run.processed,
            total: selected,
            deleted: run.deleted.len(),
            failed: run.failures.len(),
            document_url: None,
        });
    }

    run
}

/// Convert a desktop folder into a stable recursive list of regular documents.
/// URL-backed folders are rejected because mobile providers do not expose a
/// directory that Rust can enumerate safely.
#[cfg(desktop)]
fn folder_sources(folder: FilePath) -> Result<FolderSelection, String> {
    let FilePath::Path(folder) = folder else {
        return Err("Folder import is available on desktop only".into());
    };
    let root_name = folder
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "Imported Folder".into());
    let mut sources = Vec::new();
    collect_folder_sources(&folder, &[], &mut sources)?;
    Ok(FolderSelection { root_name, sources })
}

/// Walk only as deep as the Library can represent and stop once the batch cap
/// is exceeded, avoiding an unbounded scan of a mistakenly selected directory.
#[cfg(desktop)]
fn collect_folder_sources(
    folder: &Path,
    relative_folder: &[String],
    sources: &mut Vec<FolderSource>,
) -> Result<(), String> {
    let mut entries = fs::read_dir(folder)
        .map_err(|err| format!("Failed to read selected folder: {err}"))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("Failed to read folder entry: {err}"))?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let file_type = entry
            .file_type()
            .map_err(|err| format!("Failed to inspect folder entry: {err}"))?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            if relative_folder.len() < MAX_FOLDER_DEPTH {
                let mut child_folder = relative_folder.to_vec();
                child_folder.push(entry.file_name().to_string_lossy().into_owned());
                collect_folder_sources(&entry.path(), &child_folder, sources)?;
            }
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let extension = entry
            .path()
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if document_format_from(&extension, &[]).is_some() {
            sources.push(FolderSource {
                source: FilePath::Path(entry.path()),
                relative_folder: relative_folder.to_vec(),
            });
            if sources.len() > MAX_BATCH_DOCUMENTS {
                return Err(format!(
                    "Select at most {MAX_BATCH_DOCUMENTS} documents in one import"
                ));
            }
        }
    }
    Ok(())
}

/// Keep cancellation at file boundaries so a parser or persistence transaction
/// is never interrupted halfway through one document.
fn process_sources<T, C, I, P>(
    sources: Vec<FilePath>,
    mut is_cancelled: C,
    mut import: I,
    mut progress: P,
) -> Result<BatchRun<T>, String>
where
    C: FnMut() -> Result<bool, String>,
    I: FnMut(FilePath) -> Result<(T, bool), String>,
    P: FnMut(UploadedDocumentBatchProgress),
{
    let selected = sources.len();
    let mut run = BatchRun {
        selected,
        processed: 0,
        imported: Vec::new(),
        already_in_library: Vec::new(),
        failures: Vec::new(),
        cancelled: false,
    };

    for source in sources {
        if is_cancelled()? {
            run.cancelled = true;
            break;
        }
        let file_name = source_file_name(&source);
        progress(UploadedDocumentBatchProgress {
            phase: "importing".into(),
            processed: run.processed,
            total: selected,
            imported: run.imported.len() - run.already_in_library.len(),
            already_in_library: run.already_in_library.len(),
            failed: run.failures.len(),
            file_name: Some(file_name.clone()),
        });

        match import(source) {
            Ok((document, already_in_library)) => {
                if already_in_library {
                    run.already_in_library.push(file_name.clone());
                }
                run.imported.push(document);
            }
            Err(error) => run
                .failures
                .push(UploadedDocumentBatchFailure { file_name, error }),
        }
        run.processed += 1;
        progress(UploadedDocumentBatchProgress {
            phase: "importing".into(),
            processed: run.processed,
            total: selected,
            imported: run.imported.len() - run.already_in_library.len(),
            already_in_library: run.already_in_library.len(),
            failed: run.failures.len(),
            file_name: None,
        });
    }

    Ok(run)
}

/// Route every selected source through the shared format-specific validation
/// and persistence pipeline.
fn import_source<R: Runtime>(
    app: &tauri::AppHandle<R>,
    source: FilePath,
    pdf_title: Option<&str>,
) -> Result<UploadedDocument, String> {
    let original_file_name = original_source_file_name(&source);
    match document_format(app, &source)? {
        DocumentFormat::Html => import_html_source(app, source, original_file_name),
        DocumentFormat::Epub => import_epub_source(app, source, original_file_name),
        DocumentFormat::Pdf => {
            let title = pdf_title
                .map(str::to_owned)
                .unwrap_or_else(|| source_title(&source));
            import_pdf_source(app, source, title, original_file_name)
        }
    }
}

/// Native dialogs return filesystem paths on desktop and may return content
/// URLs on mobile. Some Android providers expose an extensionless `document`
/// URL, so only that ambiguous case reads a small prefix and lets the normal
/// EPUB/HTML parser perform the full validation afterward.
fn document_format<R: Runtime>(
    app: &tauri::AppHandle<R>,
    source: &FilePath,
) -> Result<DocumentFormat, String> {
    let extension = match source {
        FilePath::Path(path) => path.extension().and_then(|value| value.to_str()),
        FilePath::Url(url) => Path::new(url.path())
            .extension()
            .and_then(|value| value.to_str()),
    }
    .unwrap_or_default()
    .to_ascii_lowercase();

    if let Some(format) = document_format_from(&extension, &[]) {
        return Ok(format);
    }
    if !extension.is_empty() {
        return Err(format!(
            "Unsupported document type for {}",
            source_file_name(source)
        ));
    }

    let mut options = tauri_plugin_fs::OpenOptions::new();
    options.read(true);
    let mut file = app
        .fs()
        .open(source.clone(), options)
        .map_err(|err| format!("Failed to inspect selected document: {err}"))?;
    let mut prefix = [0u8; 512];
    let read = file
        .read(&mut prefix)
        .map_err(|err| format!("Failed to inspect selected document: {err}"))?;
    document_format_from(&extension, &prefix[..read])
        .ok_or_else(|| format!("Unsupported document type for {}", source_file_name(source)))
}

fn document_format_from(extension: &str, prefix: &[u8]) -> Option<DocumentFormat> {
    match extension {
        "html" | "htm" => return Some(DocumentFormat::Html),
        "epub" => return Some(DocumentFormat::Epub),
        "pdf" => return Some(DocumentFormat::Pdf),
        "" => {}
        _ => return None,
    }

    if prefix.starts_with(b"PK\x03\x04")
        || prefix.starts_with(b"PK\x05\x06")
        || prefix.starts_with(b"PK\x07\x08")
    {
        return Some(DocumentFormat::Epub);
    }
    if prefix.starts_with(b"%PDF-") {
        return Some(DocumentFormat::Pdf);
    }
    if prefix.starts_with(&[0xff, 0xfe]) || prefix.starts_with(&[0xfe, 0xff]) {
        return Some(DocumentFormat::Html);
    }
    let text = String::from_utf8_lossy(prefix);
    let text = text.trim_start_matches('\u{feff}').trim_start();
    text.starts_with('<').then_some(DocumentFormat::Html)
}

/// Return only a readable basename for progress/errors; never expose a user's
/// full local path to the frontend status surface.
fn source_file_name(source: &FilePath) -> String {
    original_source_file_name(source).unwrap_or_else(|| "document".into())
}

/// Preserve a real basename when the platform picker exposes one.
///
/// Some mobile content providers expose only an opaque URL. Returning `None`
/// keeps the provenance field honest while `source_file_name` still supplies a
/// harmless fallback for progress and error messages.
fn original_source_file_name(source: &FilePath) -> Option<String> {
    match source {
        FilePath::Path(path) => path
            .file_name()
            .and_then(|value| value.to_str())
            .map(str::to_owned),
        FilePath::Url(url) => url
            .path_segments()
            .and_then(|mut segments| segments.next_back())
            .filter(|value| !value.is_empty())
            .map(|value| percent_decode_str(value).decode_utf8_lossy().into_owned()),
    }
}

fn source_title(source: &FilePath) -> String {
    let file_name = source_file_name(source);
    Path::new(&file_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("Imported PDF")
        .to_string()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    #[cfg(desktop)]
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn source(name: &str) -> FilePath {
        FilePath::Path(PathBuf::from(name))
    }

    #[test]
    fn batch_continues_after_failure_and_stops_between_files() {
        let sources = vec![source("one.html"), source("bad.epub"), source("later.htm")];
        let mut cancellation_checks = 0;
        let mut events = Vec::new();
        let run = process_sources(
            sources,
            || {
                cancellation_checks += 1;
                Ok(cancellation_checks > 2)
            },
            |source| {
                let name = source_file_name(&source);
                if name == "bad.epub" {
                    Err("invalid archive".into())
                } else {
                    Ok((name, false))
                }
            },
            |progress| events.push(progress),
        )
        .expect("batch result");

        assert_eq!(run.selected, 3);
        assert_eq!(run.processed, 2);
        assert_eq!(run.imported, vec!["one.html"]);
        assert!(run.already_in_library.is_empty());
        assert_eq!(run.failures.len(), 1);
        assert_eq!(run.failures[0].file_name, "bad.epub");
        assert!(run.cancelled);
        assert_eq!(events.len(), 4);
    }

    #[test]
    fn batch_reports_reused_documents_separately_from_new_imports() {
        let sources = vec![source("new.epub"), source("existing.epub")];
        let mut events = Vec::new();
        let run = process_sources(
            sources,
            || Ok(false),
            |source| {
                let name = source_file_name(&source);
                let existing = name == "existing.epub";
                Ok((name, existing))
            },
            |progress| events.push(progress),
        )
        .expect("batch result");

        assert_eq!(run.imported, vec!["new.epub", "existing.epub"]);
        assert_eq!(run.already_in_library, vec!["existing.epub"]);
        let completed = events.last().expect("completed progress");
        assert_eq!(completed.imported, 1);
        assert_eq!(completed.already_in_library, 1);
    }

    #[test]
    fn extensionless_mobile_documents_are_sniffed_before_import() {
        assert_eq!(
            document_format_from("", b"PK\x03\x04epub payload"),
            Some(DocumentFormat::Epub)
        );
        assert_eq!(
            document_format_from("", b"  <!doctype html><html></html>"),
            Some(DocumentFormat::Html)
        );
        assert_eq!(
            document_format_from("", b"%PDF-1.7\n"),
            Some(DocumentFormat::Pdf)
        );
        assert_eq!(
            document_format_from("", &[0xff, 0xfe, b'<' as u8, 0]),
            Some(DocumentFormat::Html)
        );
        assert_eq!(document_format_from("", b"plain text"), None);
    }

    #[test]
    fn delete_batch_retains_successes_after_a_failure() {
        let urls = vec!["/uploads/aa.html", "/uploads/bb.html", "/uploads/cc.html"]
            .into_iter()
            .map(str::to_string)
            .collect();
        let mut events = Vec::new();
        let run = process_deletions(
            urls,
            |url| {
                if url.contains("bb") {
                    Err("locked".into())
                } else {
                    Ok(url)
                }
            },
            |progress| events.push(progress),
        );

        assert_eq!(run.selected, 3);
        assert_eq!(run.processed, 3);
        assert_eq!(run.deleted.len(), 2);
        assert_eq!(run.failures.len(), 1);
        assert_eq!(run.failures[0].document_url, "/uploads/bb.html");
        assert_eq!(events.len(), 6);
    }

    #[test]
    fn document_formats_are_case_insensitive() {
        assert_eq!(
            document_format_from("epub", &[]),
            Some(DocumentFormat::Epub)
        );
        assert_eq!(document_format_from("htm", &[]), Some(DocumentFormat::Html));
        assert_eq!(document_format_from("pdf", &[]), Some(DocumentFormat::Pdf));
        assert_eq!(document_format_from("txt", &[]), None);
        assert_eq!(document_format_from("txt", b"<html></html>"), None);
    }

    #[test]
    #[cfg(desktop)]
    fn folder_sources_preserve_supported_files_through_five_visible_levels() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "papercut-document-folder-test-{}-{suffix}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("nested/two/three/four/five")).expect("create test folders");
        fs::write(root.join("b.epub"), b"test").expect("write epub");
        fs::write(root.join("a.HTML"), b"test").expect("write html");
        fs::write(root.join("notes.txt"), b"test").expect("write text");
        fs::write(root.join("nested/kept.html"), b"test").expect("write nested html");
        fs::write(root.join("nested/two/three/four/kept.pdf"), b"test")
            .expect("write deepest supported pdf");
        fs::write(
            root.join("nested/two/three/four/five/ignored.html"),
            b"test",
        )
        .expect("write too-deep html");

        let selection = folder_sources(FilePath::Path(root.clone())).expect("folder sources");
        let files: Vec<_> = selection
            .sources
            .iter()
            .map(|item| (source_file_name(&item.source), item.relative_folder.clone()))
            .collect();
        assert_eq!(
            files,
            vec![
                ("a.HTML".into(), vec![]),
                ("b.epub".into(), vec![]),
                ("kept.html".into(), vec!["nested".into()]),
                (
                    "kept.pdf".into(),
                    vec!["nested".into(), "two".into(), "three".into(), "four".into()]
                ),
            ]
        );

        fs::remove_dir_all(root).expect("remove test folder");
    }
}
