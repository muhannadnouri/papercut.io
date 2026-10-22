//! Snapshot referenced reader images before a derived-document job starts.
//! Only canonical, content-addressed assets cross this storage boundary.

use kuchikiki::{parse_html, traits::TendrilSink};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeSet, HashMap},
    fs,
    io::Read,
};

use super::parsed::{is_reader_asset_file_name, ParsedDocumentAsset};
use super::pipeline::{MAX_STORED_READER_ASSET_BYTES, MAX_TOTAL_READER_ASSET_BYTES};

pub(super) fn referenced_assets(html: &str) -> Result<BTreeSet<String>, String> {
    let document = parse_html().one(html).document_node;
    let mut names = BTreeSet::new();
    for image in document
        .select("img[data-papercut-asset]")
        .map_err(|_| "Invalid asset selector")?
    {
        let attrs = image.attributes.borrow();
        let name = attrs
            .get("data-papercut-asset")
            .ok_or("Missing reader image name")?;
        if !is_reader_asset_file_name(name) {
            return Err("Invalid reader image name".into());
        }
        names.insert(name.to_string());
    }
    Ok(names)
}

fn validate_asset(asset: &ParsedDocumentAsset, total: &mut u64) -> Result<(), String> {
    if !is_reader_asset_file_name(&asset.file_name)
        || asset.bytes.is_empty()
        || asset.bytes.len() as u64 > MAX_STORED_READER_ASSET_BYTES
    {
        return Err("Reader image is invalid or exceeds the size limit".into());
    }
    let digest = format!("{:x}", Sha256::digest(&asset.bytes));
    if asset
        .file_name
        .strip_prefix("image-")
        .and_then(|s| s.split_once('.'))
        .map(|p| p.0)
        != Some(digest.as_str())
    {
        return Err("Reader image content does not match its hash".into());
    }
    *total = total
        .checked_add(asset.bytes.len() as u64)
        .ok_or("Reader image size overflow")?;
    if *total > MAX_TOTAL_READER_ASSET_BYTES {
        return Err("Reader images exceed the total size limit".into());
    }
    Ok(())
}

pub(super) fn snapshot_assets(
    html: &str,
    paths: &HashMap<String, String>,
) -> Result<Vec<ParsedDocumentAsset>, String> {
    snapshot_assets_with_evidence(html, None, paths)
}

/// Source-only images stay owned after the English reading stream no longer embeds its original.
pub(crate) fn snapshot_assets_with_evidence(
    html: &str,
    source_evidence: Option<&str>,
    paths: &HashMap<String, String>,
) -> Result<Vec<ParsedDocumentAsset>, String> {
    let mut total = 0;
    let mut names = referenced_assets(html)?;
    if let Some(source) = source_evidence {
        names.extend(referenced_assets(source)?);
    }
    names
        .into_iter()
        .map(|name| {
            let path = paths
                .get(&name)
                .ok_or("Referenced reader image is missing")?;
            let metadata = fs::symlink_metadata(path)
                .map_err(|e| format!("Cannot inspect reader image: {e}"))?;
            if !metadata.is_file() || metadata.len() > MAX_STORED_READER_ASSET_BYTES {
                return Err("Reader image is not a bounded regular file".into());
            }
            let mut bytes = Vec::new();
            fs::File::open(path)
                .map_err(|e| format!("Cannot open reader image: {e}"))?
                .take(MAX_STORED_READER_ASSET_BYTES + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| format!("Cannot read reader image: {e}"))?;
            let asset = ParsedDocumentAsset {
                file_name: name,
                bytes,
            };
            validate_asset(&asset, &mut total)?;
            Ok(asset)
        })
        .collect()
}

pub(super) fn owned_assets(
    html: &str,
    source_evidence: Option<&str>,
    assets: &[ParsedDocumentAsset],
) -> Result<Vec<ParsedDocumentAsset>, String> {
    let mut total = 0;
    let mut names = referenced_assets(html)?;
    if let Some(source) = source_evidence {
        // Review evidence outlives the original upload, but shares asset ownership.
        names.extend(referenced_assets(source)?);
    }
    names
        .into_iter()
        .map(|name| {
            let asset = assets
                .iter()
                .find(|asset| asset.file_name == name)
                .ok_or("Referenced reader image is missing from the snapshot")?;
            validate_asset(asset, &mut total)?;
            Ok(asset.clone())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_root() -> std::path::PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "papercut-reader-assets-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn snapshot_is_deduplicated_independent_and_hash_checked() {
        let root = test_root();
        let asset = ParsedDocumentAsset::new("image/png", b"image content".to_vec()).unwrap();
        let path = root.join(&asset.file_name);
        fs::write(&path, &asset.bytes).unwrap();
        let html = format!(
            "<img data-papercut-asset='{0}'><img data-papercut-asset='{0}'>",
            asset.file_name
        );
        let paths = HashMap::from([(asset.file_name.clone(), path.to_str().unwrap().into())]);
        let snapshot = snapshot_assets(&html, &paths).unwrap();
        assert_eq!(snapshot, vec![asset]);
        assert_eq!(
            snapshot_assets_with_evidence("<p>English only.</p>", Some(&html), &paths).unwrap(),
            snapshot
        );
        assert_eq!(
            snapshot_assets_with_evidence(&html, Some(&html), &paths).unwrap(),
            snapshot,
            "source and target own the same image once"
        );
        fs::write(&path, b"tampered content").unwrap();
        assert!(snapshot_assets(&html, &paths).unwrap_err().contains("hash"));
        fs::remove_file(&path).unwrap();
        assert!(snapshot_assets(&html, &paths).is_err());
        assert_eq!(owned_assets(&html, None, &snapshot).unwrap(), snapshot);
        assert!(owned_assets(&html, None, &[]).is_err());
        assert!(owned_assets("<p>No image</p>", None, &snapshot)
            .unwrap()
            .is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_invalid_names_empty_oversize_and_total_limit() {
        assert!(referenced_assets("<img data-papercut-asset='../secret'>").is_err());
        let mut asset = ParsedDocumentAsset::new("image/png", b"valid".to_vec()).unwrap();
        let mut total = MAX_TOTAL_READER_ASSET_BYTES;
        assert!(validate_asset(&asset, &mut total).is_err());
        asset.bytes.clear();
        assert!(validate_asset(&asset, &mut 0).is_err());
        asset
            .bytes
            .resize(MAX_STORED_READER_ASSET_BYTES as usize + 1, 0);
        assert!(validate_asset(&asset, &mut 0).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn refuses_symbolic_link_assets() {
        let root = test_root();
        let asset = ParsedDocumentAsset::new("image/png", b"content".to_vec()).unwrap();
        let target = root.join("target");
        fs::write(&target, &asset.bytes).unwrap();
        let link = root.join(&asset.file_name);
        std::os::unix::fs::symlink(target, &link).unwrap();
        let paths = HashMap::from([(asset.file_name.clone(), link.to_str().unwrap().into())]);
        let html = format!("<img data-papercut-asset='{}'>", asset.file_name);
        assert!(snapshot_assets(&html, &paths).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
