//! One runtime initializer for optional native consumers, including linked iOS builds.

use std::{panic::catch_unwind, sync::OnceLock};

pub(crate) fn initialize() -> Result<(), String> {
    static RESULT: OnceLock<Result<(), String>> = OnceLock::new();
    RESULT
        .get_or_init(|| {
            catch_unwind(|| {
                #[cfg(any(
                    feature = "native-text-preprocessing-dynamic",
                    feature = "experimental-search-embeddings"
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
                    feature = "experimental-search-embeddings"
                )))]
                ort::init().commit();
                Ok(())
            })
            .map_err(|_| {
                "ONNX Runtime initialization panicked; restart after repairing the runtime"
                    .to_string()
            })?
        })
        .clone()
}
