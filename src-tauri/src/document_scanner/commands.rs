use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use serde::Deserialize;
use tauri::{AppHandle, Manager, Runtime, State};
use tauri_plugin_document_scanner::{DocumentScannerExt, ScanResult, ScannerAvailability};

use crate::document_uploads::{
    import_scanner_source, DocumentUploadState, UploadedDocumentBatchResult, MAX_PDF_UPLOAD_BYTES,
};

static NEXT_CAPTURE_ID: AtomicU64 = AtomicU64::new(0);
const MAX_SCANNER_TITLE_CHARS: usize = 512;
const MAX_SCANNER_PAGES: usize = 500;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DocumentScannerImportRequest {
    title: String,
}

#[tauri::command]
pub(crate) async fn document_scanner_availability<R: Runtime>(
    app: AppHandle<R>,
) -> Result<ScannerAvailability, String> {
    tauri::async_runtime::spawn_blocking(move || app.document_scanner().availability())
        .await
        .map_err(|error| format!("Document scanner availability task failed: {error}"))?
}

/// Capture to a private staging PDF, then reuse the normal upload transaction.
/// The completed native output is removed after the import attempt because the
/// canonical importer owns successful copies and no UI can recover failed ones.
#[tauri::command]
pub(crate) async fn document_scanner_scan<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, DocumentUploadState>,
    request: DocumentScannerImportRequest,
) -> Result<UploadedDocumentBatchResult, String> {
    import_mobile_source(app, state, MobileSource::Camera, request).await
}

#[tauri::command]
pub(crate) async fn document_scanner_import_images<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, DocumentUploadState>,
    request: DocumentScannerImportRequest,
) -> Result<UploadedDocumentBatchResult, String> {
    import_mobile_source(app, state, MobileSource::Photos, request).await
}

#[derive(Clone, Copy)]
enum MobileSource {
    Camera,
    Photos,
}

/// Both native entry points converge here so imported photos and camera scans
/// use the same staged-PDF validation and atomic upload transaction.
async fn import_mobile_source<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, DocumentUploadState>,
    source: MobileSource,
    request: DocumentScannerImportRequest,
) -> Result<UploadedDocumentBatchResult, String> {
    let title = normalize_scanner_title(request.title)?;
    let control = state.begin_batch()?;
    let scan_dir = create_capture_dir(&app)?;
    let output_path = scan_dir.join(match source {
        MobileSource::Camera => "Scanned Document.pdf",
        MobileSource::Photos => "Imported Photos.pdf",
    });
    let scan_app = app.clone();
    let scan_output = output_path.clone();
    let scan_result = tauri::async_runtime::spawn_blocking(move || match source {
        MobileSource::Camera => scan_app.document_scanner().scan_to(&scan_output),
        MobileSource::Photos => scan_app.document_scanner().import_images_to(&scan_output),
    })
    .await;
    let cleanup_error = |error| {
        let _ = fs::remove_dir_all(&scan_dir);
        Err(error)
    };
    let scan_result = match scan_result {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => return cleanup_error(error),
        Err(error) => return cleanup_error(format!("Mobile document import task failed: {error}")),
    };
    if let Err(error) = validate_mobile_output(&scan_result, &output_path) {
        return cleanup_error(error);
    }

    let import_app = app.clone();
    let import_path = output_path.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        import_scanner_source(import_app, control, import_path, &title)
    })
    .await
    .map_err(|error| format!("Mobile document import task failed: {error}"));
    let _ = fs::remove_dir_all(scan_dir);

    result?
}

/// Validate display metadata before opening native capture UI. Keeping this at
/// the command boundary prevents a completed scan from failing only when its
/// staged PDF enters the upload store.
fn normalize_scanner_title(title: String) -> Result<String, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("Document title cannot be empty".into());
    }
    if title.chars().count() > MAX_SCANNER_TITLE_CHARS {
        return Err(format!(
            "Document title cannot exceed {MAX_SCANNER_TITLE_CHARS} characters"
        ));
    }
    Ok(title.to_owned())
}

/// Give every native source a human-readable filename inside a unique folder,
/// so import metadata stays useful without collision-prone renames.
fn create_capture_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    let sequence = NEXT_CAPTURE_ID.fetch_add(1, Ordering::Relaxed);
    let timestamp = crate::document_uploads::now_ms()?;
    let scan_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Failed to resolve app data for document capture: {error}"))?
        .join("document-scanner")
        .join("inbox")
        .join(format!("scan-{timestamp}-{sequence}"));
    fs::create_dir_all(&scan_dir)
        .map_err(|error| format!("Failed to prepare document capture storage: {error}"))?;
    Ok(scan_dir)
}

/// Confirm that a native adapter returned the exact staged file requested and
/// that its basic shape can safely enter the canonical PDF importer.
fn validate_mobile_output(result: &ScanResult, path: &Path) -> Result<(), String> {
    if Path::new(&result.output_path) != path {
        return Err("The mobile document import returned an unexpected output path".into());
    }
    let bytes = fs::metadata(path)
        .map_err(|error| {
            format!("The mobile document import did not create a readable PDF: {error}")
        })?
        .len();
    validate_mobile_output_limits(result.page_count, bytes)?;
    Ok(())
}

/// Recheck native output limits in Rust because mobile adapters are an IPC
/// trust boundary, even though both platform implementations reject the same
/// limits earlier to avoid wasting device resources.
fn validate_mobile_output_limits(page_count: usize, bytes: u64) -> Result<(), String> {
    if page_count == 0 {
        return Err("The mobile document source did not contain any pages".into());
    }
    if page_count > MAX_SCANNER_PAGES {
        return Err(format!(
            "The mobile document source exceeds the {MAX_SCANNER_PAGES}-page limit"
        ));
    }
    if bytes == 0 {
        return Err("The mobile document import created an empty PDF".into());
    }
    if bytes > MAX_PDF_UPLOAD_BYTES {
        return Err("The mobile document import exceeds the 250 MB PDF limit".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{normalize_scanner_title, validate_mobile_output_limits, MAX_SCANNER_PAGES};
    use crate::document_uploads::MAX_PDF_UPLOAD_BYTES;

    #[test]
    fn scanner_title_is_trimmed_and_bounded() {
        assert_eq!(
            normalize_scanner_title("  My Scan  ".into()).unwrap(),
            "My Scan"
        );
        assert!(normalize_scanner_title("   ".into()).is_err());
        assert!(normalize_scanner_title("a".repeat(513)).is_err());
    }

    #[test]
    fn scanner_output_limits_reject_empty_or_oversized_results() {
        assert!(validate_mobile_output_limits(0, 1).is_err());
        assert!(validate_mobile_output_limits(1, 0).is_err());
        assert!(validate_mobile_output_limits(MAX_SCANNER_PAGES + 1, 1).is_err());
        assert!(validate_mobile_output_limits(1, MAX_PDF_UPLOAD_BYTES + 1).is_err());
        assert!(validate_mobile_output_limits(MAX_SCANNER_PAGES, MAX_PDF_UPLOAD_BYTES).is_ok());
    }
}
