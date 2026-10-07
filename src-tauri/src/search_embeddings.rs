//! Default-off, session-only provider prototype. It does not participate in search.
//! Models must already be staged in the app-owned cache; no implicit downloads.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ModelFormat {
    Float32,
    DynamicInt8,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderStatus {
    compiled: bool,
    platform: &'static str,
    active_format: Option<ModelFormat>,
    loading_format: Option<ModelFormat>,
    encoder_key: Option<String>,
}

#[cfg(feature = "experimental-search-embeddings-core")]
pub(crate) use provider::SearchEmbeddingState;
#[cfg(not(feature = "experimental-search-embeddings-core"))]
#[derive(Clone, Default)]
pub(crate) struct SearchEmbeddingState {}

impl SearchEmbeddingState {
    fn status(&self) -> Result<ProviderStatus, String> {
        #[cfg(feature = "experimental-search-embeddings-core")]
        let (active_format, loading_format) = {
            let state = self.0.lock().map_err(|_| "Embedding state lock failed")?;
            (
                state.active.as_ref().map(|loaded| loaded.format),
                state.loading.as_ref().map(|(format, _)| *format),
            )
        };
        #[cfg(not(feature = "experimental-search-embeddings-core"))]
        let (active_format, loading_format) = (None, None);
        Ok(ProviderStatus {
            compiled: cfg!(feature = "experimental-search-embeddings-core"),
            platform: std::env::consts::OS,
            active_format,
            loading_format,
            #[cfg(feature = "experimental-search-embeddings-core")]
            encoder_key: active_format.map(ModelFormat::encoder_key),
            #[cfg(not(feature = "experimental-search-embeddings-core"))]
            encoder_key: None,
        })
    }

    fn disable(&self) -> Result<ProviderStatus, String> {
        #[cfg(feature = "experimental-search-embeddings-core")]
        {
            let mut state = self.0.lock().map_err(|_| "Embedding state lock failed")?;
            if let Some((_, cancel)) = &state.loading {
                cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            // Keep the slot locked through native cleanup to prevent overlapping loads.
            // This command runs on the blocking pool, not the async/UI thread.
            state.active = None;
        }
        self.status()
    }
}

#[tauri::command]
pub(crate) fn search_embedding_status(
    state: tauri::State<'_, SearchEmbeddingState>,
) -> Result<ProviderStatus, String> {
    state.status()
}

/// `null` disables/unloads. A format explicitly activates that verified cache.
/// Availability comes from build features, not a desktop-only platform check.
#[tauri::command]
pub(crate) async fn search_embedding_configure(
    app: tauri::AppHandle,
    state: tauri::State<'_, SearchEmbeddingState>,
    format: Option<ModelFormat>,
) -> Result<ProviderStatus, String> {
    let Some(format) = format else {
        let state = state.inner().clone();
        return tauri::async_runtime::spawn_blocking(move || state.disable())
            .await
            .map_err(|err| format!("Embedding disable task failed: {err}"))?;
    };
    #[cfg(feature = "experimental-search-embeddings-core")]
    {
        use tauri::Manager;
        let dir = app
            .path()
            .app_data_dir()
            .map_err(|err| err.to_string())?
            .join("models/search")
            .join(format.cache_key());
        let state = state.inner().clone();
        tauri::async_runtime::spawn_blocking(move || state.activate(&dir, format))
            .await
            .map_err(|err| format!("Embedding activation task failed: {err}"))?
    }
    #[cfg(not(feature = "experimental-search-embeddings-core"))]
    {
        let _ = (app, format);
        Err("The experimental embedding provider is not compiled in this build".into())
    }
}

#[cfg(feature = "experimental-search-embeddings-core")]
mod provider {
    use super::{ModelFormat, ProviderStatus};
    use fastembed::{
        InitOptionsUserDefined, Pooling, QuantizationMode, TextEmbedding, TokenizerFiles,
        UserDefinedEmbeddingModel,
    };
    use sha2::{Digest, Sha256};
    use std::{
        fs::File,
        io::Read,
        panic::{catch_unwind, AssertUnwindSafe},
        path::Path,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc, Mutex,
        },
    };

    const REVISION: &str = "614241f622f53c4eeff9890bdc4f31cfecc418b3";
    const DIMENSIONS: usize = 384;
    const MAX_TOKENS: usize = 512;

    impl ModelFormat {
        pub(super) fn cache_key(self) -> String {
            let format = match self {
                Self::Float32 => "float32",
                Self::DynamicInt8 => "dynamic-int8",
            };
            format!("multilingual-e5-small/{REVISION}/{format}")
        }

        pub(super) fn encoder_key(self) -> String {
            // Index identity must ALSO include source/extraction/chunking versions.
            format!(
                "{}/{}/mean-e5-prefix-v1/384/512/single-text",
                self.cache_key(),
                self.model_file().1
            )
        }

        fn model_file(self) -> (u64, &'static str) {
            match self {
                Self::Float32 => (
                    470_268_510,
                    "ca456c06b3a9505ddfd9131408916dd79290368331e7d76bb621f1cba6bc8665",
                ),
                Self::DynamicInt8 => (
                    118_346_824,
                    "dd476dd0c2514e9b9be83aeb3853fac0763e0bdf4a71645407587d77c48a2d88",
                ),
            }
        }
    }

    #[derive(Clone, Default)]
    pub(crate) struct SearchEmbeddingState(pub(super) Arc<Mutex<ProviderState>>);

    #[derive(Default)]
    pub(super) struct ProviderState {
        pub(super) active: Option<LoadedProvider>,
        pub(super) loading: Option<(ModelFormat, Arc<AtomicBool>)>,
    }

    pub(super) struct LoadedProvider {
        pub(super) format: ModelFormat,
        model: TextEmbedding,
    }

    impl SearchEmbeddingState {
        pub(super) fn activate(
            &self,
            dir: &Path,
            format: ModelFormat,
        ) -> Result<ProviderStatus, String> {
            self.activate_with(format, |cancel| LoadedProvider::load(dir, format, cancel))
        }

        fn activate_with(
            &self,
            format: ModelFormat,
            load: impl FnOnce(&AtomicBool) -> Result<LoadedProvider, String>,
        ) -> Result<ProviderStatus, String> {
            let cancel = Arc::new(AtomicBool::new(false));
            {
                let mut state = self.0.lock().map_err(|_| "Embedding state lock failed")?;
                if state.loading.is_some() {
                    return Err("An embedding activation is already running".into());
                }
                if let Some(loaded) = &state.active {
                    if loaded.format == format {
                        drop(state);
                        return self.status();
                    }
                    return Err(
                        "Disable embeddings before selecting a different model format".into(),
                    );
                }
                state.loading = Some((format, cancel.clone()));
            }
            // ponytail: one resident model and one loader; explicit disable before
            // switching avoids a second resident model and a rollback mechanism.
            let result = catch_unwind(AssertUnwindSafe(|| load(&cancel)))
                .unwrap_or_else(|_| Err("Embedding model activation panicked".into()));
            let mut state = self.0.lock().map_err(|_| "Embedding state lock failed")?;
            if let Err(err) = check_cancel(&cancel) {
                drop(result);
                state.loading = None;
                return Err(err);
            }
            state.loading = None;
            // Publish only after verification and a successful inference.
            state.active = Some(result?);
            drop(state);
            self.status()
        }
    }

    fn check_cancel(cancel: &AtomicBool) -> Result<(), String> {
        if cancel.load(Ordering::Relaxed) {
            Err("Embedding activation cancelled".into())
        } else {
            Ok(())
        }
    }

    /// Check exactly the bytes supplied to ORT, including every tokenizer asset.
    fn read_verified(
        path: &Path,
        expected_bytes: u64,
        expected_sha256: &str,
        cancel: &AtomicBool,
    ) -> Result<Vec<u8>, String> {
        check_cancel(cancel)?;
        let mut file = File::open(path)
            .map_err(|err| format!("Missing embedding asset {}: {err}", path.display()))?;
        let metadata = file.metadata().map_err(|err| err.to_string())?;
        if !metadata.is_file() || metadata.len() != expected_bytes {
            return Err(format!(
                "Incorrect embedding asset size: {}",
                path.display()
            ));
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(
                usize::try_from(expected_bytes)
                    .map_err(|_| "Embedding asset exceeds addressable memory")?,
            )
            .map_err(|err| format!("Insufficient memory for embedding asset: {err}"))?;
        let mut hash = Sha256::new();
        let mut buffer = [0; 64 * 1024];
        loop {
            check_cancel(cancel)?;
            let count = file.read(&mut buffer).map_err(|err| err.to_string())?;
            if count == 0 {
                break;
            }
            if bytes.len() as u64 + count as u64 > expected_bytes {
                return Err("Embedding asset changed while reading".into());
            }
            hash.update(&buffer[..count]);
            bytes.extend_from_slice(&buffer[..count]);
        }
        if bytes.len() as u64 != expected_bytes
            || format!("{:x}", hash.finalize()) != expected_sha256
        {
            return Err(format!(
                "Incorrect embedding asset checksum: {}",
                path.display()
            ));
        }
        Ok(bytes)
    }

    impl LoadedProvider {
        fn load(dir: &Path, format: ModelFormat, cancel: &AtomicBool) -> Result<Self, String> {
            let read = |name, size, hash| read_verified(&dir.join(name), size, hash, cancel);
            let files = TokenizerFiles {
                config_file: read(
                    "config.json",
                    655,
                    "69137736cab8b8903a07fe8afaafdda25aac55415a12a55d1bffa9f581abf959",
                )?,
                tokenizer_file: read(
                    "tokenizer.json",
                    17_082_730,
                    "0b44a9d7b51c3c62626640cda0e2c2f70fdacdc25bbbd68038369d14ebdf4c39",
                )?,
                special_tokens_map_file: read(
                    "special_tokens_map.json",
                    167,
                    "d05497f1da52c5e09554c0cd874037a083e1dc1b9cfd48034d1c717f1afc07a7",
                )?,
                tokenizer_config_file: read(
                    "tokenizer_config.json",
                    443,
                    "a1d6bc8734a6f635dc158508bef000f8e2e5a759c7d92f984b2c86e5ff53425b",
                )?,
            };
            let (size, hash) = format.model_file();
            let quantization = match format {
                ModelFormat::Float32 => QuantizationMode::None,
                ModelFormat::DynamicInt8 => QuantizationMode::Dynamic,
            };
            let bytes = read("onnx/model.onnx", size, hash)?;
            check_cancel(cancel)?;
            crate::onnx_runtime::initialize()?;
            let threads = if cfg!(any(target_os = "android", target_os = "ios")) {
                1
            } else {
                std::thread::available_parallelism().map_or(1, |n| n.get().min(2))
            };
            let mut model = TextEmbedding::try_new_from_user_defined(
                UserDefinedEmbeddingModel::new(bytes, files)
                    .with_pooling(Pooling::Mean)
                    .with_quantization(quantization),
                InitOptionsUserDefined::new()
                    .with_max_length(MAX_TOKENS)
                    .with_intra_threads(threads),
            )
            .map_err(|err| err.to_string())?;
            model
                .tokenizer
                .with_truncation(None)
                .map_err(|err| err.to_string())?;
            model.tokenizer.with_padding(None);
            let mut loaded = Self { format, model };
            check_cancel(cancel)?;
            loaded.encode("query: public transport")?;
            check_cancel(cancel)?;
            Ok(loaded)
        }

        // Caller supplies the E5 query/passage prefix; one text per inference
        // preserves the retained dynamic-INT8 policy. Never truncate silently.
        fn encode(&mut self, text: &str) -> Result<Vec<f32>, String> {
            let content = text
                .strip_prefix("query: ")
                .or_else(|| text.strip_prefix("passage: "))
                .ok_or("Embedding input requires an E5 query or passage prefix")?;
            if content.trim().is_empty() || text.chars().count() > 4096 {
                return Err("Embedding input is empty or exceeds 4096 characters".into());
            }
            let tokens = self
                .model
                .tokenizer
                .encode(text, true)
                .map_err(|err| err.to_string())?;
            if tokens.len() > MAX_TOKENS {
                return Err("Embedding input exceeds the model's 512-token limit".into());
            }
            let mut vectors = self
                .model
                .embed([text], None)
                .map_err(|err| err.to_string())?;
            if vectors.len() != 1 {
                return Err("Incorrect embedding output count".into());
            }
            let vector = vectors.remove(0);
            let norm = vector.iter().map(|n| n * n).sum::<f32>().sqrt();
            if vector.len() != DIMENSIONS
                || vector.iter().any(|n| !n.is_finite())
                || (norm - 1.0).abs() > 1e-4
            {
                return Err("Invalid embedding vector".into());
            }
            Ok(vector)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn asset_verification_rejects_missing_wrong_size_and_changed_bytes() {
            let dir = std::env::temp_dir()
                .join(format!("papercut-provider-assets-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let path = dir.join("asset");
            let cancel = AtomicBool::new(false);
            let hash = format!("{:x}", Sha256::digest(b"valid"));
            assert!(read_verified(&dir.join("missing"), 5, &hash, &cancel).is_err());
            std::fs::write(&path, b"bad").unwrap();
            assert!(read_verified(&path, 5, &hash, &cancel).is_err());
            std::fs::write(&path, b"wrong").unwrap();
            assert!(read_verified(&path, 5, &hash, &cancel).is_err());
            std::fs::write(&path, b"valid").unwrap();
            assert_eq!(read_verified(&path, 5, &hash, &cancel).unwrap(), b"valid");
            cancel.store(true, Ordering::Relaxed);
            assert!(read_verified(&path, 5, &hash, &cancel).is_err());
            std::fs::remove_dir_all(dir).unwrap();
        }

        #[test]
        fn disable_cancels_pending_activation_and_failures_clear_busy_state() {
            let state = SearchEmbeddingState::default();
            let (ready_tx, ready_rx) = std::sync::mpsc::channel();
            let (resume_tx, resume_rx) = std::sync::mpsc::channel();
            std::thread::scope(|scope| {
                let worker_state = state.clone();
                let worker = scope.spawn(move || {
                    worker_state.activate_with(ModelFormat::Float32, |cancel| {
                        ready_tx.send(()).unwrap();
                        resume_rx.recv().unwrap();
                        check_cancel(cancel)?;
                        panic!("a cancelled activation must not reach inference");
                    })
                });
                ready_rx.recv().unwrap();
                assert_eq!(
                    state.status().unwrap().loading_format,
                    Some(ModelFormat::Float32)
                );
                assert!(state
                    .activate_with(ModelFormat::Float32, |_| panic!("duplicate load"))
                    .is_err());
                assert!(state.disable().unwrap().active_format.is_none());
                // A cancelled loader still owns its slot until it actually returns.
                assert!(state
                    .activate_with(ModelFormat::DynamicInt8, |_| panic!("overlapping load"))
                    .is_err());
                resume_tx.send(()).unwrap();
                assert_eq!(
                    worker.join().unwrap().unwrap_err(),
                    "Embedding activation cancelled"
                );
            });
            assert!(state.status().unwrap().loading_format.is_none());
            assert!(state
                .activate_with(ModelFormat::Float32, |_| Err("missing model".into()))
                .is_err());
            assert!(state
                .activate_with(ModelFormat::Float32, |_| panic!("invalid runtime"))
                .is_err());
            assert!(state.status().unwrap().loading_format.is_none());
            assert!(state.disable().unwrap().active_format.is_none());
        }

        #[test]
        #[ignore = "requires explicit cached FP32/INT8 assets and packaged ONNX Runtime"]
        fn cached_provider_activation_switch_failure_and_unload() {
            let root = std::env::var_os("PAPERCUT_EMBEDDING_TEST_MODELS")
                .expect("set test-only cached models directory");
            let only = std::env::var("PAPERCUT_EMBEDDING_TEST_FORMAT").ok();
            assert!(only
                .as_deref()
                .is_none_or(|format| matches!(format, "float32" | "dynamic-int8")));
            let state = SearchEmbeddingState::default();
            for (format, folder) in [
                (ModelFormat::Float32, "float32"),
                (ModelFormat::DynamicInt8, "dynamic-int8"),
            ] {
                if only.as_deref().is_some_and(|requested| requested != folder) {
                    continue;
                }
                let dir = Path::new(&root).join(folder);
                eprintln!("activation start: {format:?}");
                let status = state.activate(&dir, format).unwrap();
                eprintln!("activation ready: {format:?}");
                assert_eq!(status.active_format, Some(format));
                assert_eq!(status.encoder_key, Some(format.encoder_key()));
                // An already loaded, unchanged selection does not read disk again.
                state.activate(Path::new("missing"), format).unwrap();
                let other = match format {
                    ModelFormat::Float32 => ModelFormat::DynamicInt8,
                    ModelFormat::DynamicInt8 => ModelFormat::Float32,
                };
                assert!(state
                    .activate(Path::new("missing"), other)
                    .unwrap_err()
                    .contains("Disable embeddings"));
                assert_eq!(state.status().unwrap().active_format, Some(format));
                #[cfg(target_os = "linux")]
                {
                    let maps = std::fs::read_to_string("/proc/self/maps").unwrap();
                    let runtimes = maps
                        .lines()
                        .filter_map(|line| line.split_whitespace().last())
                        .filter(|path| path.contains("libonnxruntime.so"))
                        .collect::<std::collections::BTreeSet<_>>();
                    assert_eq!(
                        runtimes.len(),
                        1,
                        "native consumers must share one ORT image"
                    );
                }
                {
                    let mut inner = state.0.lock().unwrap();
                    let loaded = inner.active.as_mut().unwrap();
                    eprintln!("inference controls: {format:?}");
                    let first = loaded.encode("query: public transport").unwrap();
                    assert_eq!(first, loaded.encode("query: public transport").unwrap());
                    loaded
                        .encode("passage: Residents travel by electric bus.")
                        .unwrap();
                    loaded.encode("query: النقل العام في المدينة").unwrap();
                    assert!(loaded.encode("").is_err());
                    assert!(loaded.encode("public transport").is_err());
                    assert!(loaded.encode("query: ").is_err());
                    assert!(loaded
                        .encode(&format!("query: {}", "a".repeat(4097)))
                        .unwrap_err()
                        .contains("4096"));
                    assert!(loaded
                        .encode(&format!("query: {}", "bus ".repeat(600)))
                        .unwrap_err()
                        .contains("512-token"));
                }
                eprintln!("inference controls complete: {format:?}");
                assert!(state.disable().unwrap().active_format.is_none());
                assert!(state.0.lock().unwrap().active.is_none());
                assert!(state.activate(Path::new("missing"), other).is_err());
                assert!(state.status().unwrap().active_format.is_none());
                if format == ModelFormat::DynamicInt8 {
                    // Disable during non-interruptible native setup: discard the
                    // completed session instead of publishing it after opting out.
                    let result = state.activate_with(format, |cancel| {
                        let loaded = LoadedProvider::load(&dir, format, cancel)?;
                        state.disable()?;
                        Ok(loaded)
                    });
                    assert_eq!(result.unwrap_err(), "Embedding activation cancelled");
                    assert!(state.status().unwrap().active_format.is_none());
                    assert!(state.status().unwrap().loading_format.is_none());
                }
                // Restart semantics: opting in is not persisted or auto-loaded.
                assert!(SearchEmbeddingState::default()
                    .status()
                    .unwrap()
                    .active_format
                    .is_none());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_is_disabled_without_startup_io_and_capabilities_are_truthful() {
        let state = SearchEmbeddingState::default();
        let status = state.status().unwrap();
        assert_eq!(
            status.compiled,
            cfg!(feature = "experimental-search-embeddings-core")
        );
        assert!(status.active_format.is_none());
        assert!(status.encoder_key.is_none());
        assert!(status.loading_format.is_none());
        state.disable().unwrap();
        assert!(serde_json::from_str::<ModelFormat>("\"unknown-model\"").is_err());
    }
}
