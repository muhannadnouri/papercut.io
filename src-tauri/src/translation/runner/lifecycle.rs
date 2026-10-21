//! Job-scoped translator ownership and cancellation cleanup through final save admission.
use super::{clear_cancelled, is_cancelled};
use crate::translation::state::{
    ActiveTranslationJob, TranslationModelOperation, TranslationState,
};

/// Release model claims and cancellation cleanup before releasing the job identity.
pub(super) struct Job<'a> {
    _translator: TranslationModelOperation,
    cleanup: CancellationCleanup<'a>,
    _active: ActiveTranslationJob,
}

impl<'a> Job<'a> {
    /// Claim the cache identity first; failed model admission still clears this attempt's cancellation.
    pub(super) fn new(
        state: &'a TranslationState,
        cache_key: &str,
        job_id: &str,
        model: &str,
    ) -> Result<Self, String> {
        let active = state.claim_job(cache_key, model)?;
        let cleanup = CancellationCleanup {
            state,
            job_id: job_id.into(),
        };
        let translator = state.claim_model_read(model)?;
        let job = Self {
            _translator: translator,
            cleanup,
            _active: active,
        };
        job.check_cancelled()?;
        Ok(job)
    }

    pub(super) fn id(&self) -> &str {
        &self.cleanup.job_id
    }

    /// Suppress late results from native loads that cannot be interrupted in flight.
    pub(super) fn check_cancelled(&self) -> Result<(), String> {
        if is_cancelled(&self.cleanup.state.cancelled_jobs, self.id())? {
            Err("Translation cancelled".into())
        } else {
            Ok(())
        }
    }
}

struct CancellationCleanup<'a> {
    state: &'a TranslationState,
    job_id: String,
}
impl Drop for CancellationCleanup<'_> {
    fn drop(&mut self) {
        // Clear before releasing the cache claim, so retry cancellation cannot be erased by this job.
        let _ = clear_cancelled(&self.state.cancelled_jobs, &self.job_id);
    }
}

#[cfg(test)]
mod tests;
