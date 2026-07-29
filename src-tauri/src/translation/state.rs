//! Runtime state for offline translation jobs and model installs.
//!
//! Keep this small and feature-local. Translation model installation should not
//! reuse the TTS state lock because the two features can install/download
//! independently and may eventually have different cancellation semantics.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct TranslationState {
    pub(crate) active_jobs: Arc<Mutex<HashSet<String>>>,
    pub(crate) cancelled_jobs: Arc<Mutex<HashSet<String>>>,
    pub(crate) model_installing: Arc<Mutex<HashSet<String>>>,
}

impl Default for TranslationState {
    fn default() -> Self {
        Self {
            active_jobs: Arc::new(Mutex::new(HashSet::new())),
            cancelled_jobs: Arc::new(Mutex::new(HashSet::new())),
            model_installing: Arc::new(Mutex::new(HashSet::new())),
        }
    }
}

impl TranslationState {
    /// Claim one document/settings cache key until the returned guard drops.
    ///
    /// The UI normally starts one job at a time, but the command boundary can
    /// receive concurrent calls. Keying this guard by cache identity prevents
    /// those calls from rewriting the same segment cache and stored variant.
    pub(crate) fn claim_job(&self, cache_key: &str) -> Result<ActiveTranslationJob, String> {
        let mut active = self
            .active_jobs
            .lock()
            .map_err(|_| "Translation active-job lock poisoned".to_string())?;
        if !active.insert(cache_key.into()) {
            return Err(
                "This document is already being translated with the selected settings".into(),
            );
        }
        Ok(ActiveTranslationJob {
            active_jobs: Arc::clone(&self.active_jobs),
            cache_key: cache_key.into(),
        })
    }
}

pub(crate) struct ActiveTranslationJob {
    active_jobs: Arc<Mutex<HashSet<String>>>,
    cache_key: String,
}

impl Drop for ActiveTranslationJob {
    fn drop(&mut self) {
        if let Ok(mut active) = self.active_jobs.lock() {
            active.remove(&self.cache_key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TranslationState;

    #[test]
    fn duplicate_job_claim_is_released_when_guard_drops() {
        let state = TranslationState::default();
        let guard = state.claim_job("same-cache-key").expect("first claim");

        assert!(state.claim_job("same-cache-key").is_err());

        drop(guard);
        assert!(state.claim_job("same-cache-key").is_ok());
    }
}
