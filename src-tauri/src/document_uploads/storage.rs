//! Filesystem layout, upload identity, size accounting, and clock.
//!
//! Pure path/id/byte helpers with no SQL or parsing knowledge. The URL prefix
//! and size limit constants also live here since they define the storage contract.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};
use tauri::{Manager, Runtime};
use tauri_plugin_dialog::FilePath;
use tauri_plugin_fs::FsExt;

use super::pdf;

/// URL prefix that marks a document as a runtime upload (vs. a bundled doc).
pub(crate) const UPLOAD_URL_PREFIX: &str = "/uploads/";
/// Hard cap on imported file size (25 MB).
pub(crate) const MAX_UPLOAD_BYTES: u64 = 25 * 1024 * 1024;
/// Hard cap on imported EPUB file size (100 MB).
pub(crate) const MAX_EPUB_UPLOAD_BYTES: u64 = 100 * 1024 * 1024;
/// Hard cap on imported PDF file size (250 MB).
pub(crate) const MAX_PDF_UPLOAD_BYTES: u64 = 250 * 1024 * 1024;

/// Read a picker-provided path or mobile content URL with a hard byte cap.
pub(crate) fn read_source_bytes<R: Runtime>(
    app: &tauri::AppHandle<R>,
    source: FilePath,
    max_bytes: u64,
    too_large_message: &str,
    open_error_prefix: &str,
    read_error_prefix: &str,
) -> Result<Vec<u8>, String> {
    let scoped_source = source.clone();
    let read_result: Result<Vec<u8>, String> = (|| {
        let mut options = tauri_plugin_fs::OpenOptions::new();
        options.read(true);
        let mut file = app
            .fs()
            .open(source, options)
            .map_err(|err| format!("{open_error_prefix}: {err}"))?;
        let mut bytes = Vec::new();
        file.by_ref()
            .take(max_bytes + 1)
            .read_to_end(&mut bytes)
            .map_err(|err| format!("{read_error_prefix}: {err}"))?;
        if bytes.len() as u64 > max_bytes {
            return Err(too_large_message.into());
        }
        Ok(bytes)
    })();
    let release_result = release_source_access(app, scoped_source);
    let bytes = read_result?;
    release_result?;
    Ok(bytes)
}

/// Tauri opens iOS file URLs as security-scoped resources. Release each one
/// after its bounded read so repeated picker/Open With imports do not exhaust
/// the process allowance; other platforms need no matching operation.
#[cfg(target_os = "ios")]
pub(crate) fn release_source_access<R: Runtime>(
    app: &tauri::AppHandle<R>,
    source: FilePath,
) -> Result<(), String> {
    app.fs()
        .stop_accessing_security_scoped_resource(source)
        .map_err(|err| format!("Failed to release imported document access: {err}"))
}

#[cfg(not(target_os = "ios"))]
pub(crate) fn release_source_access<R: Runtime>(
    _app: &tauri::AppHandle<R>,
    _source: FilePath,
) -> Result<(), String> {
    Ok(())
}

/// Stored source representation, distinct from the user-facing document format:
/// EPUB and HTML both produce reader HTML, while PDFs retain their binary source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StoredSourceKind {
    Html,
    Pdf,
}

impl StoredSourceKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::Pdf => "pdf",
        }
    }

    pub(crate) fn from_str(value: &str) -> Result<Self, String> {
        match value {
            "html" => Ok(Self::Html),
            "pdf" => Ok(Self::Pdf),
            _ => Err(format!(
                "Unsupported uploaded document source kind {value:?}"
            )),
        }
    }

    pub(crate) fn file_name(self) -> &'static str {
        match self {
            Self::Html => "source.html",
            Self::Pdf => pdf::SOURCE_FILE_NAME,
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::Pdf => "pdf",
        }
    }
}

/// Use the original file bytes as the stable identity for new uploads.
///
/// Legacy timestamp-derived ids remain valid; exact files imported after this
/// change reuse one stored document instead of creating duplicates.
pub(crate) fn source_upload_id(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Include formats when identical text bytes have different reading semantics,
/// while leaving all existing HTML, EPUB, and PDF identities unchanged.
pub(crate) fn formatted_source_upload_id(format: &str, bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format.as_bytes());
    hasher.update([0]);
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// Build the stable app-owned URL for one stored source.
pub(crate) fn upload_url(id: &str, source_kind: StoredSourceKind) -> String {
    format!("{UPLOAD_URL_PREFIX}{id}.{}", source_kind.extension())
}

/// Recover and validate both identity and source kind from an app-owned URL.
pub(crate) fn upload_reference_from_url(url: &str) -> Result<(String, StoredSourceKind), String> {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let Some(rest) = path.strip_prefix(UPLOAD_URL_PREFIX) else {
        return Err("Document is not a generic uploaded document".into());
    };
    let (id, source_kind) = if let Some(id) = rest.strip_suffix(".html") {
        (id, StoredSourceKind::Html)
    } else if let Some(id) = rest.strip_suffix(".pdf") {
        (id, StoredSourceKind::Pdf)
    } else {
        return Err("Uploaded document URL must end in .html or .pdf".into());
    };
    if id.is_empty() || !id.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err("Uploaded document id is invalid".into());
    }
    Ok((id.to_string(), source_kind))
}

/// Recover only the id for callers whose operation applies to every source kind.
pub(crate) fn upload_id_from_url(url: &str) -> Result<String, String> {
    upload_reference_from_url(url).map(|(id, _)| id)
}

/// Resolve the root directory under app data that holds all uploads + the DB.
pub(crate) fn uploads_root<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf, String> {
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|err| format!("Failed to resolve app data dir for document uploads: {err}"))?;
    Ok(app_data.join("document_uploads"))
}

/// Resolve the per-document storage directory for a given upload id.
pub(crate) fn upload_dir<R: Runtime>(
    app: &tauri::AppHandle<R>,
    id: &str,
) -> Result<PathBuf, String> {
    Ok(uploads_root(app)?.join(id))
}

/// Resolve the canonical source path without letting callers duplicate
/// format-specific filenames.
pub(crate) fn upload_source_path<R: Runtime>(
    app: &tauri::AppHandle<R>,
    id: &str,
    source_kind: StoredSourceKind,
) -> Result<PathBuf, String> {
    Ok(upload_dir(app, id)?.join(source_kind.file_name()))
}

/// Recursively total the byte size of a file or directory tree, returning 0 if
/// the path does not exist (used to report bytes freed on delete).
pub(crate) fn directory_size(path: &Path) -> Result<u64, String> {
    if !path.exists() {
        return Ok(0);
    }
    let metadata = fs::metadata(path).map_err(|err| {
        format!(
            "Failed to inspect uploaded document storage {}: {err}",
            path.display()
        )
    })?;
    if metadata.is_file() {
        return Ok(metadata.len());
    }

    let mut total = 0;
    for entry in fs::read_dir(path).map_err(|err| {
        format!(
            "Failed to read uploaded document storage {}: {err}",
            path.display()
        )
    })? {
        let entry =
            entry.map_err(|err| format!("Failed to inspect uploaded document file: {err}"))?;
        total += directory_size(&entry.path())?;
    }
    Ok(total)
}

/// Current Unix time in milliseconds, used as the import timestamp.
pub(crate) fn now_ms() -> Result<u128, String> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("System clock error: {err}"))?
        .as_millis())
}

#[cfg(test)]
mod tests {
    use super::{
        formatted_source_upload_id, source_upload_id, upload_reference_from_url, upload_url,
        StoredSourceKind,
    };

    #[test]
    fn source_upload_id_is_stable_sha256() {
        assert_eq!(
            source_upload_id(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_ne!(source_upload_id(b"abc"), source_upload_id(b"abd"));
    }

    #[test]
    fn text_identity_includes_interpretation_format() {
        assert_ne!(
            formatted_source_upload_id("txt", b"# Heading"),
            formatted_source_upload_id("markdown", b"# Heading")
        );
    }

    #[test]
    fn uploaded_urls_preserve_source_kind() {
        let id = "a".repeat(64);
        for kind in [StoredSourceKind::Html, StoredSourceKind::Pdf] {
            let url = upload_url(&id, kind);
            assert_eq!(upload_reference_from_url(&url), Ok((id.clone(), kind)));
        }
        assert!(upload_reference_from_url("/uploads/abc.epub").is_err());
        assert!(upload_reference_from_url("/documents/abc.pdf").is_err());
    }
}
