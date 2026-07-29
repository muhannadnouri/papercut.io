//! Sequential document import/delete batches with progress and partial results.

use std::collections::HashSet;
use std::io::Read;
use std::path::Path;
#[cfg(desktop)]
use std::{fs, path::PathBuf};

use percent_encoding::percent_decode_str;
use tauri::{Emitter, Runtime};
use tauri_plugin_dialog::{DialogExt, FilePath};
use tauri_plugin_fs::FsExt;

use super::pdf::import_pdf_source;
use super::pipeline::{delete_upload, import_epub_source, import_html_source};
use super::state::DocumentBatchControl;
use super::storage::upload_id_from_url;
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
    failures: Vec<UploadedDocumentBatchFailure>,
    cancelled: bool,
}

struct DeleteBatchRun<T> {
    selected: usize,
    processed: usize,
    deleted: Vec<T>,
    failures: Vec<UploadedDocumentDeleteBatchFailure>,
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
    import_sources(app, control, sources)
}

/// Pick one desktop folder and import only its direct supported file children.
/// Subfolders and symlinks are intentionally skipped; recursion can be added
/// later without changing the shared import runner if users need it.
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
    let sources = folder_sources(folder)?;
    if sources.is_empty() {
        return Err("The selected folder has no HTML, EPUB, or PDF files".into());
    }
    import_sources(app, control, sources)
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
) -> Result<UploadedDocumentBatchResult, String> {
    if sources.len() > MAX_BATCH_DOCUMENTS {
        return Err(format!(
            "Select at most {MAX_BATCH_DOCUMENTS} documents in one import"
        ));
    }

    let run = process_sources(
        sources,
        || control.is_cancelled(),
        |source| import_source(&app, source),
        |progress| {
            let _ = app.emit(DOCUMENT_IMPORT_PROGRESS_EVENT, progress);
        },
    )?;
    let phase = if run.cancelled {
        "cancelled"
    } else {
        "completed"
    };
    let _ = app.emit(
        DOCUMENT_IMPORT_PROGRESS_EVENT,
        UploadedDocumentBatchProgress {
            phase: phase.into(),
            processed: run.processed,
            total: run.selected,
            imported: run.imported.len(),
            failed: run.failures.len(),
            file_name: None,
        },
    );

    Ok(UploadedDocumentBatchResult {
        selected: run.selected,
        processed: run.processed,
        imported: run.imported,
        failures: run.failures,
        cancelled: run.cancelled,
    })
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

/// Convert a desktop folder into a stable list of direct, regular document
/// files. URL-backed folders are rejected because mobile providers do not
/// expose a directory that Rust can enumerate safely.
#[cfg(desktop)]
fn folder_sources(folder: FilePath) -> Result<Vec<FilePath>, String> {
    let FilePath::Path(folder) = folder else {
        return Err("Folder import is available on desktop only".into());
    };
    let mut paths = Vec::<PathBuf>::new();
    for entry in
        fs::read_dir(&folder).map_err(|err| format!("Failed to read selected folder: {err}"))?
    {
        let entry = entry.map_err(|err| format!("Failed to read folder entry: {err}"))?;
        let file_type = entry
            .file_type()
            .map_err(|err| format!("Failed to inspect folder entry: {err}"))?;
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
            paths.push(entry.path());
        }
    }
    paths.sort();
    Ok(paths.into_iter().map(FilePath::Path).collect())
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
    I: FnMut(FilePath) -> Result<T, String>,
    P: FnMut(UploadedDocumentBatchProgress),
{
    let selected = sources.len();
    let mut run = BatchRun {
        selected,
        processed: 0,
        imported: Vec::new(),
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
            imported: run.imported.len(),
            failed: run.failures.len(),
            file_name: Some(file_name.clone()),
        });

        match import(source) {
            Ok(document) => run.imported.push(document),
            Err(error) => run
                .failures
                .push(UploadedDocumentBatchFailure { file_name, error }),
        }
        run.processed += 1;
        progress(UploadedDocumentBatchProgress {
            phase: "importing".into(),
            processed: run.processed,
            total: selected,
            imported: run.imported.len(),
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
) -> Result<UploadedDocument, String> {
    match document_format(app, &source)? {
        DocumentFormat::Html => import_html_source(app, source),
        DocumentFormat::Epub => import_epub_source(app, source),
        DocumentFormat::Pdf => {
            let title = source_title(&source);
            import_pdf_source(app, source, title)
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
    match source {
        FilePath::Path(path) => path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("document")
            .to_string(),
        FilePath::Url(url) => url
            .path_segments()
            .and_then(|mut segments| segments.next_back())
            .filter(|value| !value.is_empty())
            .map(|value| percent_decode_str(value).decode_utf8_lossy().into_owned())
            .unwrap_or_else(|| "document".into()),
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
                    Ok(name)
                }
            },
            |progress| events.push(progress),
        )
        .expect("batch result");

        assert_eq!(run.selected, 3);
        assert_eq!(run.processed, 2);
        assert_eq!(run.imported, vec!["one.html"]);
        assert_eq!(run.failures.len(), 1);
        assert_eq!(run.failures[0].file_name, "bad.epub");
        assert!(run.cancelled);
        assert_eq!(events.len(), 4);
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
    fn folder_sources_include_only_direct_supported_regular_files() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "papercut-document-folder-test-{}-{suffix}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("nested")).expect("create test folders");
        fs::write(root.join("b.epub"), b"test").expect("write epub");
        fs::write(root.join("a.HTML"), b"test").expect("write html");
        fs::write(root.join("notes.txt"), b"test").expect("write text");
        fs::write(root.join("nested/ignored.html"), b"test").expect("write nested html");

        let sources = folder_sources(FilePath::Path(root.clone())).expect("folder sources");
        let names: Vec<_> = sources.iter().map(source_file_name).collect();
        assert_eq!(names, vec!["a.HTML", "b.epub"]);

        fs::remove_dir_all(root).expect("remove test folder");
    }
}
