//! "Erase all data": removes everything Papercut stores on this device.
//!
//! A marker file drives the erase so it can finish after a restart or crash:
//! `request` writes `PENDING_NATIVE`, the wipe runs, and the marker advances to
//! `PENDING_WEBVIEW` until the frontend has cleared WebView storage at next boot.
//! Desktop wipes during startup, before anything can hold a file open (Windows
//! cannot delete open files). Mobile has no relaunch API, so it wipes in-process.
//!
//! Removing this feature means deleting this file and `src/app-reset/`, then
//! reverting the call sites. See `docs/app-reset.md`.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use tauri::Manager;

use crate::{
    document_uploads::{semantic::SemanticIndexState, DocumentUploadState},
    library_transfer::LibraryTransferState,
    native_tts::NativeTtsState,
    search_embeddings::SearchEmbeddingState,
};

const MARKER_FILE: &str = ".papercut-reset";
const PENDING_NATIVE: &str = "papercut-reset-v1:pending-native";
const PENDING_WEBVIEW: &str = "papercut-reset-v1:pending-webview";

/// Named children of the app data and cache roots. Never delete a root: on
/// Android it is the whole sandbox, and on Linux and Windows it holds the WebView
/// profile. A new top-level store must be added here. Both roots use one list
/// because cache writers fall back to app data when the cache root can't resolve.
const ERASABLE_CHILDREN: &[&str] = &[
    "audiobook-exports",
    "audiobooks",
    "document-scanner",
    "document_uploads",
    "model-installer",
    "models",
    "runtime-installer",
    "runtimes",
    "silma-sidecar-probe",
    "user_uploads",
    "voice-previews",
];

#[derive(Debug, PartialEq)]
enum Marker {
    PendingNative,
    PendingWebview { failed_paths: Vec<String> },
}

#[derive(Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppResetSummary {
    documents: usize,
    audiobooks: usize,
    total_bytes: u64,
    locations: Vec<String>,
    blocked_by: Option<&'static str>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppResetOutcome {
    blocked_by: Option<&'static str>,
    restarting: bool,
}

#[derive(Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppResetPending {
    failed_paths: Vec<String>,
}

/// Parse the marker. The first line is the state, and any further lines are paths
/// the wipe could not remove, one per line:
///
/// ```text
/// papercut-reset-v1:pending-webview
/// /home/me/.local/share/io.papercut.desktop/document_uploads
/// ```
///
/// Anything else reads as no marker, because a stray or truncated file must never
/// trigger a wipe. That includes unreadable files, so a future format needs a new
/// version prefix rather than extra fields on these states.
fn read_marker(data: &Path) -> Option<Marker> {
    let content = fs::read_to_string(data.join(MARKER_FILE)).ok()?;
    let mut lines = content.lines();
    match lines.next()? {
        PENDING_NATIVE => Some(Marker::PendingNative),
        PENDING_WEBVIEW => Some(Marker::PendingWebview {
            failed_paths: lines.map(str::to_owned).collect(),
        }),
        _ => None,
    }
}

/// Replace the marker atomically. A crash mid-write must leave the old state or the
/// new one: a partial `pending-native` would silently cancel a confirmed erase, and
/// a partial `pending-webview` would leave bookmarks and settings behind.
fn write_marker(data: &Path, content: &str) -> io::Result<()> {
    fs::create_dir_all(data)?;
    let staged = data.join(format!("{MARKER_FILE}.tmp"));
    fs::write(&staged, content)?;
    fs::rename(staged, data.join(MARKER_FILE))
}

/// Remove one file, symlink or tree, treating an absent path as done. Uses
/// `symlink_metadata` so a symlinked child is unlinked, never followed out of the
/// app's directories. The SILMA Hugging Face cache contains symlinks.
fn remove_entry(path: &Path) -> io::Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };
    if metadata.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

/// Remove every erasable child, continuing past failures so one stuck file can't
/// preserve the rest. Returns the paths that could not be removed.
fn wipe_in(data: &Path, cache: &Path) -> Vec<PathBuf> {
    [data, cache]
        .into_iter()
        .flat_map(|root| ERASABLE_CHILDREN.iter().map(move |child| root.join(child)))
        .filter(|path| match remove_entry(path) {
            Ok(()) => false,
            Err(err) => {
                log::warn!("Failed to erase {}: {err}", path.display());
                true
            }
        })
        .collect()
}

/// Run a requested wipe at most once. Returns whether one ran. The marker advances
/// even after a partial failure, because retrying on a later launch would destroy
/// documents imported since. A crash mid-wipe does resume, as nothing new exists yet.
fn resume_wipe(data: &Path, cache: &Path) -> bool {
    if read_marker(data) != Some(Marker::PendingNative) {
        return false;
    }
    let failed = wipe_in(data, cache);
    let mut content = PENDING_WEBVIEW.to_owned();
    for path in &failed {
        content.push('\n');
        content.push_str(&path.display().to_string());
    }
    if let Err(err) = write_marker(data, &content) {
        log::warn!("Failed to advance the reset marker: {err}");
        if let Err(err) = fs::remove_file(data.join(MARKER_FILE)) {
            log::warn!("Failed to remove the reset marker: {err}");
        }
    }
    true
}

/// Bytes an erase would free. Doesn't follow symlinks, because SILMA's Hugging Face
/// snapshots link into its blob store and would otherwise count twice. Best effort:
/// unreadable entries count as zero, since this only informs the dialog.
fn tree_size(path: &Path) -> u64 {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return 0;
    };
    if !metadata.is_dir() {
        return metadata.len();
    }
    fs::read_dir(path).map_or(0, |entries| {
        entries
            .flatten()
            .map(|entry| tree_size(&entry.path()))
            .sum()
    })
}

/// Count stored items, skipping dot-prefixed staging directories.
fn count_items(dir: &Path) -> usize {
    fs::read_dir(dir).map_or(0, |entries| {
        entries
            .flatten()
            .filter(|entry| {
                entry.file_type().is_ok_and(|kind| kind.is_dir())
                    && !entry.file_name().to_string_lossy().starts_with('.')
            })
            .count()
    })
}

/// Counts, size and folders for the confirmation dialog. `audiobooks` excludes
/// imported sources in `user_uploads`, which have no audio of their own.
///
/// `locations` is the two roots rather than the erased children, so users can inspect
/// everything Papercut keeps. Those roots also hold WebView files that survive an
/// erase, which the dialog says. It's empty on mobile, where users can't browse the
/// sandbox and a path would only be noise.
fn summary_in(data: &Path, cache: &Path, blocked_by: Option<&'static str>) -> AppResetSummary {
    AppResetSummary {
        documents: count_items(&data.join("document_uploads")),
        audiobooks: count_items(&data.join("audiobooks")),
        total_bytes: [data, cache]
            .into_iter()
            .flat_map(|root| ERASABLE_CHILDREN.iter().map(move |child| root.join(child)))
            .map(|path| tree_size(&path))
            .sum(),
        locations: if cfg!(desktop) {
            [data, cache]
                .map(|root| root.display().to_string())
                .to_vec()
        } else {
            Vec::new()
        },
        blocked_by,
    }
}

/// Resolve app data and cache. On Android the cache root sits inside the data root,
/// so callers must not assume the two are disjoint.
fn roots(app: &tauri::AppHandle) -> Result<(PathBuf, PathBuf), String> {
    let paths = app.path();
    let data = paths
        .app_data_dir()
        .map_err(|err| format!("Failed to resolve app data directory: {err}"))?;
    let cache = paths
        .app_cache_dir()
        .map_err(|err| format!("Failed to resolve app cache directory: {err}"))?;
    Ok((data, cache))
}

/// Job that must finish first, as a code the frontend maps to
/// `settings.erase.blocked.*`. Excludes imports, whose only check is the
/// `begin_batch` reservation. OCR and queued audiobook state live in the WebView and
/// can't be seen from here, so the frontend passes those separately as `blockedBy`.
///
/// Refusing rather than cancelling is deliberate. TTS and SILMA installs have no
/// cancel, and a final rename from one would bring deleted data back after the wipe.
fn active_job(app: &tauri::AppHandle) -> Option<&'static str> {
    if app.state::<SemanticIndexState>().is_running()
        || app.state::<SearchEmbeddingState>().is_busy()
    {
        return Some("search");
    }
    if app.state::<NativeTtsState>().is_busy() {
        return Some("audio");
    }
    app.state::<LibraryTransferState>()
        .is_busy()
        .then_some("transfer")
}

/// Startup half of the desktop flow; also finishes a mobile wipe cut off by a
/// crash. Must run before anything opens app data. Returns whether a wipe ran.
pub(crate) fn finish_pending_wipe(app: &tauri::AppHandle) -> bool {
    match roots(app) {
        Ok((data, cache)) => resume_wipe(&data, &cache),
        Err(err) => {
            log::warn!("{err}");
            false
        }
    }
}

/// Dialog data. Reserving the batch slot is the only way to observe a running import;
/// the reservation is released before this returns, so the result is advisory and
/// `app_reset_request` checks again.
#[tauri::command]
pub(crate) async fn app_reset_summary(app: tauri::AppHandle) -> Result<AppResetSummary, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let blocked_by = match app.state::<DocumentUploadState>().begin_batch() {
            Ok(_batch) => active_job(&app),
            Err(_) => Some("imports"),
        };
        let (data, cache) = roots(&app)?;
        Ok(summary_in(&data, &cache, blocked_by))
    })
    .await
    .map_err(|err| format!("Erase summary task failed: {err}"))?
}

#[tauri::command]
pub(crate) async fn app_reset_request(app: tauri::AppHandle) -> Result<AppResetOutcome, String> {
    tauri::async_runtime::spawn_blocking(move || request_blocking(&app))
        .await
        .map_err(|err| format!("Erase task failed: {err}"))?
}

/// Start the confirmed erase. The marker is written first on every platform, so a
/// crash partway through still finishes on next launch.
///
/// - **Desktop:** requests a restart and returns. The IPC reply may never arrive,
///   so the dialog stays busy until the process exits. `request_restart` is used
///   rather than `restart` because it runs `RunEvent::Exit`, which releases the
///   single-instance lock before the new process starts.
/// - **Mobile:** wipes here, then the frontend reloads the page. Rust state survives
///   a reload, which is why engines and transfer results are dropped explicitly.
///
/// `cfg!` rather than `#[cfg]` keeps the mobile branch compiling in desktop builds.
fn request_blocking(app: &tauri::AppHandle) -> Result<AppResetOutcome, String> {
    let blocked = |code| {
        Ok(AppResetOutcome {
            blocked_by: Some(code),
            restarting: false,
        })
    };
    // Held through the wipe so no import can start while files are being removed.
    let uploads = app.state::<DocumentUploadState>();
    let Ok(_batch) = uploads.begin_batch() else {
        return blocked("imports");
    };
    if let Some(code) = active_job(app) {
        return blocked(code);
    }

    let (data, cache) = roots(app)?;
    write_marker(&data, PENDING_NATIVE)
        .map_err(|err| format!("Failed to prepare the erase: {err}"))?;

    if cfg!(desktop) {
        app.request_restart();
        return Ok(AppResetOutcome {
            blocked_by: None,
            restarting: true,
        });
    }

    // The process survives on mobile, so drop what still refers to erased files.
    app.state::<NativeTtsState>().unload_engine();
    if let Err(err) = app.state::<SearchEmbeddingState>().unload() {
        log::warn!("{err}");
    }
    resume_wipe(&data, &cache);
    crate::library_transfer::cleanup_orphaned_transfer_files(&cache);
    app.state::<LibraryTransferState>().forget_finished();
    Ok(AppResetOutcome {
        blocked_by: None,
        restarting: false,
    })
}

/// What the boot gate must finish, if an erase just ran. Returns nothing during
/// `pending-native`: by the time the frontend can call this, `setup` or the mobile
/// request has already moved the marker on.
#[tauri::command]
pub(crate) fn app_reset_pending(app: tauri::AppHandle) -> Result<Option<AppResetPending>, String> {
    let (data, _) = roots(&app)?;
    Ok(pending_in(&data))
}

/// Called after WebView storage is cleared. A crash before this repeats the
/// clear, which is harmless.
#[tauri::command]
pub(crate) fn app_reset_acknowledge(app: tauri::AppHandle) -> Result<(), String> {
    let (data, _) = roots(&app)?;
    acknowledge_in(&data).map_err(|err| format!("Failed to finish the erase: {err}"))
}

fn pending_in(data: &Path) -> Option<AppResetPending> {
    match read_marker(data) {
        Some(Marker::PendingWebview { failed_paths }) => Some(AppResetPending { failed_paths }),
        _ => None,
    }
}

/// Remove the marker once the WebView is clean. Only `pending-webview` qualifies, so
/// a stale frontend call can never cancel a wipe the user has confirmed.
fn acknowledge_in(data: &Path) -> io::Result<()> {
    if matches!(read_marker(data), Some(Marker::PendingWebview { .. })) {
        fs::remove_file(data.join(MARKER_FILE))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ROOT: AtomicU64 = AtomicU64::new(0);

    struct Roots {
        base: PathBuf,
        data: PathBuf,
        cache: PathBuf,
    }

    impl Roots {
        fn new() -> Self {
            let base = std::env::temp_dir().join(format!(
                "papercut-app-reset-{}-{}",
                std::process::id(),
                NEXT_ROOT.fetch_add(1, Ordering::Relaxed)
            ));
            let data = base.join("data");
            let cache = base.join("cache");
            fs::create_dir_all(&data).unwrap();
            fs::create_dir_all(&cache).unwrap();
            Self { base, data, cache }
        }
    }

    impl Drop for Roots {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }

    fn put(path: &Path, bytes: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn wipe_removes_listed_children_and_nothing_else() {
        let roots = Roots::new();
        put(&roots.data.join("document_uploads/abc/source.pdf"), b"x");
        put(
            &roots.data.join("document_uploads/search.sqlite3-wal"),
            b"x",
        );
        put(
            &roots.data.join("models/sherpa-onnx/kokoro/model.onnx"),
            b"x",
        );
        put(&roots.cache.join("model-installer/archive"), b"x");
        put(&roots.cache.join("document-scanner/draft/page.jpg"), b"x");
        // WebView profile and sandbox neighbours share these roots.
        put(&roots.data.join("EBWebView/Default/Local Storage"), b"keep");
        put(&roots.cache.join("WebKit/keep"), b"keep");

        assert!(wipe_in(&roots.data, &roots.cache).is_empty());

        assert!(!roots.data.join("document_uploads").exists());
        assert!(!roots.data.join("models").exists());
        assert!(!roots.cache.join("model-installer").exists());
        assert!(!roots.cache.join("document-scanner").exists());
        assert!(roots.data.join("EBWebView/Default/Local Storage").exists());
        assert!(roots.cache.join("WebKit/keep").exists());
    }

    #[cfg(unix)]
    #[test]
    fn wipe_does_not_follow_symlinks_out_of_the_root() {
        let roots = Roots::new();
        let outside = roots.base.join("outside/kept");
        put(&outside, b"keep");
        fs::create_dir_all(roots.data.join("models")).unwrap();
        std::os::unix::fs::symlink(
            roots.base.join("outside"),
            roots.data.join("models/snapshot"),
        )
        .unwrap();
        std::os::unix::fs::symlink(roots.base.join("outside"), roots.data.join("audiobooks"))
            .unwrap();

        assert!(wipe_in(&roots.data, &roots.cache).is_empty());

        assert!(!roots.data.join("models").exists());
        assert!(fs::symlink_metadata(roots.data.join("audiobooks")).is_err());
        assert!(outside.exists());
    }

    #[test]
    fn unrecognized_marker_content_never_triggers_a_wipe() {
        let roots = Roots::new();
        let document = roots.data.join("document_uploads/abc/source.pdf");
        put(&document, b"x");

        for content in ["", "pending-native", "papercut-reset-v0:pending-native"] {
            fs::write(roots.data.join(MARKER_FILE), content).unwrap();
            assert!(!resume_wipe(&roots.data, &roots.cache));
            assert!(document.exists());
        }
    }

    #[test]
    fn no_marker_means_no_wipe() {
        let roots = Roots::new();
        let document = roots.data.join("document_uploads/abc/source.pdf");
        put(&document, b"x");

        assert!(!resume_wipe(&roots.data, &roots.cache));
        assert!(document.exists());
        assert_eq!(pending_in(&roots.data), None);
    }

    #[test]
    fn full_flow_wipes_once_then_waits_for_the_webview() {
        let roots = Roots::new();
        put(&roots.data.join("audiobooks/abc/manifest.json"), b"x");
        write_marker(&roots.data, PENDING_NATIVE).unwrap();
        assert_eq!(pending_in(&roots.data), None);

        assert!(resume_wipe(&roots.data, &roots.cache));
        assert!(!roots.data.join("audiobooks").exists());
        assert_eq!(
            pending_in(&roots.data),
            Some(AppResetPending {
                failed_paths: Vec::new()
            })
        );

        // A later launch must not erase what the user added before acknowledging.
        let imported = roots.data.join("document_uploads/new/source.pdf");
        put(&imported, b"x");
        assert!(!resume_wipe(&roots.data, &roots.cache));
        assert!(imported.exists());

        acknowledge_in(&roots.data).unwrap();
        assert!(!roots.data.join(MARKER_FILE).exists());
        assert_eq!(pending_in(&roots.data), None);
    }

    #[test]
    fn acknowledge_cannot_cancel_a_pending_native_wipe() {
        let roots = Roots::new();
        write_marker(&roots.data, PENDING_NATIVE).unwrap();

        acknowledge_in(&roots.data).unwrap();

        assert_eq!(read_marker(&roots.data), Some(Marker::PendingNative));
    }

    #[cfg(unix)]
    #[test]
    fn partial_failure_is_reported_and_never_retried() {
        use std::os::unix::fs::PermissionsExt;

        let roots = Roots::new();
        let locked = roots.data.join("document_uploads");
        let stuck = locked.join("abc/source.pdf");
        put(&stuck, b"x");
        put(&roots.data.join("audiobooks/abc/manifest.json"), b"x");
        fs::set_permissions(locked.join("abc"), fs::Permissions::from_mode(0o500)).unwrap();
        write_marker(&roots.data, PENDING_NATIVE).unwrap();

        assert!(resume_wipe(&roots.data, &roots.cache));

        assert!(!roots.data.join("audiobooks").exists());
        assert_eq!(
            pending_in(&roots.data),
            Some(AppResetPending {
                failed_paths: vec![locked.display().to_string()]
            })
        );
        assert!(!resume_wipe(&roots.data, &roots.cache));
        fs::set_permissions(locked.join("abc"), fs::Permissions::from_mode(0o700)).unwrap();
        assert!(stuck.exists());
    }

    #[test]
    fn rerunning_a_wipe_over_empty_roots_succeeds() {
        let roots = Roots::new();
        assert!(wipe_in(&roots.data, &roots.cache).is_empty());
        assert!(wipe_in(&roots.data, &roots.cache).is_empty());
    }

    #[test]
    fn summary_counts_items_and_all_erasable_bytes() {
        let roots = Roots::new();
        put(
            &roots.data.join("document_uploads/one/source.pdf"),
            b"12345",
        );
        put(&roots.data.join("document_uploads/two/source.html"), b"12");
        put(&roots.data.join("document_uploads/.three.deleting/x"), b"1");
        put(&roots.data.join("document_uploads/search.sqlite3"), b"1");
        put(&roots.data.join("audiobooks/one/manifest.json"), b"1");
        put(&roots.cache.join("voice-previews/preview.wav"), b"1");
        put(&roots.data.join("EBWebView/excluded"), b"1234567890");

        assert_eq!(
            summary_in(&roots.data, &roots.cache, Some("audio")),
            AppResetSummary {
                documents: 2,
                audiobooks: 1,
                total_bytes: 11,
                locations: vec![
                    roots.data.display().to_string(),
                    roots.cache.display().to_string(),
                ],
                blocked_by: Some("audio"),
            }
        );
    }
}
