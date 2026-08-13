//! Desktop file-association requests waiting for the React import lifecycle.

#[cfg(desktop)]
use std::ffi::OsString;
#[cfg(desktop)]
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;

#[cfg(desktop)]
use tauri::{AppHandle, Emitter, Manager, Runtime};

#[cfg(desktop)]
pub(crate) const OPEN_DOCUMENTS_EVENT: &str = "desktop-open-documents";

#[derive(Default)]
pub(crate) struct DesktopOpenState {
    paths: Mutex<Vec<PathBuf>>,
}

impl DesktopOpenState {
    #[cfg(desktop)]
    fn push(&self, paths: Vec<PathBuf>) -> Result<(), String> {
        self.paths
            .lock()
            .map_err(|_| "Desktop open-request state lock poisoned".to_string())?
            .extend(paths);
        Ok(())
    }

    fn take(&self) -> Result<Vec<PathBuf>, String> {
        let mut paths = self
            .paths
            .lock()
            .map_err(|_| "Desktop open-request state lock poisoned".to_string())?;
        Ok(std::mem::take(&mut *paths))
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
    queue_paths(
        app,
        args.into_iter().map(PathBuf::from).map(|path| {
            if path.is_absolute() {
                path
            } else {
                cwd.join(path)
            }
        }),
    );
}

/// Queue a native open request until React is ready and wake an already-running
/// frontend. Keeping the paths queued also prevents requests arriving during
/// another Library operation from being lost.
#[cfg(desktop)]
pub(crate) fn queue_paths<R, I>(app: &AppHandle<R>, paths: I)
where
    R: Runtime,
    I: IntoIterator<Item = PathBuf>,
{
    let paths = paths
        .into_iter()
        .filter(|path| is_supported_open_file(path))
        .collect::<Vec<_>>();
    if paths.is_empty() {
        return;
    }

    if let Err(error) = app.state::<DesktopOpenState>().push(paths) {
        log::warn!("Unable to queue desktop open request: {error}");
        return;
    }
    if let Err(error) = app.emit(OPEN_DOCUMENTS_EVENT, ()) {
        log::warn!("Unable to notify the frontend about a desktop open request: {error}");
    }
}

/// Atomically hand queued paths to the one frontend import consumer.
#[tauri::command]
pub(crate) fn desktop_open_take_paths(
    state: tauri::State<'_, DesktopOpenState>,
) -> Result<Vec<PathBuf>, String> {
    state.take()
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
}
