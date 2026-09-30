//! Optional EPUB asset loading for generated reading HTML.

use std::collections::{HashMap, HashSet};
use std::io::Read;

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use kuchikiki::{parse_html, traits::TendrilSink};
use zip::ZipArchive;

use crate::document_uploads::parsed::ParsedDocumentAsset;

pub(super) const MAX_IMAGE_BYTES: u64 = 5 * 1024 * 1024;
const MAX_TOTAL_IMAGE_BYTES: u64 = 100 * 1024 * 1024;

#[derive(Default)]
pub(super) struct LoadedImageAssets {
    pub(super) paths: HashMap<String, String>,
    pub(super) errors: HashMap<String, &'static str>,
    pub(super) files: Vec<ParsedDocumentAsset>,
    stored_names: HashSet<String>,
    total_bytes: u64,
}

impl LoadedImageAssets {
    pub(super) fn insert(&mut self, path: String, asset: ParsedDocumentAsset) -> bool {
        let size = asset.bytes.len() as u64;
        let is_new = !self.stored_names.contains(&asset.file_name);
        if size > MAX_IMAGE_BYTES || (is_new && size > MAX_TOTAL_IMAGE_BYTES - self.total_bytes) {
            return false;
        }
        self.paths.insert(path, asset.file_name.clone());
        if is_new {
            self.total_bytes += size;
            self.stored_names.insert(asset.file_name.clone());
            self.files.push(asset);
        }
        true
    }
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

/// Return the media type and fixed cover name for a supported image format.
pub(super) fn image_format(media_type: &str, href: &str) -> Option<(&'static str, &'static str)> {
    let lower_href = href.to_ascii_lowercase();
    match media_type {
        "image/png" => Some(("image/png", "cover.png")),
        "image/jpeg" | "image/jpg" => Some(("image/jpeg", "cover.jpg")),
        "image/gif" => Some(("image/gif", "cover.gif")),
        "image/webp" => Some(("image/webp", "cover.webp")),
        "image/svg+xml" => Some(("image/svg+xml", "cover.svg")),
        _ if lower_href.ends_with(".png") => Some(("image/png", "cover.png")),
        _ if lower_href.ends_with(".jpg") || lower_href.ends_with(".jpeg") => {
            Some(("image/jpeg", "cover.jpg"))
        }
        _ if lower_href.ends_with(".gif") => Some(("image/gif", "cover.gif")),
        _ if lower_href.ends_with(".webp") => Some(("image/webp", "cover.webp")),
        _ if lower_href.ends_with(".svg") => Some(("image/svg+xml", "cover.svg")),
        _ => None,
    }
}

/// Read a declared raster cover; thumbnail generation does not decode SVG.
pub(super) fn load_cover_asset<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    item: Option<&ManifestItem>,
) -> Option<LoadedCover> {
    let item = item?;
    let (media_type, file_name) = image_format(&item.media_type, &item.href)?;
    if media_type == "image/svg+xml" {
        return None;
    }
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
pub(super) fn read_zip_bytes_limited<R: Read + std::io::Seek>(
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

/// Retain referenced local images under generated content-hash names.
///
/// SVG stays external and is loaded only through an HTML img, whose image
/// processing mode disables scripts and external resources. Never inline these
/// bytes into the reader DOM or embed them as an object/frame.
///
/// Reading HTML stores only those generated names. Per-file and aggregate caps
/// bound hostile archives without letting unused manifest items crowd out images
/// that actually appear in retained chapters.
pub(super) fn load_image_assets<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    manifest: &[ManifestItem],
    referenced_paths: &HashSet<String>,
    assets: &mut LoadedImageAssets,
) {
    for item in manifest {
        if !referenced_paths.contains(&item.href) {
            continue;
        }
        let Some((media_type, _)) = image_format(&item.media_type, &item.href) else {
            assets.errors.insert(item.href.clone(), "unsupported");
            continue;
        };
        let size = archive.by_name(&item.href).map(|file| file.size());
        let reason = match size {
            Err(_) => Some("missing"),
            Ok(size) if size > MAX_IMAGE_BYTES => Some("size-limit"),
            Ok(size) if size > MAX_TOTAL_IMAGE_BYTES - assets.total_bytes => Some("total-limit"),
            _ => None,
        };
        if let Some(reason) = reason {
            assets.errors.insert(item.href.clone(), reason);
            continue;
        }
        let Some(bytes) = read_zip_bytes_limited(archive, &item.href, MAX_IMAGE_BYTES) else {
            assets.errors.insert(item.href.clone(), "unavailable");
            continue;
        };
        let Some(asset) = ParsedDocumentAsset::new(media_type, bytes) else {
            assets.errors.insert(item.href.clone(), "unavailable");
            continue;
        };
        if !assets.insert(item.href.clone(), asset) {
            assets.errors.insert(item.href.clone(), "total-limit");
        }
    }
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

/// Recognize base64 raster data URLs shared by legacy imports and SVG images;
/// callers enforce decoding and size limits. This is not a general URL parser.
pub(super) fn inline_raster_parts(value: &str) -> Option<(&'static str, &str)> {
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
    fn manifest_omissions_record_reasons() {
        use std::io::{Cursor, Write};
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, size) in [
            ("huge.png", MAX_IMAGE_BYTES as usize + 1),
            ("budget.png", 1),
            ("empty.png", 0),
        ] {
            zip.start_file(name, zip::write::FileOptions::default())
                .unwrap();
            zip.write_all(&vec![0; size]).unwrap();
        }
        let mut archive = ZipArchive::new(zip.finish().unwrap()).unwrap();
        let manifest: Vec<_> = [
            "missing.png",
            "huge.png",
            "budget.png",
            "empty.png",
            "photo.bmp",
        ]
        .into_iter()
        .map(|href| ManifestItem {
            href: href.into(),
            media_type: String::new(),
        })
        .collect();
        let referenced = manifest.iter().map(|item| item.href.clone()).collect();
        let mut assets = LoadedImageAssets::default();
        assets.total_bytes = MAX_TOTAL_IMAGE_BYTES;
        load_image_assets(&mut archive, &manifest, &referenced, &mut assets);
        assert!(assets.files.is_empty());
        for (path, reason) in [
            ("missing.png", "missing"),
            ("huge.png", "size-limit"),
            ("budget.png", "total-limit"),
            ("empty.png", "unavailable"),
            ("photo.bmp", "unsupported"),
        ] {
            assert_eq!(assets.errors.get(path), Some(&reason));
        }
    }

    #[test]
    fn inline_and_manifest_assets_share_limits_and_deduplication() {
        let mut assets = LoadedImageAssets::default();
        let image = || ParsedDocumentAsset::new("image/svg+xml", b"<svg/>".to_vec()).unwrap();
        assert!(assets.insert("inline.svg".into(), image()));
        assert!(assets.insert("manifest.svg".into(), image()));
        assert_eq!(assets.files.len(), 1);
        assert_eq!(assets.total_bytes, 6);
        // Exercise the aggregate boundary without allocating a 100 MB fixture.
        assets.total_bytes = MAX_TOTAL_IMAGE_BYTES;
        assert!(assets.insert("duplicate.svg".into(), image()));
        assert!(!assets.insert(
            "new.svg".into(),
            ParsedDocumentAsset::new("image/svg+xml", b"<svg />".to_vec()).unwrap()
        ));
        let mut assets = LoadedImageAssets::default();
        assert!(!assets.insert(
            "large.svg".into(),
            ParsedDocumentAsset::new("image/svg+xml", vec![b' '; MAX_IMAGE_BYTES as usize + 1])
                .unwrap()
        ));
        assert!(assets.paths.is_empty());
    }

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
