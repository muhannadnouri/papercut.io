//! Removable native document-capture boundary for Papercut mobile builds.

use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

#[cfg(desktop)]
mod desktop;
#[cfg(mobile)]
mod mobile;

#[cfg(desktop)]
use desktop::DocumentScanner;
#[cfg(mobile)]
use mobile::DocumentScanner;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScannerAvailability {
    pub supported: bool,
    pub photo_import_supported: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub output_path: String,
    pub page_count: usize,
}

/// Access the native scanner through Tauri's managed plugin state.
pub trait DocumentScannerExt<R: Runtime> {
    fn document_scanner(&self) -> &DocumentScanner<R>;
}

impl<R: Runtime, T: Manager<R>> DocumentScannerExt<R> for T {
    fn document_scanner(&self) -> &DocumentScanner<R> {
        self.state::<DocumentScanner<R>>().inner()
    }
}

impl<R: Runtime> DocumentScanner<R> {
    pub fn availability(&self) -> Result<ScannerAvailability, String> {
        self.platform_availability()
    }

    /// Ask the platform scanner to write one multi-page PDF directly to the
    /// supplied app-owned path. Image bytes never cross Tauri's JSON bridge.
    pub fn scan_to(&self, output_path: &Path) -> Result<ScanResult, String> {
        self.platform_scan_to(output_path)
    }

    /// Ask the native photo picker to convert selected images directly into
    /// one app-owned PDF without moving image bytes through Tauri IPC.
    pub fn import_images_to(&self, output_path: &Path) -> Result<ScanResult, String> {
        self.platform_import_images_to(output_path)
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("document-scanner")
        .setup(|app, api| {
            let scanner = DocumentScanner::new(app, api)?;
            app.manage(scanner);
            Ok(())
        })
        .build()
}
