//! Explicit app-index jobs. Search remains lexical until the later query integration.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

#[cfg(any(test, feature = "experimental-search-embeddings-core"))]
pub(crate) mod store;

#[derive(Clone, Default)]
pub(crate) struct SemanticIndexState(Arc<Mutex<Job>>);

#[derive(Default)]
struct Job {
    cancel: Option<Arc<AtomicBool>>,
    embedded: usize,
    reused: usize,
    error: Option<String>,
}

#[derive(Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IndexStatus {
    compiled: bool,
    running: bool,
    cancel_requested: bool,
    indexed_documents: usize,
    eligible_documents: usize,
    passages: usize,
    embedded_passages: usize,
    reused_passages: usize,
    error: Option<String>,
}

#[cfg(any(test, feature = "experimental-search-embeddings-core"))]
struct Control {
    state: SemanticIndexState,
    cancel: Arc<AtomicBool>,
}

impl SemanticIndexState {
    // Reserve before dispatch so two IPC calls cannot queue competing writers.
    #[cfg(any(test, feature = "experimental-search-embeddings-core"))]
    fn begin(&self) -> Result<Control, String> {
        let mut job = self
            .0
            .lock()
            .map_err(|_| "Semantic index job lock failed")?;
        if job.cancel.is_some() {
            return Err("Semantic indexing is already running".into());
        }
        let cancel = Arc::new(AtomicBool::new(false));
        *job = Job {
            cancel: Some(cancel.clone()),
            ..Job::default()
        };
        Ok(Control {
            state: self.clone(),
            cancel,
        })
    }

    fn snapshot(&self) -> Result<IndexStatus, String> {
        let job = self
            .0
            .lock()
            .map_err(|_| "Semantic index job lock failed")?;
        Ok(IndexStatus {
            compiled: cfg!(feature = "experimental-search-embeddings-core"),
            running: job.cancel.is_some(),
            cancel_requested: job
                .cancel
                .as_ref()
                .is_some_and(|cancel| cancel.load(Ordering::Relaxed)),
            embedded_passages: job.embedded,
            reused_passages: job.reused,
            error: job.error.clone(),
            ..IndexStatus::default()
        })
    }
}

#[cfg(any(test, feature = "experimental-search-embeddings-core"))]
impl Control {
    fn check(&self) -> Result<(), String> {
        if self.cancel.load(Ordering::Relaxed) {
            Err("Semantic indexing cancelled".into())
        } else {
            Ok(())
        }
    }

    fn progress(&self, reused: bool) {
        if let Ok(mut job) = self.state.0.lock() {
            if reused {
                job.reused += 1
            } else {
                job.embedded += 1
            }
        }
    }

    fn finish(&self, result: &Result<(), String>) {
        if let Ok(mut job) = self.state.0.lock() {
            job.error = result.as_ref().err().cloned()
        }
    }
}

#[cfg(any(test, feature = "experimental-search-embeddings-core"))]
impl Drop for Control {
    fn drop(&mut self) {
        if let Ok(mut job) = self.state.0.lock() {
            job.cancel = None
        }
    }
}

/// Read coverage without creating semantic tables or activating a model.
#[tauri::command]
pub(crate) async fn search_semantic_index_status<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, SemanticIndexState>,
    provider: tauri::State<'_, crate::search_embeddings::SearchEmbeddingState>,
) -> Result<IndexStatus, String> {
    let status = state.snapshot()?;
    #[cfg(not(feature = "experimental-search-embeddings-core"))]
    {
        let _ = (app, provider);
        Ok(status)
    }
    #[cfg(feature = "experimental-search-embeddings-core")]
    {
        let provider = provider.inner().clone();
        tauri::async_runtime::spawn_blocking(move || {
            let mut status = status;
            if super::storage::uploads_root(&app)?
                .join("search.sqlite3")
                .exists()
            {
                let mut db = super::store::open_db(&app)?;
                let encoder = provider.index_encoder_key().ok();
                let identity = encoder
                    .as_deref()
                    .map(|key| store::Identity::new(key, crate::search_embeddings::DIMENSIONS))
                    .transpose()?;
                let coverage = store::coverage(&mut db, identity.as_ref())?;
                status.indexed_documents = coverage.indexed_documents;
                status.eligible_documents = coverage.eligible_documents;
                status.passages = coverage.passages;
            }
            Ok(status)
        })
        .await
        .map_err(|err| format!("Semantic index status task failed: {err}"))?
    }
}

/// Only an explicit request starts work; a disabled provider fails before storage is changed.
/// One document is read at a time and checkpoints survive cancellation or process restart.
#[tauri::command]
pub(crate) async fn search_semantic_index_build<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, SemanticIndexState>,
    provider: tauri::State<'_, crate::search_embeddings::SearchEmbeddingState>,
) -> Result<(), String> {
    #[cfg(not(feature = "experimental-search-embeddings-core"))]
    {
        let _ = (app, state, provider);
        Err("Experimental embeddings are not compiled in this build".into())
    }
    #[cfg(feature = "experimental-search-embeddings-core")]
    {
        let control = state.begin()?;
        let provider = provider.inner().clone();
        tauri::async_runtime::spawn_blocking(move || {
            let result = (|| {
                control.check()?;
                let encoder = provider.index_encoder_key()?;
                let identity =
                    store::Identity::new(&encoder, crate::search_embeddings::DIMENSIONS)?;
                let root = super::storage::uploads_root(&app)?;
                std::fs::create_dir_all(&root).map_err(|err| err.to_string())?;
                crate::library_transfer::ensure_available_space(&root, 1536)
                    .map_err(|err| err.to_string())?;
                let mut db = super::store::open_db(&app)?;
                store::initialize(&mut db, &identity)?;
                for id in store::document_ids(&db)? {
                    control.check()?;
                    let Some(source) = store::source(&mut db, &id)? else {
                        continue;
                    };
                    store::synchronize(
                        &mut db,
                        &identity,
                        &source,
                        |title, heading, text| {
                            let ranges =
                                provider.index_passage_ranges(&encoder, title, heading, text)?;
                            // Include vector/row/WAL headroom; SQLite errors remain recoverable if this estimate is exceeded.
                            crate::library_transfer::ensure_available_space(
                                &root,
                                ranges.len() as u64 * 4096,
                            )
                            .map_err(|err| err.to_string())?;
                            Ok(ranges)
                        },
                        |text| provider.index_encode(&encoder, text),
                        || {
                            control.check()?;
                            if provider.index_encoder_key()? != encoder {
                                return Err("Semantic model changed; retry indexing".into());
                            }
                            Ok(())
                        },
                        |reused| control.progress(reused),
                    )?;
                }
                control.check()
            })();
            control.finish(&result);
            result
        })
        .await
        .map_err(|err| format!("Semantic indexing task failed: {err}"))?
    }
}

/// Cancellation is cooperative; the writer keeps its slot until native work and cleanup return.
#[tauri::command]
pub(crate) fn search_semantic_index_cancel(
    state: tauri::State<'_, SemanticIndexState>,
) -> Result<(), String> {
    let job = state
        .0
        .lock()
        .map_err(|_| "Semantic index job lock failed")?;
    if let Some(cancel) = &job.cancel {
        cancel.store(true, Ordering::Relaxed)
    }
    Ok(())
}

/// Removal shares the job slot so it cannot race an indexing job in this app process.
#[tauri::command]
pub(crate) async fn search_semantic_index_remove<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, SemanticIndexState>,
) -> Result<(), String> {
    #[cfg(not(feature = "experimental-search-embeddings-core"))]
    {
        let _ = (app, state);
        Err("Experimental embeddings are not compiled in this build".into())
    }
    #[cfg(feature = "experimental-search-embeddings-core")]
    {
        let control = state.begin()?;
        tauri::async_runtime::spawn_blocking(move || {
            let result = (|| {
                control.check()?;
                if super::storage::uploads_root(&app)?
                    .join("search.sqlite3")
                    .exists()
                {
                    store::remove(&mut super::store::open_db(&app)?)?;
                }
                Ok(())
            })();
            control.finish(&result);
            result
        })
        .await
        .map_err(|err| format!("Semantic index removal task failed: {err}"))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_job_cancels_and_releases_after_error_or_unwind() {
        let state = SemanticIndexState::default();
        let control = state.begin().unwrap();
        control.progress(false);
        assert!(state.begin().is_err());
        control.cancel.store(true, Ordering::Relaxed);
        assert!(control.check().is_err());
        assert!(state.snapshot().unwrap().cancel_requested);
        control.finish(&Err("cancelled".into()));
        drop(control);
        assert!(!state.snapshot().unwrap().running);
        assert_eq!(state.snapshot().unwrap().embedded_passages, 1);
        assert_eq!(
            state.snapshot().unwrap().error.as_deref(),
            Some("cancelled")
        );
        let panic = std::panic::catch_unwind(|| {
            let _control = state.begin().unwrap();
            panic!("worker");
        });
        assert!(panic.is_err());
        assert!(!state.snapshot().unwrap().running);
        assert!(state.begin().is_ok());
    }
}
