//! ONNX Runtime execution-provider capability discovery.

#[cfg(all(
    feature = "native-text-preprocessing-core",
    any(target_os = "macos", target_os = "ios")
))]
use ort::CoreMLExecutionProvider;
#[cfg(all(feature = "native-text-preprocessing-core", windows))]
use ort::DirectMLExecutionProvider;
#[cfg(feature = "native-text-preprocessing-core")]
use ort::ExecutionProvider;
#[cfg(all(
    feature = "native-text-preprocessing-core",
    any(target_os = "linux", windows)
))]
use ort::{CUDAExecutionProvider, TensorRTExecutionProvider};
#[cfg(all(feature = "native-text-preprocessing-core", target_os = "android"))]
use ort::{NNAPIExecutionProvider, XNNPACKExecutionProvider};

#[cfg(feature = "native-text-preprocessing-core")]
use super::preprocess::initialize_ort;

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
    let mut providers = vec!["cpu"];

    #[cfg(feature = "native-text-preprocessing-core")]
    {
        initialize_ort()?;

        #[cfg(any(target_os = "linux", windows))]
        push_available_provider(&mut providers, "cuda", &CUDAExecutionProvider::default())?;
        #[cfg(any(target_os = "linux", windows))]
        push_available_provider(&mut providers, "trt", &TensorRTExecutionProvider::default())?;
        #[cfg(windows)]
        push_available_provider(
            &mut providers,
            "directml",
            &DirectMLExecutionProvider::default(),
        )?;
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        push_available_provider(
            &mut providers,
            "coreml",
            &CoreMLExecutionProvider::default(),
        )?;
        #[cfg(target_os = "android")]
        push_available_provider(&mut providers, "nnapi", &NNAPIExecutionProvider::default())?;
        #[cfg(target_os = "android")]
        push_available_provider(
            &mut providers,
            "xnnpack",
            &XNNPACKExecutionProvider::default(),
        )?;
    }

    Ok(providers)
}

#[cfg(feature = "native-text-preprocessing-core")]
/// Append a provider only when the loaded ONNX Runtime reports it as compiled.
fn push_available_provider(
    providers: &mut Vec<&'static str>,
    sherpa_id: &'static str,
    provider: &dyn ExecutionProvider,
) -> Result<(), String> {
    if provider
        .is_available()
        .map_err(|error| format!("Failed to inspect ONNX Runtime providers: {error}"))?
    {
        providers.push(sherpa_id);
    }
    Ok(())
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
