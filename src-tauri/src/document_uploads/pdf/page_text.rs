//! Versioned, bounded per-page text layers produced by PDF.js.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde::{Deserialize, Serialize};

const SCHEMA_VERSION: u32 = 1;
const PAGE_TEXT_DIR: &str = "page-text";
const OCR_TEXT_MARKER_FILE: &str = ".has-ocr-text";
const MAX_LAYER_BYTES: u64 = 4 * 1024 * 1024;
const MAX_BLOCKS: usize = 50_000;
const MAX_TEXT_BYTES: usize = 2 * 1024 * 1024;

static NEXT_TEMP_FILE: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PageTextLayer {
    pub(crate) schema_version: u32,
    pub(crate) page_index: u32,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) blocks: Vec<PageTextBlock>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PageTextBlock {
    pub(crate) text: String,
    /// PDF.js viewport coordinates: x, y, width, height.
    pub(crate) bounds: [f32; 4],
    pub(crate) order: u32,
    pub(crate) confidence: Option<f32>,
}

/// Persist one rebuildable page layer. The per-page cap prevents malformed
/// extractor output from turning app data into an unbounded JSON cache.
pub(crate) fn write_page_text_layer(
    upload_dir: &Path,
    layer: &PageTextLayer,
) -> Result<(), String> {
    validate(layer)?;
    let bytes = serde_json::to_vec(layer)
        .map_err(|err| format!("Failed to serialize PDF page text: {err}"))?;
    if bytes.len() as u64 > MAX_LAYER_BYTES {
        return Err("PDF page text layer exceeds the 4 MB storage limit".into());
    }

    let path = layer_path(upload_dir, layer.page_index);
    let parent = path
        .parent()
        .ok_or_else(|| "PDF page text path is invalid".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|err| format!("Failed to create PDF page text directory: {err}"))?;
    write_atomically(&path, &bytes)
}

/// Read one page layer with a decompressed byte cap and revalidate every value;
/// these sidecars are derived data and callers may rebuild them on any failure.
pub(crate) fn read_page_text_layer(
    upload_dir: &Path,
    page_index: u32,
) -> Result<PageTextLayer, String> {
    let path = layer_path(upload_dir, page_index);
    let mut file =
        File::open(&path).map_err(|err| format!("Failed to open PDF page text: {err}"))?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(MAX_LAYER_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|err| format!("Failed to read PDF page text: {err}"))?;
    if bytes.len() as u64 > MAX_LAYER_BYTES {
        return Err("PDF page text layer exceeds the 4 MB storage limit".into());
    }
    let layer: PageTextLayer = serde_json::from_slice(&bytes)
        .map_err(|err| format!("Failed to parse PDF page text: {err}"))?;
    validate(&layer)?;
    if layer.page_index != page_index {
        return Err("PDF page text layer has the wrong page index".into());
    }
    Ok(layer)
}

fn layer_path(upload_dir: &Path, page_index: u32) -> PathBuf {
    upload_dir
        .join(PAGE_TEXT_DIR)
        .join(format!("{page_index:08}.json"))
}

/// Replace one derived sidecar only after its complete contents are staged.
/// A same-directory temporary file keeps the final rename atomic and leaves a
/// previous valid layer readable if writing or committing fails. These files
/// are rebuildable, so forcing a disk sync for every PDF page is unnecessary.
fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "PDF page text file name is invalid".to_string())?;
    let nonce = NEXT_TEMP_FILE.fetch_add(1, Ordering::Relaxed);
    let temporary =
        path.with_file_name(format!(".{file_name}.{}.{}.tmp", std::process::id(), nonce));

    let result = (|| {
        fs::write(&temporary, bytes)
            .map_err(|err| format!("Failed to stage PDF page text: {err}"))?;
        fs::rename(&temporary, path).map_err(|err| format!("Failed to commit PDF page text: {err}"))
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

/// Record whether finalized page sidecars contain OCR-derived text.
///
/// The viewer reads this tiny derived marker when choosing its Find adapter,
/// avoiding an O(page count) sidecar scan every time a PDF is opened.
pub(crate) fn sync_ocr_text_marker(upload_dir: &Path, present: bool) -> Result<(), String> {
    let path = upload_dir.join(PAGE_TEXT_DIR).join(OCR_TEXT_MARKER_FILE);
    if present {
        let parent = path
            .parent()
            .ok_or_else(|| "PDF OCR marker path is invalid".to_string())?;
        fs::create_dir_all(parent)
            .map_err(|err| format!("Failed to create PDF page text directory: {err}"))?;
        fs::write(path, []).map_err(|err| format!("Failed to store PDF OCR marker: {err}"))
    } else {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(format!("Failed to remove PDF OCR marker: {err}")),
        }
    }
}

/// Return the finalized OCR-presence decision without opening page sidecars.
pub(crate) fn has_ocr_text_marker(upload_dir: &Path) -> bool {
    upload_dir
        .join(PAGE_TEXT_DIR)
        .join(OCR_TEXT_MARKER_FILE)
        .is_file()
}

/// Enforce the sidecar trust boundary on both writes and reads. Derived files
/// may be stale or damaged, so a successful write in an older app version is
/// not treated as proof that the data remains valid.
fn validate(layer: &PageTextLayer) -> Result<(), String> {
    if layer.schema_version != SCHEMA_VERSION {
        return Err(format!(
            "Unsupported PDF page text schema version {}",
            layer.schema_version
        ));
    }
    if !layer.width.is_finite()
        || !layer.height.is_finite()
        || layer.width <= 0.0
        || layer.height <= 0.0
    {
        return Err("PDF page text dimensions are invalid".into());
    }
    if layer.blocks.len() > MAX_BLOCKS {
        return Err("PDF page text layer contains too many blocks".into());
    }
    let mut text_bytes = 0usize;
    for block in &layer.blocks {
        if block.bounds.iter().any(|value| !value.is_finite())
            || block.bounds[2] < 0.0
            || block.bounds[3] < 0.0
            || block
                .confidence
                .is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
        {
            return Err("PDF page text block metadata is invalid".into());
        }
        text_bytes = text_bytes
            .checked_add(block.text.len())
            .ok_or_else(|| "PDF page text layer is too large".to_string())?;
        if text_bytes > MAX_TEXT_BYTES {
            return Err("PDF page text layer contains too much text".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn page_text_layer_replaces_atomically_and_rejects_invalid_geometry() {
        let dir = std::env::temp_dir().join(format!(
            "papercut-pdf-layer-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let layer = PageTextLayer {
            schema_version: SCHEMA_VERSION,
            page_index: 3,
            width: 612.0,
            height: 792.0,
            blocks: vec![PageTextBlock {
                text: "Readable text".into(),
                bounds: [10.0, 20.0, 100.0, 12.0],
                order: 0,
                confidence: None,
            }],
        };

        write_page_text_layer(&dir, &layer).expect("write layer");
        assert_eq!(read_page_text_layer(&dir, 3).expect("read layer"), layer);

        let mut replacement = layer.clone();
        replacement.blocks[0].text = "Replacement text".into();
        write_page_text_layer(&dir, &replacement).expect("replace layer");
        assert_eq!(
            read_page_text_layer(&dir, 3).expect("read replacement"),
            replacement
        );

        let blocked_destination = dir.join(PAGE_TEXT_DIR).join("blocked.json");
        fs::create_dir(&blocked_destination).expect("create blocked destination");
        assert!(write_atomically(&blocked_destination, b"staged text").is_err());
        assert!(blocked_destination.is_dir());
        assert!(fs::read_dir(dir.join(PAGE_TEXT_DIR))
            .expect("read page text directory")
            .all(|entry| !entry
                .expect("read page text entry")
                .path()
                .to_string_lossy()
                .ends_with(".tmp")));

        assert!(!has_ocr_text_marker(&dir));
        sync_ocr_text_marker(&dir, true).expect("store OCR marker");
        assert!(has_ocr_text_marker(&dir));
        sync_ocr_text_marker(&dir, false).expect("remove OCR marker");
        assert!(!has_ocr_text_marker(&dir));

        let mut invalid = layer;
        invalid.width = f32::NAN;
        assert!(write_page_text_layer(&dir, &invalid).is_err());
        fs::remove_dir_all(dir).expect("clean test storage");
    }
}
