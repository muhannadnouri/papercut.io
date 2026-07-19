//! Versioned `.papercut-library` archive contract and trust-boundary checks.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek, Write};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

pub(super) const PACKAGE_KIND: &str = "papercut-library";
pub(super) const PACKAGE_VERSION: u32 = 1;
const MANIFEST_PATH: &str = "manifest.json";
const MAX_DOCUMENTS: usize = 500;
const MAX_FOLDERS: usize = 2_000;
const MAX_FOLDER_DEPTH: usize = 4;
const MAX_FOLDER_NAME_CHARS: usize = 80;
const MAX_MANIFEST_BYTES: u64 = 4 * 1024 * 1024;
pub(super) const MAX_PACKAGE_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_DOCUMENT_SOURCE_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TransferManifest {
    pub(super) kind: String,
    pub(super) schema_version: u32,
    pub(super) created_at_ms: u64,
    pub(super) documents: Vec<TransferDocument>,
    pub(super) organization: TransferOrganization,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TransferDocument {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) format: String,
    pub(super) imported_at_ms: u64,
    pub(super) original_bytes: u64,
    pub(super) source_path: String,
    pub(super) source_bytes: u64,
    pub(super) source_sha256: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TransferOrganization {
    pub(super) folders: Vec<TransferFolder>,
    pub(super) document_locations: Vec<TransferDocumentLocation>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TransferFolder {
    pub(super) id: String,
    pub(super) parent_id: Option<String>,
    pub(super) name: String,
    pub(super) depth: usize,
    pub(super) sort_order: i64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TransferDocumentLocation {
    pub(super) document_id: String,
    pub(super) folder_id: Option<String>,
    pub(super) sort_order: i64,
}

pub(super) fn document_source_path(id: &str) -> String {
    format!("documents/{id}/source.html")
}

/// Hash a reader without buffering an entire document and return bytes consumed.
pub(super) fn sha256_reader<R: Read>(reader: &mut R) -> Result<(String, u64), String> {
    let mut hasher = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|err| format!("Failed to read library-transfer payload: {err}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        total = total
            .checked_add(read as u64)
            .ok_or_else(|| "Library-transfer payload is too large".to_string())?;
    }
    Ok((format!("{:x}", hasher.finalize()), total))
}

/// Write the prevalidated manifest and payloads. A second hash while writing
/// detects a source file changing between manifest preparation and archive copy.
pub(super) fn write_package<W, O>(
    writer: W,
    manifest: &TransferManifest,
    mut open_source: O,
) -> Result<(), String>
where
    W: Write + Seek,
    O: FnMut(&TransferDocument) -> Result<Box<dyn Read>, String>,
{
    validate_manifest(manifest)?;
    let manifest_json = serde_json::to_vec_pretty(manifest)
        .map_err(|err| format!("Failed to serialize library-transfer manifest: {err}"))?;
    if manifest_json.len() as u64 > MAX_MANIFEST_BYTES {
        return Err("Library-transfer manifest is too large".into());
    }

    let options = FileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);
    let mut archive = ZipWriter::new(writer);
    archive
        .start_file(MANIFEST_PATH, options)
        .map_err(zip_write_err)?;
    archive
        .write_all(&manifest_json)
        .map_err(|err| format!("Failed to write library-transfer package: {err}"))?;

    for document in &manifest.documents {
        archive
            .start_file(&document.source_path, options)
            .map_err(zip_write_err)?;
        let mut source = open_source(document)?;
        let mut hasher = Sha256::new();
        let mut total = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let read = source
                .read(&mut buffer)
                .map_err(|err| format!("Failed to read {}: {err}", document.source_path))?;
            if read == 0 {
                break;
            }
            archive
                .write_all(&buffer[..read])
                .map_err(|err| format!("Failed to write library-transfer package: {err}"))?;
            hasher.update(&buffer[..read]);
            total += read as u64;
        }
        let checksum = format!("{:x}", hasher.finalize());
        if total != document.source_bytes || checksum != document.source_sha256 {
            return Err(format!(
                "Document changed while exporting: {}",
                document.title
            ));
        }
    }

    archive.finish().map_err(zip_write_err)?;
    Ok(())
}

/// Read and validate the manifest and exact archive entry set before any target
/// storage is mutated. Payload contents are checksum-verified per document later.
pub(super) fn read_manifest<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
) -> Result<TransferManifest, String> {
    let manifest = {
        let mut entry = archive
            .by_name(MANIFEST_PATH)
            .map_err(|_| "Library-transfer package is missing manifest.json".to_string())?;
        if entry.is_dir() || entry.size() > MAX_MANIFEST_BYTES {
            return Err("Library-transfer manifest is invalid or too large".into());
        }
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry
            .read_to_end(&mut bytes)
            .map_err(|err| format!("Failed to read library-transfer manifest: {err}"))?;
        serde_json::from_slice::<TransferManifest>(&bytes)
            .map_err(|err| format!("Failed to parse library-transfer manifest: {err}"))?
    };
    validate_manifest(&manifest)?;
    validate_archive_entries(archive, &manifest)?;
    Ok(manifest)
}

/// Read one exact manifest-owned payload with both decompressed-size and digest checks.
pub(super) fn read_document_source<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    document: &TransferDocument,
) -> Result<String, String> {
    let mut entry = archive.by_name(&document.source_path).map_err(|_| {
        format!(
            "Library-transfer payload is missing: {}",
            document.source_path
        )
    })?;
    if entry.is_dir() || entry.size() != document.source_bytes {
        return Err(format!(
            "Library-transfer payload size does not match: {}",
            document.source_path
        ));
    }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry
        .read_to_end(&mut bytes)
        .map_err(|err| format!("Failed to read {}: {err}", document.source_path))?;
    let checksum = format!("{:x}", Sha256::digest(&bytes));
    if checksum != document.source_sha256 {
        return Err(format!(
            "Library-transfer payload checksum does not match: {}",
            document.source_path
        ));
    }
    String::from_utf8(bytes).map_err(|_| {
        format!(
            "Library-transfer document is not UTF-8: {}",
            document.source_path
        )
    })
}

/// Enforce package bounds and referential integrity before export or import.
/// Archive entry validation remains separate because export has no ZIP to read.
fn validate_manifest(manifest: &TransferManifest) -> Result<(), String> {
    if manifest.kind != PACKAGE_KIND || manifest.schema_version != PACKAGE_VERSION {
        return Err(format!(
            "Unsupported library-transfer package {:?} version {}",
            manifest.kind, manifest.schema_version
        ));
    }
    if manifest.documents.is_empty() {
        return Err("Library-transfer package contains no documents".into());
    }
    if manifest.documents.len() > MAX_DOCUMENTS {
        return Err(format!(
            "Library-transfer package contains more than {MAX_DOCUMENTS} documents"
        ));
    }
    if manifest.organization.folders.len() > MAX_FOLDERS {
        return Err("Library-transfer package contains too many folders".into());
    }

    let mut document_ids = HashSet::new();
    let mut source_paths = HashSet::new();
    let mut total_source_bytes = 0u64;
    for document in &manifest.documents {
        validate_hex_id(&document.id, "document")?;
        if !document_ids.insert(document.id.as_str()) {
            return Err(format!(
                "Duplicate transferred document id: {}",
                document.id
            ));
        }
        let expected_path = document_source_path(&document.id);
        if document.source_path != expected_path
            || !source_paths.insert(document.source_path.as_str())
        {
            return Err(format!(
                "Invalid transferred document path: {}",
                document.source_path
            ));
        }
        if !matches!(document.format.as_str(), "html" | "epub") {
            return Err(format!(
                "Unsupported transferred document format: {}",
                document.format
            ));
        }
        if document.imported_at_ms > i64::MAX as u64 {
            return Err("Transferred document timestamp is invalid".into());
        }
        if document.source_bytes == 0 || document.source_bytes > MAX_DOCUMENT_SOURCE_BYTES {
            return Err(format!(
                "Transferred document payload is too large: {}",
                document.title
            ));
        }
        validate_sha256(&document.source_sha256)?;
        total_source_bytes = total_source_bytes
            .checked_add(document.source_bytes)
            .ok_or_else(|| "Library-transfer package is too large".to_string())?;
        if total_source_bytes > MAX_PACKAGE_BYTES {
            return Err("Library-transfer package expands beyond the supported size".into());
        }
    }

    validate_organization(&manifest.organization, &document_ids)
}

/// Validate the folder graph and document placements without trusting order in
/// the manifest; parents may appear before or after their children.
fn validate_organization(
    organization: &TransferOrganization,
    document_ids: &HashSet<&str>,
) -> Result<(), String> {
    let mut folders = HashMap::new();
    for folder in &organization.folders {
        validate_hex_id(&folder.id, "folder")?;
        if folder.name.trim().is_empty() || folder.name.chars().count() > MAX_FOLDER_NAME_CHARS {
            return Err("Transferred folder name is invalid".into());
        }
        if folder.depth > MAX_FOLDER_DEPTH || folders.insert(folder.id.as_str(), folder).is_some() {
            return Err(format!(
                "Transferred folder metadata is invalid: {}",
                folder.name
            ));
        }
    }

    let mut sibling_names = HashSet::new();
    for folder in organization.folders.iter() {
        let expected_depth = match folder.parent_id.as_deref() {
            Some(parent_id) => {
                let parent = folders.get(parent_id).ok_or_else(|| {
                    format!("Transferred folder parent is missing: {}", folder.name)
                })?;
                parent.depth + 1
            }
            None => 0,
        };
        if expected_depth != folder.depth {
            return Err(format!(
                "Transferred folder depth is invalid: {}",
                folder.name
            ));
        }
        let sibling_key = (
            folder.parent_id.as_deref().unwrap_or_default(),
            folder.name.to_lowercase(),
        );
        if !sibling_names.insert(sibling_key) {
            return Err(format!(
                "Duplicate transferred folder name: {}",
                folder.name
            ));
        }
    }

    let mut located_documents = HashSet::new();
    for location in &organization.document_locations {
        if !document_ids.contains(location.document_id.as_str())
            || !located_documents.insert(location.document_id.as_str())
        {
            return Err("Transferred document placement is invalid".into());
        }
        if let Some(folder_id) = location.folder_id.as_deref() {
            if !folders.contains_key(folder_id) {
                return Err("Transferred document folder is missing".into());
            }
        }
    }
    Ok(())
}

/// Reject duplicate and unmanifested entries rather than delegating extraction
/// to ZIP paths, closing traversal and hidden-payload classes of archive bugs.
fn validate_archive_entries<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    manifest: &TransferManifest,
) -> Result<(), String> {
    let mut expected: HashSet<&str> = manifest
        .documents
        .iter()
        .map(|document| document.source_path.as_str())
        .collect();
    expected.insert(MANIFEST_PATH);
    let mut seen = HashSet::new();
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|err| format!("Failed to inspect library-transfer archive: {err}"))?;
        let name = entry.name().to_string();
        if entry.is_dir() || !expected.contains(name.as_str()) || !seen.insert(name.clone()) {
            return Err(format!("Unexpected library-transfer archive entry: {name}"));
        }
    }
    if seen.len() != expected.len() {
        return Err("Library-transfer package is missing a required payload".into());
    }
    Ok(())
}

fn validate_hex_id(id: &str, label: &str) -> Result<(), String> {
    if id.is_empty() || id.len() > 128 || !id.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(format!("Transferred {label} id is invalid"));
    }
    Ok(())
}

fn validate_sha256(value: &str) -> Result<(), String> {
    if value.len() != 64 || !value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err("Transferred document checksum is invalid".into());
    }
    Ok(())
}

fn zip_write_err(err: zip::result::ZipError) -> String {
    format!("Failed to write library-transfer package: {err}")
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn package_round_trip_validates_manifest_entries_and_checksum() {
        let source = b"<html><title>Test</title><body><p>Hello</p></body></html>".to_vec();
        let manifest = test_manifest(&source);
        let mut bytes = Cursor::new(Vec::new());
        write_package(&mut bytes, &manifest, |_| {
            Ok(Box::new(Cursor::new(source.clone())))
        })
        .expect("write package");

        bytes.set_position(0);
        let mut archive = ZipArchive::new(bytes).expect("open package");
        let restored_manifest = read_manifest(&mut archive).expect("read manifest");
        let restored = read_document_source(&mut archive, &restored_manifest.documents[0])
            .expect("read source");

        assert_eq!(restored.as_bytes(), source);
    }

    #[test]
    fn package_rejects_an_unmanifested_entry() {
        let source = b"<p>Hello</p>".to_vec();
        let manifest = test_manifest(&source);
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut zip = ZipWriter::new(&mut bytes);
            let options = FileOptions::default();
            zip.start_file(MANIFEST_PATH, options).unwrap();
            zip.write_all(&serde_json::to_vec(&manifest).unwrap())
                .unwrap();
            zip.start_file(&manifest.documents[0].source_path, options)
                .unwrap();
            zip.write_all(&source).unwrap();
            zip.start_file("../unexpected", options).unwrap();
            zip.write_all(b"nope").unwrap();
            zip.finish().unwrap();
        }

        bytes.set_position(0);
        let mut archive = ZipArchive::new(bytes).unwrap();
        assert!(read_manifest(&mut archive)
            .expect_err("unexpected entry")
            .contains("Unexpected"));
    }

    #[test]
    fn package_rejects_a_payload_checksum_mismatch() {
        let source = b"<p>Hello</p>".to_vec();
        let manifest = test_manifest(&source);
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut zip = ZipWriter::new(&mut bytes);
            let options = FileOptions::default();
            zip.start_file(MANIFEST_PATH, options).unwrap();
            zip.write_all(&serde_json::to_vec(&manifest).unwrap())
                .unwrap();
            zip.start_file(&manifest.documents[0].source_path, options)
                .unwrap();
            zip.write_all(b"<p>Jello</p>").unwrap();
            zip.finish().unwrap();
        }

        bytes.set_position(0);
        let mut archive = ZipArchive::new(bytes).unwrap();
        let manifest = read_manifest(&mut archive).expect("valid manifest");
        assert!(read_document_source(&mut archive, &manifest.documents[0]).is_err());
    }

    #[test]
    fn package_rejects_duplicate_document_ids_before_writing() {
        let source = b"<p>Hello</p>".to_vec();
        let mut manifest = test_manifest(&source);
        let mut duplicate_manifest = test_manifest(&source);
        let mut duplicate = duplicate_manifest.documents.remove(0);
        duplicate.title = "Duplicate".into();
        manifest.documents.push(duplicate);
        let mut bytes = Cursor::new(Vec::new());

        assert!(write_package(&mut bytes, &manifest, |_| {
            Ok(Box::new(Cursor::new(source.clone())))
        })
        .expect_err("duplicate id")
        .contains("Duplicate transferred document id"));
    }

    fn test_manifest(source: &[u8]) -> TransferManifest {
        let id = "a".repeat(64);
        TransferManifest {
            kind: PACKAGE_KIND.into(),
            schema_version: PACKAGE_VERSION,
            created_at_ms: 1,
            documents: vec![TransferDocument {
                source_path: document_source_path(&id),
                id,
                title: "Test".into(),
                format: "html".into(),
                imported_at_ms: 1,
                original_bytes: source.len() as u64,
                source_bytes: source.len() as u64,
                source_sha256: format!("{:x}", Sha256::digest(source)),
            }],
            organization: TransferOrganization::default(),
        }
    }
}
