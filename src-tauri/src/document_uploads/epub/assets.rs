//! Optional EPUB asset loading for generated reading HTML.

use std::collections::{HashMap, HashSet};
use std::io::Read;

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use kuchikiki::{parse_html, traits::TendrilSink};
use zip::ZipArchive;

use crate::document_uploads::parsed::ParsedDocumentAsset;

const MAX_IMAGE_BYTES: u64 = 5 * 1024 * 1024;
const MAX_TOTAL_IMAGE_BYTES: u64 = 100 * 1024 * 1024;

pub(super) struct LoadedImageAssets {
    pub(super) paths: HashMap<String, String>,
    pub(super) files: Vec<ParsedDocumentAsset>,
}

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
    let href = href.to_ascii_lowercase();
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
    let lower_href = href.to_ascii_lowercase();
    match media_type {
        "image/png" => Some(("image/png", "cover.png")),
        "image/jpeg" | "image/jpg" => Some(("image/jpeg", "cover.jpg")),
        "image/gif" => Some(("image/gif", "cover.gif")),
        "image/webp" => Some(("image/webp", "cover.webp")),
        _ if lower_href.ends_with(".png") => Some(("image/png", "cover.png")),
        _ if lower_href.ends_with(".jpg") || lower_href.ends_with(".jpeg") => {
            Some(("image/jpeg", "cover.jpg"))
        }
        _ if lower_href.ends_with(".gif") => Some(("image/gif", "cover.gif")),
        _ if lower_href.ends_with(".webp") => Some(("image/webp", "cover.webp")),
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

/// Retain referenced local raster images under generated content-hash names.
///
/// Reading HTML stores only those generated names. Per-file and aggregate caps
/// bound hostile archives without letting unused manifest items crowd out images
/// that actually appear in retained chapters.
pub(super) fn load_image_assets<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    manifest: &[ManifestItem],
    referenced_paths: &HashSet<String>,
) -> LoadedImageAssets {
    let mut paths = HashMap::new();
    let mut files = Vec::new();
    let mut stored_names = HashSet::new();
    let mut total = 0u64;
    for item in manifest {
        if !referenced_paths.contains(&item.href)
            || !is_supported_image_item(&item.media_type, &item.href)
        {
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
        let Some(asset) = ParsedDocumentAsset::new(media_type, bytes) else {
            continue;
        };
        paths.insert(item.href.clone(), asset.file_name.clone());
        if stored_names.insert(asset.file_name.clone()) {
            total += asset.bytes.len() as u64;
            files.push(asset);
        }
    }
    LoadedImageAssets { paths, files }
}

/// Convert bounded legacy inline raster data into the current stored-asset form.
pub(crate) fn externalize_inline_image_assets(html: &str) -> (String, Vec<ParsedDocumentAsset>) {
    if !html.contains("data:image/") {
        return (html.to_string(), Vec::new());
    }
    let document = parse_html().one(html).document_node;
    let mut files = Vec::new();
    let mut stored_names = HashSet::new();
    let mut total = 0u64;
    let Ok(images) = document.select("img[src]") else {
        return (html.to_string(), Vec::new());
    };
    for image in images {
        let mut attrs = image.attributes.borrow_mut();
        let Some(src) = attrs.get("src").map(ToOwned::to_owned) else {
            continue;
        };
        let Some((media_type, encoded)) = inline_raster_parts(&src) else {
            continue;
        };
        let max_encoded = (MAX_IMAGE_BYTES as usize).saturating_mul(4) / 3 + 4;
        let asset = (encoded.len() <= max_encoded)
            .then(|| BASE64_STANDARD.decode(encoded).ok())
            .flatten()
            .filter(|bytes| !bytes.is_empty() && bytes.len() as u64 <= MAX_IMAGE_BYTES)
            .and_then(|bytes| ParsedDocumentAsset::new(media_type, bytes));
        let Some(asset) = asset else {
            attrs.remove("src");
            continue;
        };
        let is_new = stored_names.insert(asset.file_name.clone());
        let next_total = total.saturating_add(asset.bytes.len() as u64);
        if is_new && next_total > MAX_TOTAL_IMAGE_BYTES {
            attrs.remove("src");
            continue;
        }
        attrs.remove("src");
        attrs.insert("data-papercut-asset", asset.file_name.clone());
        attrs.insert("loading", "lazy".into());
        attrs.insert("decoding", "async".into());
        if is_new {
            total = next_total;
            files.push(asset);
        }
    }
    let mut bytes = Vec::new();
    if document.serialize(&mut bytes).is_err() {
        return (html.to_string(), Vec::new());
    }
    (
        String::from_utf8(bytes).unwrap_or_else(|_| html.to_string()),
        files,
    )
}

/// Recognize only the legacy data-URL forms the previous importer generated;
/// this is migration compatibility, not a general data-URL parser.
fn inline_raster_parts(value: &str) -> Option<(&'static str, &str)> {
    let (header, encoded) = value.trim().split_once(',')?;
    let media_type = match header.to_ascii_lowercase().as_str() {
        "data:image/png;base64" => "image/png",
        "data:image/jpeg;base64" => "image/jpeg",
        "data:image/gif;base64" => "image/gif",
        "data:image/webp;base64" => "image/webp",
        _ => return None,
    };
    Some((media_type, encoded))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn externalizes_and_deduplicates_legacy_inline_images() {
        let encoded = BASE64_STANDARD.encode(b"small png");
        let html = format!(
            "<html><body><img src=\"data:image/png;base64,{encoded}\"><img src=\"data:image/png;base64,{encoded}\"></body></html>"
        );

        let (rewritten, files) = externalize_inline_image_assets(&html);

        assert_eq!(files.len(), 1);
        assert_eq!(rewritten.matches("data-papercut-asset").count(), 2);
        assert_eq!(rewritten.matches("loading=\"lazy\"").count(), 2);
        assert!(!rewritten.contains("base64"));
    }
}
