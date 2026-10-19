use serde::{de::DeserializeOwned, Serialize};
use std::path::Path;
use tauri::{
    plugin::{PluginApi, PluginHandle},
    AppHandle, Runtime,
};

use crate::{ScanResult, ScannerAvailability};

#[cfg(target_os = "android")]
const PLUGIN_IDENTIFIER: &str = "io.papercut.documentscanner";

#[cfg(target_os = "ios")]
tauri::ios_plugin_binding!(init_plugin_document_scanner);

pub struct DocumentScanner<R: Runtime>(PluginHandle<R>);

impl<R: Runtime> DocumentScanner<R> {
    pub(crate) fn new<C: DeserializeOwned>(
        _app: &AppHandle<R>,
        api: PluginApi<R, C>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        #[cfg(target_os = "android")]
        let handle = api.register_android_plugin(PLUGIN_IDENTIFIER, "DocumentScannerPlugin")?;
        #[cfg(target_os = "ios")]
        let handle = api.register_ios_plugin(init_plugin_document_scanner)?;
        Ok(Self(handle))
    }

    pub(crate) fn platform_availability(&self) -> Result<ScannerAvailability, String> {
        self.0
            .run_mobile_plugin("availability", ())
            .map_err(|error| error.to_string())
    }

    pub(crate) fn platform_scan_to(&self, output_path: &Path) -> Result<ScanResult, String> {
        let output_path = output_path
            .to_str()
            .ok_or_else(|| "The scan destination is not valid UTF-8".to_string())?;
        self.0
            .run_mobile_plugin("scan", ScanArgs { output_path })
            .map_err(|error| error.to_string())
    }

    pub(crate) fn platform_import_images_to(
        &self,
        output_path: &Path,
    ) -> Result<ScanResult, String> {
        let output_path = output_path
            .to_str()
            .ok_or_else(|| "The photo import destination is not valid UTF-8".to_string())?;
        self.0
            .run_mobile_plugin("importImages", ScanArgs { output_path })
            .map_err(|error| error.to_string())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ScanArgs<'a> {
    output_path: &'a str,
}
