use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use tauri::{AppHandle, Manager, Runtime, State};
use tauri_plugin_document_scanner::{DocumentScannerExt, ScanResult, ScannerAvailability};

use crate::document_uploads::{
    import_scanner_source, DocumentUploadState, UploadedDocumentBatchResult,
};

static NEXT_CAPTURE_ID: AtomicU64 = AtomicU64::new(0);

#[tauri::command]
pub(crate) async fn document_scanner_availability<R: Runtime>(
    app: AppHandle<R>,
) -> Result<ScannerAvailability, String> {
    tauri::async_runtime::spawn_blocking(move || app.document_scanner().availability())
        .await
        .map_err(|error| format!("Document scanner availability task failed: {error}"))?
}

/// Capture to a private staging PDF, then reuse the normal upload transaction.
/// A failed import retains the capture below the scanner inbox for later
/// recovery; successful imports remove the redundant staged source.
#[tauri::command]
pub(crate) async fn document_scanner_scan<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, DocumentUploadState>,
) -> Result<UploadedDocumentBatchResult, String> {
    import_mobile_source(app, state, MobileSource::Camera).await
}

#[tauri::command]
pub(crate) async fn document_scanner_import_images<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, DocumentUploadState>,
) -> Result<UploadedDocumentBatchResult, String> {
    import_mobile_source(app, state, MobileSource::Photos).await
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
) -> Result<UploadedDocumentBatchResult, String> {
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
        import_scanner_source(import_app, control, import_path)
    })
    .await
    .map_err(|error| format!("Mobile document import task failed: {error}"))??;

    if result.imported.len() == 1 && result.failures.is_empty() && !result.cancelled {
        let _ = fs::remove_dir_all(scan_dir);
    }
    Ok(result)
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

fn validate_mobile_output(result: &ScanResult, path: &Path) -> Result<(), String> {
    if result.page_count == 0 {
        return Err("The mobile document source did not contain any pages".into());
    }
    if Path::new(&result.output_path) != path {
        return Err("The mobile document import returned an unexpected output path".into());
    }
    let bytes = fs::metadata(path)
        .map_err(|error| {
            format!("The mobile document import did not create a readable PDF: {error}")
        })?
        .len();
    if bytes == 0 {
        return Err("The mobile document import created an empty PDF".into());
    }
    Ok(())
}
