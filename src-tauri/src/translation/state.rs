//! Runtime state for offline translation jobs and model installs.
//!
//! Keep this small and feature-local. Translation model installation should not
//! reuse the TTS state lock because the two features can install/download
//! independently and may eventually have different cancellation semantics.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct TranslationModelActivity {
    active_jobs: HashSet<String>,
    model_operations: HashSet<String>,
}

#[derive(Clone)]
pub struct TranslationState {
    model_activity: Arc<Mutex<TranslationModelActivity>>,
    pub(crate) cancelled_jobs: Arc<Mutex<HashSet<String>>>,
}

impl Default for TranslationState {
    fn default() -> Self {
        Self {
            model_activity: Arc::new(Mutex::new(TranslationModelActivity::default())),
            cancelled_jobs: Arc::new(Mutex::new(HashSet::new())),
        }
    }
}

impl TranslationState {
    /// Claim one document/settings cache key until the returned guard drops.
    ///
    /// The UI normally starts one job at a time, but the command boundary can
    /// receive concurrent calls. Keying this guard by cache identity prevents
    /// those calls from rewriting the same segment cache and stored variant.
    pub(crate) fn claim_job(
        &self,
        cache_key: &str,
        model_id: &str,
    ) -> Result<ActiveTranslationJob, String> {
        let mut activity = self
            .model_activity
            .lock()
            .map_err(|_| "Translation active-job lock poisoned".to_string())?;
        if activity.model_operations.contains(model_id) {
            return Err(format!(
                "Translation model {model_id:?} is already being installed or removed"
            ));
        }
        if !activity.active_jobs.insert(cache_key.into()) {
            return Err(
                "This document is already being translated with the selected settings".into(),
            );
        }
        Ok(ActiveTranslationJob {
            model_activity: Arc::clone(&self.model_activity),
            cache_key: cache_key.into(),
        })
    }

    /// Serialize installation for one model without blocking unrelated jobs.
    pub(crate) fn claim_model_install(
        &self,
        model_id: &str,
    ) -> Result<TranslationModelOperation, String> {
        self.claim_model_operation(model_id, false)
    }

    /// Prevent model deletion while any translation job may still use its files.
    ///
    /// Papercut currently runs one foreground translation job, so a global
    /// active-job check is both safer and simpler than tracking engine file
    /// ownership separately.
    pub(crate) fn claim_model_removal(
        &self,
        model_id: &str,
    ) -> Result<TranslationModelOperation, String> {
        self.claim_model_operation(model_id, true)
    }

    pub(crate) fn model_operation_active(&self, model_id: &str) -> bool {
        self.model_activity
            .lock()
            .map(|activity| activity.model_operations.contains(model_id))
            .unwrap_or(false)
    }

    /// Claim one model mutation under the same lock used to admit jobs.
    fn claim_model_operation(
        &self,
        model_id: &str,
        require_idle: bool,
    ) -> Result<TranslationModelOperation, String> {
        let mut activity = self
            .model_activity
            .lock()
            .map_err(|_| "Translation model-operation lock poisoned".to_string())?;
        if require_idle && !activity.active_jobs.is_empty() {
            return Err(
                "A translation operation is already in progress; models cannot be removed".into(),
            );
        }
        if !activity.model_operations.insert(model_id.into()) {
            return Err(format!(
                "Translation model {model_id:?} is already being installed or removed"
            ));
        }
        Ok(TranslationModelOperation {
            model_activity: Arc::clone(&self.model_activity),
            model_id: model_id.into(),
        })
    }
}

pub(crate) struct ActiveTranslationJob {
    model_activity: Arc<Mutex<TranslationModelActivity>>,
    cache_key: String,
}

impl Drop for ActiveTranslationJob {
    fn drop(&mut self) {
        if let Ok(mut activity) = self.model_activity.lock() {
            activity.active_jobs.remove(&self.cache_key);
        }
    }
}

pub(crate) struct TranslationModelOperation {
    model_activity: Arc<Mutex<TranslationModelActivity>>,
    model_id: String,
}

impl Drop for TranslationModelOperation {
    fn drop(&mut self) {
        if let Ok(mut activity) = self.model_activity.lock() {
            activity.model_operations.remove(&self.model_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TranslationState;

    #[test]
    fn duplicate_job_claim_is_released_when_guard_drops() {
        let state = TranslationState::default();
        let guard = state
            .claim_job("same-cache-key", "model")
            .expect("first claim");

        assert!(state.claim_job("same-cache-key", "model").is_err());

        drop(guard);
        assert!(state.claim_job("same-cache-key", "model").is_ok());
    }

    #[test]
    fn model_removal_and_jobs_cannot_overlap() {
        let state = TranslationState::default();
        let job = state.claim_job("job", "model").expect("job");

        assert!(state.claim_model_removal("model").is_err());
        drop(job);

        let removal = state.claim_model_removal("model").expect("removal");
        assert!(state.claim_job("next-job", "model").is_err());
        drop(removal);

        assert!(state.claim_job("next-job", "model").is_ok());

        let install = state.claim_model_install("other-model").expect("install");
        assert!(state.claim_model_removal("other-model").is_err());
        drop(install);

        assert!(state.claim_model_removal("other-model").is_ok());
    }
}
