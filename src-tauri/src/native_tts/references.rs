//! Lightweight saved-audiobook dependency checks shared with document storage.

use std::fs;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use serde::Deserialize;
use tauri::{Manager, Runtime};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AudiobookDocumentReference {
    document_url: String,
}

// ponytail: One process-wide lock is enough while these transactions stay brief;
// split by document only if concurrent save/delete startup becomes measurable.
static AUDIOBOOK_REFERENCE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// Serialize the short source-reference publication and deletion transactions.
///
/// Generation intentionally runs outside this lock: only the pending manifest
/// needs to become visible atomically with source validation.
pub(crate) fn with_audiobook_reference_lock<T>(
    operation: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let _guard = AUDIOBOOK_REFERENCE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| "Audiobook source-reference lock poisoned".to_string())?;
    operation()
}

/// Detect whether any manifest still depends on a document before deletion.
///
/// This intentionally reads pending as well as completed manifests: deleting a
/// source while its audiobook is being saved would create the same orphan as
/// deleting a source after completion. Unreadable manifests are already unusable
/// and are skipped without preventing unrelated Library cleanup.
pub(crate) fn document_has_audiobook_reference<R: Runtime>(
    app: &tauri::AppHandle<R>,
    document_url: &str,
) -> Result<bool, String> {
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|err| format!("Failed to resolve app data for audiobook references: {err}"))?;
    has_reference(&app_data.join("audiobooks"), document_url)
}

fn has_reference(root: &Path, document_url: &str) -> Result<bool, String> {
    if !root.is_dir() {
        return Ok(false);
    }

    let entries = fs::read_dir(root)
        .map_err(|err| format!("Failed to inspect saved audiobook references: {err}"))?;
    for entry in entries.flatten() {
        let path = entry.path().join("manifest.json");
        if !path.is_file() {
            continue;
        }
        let reference = match fs::read(&path)
            .map_err(|err| err.to_string())
            .and_then(|bytes| {
                serde_json::from_slice::<AudiobookDocumentReference>(&bytes)
                    .map_err(|err| err.to_string())
            }) {
            Ok(reference) => reference,
            Err(err) => {
                log::warn!(
                    "Ignoring unreadable audiobook reference {}: {err}",
                    path.display()
                );
                continue;
            }
        };
        if reference.document_url == document_url {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn finds_only_matching_readable_manifests() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("papercut-audiobook-refs-{nonce}"));
        for (name, manifest) in [
            ("matching-a", r#"{"documentUrl":"/uploads/a.html"}"#),
            ("matching-b", r#"{"documentUrl":"/uploads/a.html"}"#),
            ("other", r#"{"documentUrl":"/uploads/b.html"}"#),
            ("broken", "{"),
        ] {
            let dir = root.join(name);
            fs::create_dir_all(&dir).expect("create manifest dir");
            fs::write(dir.join("manifest.json"), manifest).expect("write manifest");
        }

        assert!(has_reference(&root, "/uploads/a.html").expect("find reference"));
        assert!(!has_reference(&root, "/uploads/missing.html").expect("find reference"));
        fs::remove_dir_all(root).expect("remove fixture");
    }
}
