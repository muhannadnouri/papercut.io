//! Local outline metadata shares the upload directory's write, rollback, and delete lifecycle.
use std::{
    fs,
    io::{ErrorKind, Read},
    path::Path,
};

use super::super::parsed::OutlineEntry;
use super::outline::{MAX_DEPTH, MAX_ENTRIES, MAX_LABEL_BYTES};

const FILE_NAME: &str = "outline.json";
// Allow JSON escaping and entry fields for a navigation source bounded at 4 MiB.
const MAX_JSON_BYTES: usize = 32 * 1024 * 1024;

/// Omit empty outlines, including when repairing a partially missing upload.
pub(in crate::document_uploads) fn store_outline(
    dir: &Path,
    outline: &[OutlineEntry],
) -> Result<(), String> {
    let path = dir.join(FILE_NAME);
    if outline.is_empty() {
        return match fs::remove_file(path) {
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
            result => result.map_err(|e| format!("Failed to remove stale EPUB outline: {e}")),
        };
    }
    let bytes = serde_json::to_vec(outline).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_JSON_BYTES {
        return Err("EPUB outline exceeds storage limit".into());
    }
    fs::write(path, bytes).map_err(|e| format!("Failed to write EPUB outline: {e}"))
}

/// A missing sidecar is normal. Return other errors so the reader can log them and still open the HTML.
pub(in crate::document_uploads) fn read_outline(dir: &Path) -> Result<Vec<OutlineEntry>, String> {
    let path = dir.join(FILE_NAME);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(value) => value,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    if !metadata.is_file() || metadata.len() > MAX_JSON_BYTES as u64 {
        return Err("Invalid EPUB outline file".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .and_then(|f| f.take(MAX_JSON_BYTES as u64 + 1).read_to_end(&mut bytes))
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_JSON_BYTES {
        return Err("EPUB outline exceeds storage limit".into());
    }
    let entries: Vec<OutlineEntry> = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let mut remaining = MAX_ENTRIES;
    if !valid_entries(&entries, 1, &mut remaining) {
        return Err("Invalid EPUB outline entries".into());
    }
    Ok(entries)
}

/// Recheck parser bounds and the generated reader-ID grammar before exposing stored JSON.
fn valid_entries(entries: &[OutlineEntry], depth: usize, remaining: &mut usize) -> bool {
    if entries.is_empty() {
        return true;
    }
    if depth > MAX_DEPTH || entries.len() > *remaining {
        return false;
    }
    *remaining -= entries.len();
    entries.iter().all(|entry| {
        !entry.label.trim().is_empty()
            && entry.label.len() <= MAX_LABEL_BYTES
            && match &entry.target {
                None => !entry.children.is_empty(),
                Some(id) => valid_target(id),
            }
            && valid_entries(&entry.children, depth + 1, remaining)
    })
}

/// Imports generate IDs such as `chapter-0` and `ch0-valley`. The leading hash belongs to the reader link.
fn valid_target(id: &str) -> bool {
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if let Some(index) = id.strip_prefix("chapter-") {
        return digits(index);
    }
    id.strip_prefix("ch")
        .and_then(|s| s.split_once('-'))
        .is_some_and(|(index, anchor)| {
            digits(index)
                && !anchor.is_empty()
                && anchor
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b':' | b'.'))
        })
}
