//! Optional EPUB asset loading for generated reading HTML.

use std::collections::HashMap;
use std::io::Read;

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use zip::ZipArchive;

const MAX_IMAGE_BYTES: u64 = 5 * 1024 * 1024;
const MAX_TOTAL_IMAGE_BYTES: u64 = 30 * 1024 * 1024;

pub(super) struct LoadedCover {
    pub(super) media_type: &'static str,
    pub(super) file_name: &'static str,
    pub(super) bytes: Vec<u8>,
}

#[derive(Clone)]
pub(super) struct ManifestItem {
    pub(super) href: String,
    pub(super) media_type: String,
}

pub(super) fn is_supported_image_item(media_type: &str, href: &str) -> bool {
    matches!(
        media_type,
        "image/png" | "image/jpeg" | "image/jpg" | "image/gif" | "image/webp"
    ) || href.ends_with(".png")
        || href.ends_with(".jpg")
        || href.ends_with(".jpeg")
        || href.ends_with(".gif")
        || href.ends_with(".webp")
}

/// Return the safe media type and fixed stored-cover name for an allowed image.
///
/// SVG is intentionally excluded: it can carry active content and is harder to
/// sanitize correctly than the raster formats we need for current EPUB covers and
/// illustrations.
fn image_format(media_type: &str, href: &str) -> Option<(&'static str, &'static str)> {
    match media_type {
        "image/png" => Some(("image/png", "cover.png")),
        "image/jpeg" | "image/jpg" => Some(("image/jpeg", "cover.jpg")),
        "image/gif" => Some(("image/gif", "cover.gif")),
        "image/webp" => Some(("image/webp", "cover.webp")),
        _ if href.ends_with(".png") => Some(("image/png", "cover.png")),
        _ if href.ends_with(".jpg") || href.ends_with(".jpeg") => Some(("image/jpeg", "cover.jpg")),
        _ if href.ends_with(".gif") => Some(("image/gif", "cover.gif")),
        _ if href.ends_with(".webp") => Some(("image/webp", "cover.webp")),
        _ => None,
    }
}

/// Read a declared EPUB cover through the same raster allowlist and size cap used
/// for reader images. Invalid, active, or oversized cover assets are simply absent.
pub(super) fn load_cover_asset<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    item: Option<&ManifestItem>,
) -> Option<LoadedCover> {
    let item = item?;
    let (media_type, file_name) = image_format(&item.media_type, &item.href)?;
    let bytes = read_zip_bytes_limited(archive, &item.href, MAX_IMAGE_BYTES)?;
    Some(LoadedCover {
        media_type,
        file_name,
        bytes,
    })
}

/// Read a binary ZIP member only when its declared and actual size fit a cap.
///
/// The extra-byte read protects against entries whose metadata understates size.
/// Returning `None` makes oversized or unreadable optional assets skippable.
fn read_zip_bytes_limited<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    path: &str,
    max_bytes: u64,
) -> Option<Vec<u8>> {
    let mut file = archive.by_name(path).ok()?;
    if file.size() > max_bytes {
        return None;
    }
    let mut bytes = Vec::with_capacity(file.size() as usize);
    file.by_ref()
        .take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() as u64 <= max_bytes).then_some(bytes)
}

/// Inline safe local raster images as data URLs for the generated reader.
///
/// The original EPUB is not retained, so local image references would otherwise
/// break after import. Per-image and total caps keep hostile or huge EPUBs from
/// exploding stored HTML size.
pub(super) fn load_image_assets<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    manifest: &[ManifestItem],
) -> HashMap<String, String> {
    let mut assets = HashMap::new();
    let mut total = 0u64;
    for item in manifest {
        if !is_supported_image_item(&item.media_type, &item.href) {
            continue;
        }
        let Some((media_type, _)) = image_format(&item.media_type, &item.href) else {
            continue;
        };
        if total >= MAX_TOTAL_IMAGE_BYTES {
            break;
        }
        let remaining = MAX_TOTAL_IMAGE_BYTES - total;
        let cap = MAX_IMAGE_BYTES.min(remaining);
        let Some(bytes) = read_zip_bytes_limited(archive, &item.href, cap) else {
            continue;
        };
        total += bytes.len() as u64;
        assets.insert(
            item.href.clone(),
            format!(
                "data:{media_type};base64,{}",
                BASE64_STANDARD.encode(&bytes)
            ),
        );
    }
    assets
}
