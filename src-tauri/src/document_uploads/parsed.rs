//! Format-neutral parsed document shape used by upload parsers, storage, search,
//! and TTS-facing source retrieval.

use sha2::{Digest, Sha256};

pub(crate) const READER_ASSET_DIR_NAME: &str = "assets";

/// A parsed, sanitized document ready to store and index.
pub(crate) struct ParsedDocument {
    pub(crate) title: String,
    pub(crate) format: String,
    pub(crate) view_html: String,
    pub(crate) sections: Vec<ParsedSection>,
    pub(crate) cover: Option<ParsedDocumentCover>,
    pub(crate) assets: Vec<ParsedDocumentAsset>,
}

/// One bounded raster cover selected from format metadata during import.
pub(crate) struct ParsedDocumentCover {
    pub(crate) media_type: &'static str,
    pub(crate) file_name: &'static str,
    pub(crate) bytes: Vec<u8>,
}

/// One generated-name raster resource owned by a sanitized reader document.
pub(crate) struct ParsedDocumentAsset {
    pub(crate) file_name: String,
    pub(crate) bytes: Vec<u8>,
}

impl ParsedDocumentAsset {
    /// Derive the only filename shape accepted by storage, transfer, and the
    /// frontend resolver. Content hashes deduplicate identical images and keep
    /// untrusted EPUB paths out of app-data and generated HTML.
    pub(crate) fn new(media_type: &str, bytes: Vec<u8>) -> Option<Self> {
        if bytes.is_empty() {
            return None;
        }
        let extension = match media_type {
            "image/png" => "png",
            "image/jpeg" => "jpg",
            "image/gif" => "gif",
            "image/webp" => "webp",
            _ => return None,
        };
        let digest = format!("{:x}", Sha256::digest(&bytes));
        Some(Self {
            file_name: format!("image-{digest}.{extension}"),
            bytes,
        })
    }
}

/// Validate a generated basename, not an arbitrary path. Keep this shared by
/// sanitizer, storage, and transfer so none of those boundaries drift apart.
pub(crate) fn is_reader_asset_file_name(value: &str) -> bool {
    let Some((digest, extension)) = value
        .strip_prefix("image-")
        .and_then(|rest| rest.rsplit_once('.'))
    else {
        return false;
    };
    digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && matches!(extension, "png" | "jpg" | "gif" | "webp")
}

/// One ordered readable section, optionally carrying the heading it falls under.
pub(crate) struct ParsedSection {
    pub(crate) heading: Option<String>,
    pub(crate) text: String,
    pub(crate) page_index: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reader_asset_names_are_generated_and_path_free() {
        let asset = ParsedDocumentAsset::new("image/png", b"image".to_vec()).expect("asset");

        assert!(is_reader_asset_file_name(&asset.file_name));
        assert!(!is_reader_asset_file_name(&format!(
            "image-{}.png",
            "A".repeat(64)
        )));
        assert!(!is_reader_asset_file_name(&format!(
            "../image-{}.png",
            "a".repeat(64)
        )));
        assert!(ParsedDocumentAsset::new("image/svg+xml", b"svg".to_vec()).is_none());
        assert!(ParsedDocumentAsset::new("image/png", Vec::new()).is_none());
    }
}
