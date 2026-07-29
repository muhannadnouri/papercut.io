//! Versioned, bounded per-page text layers produced by PDF.js.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const SCHEMA_VERSION: u32 = 1;
const PAGE_TEXT_DIR: &str = "page-text";
const MAX_LAYER_BYTES: u64 = 4 * 1024 * 1024;
const MAX_BLOCKS: usize = 50_000;
const MAX_TEXT_BYTES: usize = 2 * 1024 * 1024;

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
    fs::write(&path, bytes).map_err(|err| format!("Failed to store PDF page text: {err}"))
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
    fn page_text_layer_round_trips_and_rejects_invalid_geometry() {
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

        let mut invalid = layer;
        invalid.width = f32::NAN;
        assert!(write_page_text_layer(&dir, &invalid).is_err());
        fs::remove_dir_all(dir).expect("clean test storage");
    }
}
