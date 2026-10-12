//! Default-off provider for opt-in indexing and conceptual queries; lexical search is independent.
//! Model downloads are explicit and never activate the provider.

use serde::{Deserialize, Serialize};

#[cfg(feature = "experimental-search-embeddings-core")]
mod cache;
#[cfg(feature = "experimental-search-embeddings-core")]
mod models;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
// Preserve existing serialized Small selections and saved preferences while adding model capacities.
pub(crate) enum ModelSelection {
    Float32,
    DynamicInt8,
    BaseFloat32,
    LargeFloat32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProviderStatus {
    compiled: bool,
    platform: &'static str,
    active_format: Option<ModelSelection>,
    loading_format: Option<ModelSelection>,
    downloading_format: Option<ModelSelection>,
    encoder_key: Option<String>,
}

#[cfg(feature = "experimental-search-embeddings-core")]
pub(crate) use provider::SearchEmbeddingState;
#[cfg(not(feature = "experimental-search-embeddings-core"))]
#[derive(Clone, Default)]
pub(crate) struct SearchEmbeddingState {}

impl SearchEmbeddingState {
    #[cfg(feature = "experimental-search-embeddings-core")]
    fn invalidate_activation(&self) -> u64 {
        self.1.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1
    }

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
            encoder_key: active_format.map(ModelSelection::encoder_key),
            #[cfg(not(feature = "experimental-search-embeddings-core"))]
            encoder_key: None,
        })
    }

    #[cfg(any(test, not(feature = "experimental-search-embeddings-core")))]
    fn disable(&self) -> Result<ProviderStatus, String> {
        #[cfg(feature = "experimental-search-embeddings-core")]
        return self.disable_at(self.invalidate_activation());
        #[cfg(not(feature = "experimental-search-embeddings-core"))]
        self.status()
    }

    #[cfg(feature = "experimental-search-embeddings-core")]
    fn disable_at(&self, version: u64) -> Result<ProviderStatus, String> {
        {
            let mut state = self.0.lock().map_err(|_| "Embedding state lock failed")?;
            if self.1.load(std::sync::atomic::Ordering::SeqCst) != version {
                return Err("Embedding configuration superseded".into());
            }
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
    format: ModelSelection,
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
    format: ModelSelection,
    files_present: bool,
    has_stored_files: bool,
    expected_bytes: u64,
}

/// Presence/size information only. Activation always rechecks actual checksums.
#[tauri::command]
pub(crate) async fn search_embedding_cache_status<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<CacheStatus>, String> {
    #[cfg(feature = "experimental-search-embeddings-core")]
    {
        let dirs = [
            ModelSelection::Float32,
            ModelSelection::DynamicInt8,
            ModelSelection::BaseFloat32,
            ModelSelection::LargeFloat32,
        ]
        .map(|format| cache_dir(&app, format).map(|dir| (format, dir)));
        tauri::async_runtime::spawn_blocking(move || {
            dirs.into_iter()
                .map(|entry| {
                    let (format, dir) = entry?;
                    Ok(CacheStatus {
                        format,
                        files_present: cache::files_present(&dir, &format.assets()),
                        has_stored_files: cache::has_stored_files(&dir)?,
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
    format: ModelSelection,
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
    format: ModelSelection,
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
pub(crate) async fn search_embedding_cancel_download(
    state: tauri::State<'_, SearchEmbeddingState>,
) -> Result<ProviderStatus, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(feature = "experimental-search-embeddings-core")]
        {
            let inner = state.0.lock().map_err(|_| "Embedding state lock failed")?;
            if let Some((_, cancel)) = &inner.downloading {
                cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
        state.status()
    })
    .await
    .map_err(|err| format!("Embedding cancellation task failed: {err}"))?
}

#[tauri::command]
pub(crate) async fn search_embedding_status(
    state: tauri::State<'_, SearchEmbeddingState>,
) -> Result<ProviderStatus, String> {
    // Indexing holds this lock during one native call; never wait on the UI thread.
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || state.status())
        .await
        .map_err(|err| format!("Embedding status task failed: {err}"))?
}

/// `null` disables/unloads. A format explicitly activates that verified cache.
/// Availability comes from build features, not a desktop-only platform check.
#[tauri::command]
pub(crate) async fn search_embedding_configure<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, SearchEmbeddingState>,
    index: tauri::State<'_, crate::document_uploads::semantic::SemanticIndexState>,
    format: Option<ModelSelection>,
) -> Result<ProviderStatus, String> {
    // Invalidate queued workers before dispatch; a late load must not undo a newer disable.
    #[cfg(feature = "experimental-search-embeddings-core")]
    let version = if format.is_none() {
        state.invalidate_activation()
    } else {
        state.1.load(std::sync::atomic::Ordering::SeqCst)
    };
    let Some(format) = format else {
        // Stop the writer before unloading; completed checkpoints remain reusable.
        index.cancel()?;
        let state = state.inner().clone();
        return tauri::async_runtime::spawn_blocking(move || {
            #[cfg(feature = "experimental-search-embeddings-core")]
            return state.disable_at(version);
            #[cfg(not(feature = "experimental-search-embeddings-core"))]
            state.disable()
        })
        .await
        .map_err(|err| format!("Embedding disable task failed: {err}"))?;
    };
    #[cfg(feature = "experimental-search-embeddings-core")]
    {
        let dir = cache_dir(&app, format)?;
        let state = state.inner().clone();
        tauri::async_runtime::spawn_blocking(move || state.activate_at(&dir, format, version))
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
    use super::{ModelSelection, ProviderStatus};
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
            atomic::{AtomicBool, AtomicU64, Ordering},
            Arc, Mutex,
        },
    };

    const MAX_TOKENS: usize = 512;

    #[derive(Clone, Default)]
    pub(crate) struct SearchEmbeddingState(
        pub(super) Arc<Mutex<ProviderState>>,
        // Disable invalidates queued loads without waiting for the native lock.
        pub(super) Arc<AtomicU64>,
    );

    #[derive(Default)]
    pub(super) struct ProviderState {
        pub(super) active: Option<LoadedProvider>,
        pub(super) loading: Option<(ModelSelection, Arc<AtomicBool>)>,
        pub(super) downloading: Option<(ModelSelection, Arc<AtomicBool>)>,
    }

    pub(super) struct LoadedProvider {
        pub(super) format: ModelSelection,
        model: TextEmbedding,
    }

    impl SearchEmbeddingState {
        /// A disable/reload invalidates a pending query even when asset identity is unchanged.
        pub(crate) fn index_generation(&self) -> u64 {
            self.1.load(Ordering::SeqCst)
        }
        /// Read the checksummed identity and dimensions together so a model switch cannot mix them.
        pub(crate) fn index_encoder_identity(&self) -> Result<(String, usize), String> {
            let state = self.0.lock().map_err(|_| "Embedding state lock failed")?;
            state
                .active
                .as_ref()
                .map(|loaded| (loaded.index_key(), loaded.format.dimensions()))
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

        pub(super) fn activate_at(
            &self,
            dir: &Path,
            format: ModelSelection,
            version: u64,
        ) -> Result<ProviderStatus, String> {
            self.activate_with_version(format, version, |cancel| {
                LoadedProvider::load(dir, format, cancel)
            })
        }

        #[cfg(test)]
        pub(super) fn activate(
            &self,
            dir: &Path,
            format: ModelSelection,
        ) -> Result<ProviderStatus, String> {
            self.activate_at(dir, format, self.1.load(Ordering::SeqCst))
        }

        #[cfg(test)]
        fn activate_with(
            &self,
            format: ModelSelection,
            load: impl FnOnce(&AtomicBool) -> Result<LoadedProvider, String>,
        ) -> Result<ProviderStatus, String> {
            self.activate_with_version(format, self.1.load(Ordering::SeqCst), load)
        }

        fn activate_with_version(
            &self,
            format: ModelSelection,
            version: u64,
            load: impl FnOnce(&AtomicBool) -> Result<LoadedProvider, String>,
        ) -> Result<ProviderStatus, String> {
            let cancel = Arc::new(AtomicBool::new(false));
            {
                let mut state = self.0.lock().map_err(|_| "Embedding state lock failed")?;
                if self.1.load(Ordering::SeqCst) != version {
                    return Err("Embedding configuration superseded".into());
                }
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
            let current = check_cancel(&cancel).and_then(|_| {
                if self.1.load(Ordering::SeqCst) != version {
                    Err("Embedding configuration superseded".into())
                } else {
                    Ok(())
                }
            });
            if let Err(err) = current {
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

        /// Prefer natural boundaries inside the 320-token ceiling, including metadata and E5 prefix.
        /// Context overlap stays source-local; oversized metadata fails instead of silently truncating.
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
            let characters = 4096usize
                .checked_sub(header.chars().count())
                .filter(|remaining| *remaining > 0)
                .ok_or("Title/heading leaves no character room for semantic passage text")?;
            let result = crate::document_uploads::semantic::passages::windows(
                text,
                encoding.get_offsets(),
                budget,
                characters,
            )?;
            // Re-tokenize the complete input: slicing can change tokenization at either boundary.
            for span in &result {
                let input = passage_input(title, heading, &text[span.clone()]);
                if input.chars().count() > 4096
                    || tokenizer
                        .encode(input.as_str(), true)
                        .map_err(|err| err.to_string())?
                        .len()
                        > CHUNK_TOKENS
                {
                    return Err("Semantic passage exceeds token/character budget".into());
                }
            }
            Ok(result)
        }

        fn load(dir: &Path, format: ModelSelection, cancel: &AtomicBool) -> Result<Self, String> {
            let assets = format.assets();
            let read = |asset: &super::models::Asset| {
                read_verified(&dir.join(asset.path), asset.bytes, asset.sha256, cancel)
            };
            let files = TokenizerFiles {
                config_file: read(&assets[0])?,
                tokenizer_file: read(&assets[1])?,
                special_tokens_map_file: read(&assets[2])?,
                tokenizer_config_file: read(&assets[3])?,
            };
            let quantization = match format {
                ModelSelection::Float32
                | ModelSelection::BaseFloat32
                | ModelSelection::LargeFloat32 => QuantizationMode::None,
                ModelSelection::DynamicInt8 => QuantizationMode::Dynamic,
            };
            let bytes = read(&assets[4])?;
            check_cancel(cancel)?;
            crate::onnx_runtime::initialize()?;
            let threads = if cfg!(any(target_os = "android", target_os = "ios")) {
                1
            } else {
                std::thread::available_parallelism().map_or(1, |n| n.get().min(2))
            };
            let mut definition = UserDefinedEmbeddingModel::new(bytes, files)
                .with_pooling(Pooling::Mean)
                .with_quantization(quantization);
            // Large's graph refers to external weights; verify the bytes before handing them to ORT.
            for asset in &assets[5..] {
                definition = definition.with_external_initializer(
                    asset
                        .path
                        .strip_prefix("onnx/")
                        .ok_or("Invalid external embedding asset path")?
                        .into(),
                    read(asset)?,
                );
            }
            let mut model = TextEmbedding::try_new_from_user_defined(
                definition,
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
            if vector.len() != self.format.dimensions()
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
                    worker_state.activate_with(ModelSelection::Float32, |cancel| {
                        ready_tx.send(()).unwrap();
                        resume_rx.recv().unwrap();
                        check_cancel(cancel)?;
                        panic!("a cancelled activation must not reach inference");
                    })
                });
                ready_rx.recv().unwrap();
                assert_eq!(
                    state.status().unwrap().loading_format,
                    Some(ModelSelection::Float32)
                );
                assert!(state
                    .activate_with(ModelSelection::Float32, |_| panic!("duplicate load"))
                    .is_err());
                assert!(state.disable().unwrap().active_format.is_none());
                // A cancelled loader still owns its slot until it actually returns.
                assert!(state
                    .activate_with(ModelSelection::DynamicInt8, |_| panic!("overlapping load"))
                    .is_err());
                resume_tx.send(()).unwrap();
                assert_eq!(
                    worker.join().unwrap().unwrap_err(),
                    "Embedding activation cancelled"
                );
            });
            assert!(state.status().unwrap().loading_format.is_none());
            assert!(state
                .activate_with(ModelSelection::Float32, |_| Err("missing model".into()))
                .is_err());
            assert!(state
                .activate_with(ModelSelection::Float32, |_| panic!("invalid runtime"))
                .is_err());
            assert!(state.status().unwrap().loading_format.is_none());
            assert!(state.disable().unwrap().active_format.is_none());
        }

        #[test]
        fn disable_invalidates_activation_queued_before_native_reservation() {
            let state = SearchEmbeddingState::default();
            let queued = state.1.load(Ordering::SeqCst);
            state.disable_at(state.invalidate_activation()).unwrap();
            assert_eq!(
                state
                    .activate_with_version(ModelSelection::Float32, queued, |_| panic!(
                        "a superseded queued request must never load native assets"
                    ))
                    .unwrap_err(),
                "Embedding configuration superseded"
            );
            assert!(state.status().unwrap().active_format.is_none());
            assert!(state.status().unwrap().loading_format.is_none());
            let current = state.1.load(Ordering::SeqCst);
            assert!(state
                .activate_with_version(ModelSelection::Float32, current, |_| Err(
                    "fresh request reached loader".into()
                ))
                .unwrap_err()
                .contains("fresh request"));
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
            assert!(only.as_deref().is_none_or(|format| matches!(
                format,
                "float32" | "dynamic-int8" | "base-float32" | "large-float32"
            )));
            let state = SearchEmbeddingState::default();
            // Manual runtime CI may install pinned test assets; ordinary/cached runs never fetch.
            let installer = (std::env::var("PAPERCUT_EMBEDDING_TEST_DOWNLOAD").as_deref()
                == Ok("1"))
            .then(|| {
                tauri::test::mock_builder()
                    .build(tauri::test::mock_context(tauri::test::noop_assets()))
                    .unwrap()
            });
            let mut baselines = Vec::new();
            for (cycle, (format, folder)) in (1..=cycles).flat_map(|cycle| {
                [
                    (ModelSelection::Float32, "float32"),
                    (ModelSelection::DynamicInt8, "dynamic-int8"),
                    (ModelSelection::BaseFloat32, "base-float32"),
                    (ModelSelection::LargeFloat32, "large-float32"),
                ]
                .map(|selection| (cycle, selection))
            }) {
                // Routine CI keeps its two Small runs; larger, heavier checks require an explicit selection.
                if only.as_deref().map_or(
                    !matches!(
                        format,
                        ModelSelection::Float32 | ModelSelection::DynamicInt8
                    ),
                    |requested| requested != folder,
                ) {
                    continue;
                }
                let dir = Path::new(&root).join(folder);
                if let Some(app) = &installer {
                    state.download(app.handle(), &dir, format).unwrap();
                }
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
                    ModelSelection::Float32
                    | ModelSelection::BaseFloat32
                    | ModelSelection::LargeFloat32 => ModelSelection::DynamicInt8,
                    ModelSelection::DynamicInt8 => ModelSelection::Float32,
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
                    assert_eq!(first.len(), format.dimensions());
                    assert_eq!(first, loaded.encode("query: public transport").unwrap());
                    if let Some((_, expected)) = baselines.iter().find(|(key, _)| *key == format) {
                        assert_eq!(&first, expected, "vector changed across provider reloads");
                    } else {
                        baselines.push((format, first.clone()));
                    }
                    assert_ne!(
                        first,
                        loaded
                            .encode("passage: Residents travel by electric bus.")
                            .unwrap(),
                        "different inputs must not reuse one vector"
                    );
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
                    // Test actual model offsets/budgets; synthetic word tokens cannot expose subword cuts.
                    for text in [
                        "Electric ferries connect the harbor and station. Riders buy tickets before boarding. ".repeat(80),
                        "港口有电动渡船。乘客在车站买票！".repeat(80),
                        "الحافلات تصل إلى الميناء؟ يشتري الركاب التذاكر قبل الرحلة. ".repeat(80),
                    ] {
                        let spans = loaded.passage_ranges("Transit guide", None, &text).unwrap();
                        assert!(spans.len() > 1);
                        for span in &spans {
                            let body = text.get(span.clone()).unwrap();
                            assert!(body.trim_end().ends_with(['.', '!', '؟', '。', '！']), "mid-sentence end: {span:?}, body: {body}");
                            assert!(span.start == 0 || text[..span.start].trim_end().ends_with(['.', '!', '؟', '。', '！']), "mid-sentence start: {span:?}, body: {body}");
                        }
                        assert!(spans.windows(2).all(|pair| pair[0].start < pair[1].start && pair[0].end >= pair[1].start));
                        assert!(text[spans.last().unwrap().end..].trim().is_empty());
                        loaded.encode(&crate::document_uploads::semantic::store::passage_input(
                            "Transit guide", None, &text[spans[0].clone()],
                        )).unwrap();
                    }
                }
                eprintln!("inference controls complete: {format:?}");
                if cycle == 1 {
                    crate::document_uploads::semantic::verify_native_index_queries(&state, || {
                        state.disable().map(|_| ())
                    });
                    eprintln!("index/query checks complete: {format:?}");
                }
                assert!(state.disable().unwrap().active_format.is_none());
                assert!(state.0.lock().unwrap().active.is_none());
                eprintln!("provider disabled: {format:?}");
                assert!(state.activate(Path::new("missing"), other).is_err());
                assert!(state.status().unwrap().active_format.is_none());
                assert!(state.status().unwrap().loading_format.is_none());
                if format == ModelSelection::DynamicInt8 {
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
        assert!(serde_json::from_str::<ModelSelection>("\"unknown-model\"").is_err());
    }
}

#[cfg(all(test, target_os = "linux"))]
mod ipc_tests;
