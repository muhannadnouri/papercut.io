//! One runtime initializer for optional native consumers, including linked iOS builds.

use std::{panic::catch_unwind, sync::OnceLock};

/// Initialize on first native use; all Rust consumers must share the same environment.
pub(crate) fn initialize() -> Result<(), String> {
    static RESULT: OnceLock<Result<(), String>> = OnceLock::new();
    RESULT
        .get_or_init(|| {
            catch_unwind(|| {
                #[cfg(any(
                    feature = "native-text-preprocessing-dynamic",
                    feature = "experimental-search-embeddings",
                    feature = "native-translation-alignment-runtime"
                ))]
                {
                    let library = format!(
                        "{}onnxruntime{}",
                        std::env::consts::DLL_PREFIX,
                        std::env::consts::DLL_SUFFIX
                    );
                    ort::init_from(library)
                        .map_err(|error| format!("Failed to load ONNX Runtime: {error}"))?
                        .commit();
                }
                #[cfg(not(any(
                    feature = "native-text-preprocessing-dynamic",
                    feature = "experimental-search-embeddings",
                    feature = "native-translation-alignment-runtime"
                )))]
                ort::init().commit();
                #[cfg(target_os = "android")]
                {
                    // shortcut: rc.13's exit hook can release the environment after native globals;
                    // remove this guard when an upstream teardown fix is adopted and validated.
                    // Only the global environment survives; disabled model sessions still drop.
                    static ENVIRONMENT: OnceLock<std::sync::Arc<ort::environment::Environment>> =
                        OnceLock::new();
                    let environment = ort::environment::Environment::current()
                        .map_err(|error| format!("Failed to initialize ONNX Runtime: {error}"))?;
                    ENVIRONMENT.get_or_init(|| environment);
                }
                Ok(())
            })
            .map_err(|_| {
                "ONNX Runtime initialization panicked; restart after repairing the runtime"
                    .to_string()
            })?
        })
        .clone()
}
