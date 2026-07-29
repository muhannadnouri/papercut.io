//! Display-sized thumbnail generation for retained document covers.

use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use image::{ImageFormat, ImageReader, Limits};

pub(super) const THUMBNAIL_FILE_NAME: &str = "cover-thumbnail.png";
pub(super) const THUMBNAIL_MEDIA_TYPE: &str = "image/png";
pub(super) const PNG_COVER_FILE_NAME: &str = "cover.png";

const THUMBNAIL_MAX_WIDTH: u32 = 480;
const THUMBNAIL_MAX_HEIGHT: u32 = 720;
const MAX_SOURCE_BYTES: u64 = 5 * 1024 * 1024;
const MAX_SOURCE_DIMENSION: u32 = 12_000;
const MAX_SOURCE_PIXELS: u64 = 40_000_000;
const MAX_DECODE_BYTES: u64 = 192 * 1024 * 1024;

static THUMBNAIL_BACKFILL: OnceLock<Mutex<()>> = OnceLock::new();

/// Persist a display-sized PNG instead of making the gallery decode the original cover.
pub(super) fn write_thumbnail(dir: &Path, source: &[u8]) -> Result<(), String> {
    let bytes = thumbnail_bytes(source)?;
    write_thumbnail_bytes(dir, &bytes)
}

/// Store a PDF.js-rendered first page as both the validated cover source and
/// the ready-to-serve gallery thumbnail used by the shared cover pipeline.
pub(super) fn write_pdf_thumbnail(dir: &Path, source: &[u8]) -> Result<(), String> {
    let bytes = thumbnail_bytes(source)?;
    write_thumbnail_bytes(dir, &bytes)?;
    let path = dir.join(PNG_COVER_FILE_NAME);
    let staging = dir.join(".cover.png.tmp");
    fs::write(&staging, bytes)
        .map_err(|error| format!("Failed to write imported PDF cover: {error}"))?;
    fs::rename(&staging, &path).map_err(|error| {
        let _ = fs::remove_file(&staging);
        format!("Failed to store imported PDF cover: {error}")
    })
}

fn write_thumbnail_bytes(dir: &Path, bytes: &[u8]) -> Result<(), String> {
    let path = dir.join(THUMBNAIL_FILE_NAME);
    let staging = dir.join(".cover-thumbnail.png.tmp");
    fs::write(&staging, bytes)
        .map_err(|error| format!("Failed to write imported document cover thumbnail: {error}"))?;
    fs::rename(&staging, &path).map_err(|error| {
        let _ = fs::remove_file(&staging);
        format!("Failed to store imported document cover thumbnail: {error}")
    })
}

/// Lazily create one missing thumbnail at a time so existing large covers cannot
/// produce a burst of concurrent full-resolution decodes when the gallery opens.
pub(super) fn backfill_thumbnail(dir: &Path, source_path: &Path) -> Result<(), String> {
    let _guard = THUMBNAIL_BACKFILL
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| "Document cover thumbnail lock was poisoned".to_string())?;
    let thumbnail_path = dir.join(THUMBNAIL_FILE_NAME);
    if thumbnail_path.is_file() {
        return Ok(());
    }
    let mut file = fs::File::open(source_path).map_err(|error| {
        format!(
            "Failed to open uploaded document cover {}: {error}",
            source_path.display()
        )
    })?;
    let mut source = Vec::new();
    file.by_ref()
        .take(MAX_SOURCE_BYTES + 1)
        .read_to_end(&mut source)
        .map_err(|error| format!("Failed to read uploaded document cover: {error}"))?;
    if source.len() as u64 > MAX_SOURCE_BYTES {
        return Err("Uploaded document cover exceeds the 5 MB limit".into());
    }
    write_thumbnail(dir, &source)
}

/// Validate dimensions before decoding, then encode one bounded static PNG frame.
fn thumbnail_bytes(source: &[u8]) -> Result<Vec<u8>, String> {
    if source.len() as u64 > MAX_SOURCE_BYTES {
        return Err("Imported document cover exceeds the 5 MB limit".into());
    }
    let reader = ImageReader::new(Cursor::new(source))
        .with_guessed_format()
        .map_err(|error| format!("Failed to identify imported document cover: {error}"))?;
    let (width, height) = reader
        .into_dimensions()
        .map_err(|error| format!("Failed to inspect imported document cover: {error}"))?;
    if width == 0
        || height == 0
        || u64::from(width)
            .checked_mul(u64::from(height))
            .map_or(true, |pixels| pixels > MAX_SOURCE_PIXELS)
    {
        return Err("Imported document cover has unsupported dimensions".into());
    }

    let mut reader = ImageReader::new(Cursor::new(source))
        .with_guessed_format()
        .map_err(|error| format!("Failed to identify imported document cover: {error}"))?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SOURCE_DIMENSION);
    limits.max_image_height = Some(MAX_SOURCE_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|error| format!("Failed to decode imported document cover: {error}"))?;
    let thumbnail = image.thumbnail(THUMBNAIL_MAX_WIDTH, THUMBNAIL_MAX_HEIGHT);
    let mut output = Cursor::new(Vec::new());
    thumbnail
        .write_to(&mut output, ImageFormat::Png)
        .map_err(|error| format!("Failed to encode imported document cover thumbnail: {error}"))?;
    Ok(output.into_inner())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use image::{DynamicImage, GenericImageView, ImageFormat};

    use super::{thumbnail_bytes, THUMBNAIL_MAX_HEIGHT, THUMBNAIL_MAX_WIDTH};

    #[test]
    fn thumbnail_is_bounded_to_gallery_dimensions() {
        let source = DynamicImage::new_rgb8(1200, 1800);
        let mut encoded = Cursor::new(Vec::new());
        source.write_to(&mut encoded, ImageFormat::Jpeg).unwrap();

        let thumbnail = image::load_from_memory(&thumbnail_bytes(encoded.get_ref()).unwrap())
            .expect("decode generated thumbnail");

        assert_eq!(
            thumbnail.dimensions(),
            (THUMBNAIL_MAX_WIDTH, THUMBNAIL_MAX_HEIGHT)
        );
    }
}
