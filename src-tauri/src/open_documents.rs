//! Platform file-open requests waiting for the React import lifecycle.

#[cfg(desktop)]
use std::ffi::OsString;
#[cfg(desktop)]
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_dialog::FilePath;

pub(crate) const OPEN_DOCUMENTS_EVENT: &str = "open-documents";

#[derive(Default)]
pub(crate) struct OpenDocumentState {
    sources: Mutex<Vec<FilePath>>,
}

impl OpenDocumentState {
    fn push(&self, sources: Vec<FilePath>) -> Result<(), String> {
        self.sources
            .lock()
            .map_err(|_| "Document open-request state lock poisoned".to_string())?
            .extend(sources);
        Ok(())
    }

    fn take(&self) -> Result<Vec<FilePath>, String> {
        let mut sources = self
            .sources
            .lock()
            .map_err(|_| "Document open-request state lock poisoned".to_string())?;
        Ok(std::mem::take(&mut *sources))
    }
}

/// Resolve OS command-line arguments against the launch directory and retain
/// only existing document files that Papercut already knows how to import.
#[cfg(desktop)]
pub(crate) fn queue_cli_paths<R, I>(app: &AppHandle<R>, args: I, cwd: &Path)
where
    R: Runtime,
    I: IntoIterator<Item = OsString>,
{
    queue_sources(
        app,
        args.into_iter()
            .map(PathBuf::from)
            .map(|path| {
                if path.is_absolute() {
                    path
                } else {
                    cwd.join(path)
                }
            })
            .filter(|path| is_supported_open_file(path))
            .map(FilePath::Path)
            .collect(),
    );
}

/// Preserve native URLs until the importer opens them: Android `content://`
/// and iOS security-scoped `file://` access cannot safely become `PathBuf`s.
#[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
pub(crate) fn queue_opened_urls<R, I>(app: &AppHandle<R>, urls: I)
where
    R: Runtime,
    I: IntoIterator<Item = tauri::Url>,
{
    queue_sources(
        app,
        urls.into_iter().filter_map(opened_url_source).collect(),
    );
}

/// Queue a native request until React is ready and wake an already-running
/// frontend. The queue also retains requests received during another import.
fn queue_sources<R: Runtime>(app: &AppHandle<R>, sources: Vec<FilePath>) {
    if sources.is_empty() {
        return;
    }

    if let Err(error) = app.state::<OpenDocumentState>().push(sources) {
        log::warn!("Unable to queue document open request: {error}");
        return;
    }
    if let Err(error) = app.emit(OPEN_DOCUMENTS_EVENT, ()) {
        log::warn!("Unable to notify the frontend about a document open request: {error}");
    }
}

/// Atomically hand queued paths or provider URLs to the one import consumer.
#[tauri::command]
pub(crate) fn open_documents_take_sources(
    state: tauri::State<'_, OpenDocumentState>,
) -> Result<Vec<FilePath>, String> {
    state.take()
}

/// Accept only local OS document handles. Desktop can validate the resolved
/// file immediately; mobile provider URLs remain opaque until the importer.
#[cfg(any(target_os = "macos", target_os = "ios", target_os = "android", test))]
fn opened_url_source(url: tauri::Url) -> Option<FilePath> {
    #[cfg(target_os = "android")]
    if matches!(url.scheme(), "content" | "file") {
        return Some(FilePath::Url(url));
    }

    #[cfg(target_os = "ios")]
    if url.scheme() == "file" {
        return Some(FilePath::Url(url));
    }

    #[cfg(desktop)]
    if url.scheme() == "file" {
        return url
            .to_file_path()
            .ok()
            .filter(|path| is_supported_open_file(path))
            .map(FilePath::Path);
    }

    None
}

#[cfg(desktop)]
fn is_supported_open_file(path: &Path) -> bool {
    path.is_file()
        && matches!(
            path.extension()
                .and_then(|extension| extension.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("html" | "htm" | "epub" | "pdf" | "txt" | "md" | "markdown")
        )
}

#[cfg(all(test, desktop))]
mod tests {
    use super::*;

    #[test]
    fn open_requests_keep_only_supported_existing_files() {
        let root = std::env::temp_dir().join(format!("papercut-open-test-{}", std::process::id()));
        let supported = root.join("book.EPUB");
        let unsupported = root.join("notes.docx");
        std::fs::create_dir_all(&root).expect("create test directory");
        std::fs::write(&supported, b"epub").expect("write supported file");
        std::fs::write(&unsupported, b"docx").expect("write unsupported file");

        assert!(is_supported_open_file(&supported));
        assert!(!is_supported_open_file(&unsupported));
        assert!(!is_supported_open_file(&root.join("missing.pdf")));

        std::fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn open_requests_reject_non_file_urls() {
        let url = tauri::Url::parse("https://example.com/book.epub").expect("parse URL");
        assert!(opened_url_source(url).is_none());
    }
}
