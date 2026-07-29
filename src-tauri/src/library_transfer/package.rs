//! Versioned `.papercut-library` archive contract and trust-boundary checks.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek, Write};
use std::path::{Component, Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

pub(super) const PACKAGE_KIND: &str = "papercut-library";
pub(super) const PACKAGE_VERSION: u32 = 3;
const DOCUMENTS_ONLY_PACKAGE_VERSION: u32 = 1;
const AUDIOBOOK_PACKAGE_VERSION: u32 = 2;
const MANIFEST_PATH: &str = "manifest.json";
const MAX_DOCUMENTS: usize = 500;
const MAX_AUDIOBOOKS: usize = 500;
const MAX_AUDIOBOOK_FILES: usize = 100_000;
const MAX_FOLDERS: usize = 2_000;
const MAX_FOLDER_DEPTH: usize = 4;
const MAX_FOLDER_NAME_CHARS: usize = 80;
const MAX_MANIFEST_BYTES: u64 = 4 * 1024 * 1024;
const MAX_NATIVE_AUDIOBOOK_MANIFEST_BYTES: u64 = 16 * 1024 * 1024;
const MAX_IMPORTED_AUDIOBOOK_SOURCE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_IMPORTED_AUDIOBOOK_METADATA_BYTES: u64 = 4 * 1024 * 1024;
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
    #[serde(default)]
    pub(super) audiobooks: Vec<TransferAudiobook>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TransferDocument {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) format: String,
    #[serde(default = "default_source_kind")]
    pub(super) source_kind: String,
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TransferAudiobook {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) storage_key: String,
    pub(super) files: Vec<TransferAudiobookFile>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TransferAudiobookFile {
    pub(super) relative_path: String,
    pub(super) path: String,
    pub(super) bytes: u64,
    pub(super) sha256: String,
}

/// Versions 1 and 2 predate explicit source kinds and always carried generated
/// reader HTML, so omitted fields must deserialize as HTML for compatibility.
fn default_source_kind() -> String {
    "html".into()
}

/// Keep each source at one manifest-verifiable canonical path; import rejects
/// mismatched paths before reading any archive payload.
pub(super) fn document_source_path(id: &str, source_kind: &str) -> String {
    format!("documents/{id}/source.{source_kind}")
}

pub(super) fn audiobook_file_path(storage_key: &str, relative_path: &str) -> String {
    format!("audiobooks/{storage_key}/{relative_path}")
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
    mut open_payload: O,
) -> Result<(), String>
where
    W: Write + Seek,
    O: FnMut(&str) -> Result<Box<dyn Read>, String>,
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
        write_payload(
            &mut archive,
            options,
            &document.source_path,
            document.source_bytes,
            &document.source_sha256,
            &mut open_payload,
        )?;
    }
    for audiobook in &manifest.audiobooks {
        for file in &audiobook.files {
            write_payload(
                &mut archive,
                options,
                &file.path,
                file.bytes,
                &file.sha256,
                &mut open_payload,
            )?;
        }
    }

    archive.finish().map_err(zip_write_err)?;
    Ok(())
}

fn write_payload<W: Write + Seek, O: FnMut(&str) -> Result<Box<dyn Read>, String>>(
    archive: &mut ZipWriter<W>,
    options: FileOptions,
    path: &str,
    expected_bytes: u64,
    expected_sha256: &str,
    open_payload: &mut O,
) -> Result<(), String> {
    archive.start_file(path, options).map_err(zip_write_err)?;
    let mut source = open_payload(path)?;
    let mut hasher = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = source
            .read(&mut buffer)
            .map_err(|err| format!("Failed to read {path}: {err}"))?;
        if read == 0 {
            break;
        }
        archive
            .write_all(&buffer[..read])
            .map_err(|err| format!("Failed to write library-transfer package: {err}"))?;
        hasher.update(&buffer[..read]);
        total += read as u64;
    }
    if total != expected_bytes || format!("{:x}", hasher.finalize()) != expected_sha256 {
        return Err(format!("Payload changed while exporting: {path}"));
    }
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
) -> Result<Vec<u8>, String> {
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
    Ok(bytes)
}

/// Stream one declared binary payload to a staged path while enforcing its
/// decompressed size and checksum. No archive pathname is ever extracted.
pub(super) fn copy_audiobook_file<R: Read + Seek, W: Write>(
    archive: &mut ZipArchive<R>,
    file: &TransferAudiobookFile,
    target: &mut W,
) -> Result<(), String> {
    let mut entry = archive
        .by_name(&file.path)
        .map_err(|_| format!("Library-transfer payload is missing: {}", file.path))?;
    if entry.is_dir() || entry.size() != file.bytes {
        return Err(format!(
            "Library-transfer payload size does not match: {}",
            file.path
        ));
    }
    let mut hasher = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = entry
            .read(&mut buffer)
            .map_err(|err| format!("Failed to read {}: {err}", file.path))?;
        if read == 0 {
            break;
        }
        target
            .write_all(&buffer[..read])
            .map_err(|err| format!("Failed to restore {}: {err}", file.path))?;
        hasher.update(&buffer[..read]);
        total += read as u64;
    }
    if total != file.bytes || format!("{:x}", hasher.finalize()) != file.sha256 {
        return Err(format!(
            "Library-transfer payload checksum does not match: {}",
            file.path
        ));
    }
    Ok(())
}

/// Enforce package bounds and referential integrity before export or import.
/// Archive entry validation remains separate because export has no ZIP to read.
fn validate_manifest(manifest: &TransferManifest) -> Result<(), String> {
    if manifest.kind != PACKAGE_KIND
        || !matches!(
            manifest.schema_version,
            DOCUMENTS_ONLY_PACKAGE_VERSION | AUDIOBOOK_PACKAGE_VERSION | PACKAGE_VERSION
        )
    {
        return Err(format!(
            "Unsupported library-transfer package {:?} version {}",
            manifest.kind, manifest.schema_version
        ));
    }
    if manifest.documents.is_empty() && manifest.audiobooks.is_empty() {
        return Err("Library-transfer package contains no content".into());
    }
    if manifest.schema_version == DOCUMENTS_ONLY_PACKAGE_VERSION && !manifest.audiobooks.is_empty()
    {
        return Err("Library-transfer package version 1 cannot contain audiobooks".into());
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
        if manifest.schema_version < PACKAGE_VERSION && document.source_kind != "html" {
            return Err("Library-transfer package versions 1 and 2 cannot contain PDFs".into());
        }
        let valid_source = matches!(
            (document.format.as_str(), document.source_kind.as_str()),
            ("html" | "epub", "html") | ("pdf", "pdf")
        );
        if !valid_source {
            return Err(format!(
                "Unsupported transferred document format/source pair: {}/{}",
                document.format, document.source_kind
            ));
        }
        let expected_path = document_source_path(&document.id, &document.source_kind);
        if document.source_path != expected_path
            || !source_paths.insert(document.source_path.as_str())
        {
            return Err(format!(
                "Invalid transferred document path: {}",
                document.source_path
            ));
        }
        if document.imported_at_ms > i64::MAX as u64 {
            return Err("Transferred document timestamp is invalid".into());
        }
        if document.original_bytes > i64::MAX as u64 {
            return Err("Transferred document byte count is invalid".into());
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

    validate_organization(&manifest.organization, &document_ids)?;
    validate_audiobooks(&manifest.audiobooks, &mut total_source_bytes)
}

/// Validate the exact canonical audiobook file set without interpreting native
/// manifest JSON; the native registry performs that semantic check after staging.
fn validate_audiobooks(
    audiobooks: &[TransferAudiobook],
    total_payload_bytes: &mut u64,
) -> Result<(), String> {
    if audiobooks.len() > MAX_AUDIOBOOKS {
        return Err(format!(
            "Library-transfer package contains more than {MAX_AUDIOBOOKS} audiobooks"
        ));
    }
    let mut ids = HashSet::new();
    let mut paths = HashSet::new();
    let mut file_count = 0usize;
    for audiobook in audiobooks {
        if audiobook.id.is_empty()
            || audiobook.id.len() > 4096
            || audiobook.id.chars().any(char::is_control)
            || !ids.insert(audiobook.id.as_str())
        {
            return Err("Transferred audiobook id is invalid or duplicated".into());
        }
        validate_hex_id(&audiobook.storage_key, "audiobook storage")?;
        if audiobook.storage_key.len() != 16 || audiobook.title.trim().is_empty() {
            return Err("Transferred audiobook metadata is invalid".into());
        }
        let mut relative_paths = HashSet::new();
        let mut has_manifest = false;
        let mut has_chunk = false;
        let mut has_source = false;
        for file in &audiobook.files {
            file_count += 1;
            if file_count > MAX_AUDIOBOOK_FILES {
                return Err("Library-transfer package contains too many audiobook files".into());
            }
            let valid_relative = file.relative_path == "manifest.json"
                || file.relative_path == "source/source.html"
                || file.relative_path == "source/metadata.json"
                || is_canonical_chunk_path(&file.relative_path);
            let expected_path = audiobook_file_path(&audiobook.storage_key, &file.relative_path);
            if !valid_relative
                || file.path != expected_path
                || !relative_paths.insert(file.relative_path.as_str())
                || !paths.insert(file.path.as_str())
                || file.bytes == 0
            {
                return Err(format!("Invalid transferred audiobook path: {}", file.path));
            }
            let exceeds_role_limit = (file.relative_path == "manifest.json"
                && file.bytes > MAX_NATIVE_AUDIOBOOK_MANIFEST_BYTES)
                || (file.relative_path == "source/source.html"
                    && file.bytes > MAX_IMPORTED_AUDIOBOOK_SOURCE_BYTES)
                || (file.relative_path == "source/metadata.json"
                    && file.bytes > MAX_IMPORTED_AUDIOBOOK_METADATA_BYTES);
            if exceeds_role_limit {
                return Err(format!(
                    "Transferred audiobook payload is too large: {}",
                    file.path
                ));
            }
            has_manifest |= file.relative_path == "manifest.json";
            has_chunk |= file.relative_path.starts_with("chunks/");
            has_source |= file.relative_path == "source/source.html";
            validate_sha256(&file.sha256)?;
            *total_payload_bytes = total_payload_bytes
                .checked_add(file.bytes)
                .ok_or_else(|| "Library-transfer package is too large".to_string())?;
            if *total_payload_bytes > MAX_PACKAGE_BYTES {
                return Err("Library-transfer package expands beyond the supported size".into());
            }
        }
        let has_source_metadata = relative_paths.contains("source/metadata.json");
        if !has_manifest || !has_chunk || (has_source_metadata && !has_source) {
            return Err(format!(
                "Transferred audiobook payload is incomplete: {}",
                audiobook.title
            ));
        }
    }
    Ok(())
}

/// Accept exactly one normal filename below `chunks`; rejecting both separator
/// forms keeps validation identical on Unix and Windows hosts.
fn is_canonical_chunk_path(relative_path: &str) -> bool {
    let Some(file_name) = relative_path.strip_prefix("chunks/") else {
        return false;
    };
    if file_name.is_empty()
        || !file_name.ends_with(".wav")
        || file_name.contains(['/', '\\'])
        || file_name.chars().any(char::is_control)
    {
        return false;
    }
    let mut components = Path::new(file_name).components();
    matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none()
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
    expected.extend(
        manifest
            .audiobooks
            .iter()
            .flat_map(|audiobook| audiobook.files.iter().map(|file| file.path.as_str())),
    );
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

        assert_eq!(restored, source);
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
    fn package_v3_round_trips_a_canonical_pdf_source() {
        let source = b"%PDF-1.7\nfixture".to_vec();
        let mut manifest = test_manifest(&source);
        let document = &mut manifest.documents[0];
        document.format = "pdf".into();
        document.source_kind = "pdf".into();
        document.source_path = document_source_path(&document.id, &document.source_kind);
        let mut bytes = Cursor::new(Vec::new());

        write_package(&mut bytes, &manifest, |_| {
            Ok(Box::new(Cursor::new(source.clone())))
        })
        .expect("write PDF package");
        bytes.set_position(0);
        let mut archive = ZipArchive::new(bytes).expect("open PDF package");
        let restored_manifest = read_manifest(&mut archive).expect("read PDF manifest");
        assert_eq!(
            read_document_source(&mut archive, &restored_manifest.documents[0])
                .expect("read PDF source"),
            source
        );
    }

    #[test]
    fn legacy_packages_cannot_smuggle_pdf_sources() {
        let source = b"%PDF-1.7\nfixture".to_vec();
        let mut manifest = test_manifest(&source);
        manifest.schema_version = AUDIOBOOK_PACKAGE_VERSION;
        let document = &mut manifest.documents[0];
        document.format = "pdf".into();
        document.source_kind = "pdf".into();
        document.source_path = document_source_path(&document.id, &document.source_kind);

        assert!(validate_manifest(&manifest)
            .expect_err("legacy PDF package")
            .contains("cannot contain PDFs"));
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

    #[test]
    fn package_round_trip_streams_optional_audiobook_files() {
        let document = b"<p>Hello</p>".to_vec();
        let native_manifest = br#"{"version":4}"#.to_vec();
        let wav = b"RIFF-test".to_vec();
        let mut manifest = test_manifest(&document);
        let storage_key = "b".repeat(16);
        manifest.audiobooks.push(TransferAudiobook {
            id: "kokoro|test".into(),
            title: "Test audio".into(),
            storage_key: storage_key.clone(),
            files: vec![
                test_audiobook_file(&storage_key, "manifest.json", &native_manifest),
                test_audiobook_file(&storage_key, "chunks/00001-test.wav", &wav),
            ],
        });
        let mut bytes = Cursor::new(Vec::new());
        write_package(&mut bytes, &manifest, |path| {
            let payload = if path == manifest.documents[0].source_path.as_str() {
                document.clone()
            } else if path.ends_with("manifest.json") {
                native_manifest.clone()
            } else {
                wav.clone()
            };
            Ok(Box::new(Cursor::new(payload)))
        })
        .expect("write package with audiobook");

        bytes.set_position(0);
        let mut archive = ZipArchive::new(bytes).expect("open package");
        let restored = read_manifest(&mut archive).expect("read manifest");
        let mut output = Vec::new();
        copy_audiobook_file(&mut archive, &restored.audiobooks[0].files[1], &mut output)
            .expect("copy audio");
        assert_eq!(output, wav);
    }

    #[test]
    fn package_rejects_windows_chunk_path_traversal() {
        let document = b"<p>Hello</p>".to_vec();
        let native_manifest = br#"{"version":4}"#.to_vec();
        let wav = b"RIFF-test".to_vec();
        let mut manifest = test_manifest(&document);
        let storage_key = "b".repeat(16);
        manifest.audiobooks.push(TransferAudiobook {
            id: "kokoro|test".into(),
            title: "Test audio".into(),
            storage_key: storage_key.clone(),
            files: vec![
                test_audiobook_file(&storage_key, "manifest.json", &native_manifest),
                test_audiobook_file(&storage_key, "chunks/..\\outside.wav", &wav),
            ],
        });

        let error = validate_manifest(&manifest).expect_err("backslash traversal");
        assert!(error.contains("Invalid transferred audiobook path"));
    }

    fn test_manifest(source: &[u8]) -> TransferManifest {
        let id = "a".repeat(64);
        TransferManifest {
            kind: PACKAGE_KIND.into(),
            schema_version: PACKAGE_VERSION,
            created_at_ms: 1,
            documents: vec![TransferDocument {
                source_path: document_source_path(&id, "html"),
                id,
                title: "Test".into(),
                format: "html".into(),
                source_kind: "html".into(),
                imported_at_ms: 1,
                original_bytes: source.len() as u64,
                source_bytes: source.len() as u64,
                source_sha256: format!("{:x}", Sha256::digest(source)),
            }],
            organization: TransferOrganization::default(),
            audiobooks: Vec::new(),
        }
    }

    fn test_audiobook_file(
        storage_key: &str,
        relative_path: &str,
        payload: &[u8],
    ) -> TransferAudiobookFile {
        TransferAudiobookFile {
            relative_path: relative_path.into(),
            path: audiobook_file_path(storage_key, relative_path),
            bytes: payload.len() as u64,
            sha256: format!("{:x}", Sha256::digest(payload)),
        }
    }
}
