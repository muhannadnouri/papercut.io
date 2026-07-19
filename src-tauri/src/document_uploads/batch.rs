//! Sequential multi-file/folder document import with progress and partial results.

use std::path::Path;
#[cfg(desktop)]
use std::{fs, path::PathBuf};

use percent_encoding::percent_decode_str;
use tauri::{Emitter, Runtime};
use tauri_plugin_dialog::{DialogExt, FilePath};

use super::pipeline::{import_epub_source, import_html_source};
use super::state::DocumentBatchControl;
use super::types::{
    UploadedDocument, UploadedDocumentBatchFailure, UploadedDocumentBatchProgress,
    UploadedDocumentBatchResult,
};

pub(crate) const DOCUMENT_IMPORT_PROGRESS_EVENT: &str = "document-uploads-import-progress";
const MAX_BATCH_FILES: usize = 500;

enum DocumentFormat {
    Html,
    Epub,
}

struct BatchRun<T> {
    selected: usize,
    processed: usize,
    imported: Vec<T>,
    failures: Vec<UploadedDocumentBatchFailure>,
    cancelled: bool,
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
        .add_filter("HTML and EPUB Documents", &["html", "htm", "epub"])
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
        return Err("The selected folder has no HTML or EPUB files".into());
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
    if sources.len() > MAX_BATCH_FILES {
        return Err(format!(
            "Select at most {MAX_BATCH_FILES} documents in one import"
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
        let source = FilePath::Path(entry.path());
        if document_format(&source).is_ok() {
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
    match document_format(&source)? {
        DocumentFormat::Html => import_html_source(app, source),
        DocumentFormat::Epub => import_epub_source(app, source),
    }
}

/// Native dialogs return filesystem paths on desktop and may return content
/// URLs on mobile, so format detection must support both representations.
fn document_format(source: &FilePath) -> Result<DocumentFormat, String> {
    let extension = match source {
        FilePath::Path(path) => path.extension().and_then(|value| value.to_str()),
        FilePath::Url(url) => Path::new(url.path())
            .extension()
            .and_then(|value| value.to_str()),
    }
    .unwrap_or_default()
    .to_ascii_lowercase();

    match extension.as_str() {
        "html" | "htm" => Ok(DocumentFormat::Html),
        "epub" => Ok(DocumentFormat::Epub),
        _ => Err(format!(
            "Unsupported document type for {}",
            source_file_name(source)
        )),
    }
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

#[cfg(test)]
mod tests {
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
    fn document_formats_are_case_insensitive() {
        assert!(matches!(
            document_format(&source("book.EPUB")),
            Ok(DocumentFormat::Epub)
        ));
        assert!(matches!(
            document_format(&source("page.HTM")),
            Ok(DocumentFormat::Html)
        ));
        assert!(document_format(&source("notes.txt")).is_err());
    }

    #[test]
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
