//! ONNX Runtime execution-provider capability discovery.

/// Return the provider selected by this artifact, defaulting unknown builds to CPU.
pub(super) fn default_sherpa_execution_provider() -> &'static str {
    normalize_default_provider(option_env!("PAPERCUT_SHERPA_DEFAULT_PROVIDER"))
}

fn normalize_default_provider(provider: Option<&str>) -> &'static str {
    match provider {
        Some("cuda") => "cuda",
        Some("coreml") => "coreml",
        _ => "cpu",
    }
}

/// Ask the packaged ONNX Runtime which sherpa provider families it contains.
///
/// This is a build capability probe, not a model compatibility guarantee:
/// providers still need per-model/device validation before the UI can offer
/// them. CPU is always present and remains the universal fallback.
pub(super) fn compiled_sherpa_execution_providers() -> Result<Vec<&'static str>, String> {
    #[cfg(not(feature = "native-text-preprocessing-core"))]
    return Ok(vec!["cpu"]);

    #[cfg(feature = "native-text-preprocessing-core")]
    {
        use std::{ffi::CStr, ptr};

        crate::onnx_runtime::initialize()?;
        let api = ort::api();
        let mut names = ptr::null_mut();
        let mut count = 0;
        let mut providers = vec!["cpu"];
        // ORT owns these count NUL-terminated names until ReleaseAvailableProviders.
        unsafe {
            ort::Error::result_from_status((api.GetAvailableProviders)(&mut names, &mut count))
                .map_err(|e| format!("Failed to inspect ONNX Runtime providers: {e}"))?;
            if !names.is_null() {
                for index in 0..count {
                    match CStr::from_ptr(*names.add(index as usize)).to_bytes() {
                        #[cfg(any(target_os = "linux", windows))]
                        b"CUDAExecutionProvider" => providers.push("cuda"),
                        #[cfg(any(target_os = "linux", windows))]
                        b"TensorrtExecutionProvider" => providers.push("trt"),
                        #[cfg(windows)]
                        b"DmlExecutionProvider" => providers.push("directml"),
                        #[cfg(any(target_os = "macos", target_os = "ios"))]
                        b"CoreMLExecutionProvider" => providers.push("coreml"),
                        #[cfg(target_os = "android")]
                        b"NnapiExecutionProvider" => providers.push("nnapi"),
                        #[cfg(target_os = "android")]
                        b"XnnpackExecutionProvider" => providers.push("xnnpack"),
                        _ => {}
                    }
                }
                ort::Error::result_from_status((api.ReleaseAvailableProviders)(names, count))
                    .map_err(|e| format!("Failed to release ONNX Runtime provider list: {e}"))?;
            }
        }
        Ok(providers)
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_default_provider;

    #[test]
    fn provider_defaults_are_bounded_to_supported_artifacts() {
        assert_eq!(normalize_default_provider(None), "cpu");
        assert_eq!(normalize_default_provider(Some("cuda")), "cuda");
        assert_eq!(normalize_default_provider(Some("coreml")), "coreml");
        assert_eq!(normalize_default_provider(Some("unknown")), "cpu");
    }
}
