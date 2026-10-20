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
    saving_jobs: HashSet<String>,
    model_operations: HashSet<String>,
    #[cfg(feature = "native-translation-alignment-runtime")]
    alignment_cancellations:
        std::collections::HashMap<String, Arc<super::alignment::runtime::Cancellation>>,
}

#[derive(Clone)]
pub struct TranslationState {
    pub(crate) bergamot: Arc<super::bergamot::WorkerBridge>,
    model_activity: Arc<Mutex<TranslationModelActivity>>,
    pub(crate) cancelled_jobs: Arc<Mutex<HashSet<String>>>,
}

impl Default for TranslationState {
    fn default() -> Self {
        Self {
            bergamot: Arc::default(),
            model_activity: Arc::new(Mutex::new(TranslationModelActivity::default())),
            cancelled_jobs: Arc::new(Mutex::new(HashSet::new())),
        }
    }
}

impl TranslationState {
    pub(crate) fn cancel(&self, job_id: &str) -> Result<(), String> {
        let mut cancelled = self
            .cancelled_jobs
            .lock()
            .map_err(|_| "Translation cancellation lock poisoned")?;
        let activity = self
            .model_activity
            .lock()
            .map_err(|_| "Translation activity lock poisoned")?;
        if activity.saving_jobs.contains(job_id) {
            return Err("Translation is already being saved; wait for completion".into());
        }
        cancelled.insert(job_id.into());
        #[cfg(feature = "native-translation-alignment-runtime")]
        let alignment = activity.alignment_cancellations.get(job_id).cloned();
        // Native termination must not run while holding application state locks.
        drop(activity);
        drop(cancelled);
        #[cfg(feature = "native-translation-alignment-runtime")]
        if let Some(alignment) = alignment {
            alignment
                .cancel()
                .map_err(|_| "Alignment cancellation failed")?;
        }
        Ok(())
    }

    /// Registration shares cancel/save lock order, so an earlier cancellation cannot be lost.
    #[cfg(feature = "native-translation-alignment-runtime")]
    pub(crate) fn register_alignment_cancellation(
        &self,
        job_id: &str,
    ) -> Result<AlignmentCancellation, String> {
        let cancelled = self
            .cancelled_jobs
            .lock()
            .map_err(|_| "Translation cancellation lock poisoned")?;
        let mut activity = self
            .model_activity
            .lock()
            .map_err(|_| "Translation activity lock poisoned")?;
        if cancelled.contains(job_id) {
            return Err("Translation cancelled".into());
        }
        if activity.saving_jobs.contains(job_id)
            || activity.alignment_cancellations.contains_key(job_id)
        {
            return Err("Alignment job is already active or saving".into());
        }
        let cancellation = Arc::new(super::alignment::runtime::Cancellation::default());
        activity
            .alignment_cancellations
            .insert(job_id.into(), cancellation.clone());
        Ok(AlignmentCancellation {
            activity: self.model_activity.clone(),
            job_id: job_id.into(),
            cancellation,
        })
    }

    /// Serialize cancellation with the last check before the atomic document writer.
    pub(crate) fn claim_save(&self, job_id: &str) -> Result<TranslationSave, String> {
        let cancelled = self
            .cancelled_jobs
            .lock()
            .map_err(|_| "Translation cancellation lock poisoned")?;
        let mut activity = self
            .model_activity
            .lock()
            .map_err(|_| "Translation activity lock poisoned")?;
        if cancelled.contains(job_id) {
            return Err("Translation cancelled".into());
        }
        if !activity.saving_jobs.insert(job_id.into()) {
            return Err("Translation is already being saved".into());
        }
        Ok(TranslationSave {
            activity: self.model_activity.clone(),
            job_id: job_id.into(),
        })
    }

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

    /// Hold the same exclusive file lease as install/remove during binary delivery.
    pub(crate) fn claim_model_read(
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

/// Scope one native cancellation handle to a job; retries always receive a fresh handle.
#[cfg(feature = "native-translation-alignment-runtime")]
pub(crate) struct AlignmentCancellation {
    activity: Arc<Mutex<TranslationModelActivity>>,
    job_id: String,
    pub(crate) cancellation: Arc<super::alignment::runtime::Cancellation>,
}
#[cfg(feature = "native-translation-alignment-runtime")]
impl Drop for AlignmentCancellation {
    fn drop(&mut self) {
        let mut activity = self.activity.lock().unwrap_or_else(|e| e.into_inner());
        activity.alignment_cancellations.remove(&self.job_id);
    }
}

pub(crate) struct TranslationSave {
    activity: Arc<Mutex<TranslationModelActivity>>,
    job_id: String,
}
impl Drop for TranslationSave {
    fn drop(&mut self) {
        if let Ok(mut activity) = self.activity.lock() {
            activity.saving_jobs.remove(&self.job_id);
        }
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
    fn cancellation_and_save_have_one_atomic_cutoff() {
        let state = TranslationState::default();
        state.cancel("cancel-first").unwrap();
        assert!(state.claim_save("cancel-first").is_err());
        let saving = state.claim_save("save-first").unwrap();
        assert!(state
            .cancel("save-first")
            .unwrap_err()
            .contains("already being saved"));
        let error = super::super::types::TranslationCommandError::new(
            state.cancel("save-first").unwrap_err(),
        );
        assert_ne!(error.code, "operation-cancelled");
        assert!(!state.cancelled_jobs.lock().unwrap().contains("save-first"));
        drop(saving);
        assert!(state.cancel("save-first").is_ok());
    }

    #[cfg(feature = "native-translation-alignment-runtime")]
    #[test]
    fn native_cancel_registration_handles_both_orderings_save_cutoff_and_clean_retries() {
        let state = TranslationState::default();
        state.cancel("early").unwrap();
        assert!(state.register_alignment_cancellation("early").is_err());
        let handle = state.register_alignment_cancellation("running").unwrap();
        let other = state.register_alignment_cancellation("other").unwrap();
        assert!(state.register_alignment_cancellation("running").is_err());
        let cancel = state.clone();
        std::thread::spawn(move || cancel.cancel("running").unwrap())
            .join()
            .unwrap();
        assert!(handle.cancellation.is_cancelled());
        assert!(!other.cancellation.is_cancelled());
        assert!(state.claim_save("running").is_err());
        drop(handle);
        state.cancelled_jobs.lock().unwrap().remove("running");
        let retry = state.register_alignment_cancellation("running").unwrap();
        assert!(!retry.cancellation.is_cancelled());
        let saving = state.claim_save("running").unwrap();
        assert!(state
            .cancel("running")
            .unwrap_err()
            .contains("already being saved"));
        assert!(!retry.cancellation.is_cancelled());
        assert!(state.register_alignment_cancellation("running").is_err());
        drop(saving);
        drop(retry);
        assert!(state.register_alignment_cancellation("running").is_ok());
    }

    #[cfg(feature = "native-translation-alignment-runtime")]
    #[test]
    fn racing_native_registration_and_cancel_never_lose_the_request() {
        for _ in 0..32 {
            let state = TranslationState::default();
            let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
            let cancel = state.clone();
            let start = barrier.clone();
            let thread = std::thread::spawn(move || {
                start.wait();
                cancel.cancel("job").unwrap();
            });
            barrier.wait();
            let handle = state.register_alignment_cancellation("job");
            thread.join().unwrap();
            match handle {
                Ok(handle) => assert!(handle.cancellation.is_cancelled()),
                Err(error) => assert!(error.contains("cancelled")),
            }
        }
    }

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
