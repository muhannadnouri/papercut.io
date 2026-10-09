//! Default-off, session-only provider prototype. It does not participate in search.
//! Model downloads are explicit and never activate the provider.

use serde::{Deserialize, Serialize};

#[cfg(feature = "experimental-search-embeddings-core")]
mod cache;

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
    downloading_format: Option<ModelFormat>,
    encoder_key: Option<String>,
}

#[cfg(feature = "experimental-search-embeddings-core")]
pub(crate) use provider::{SearchEmbeddingState, DIMENSIONS};
#[cfg(not(feature = "experimental-search-embeddings-core"))]
#[derive(Clone, Default)]
pub(crate) struct SearchEmbeddingState {}

impl SearchEmbeddingState {
    fn status(&self) -> Result<ProviderStatus, String> {
        #[cfg(feature = "experimental-search-embeddings-core")]
        let (active_format, loading_format, downloading_format) = {
            let state = self.0.lock().map_err(|_| "Embedding state lock failed")?;
            (
                state.active.as_ref().map(|loaded| loaded.format),
                state.loading.as_ref().map(|(format, _)| *format),
                state.downloading.as_ref().map(|(format, _)| *format),
            )
        };
        #[cfg(not(feature = "experimental-search-embeddings-core"))]
        let (active_format, loading_format, downloading_format) = (None, None, None);
        Ok(ProviderStatus {
            compiled: cfg!(feature = "experimental-search-embeddings-core"),
            platform: std::env::consts::OS,
            active_format,
            loading_format,
            downloading_format,
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
            if let Some((_, cancel)) = &state.downloading {
                cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            // Keep the slot locked through native cleanup to prevent overlapping loads.
            // This command runs on the blocking pool, not the async/UI thread.
            state.active = None;
        }
        self.status()
    }
}

#[cfg(feature = "experimental-search-embeddings-core")]
fn cache_dir<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    format: ModelFormat,
) -> Result<std::path::PathBuf, String> {
    use tauri::Manager;
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|err| err.to_string())?
        .join("models/search")
        .join(format.cache_key()))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CacheStatus {
    format: ModelFormat,
    files_present: bool,
    expected_bytes: u64,
}

/// Presence/size information only. Activation always rechecks actual checksums.
#[tauri::command]
pub(crate) async fn search_embedding_cache_status<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<CacheStatus>, String> {
    #[cfg(feature = "experimental-search-embeddings-core")]
    {
        let dirs = [ModelFormat::Float32, ModelFormat::DynamicInt8]
            .map(|format| cache_dir(&app, format).map(|dir| (format, dir)));
        tauri::async_runtime::spawn_blocking(move || {
            dirs.into_iter()
                .map(|entry| {
                    let (format, dir) = entry?;
                    Ok(CacheStatus {
                        format,
                        files_present: cache::files_present(&dir, &format.assets()),
                        expected_bytes: format.installed_bytes(),
                    })
                })
                .collect()
        })
        .await
        .map_err(|err| err.to_string())?
    }
    #[cfg(not(feature = "experimental-search-embeddings-core"))]
    {
        let _ = app;
        Err("The experimental embedding provider is not compiled in this build".into())
    }
}

#[tauri::command]
pub(crate) async fn search_embedding_download<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, SearchEmbeddingState>,
    format: ModelFormat,
) -> Result<ProviderStatus, String> {
    #[cfg(feature = "experimental-search-embeddings-core")]
    {
        let dir = cache_dir(&app, format)?;
        let state = state.inner().clone();
        tauri::async_runtime::spawn_blocking(move || state.download(&app, &dir, format))
            .await
            .map_err(|err| err.to_string())?
    }
    #[cfg(not(feature = "experimental-search-embeddings-core"))]
    {
        let _ = (app, state, format);
        Err("The experimental embedding provider is not compiled in this build".into())
    }
}

#[tauri::command]
pub(crate) async fn search_embedding_remove<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, SearchEmbeddingState>,
    format: ModelFormat,
) -> Result<ProviderStatus, String> {
    #[cfg(feature = "experimental-search-embeddings-core")]
    {
        let dir = cache_dir(&app, format)?;
        let state = state.inner().clone();
        tauri::async_runtime::spawn_blocking(move || state.remove(&dir, format))
            .await
            .map_err(|err| err.to_string())?
    }
    #[cfg(not(feature = "experimental-search-embeddings-core"))]
    {
        let _ = (app, state, format);
        Err("The experimental embedding provider is not compiled in this build".into())
    }
}

/// Cancel download without unloading another active model. Cleanup finishes asynchronously.
#[tauri::command]
pub(crate) fn search_embedding_cancel_download(
    state: tauri::State<'_, SearchEmbeddingState>,
) -> Result<ProviderStatus, String> {
    #[cfg(feature = "experimental-search-embeddings-core")]
    {
        let inner = state.0.lock().map_err(|_| "Embedding state lock failed")?;
        if let Some((_, cancel)) = &inner.downloading {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }
    state.status()
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
pub(crate) async fn search_embedding_configure<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
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
        let dir = cache_dir(&app, format)?;
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
    #[cfg(test)]
    use sha2::{Digest, Sha256};
    use std::{
        fs::File,
        panic::{catch_unwind, AssertUnwindSafe},
        path::Path,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc, Mutex,
        },
    };

    pub(crate) const DIMENSIONS: usize = 384;
    const MAX_TOKENS: usize = 512;

    #[derive(Clone, Default)]
    pub(crate) struct SearchEmbeddingState(pub(super) Arc<Mutex<ProviderState>>);

    #[derive(Default)]
    pub(super) struct ProviderState {
        pub(super) active: Option<LoadedProvider>,
        pub(super) loading: Option<(ModelFormat, Arc<AtomicBool>)>,
        pub(super) downloading: Option<(ModelFormat, Arc<AtomicBool>)>,
    }

    pub(super) struct LoadedProvider {
        pub(super) format: ModelFormat,
        model: TextEmbedding,
    }

    impl SearchEmbeddingState {
        /// Include tokenizer/config checksums as well as the provider's model identity.
        pub(crate) fn index_encoder_key(&self) -> Result<String, String> {
            let state = self.0.lock().map_err(|_| "Embedding state lock failed")?;
            state
                .active
                .as_ref()
                .map(LoadedProvider::index_key)
                .ok_or_else(|| "Enable a verified embedding model before indexing".into())
        }

        /// Hold the provider lock per inference; status/disable may wait for that native call.
        pub(crate) fn index_encode(&self, expected: &str, text: &str) -> Result<Vec<f32>, String> {
            let mut state = self.0.lock().map_err(|_| "Embedding state lock failed")?;
            let loaded = state
                .active
                .as_mut()
                .ok_or("Embedding provider is disabled")?;
            if loaded.index_key() != expected {
                return Err("Semantic model changed; retry indexing".into());
            }
            loaded.encode(text)
        }

        /// Return authored UTF-8 spans using the active model tokenizer, with no silent truncation.
        pub(crate) fn index_passage_ranges(
            &self,
            expected: &str,
            title: &str,
            heading: Option<&str>,
            text: &str,
        ) -> Result<Vec<std::ops::Range<usize>>, String> {
            let state = self.0.lock().map_err(|_| "Embedding state lock failed")?;
            let loaded = state
                .active
                .as_ref()
                .ok_or("Embedding provider is disabled")?;
            if loaded.index_key() != expected {
                return Err("Semantic model changed; retry indexing".into());
            }
            loaded.passage_ranges(title, heading, text)
        }

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
                if state.loading.is_some() || state.downloading.is_some() {
                    return Err("An embedding activation or download is already running".into());
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
        super::cache::copy_verified(
            &mut file,
            &mut bytes,
            expected_bytes,
            expected_sha256,
            cancel,
            |_| {},
        )
        .map_err(|err| format!("{}: {err}", path.display()))?;
        Ok(bytes)
    }

    impl LoadedProvider {
        fn index_key(&self) -> String {
            format!(
                "{}/assets:{}",
                self.format.encoder_key(),
                self.format
                    .assets()
                    .iter()
                    .map(|asset| asset.sha256)
                    .collect::<Vec<_>>()
                    .join("/")
            )
        }

        /// Keep the experiment's 320-token/40-overlap policy, including metadata and E5 prefix.
        /// Source spans remain section-local; headers too large to leave body room fail explicitly.
        fn passage_ranges(
            &self,
            title: &str,
            heading: Option<&str>,
            text: &str,
        ) -> Result<Vec<std::ops::Range<usize>>, String> {
            use crate::document_uploads::semantic::store::{
                passage_input, CHUNK_TOKENS, OVERLAP_TOKENS,
            };
            let tokenizer = &self.model.tokenizer;
            let header = passage_input(title, heading, "");
            let header_tokens = tokenizer
                .encode(header.as_str(), false)
                .map_err(|err| err.to_string())?
                .len();
            let budget = CHUNK_TOKENS
                .checked_sub(header_tokens + 4)
                .filter(|budget| *budget > OVERLAP_TOKENS)
                .ok_or("Title/heading leaves too little room for semantic passage text")?;
            let encoding = tokenizer
                .encode(text, false)
                .map_err(|err| err.to_string())?;
            let offsets = encoding.get_offsets();
            let mut result = Vec::new();
            let mut first = 0;
            while first < offsets.len() {
                let last = (first + budget).min(offsets.len());
                let span = offsets[first].0..offsets[last - 1].1;
                let body = text
                    .get(span.clone())
                    .filter(|body| !body.is_empty())
                    .ok_or("Invalid semantic tokenizer offsets")?;
                let input = passage_input(title, heading, body);
                if input.chars().count() > 4096
                    || tokenizer
                        .encode(input.as_str(), true)
                        .map_err(|err| err.to_string())?
                        .len()
                        > CHUNK_TOKENS
                {
                    return Err("Semantic passage exceeds token/character budget".into());
                }
                result.push(span);
                if last == offsets.len() {
                    break;
                }
                first = last - OVERLAP_TOKENS;
            }
            Ok(result)
        }

        fn load(dir: &Path, format: ModelFormat, cancel: &AtomicBool) -> Result<Self, String> {
            let assets = format.assets();
            let read = |asset: &super::cache::Asset| {
                read_verified(&dir.join(asset.path), asset.bytes, asset.sha256, cancel)
            };
            let files = TokenizerFiles {
                config_file: read(&assets[0])?,
                tokenizer_file: read(&assets[1])?,
                special_tokens_map_file: read(&assets[2])?,
                tokenizer_config_file: read(&assets[3])?,
            };
            let quantization = match format {
                ModelFormat::Float32 => QuantizationMode::None,
                ModelFormat::DynamicInt8 => QuantizationMode::Dynamic,
            };
            let bytes = read(&assets[4])?;
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
            let cycles = std::env::var("PAPERCUT_EMBEDDING_TEST_CYCLES")
                .map(|value| value.parse::<usize>().expect("cycles must be an integer"))
                .unwrap_or(1);
            assert!((1..=20).contains(&cycles), "cycles must be in 1..=20");
            let root = std::env::var_os("PAPERCUT_EMBEDDING_TEST_MODELS")
                .expect("set test-only cached models directory");
            let only = std::env::var("PAPERCUT_EMBEDDING_TEST_FORMAT").ok();
            assert!(only
                .as_deref()
                .is_none_or(|format| matches!(format, "float32" | "dynamic-int8")));
            let state = SearchEmbeddingState::default();
            let mut baselines = Vec::new();
            for (cycle, (format, folder)) in (1..=cycles).flat_map(|cycle| {
                [
                    (ModelFormat::Float32, "float32"),
                    (ModelFormat::DynamicInt8, "dynamic-int8"),
                ]
                .map(|selection| (cycle, selection))
            }) {
                if only.as_deref().is_some_and(|requested| requested != folder) {
                    continue;
                }
                let dir = Path::new(&root).join(folder);
                eprintln!("lifecycle cycle: {cycle}/{cycles} {format:?}");
                eprintln!("activation start: {format:?}");
                let status = state.activate(&dir, format).unwrap();
                eprintln!("activation ready: {format:?}");
                assert!(state
                    .remove(Path::new("missing"), format)
                    .unwrap_err()
                    .contains("Disable embeddings"));
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
                    if let Some((_, expected)) = baselines.iter().find(|(key, _)| *key == format) {
                        assert_eq!(&first, expected, "vector changed across provider reloads");
                    } else {
                        baselines.push((format, first));
                    }
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
                eprintln!("provider disabled: {format:?}");
                assert!(state.activate(Path::new("missing"), other).is_err());
                assert!(state.status().unwrap().active_format.is_none());
                assert!(state.status().unwrap().loading_format.is_none());
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
                eprintln!("lifecycle cycle complete: {cycle}/{cycles} {format:?}");
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
        assert!(status.downloading_format.is_none());
        state.disable().unwrap();
        assert!(serde_json::from_str::<ModelFormat>("\"unknown-model\"").is_err());
    }
}

#[cfg(all(test, target_os = "linux"))]
mod ipc_tests;
