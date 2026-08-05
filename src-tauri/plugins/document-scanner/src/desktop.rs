use std::{marker::PhantomData, path::Path};
use tauri::{plugin::PluginApi, AppHandle, Runtime};

use crate::{ScanResult, ScannerAvailability};

pub struct DocumentScanner<R: Runtime>(PhantomData<fn() -> R>);

impl<R: Runtime> DocumentScanner<R> {
    pub(crate) fn new<C: serde::de::DeserializeOwned>(
        _app: &AppHandle<R>,
        _api: PluginApi<R, C>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self(PhantomData))
    }

    pub(crate) fn platform_availability(&self) -> Result<ScannerAvailability, String> {
        Ok(ScannerAvailability {
            supported: false,
            photo_import_supported: false,
            platform: std::env::consts::OS.into(),
            reason: Some("Document capture is available on supported mobile devices".into()),
        })
    }

    pub(crate) fn platform_scan_to(&self, _output_path: &Path) -> Result<ScanResult, String> {
        Err("Document capture is available on supported mobile devices".into())
    }

    pub(crate) fn platform_import_images_to(
        &self,
        _output_path: &Path,
    ) -> Result<ScanResult, String> {
        Err("Photo import is available on supported mobile devices".into())
    }
}
