//! Job-scoped model ownership. Public starts remain translator-only until mapping admission is ready.
use super::{clear_cancelled, is_cancelled};
use crate::translation::state::{
    ActiveTranslationJob, TranslationModelOperation, TranslationState,
};

/// Field order drops the auxiliary session before its registration, then model claims before job identity.
pub(super) struct Job<'a> {
    #[cfg(feature = "native-translation-alignment-runtime")]
    alignment: Option<crate::translation::alignment::runtime::AlignmentSession>,
    #[cfg(feature = "native-translation-alignment-runtime")]
    alignment_cancel: Option<crate::translation::state::AlignmentCancellation>,
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
            #[cfg(feature = "native-translation-alignment-runtime")]
            alignment: None,
            #[cfg(feature = "native-translation-alignment-runtime")]
            alignment_cancel: None,
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

    /// Internal opt-in seam only: no command/catalog path calls this before strict mapping is implemented.
    #[cfg(feature = "native-translation-alignment-runtime")]
    #[allow(dead_code)]
    pub(super) fn load_alignment<R: tauri::Runtime>(
        &mut self,
        app: &tauri::AppHandle<R>,
    ) -> Result<(), String> {
        use crate::translation::{
            alignment::{assets, runtime::AlignmentSession},
            model_store,
        };
        self.check_cancelled()?;
        if self.alignment.is_some() {
            return Ok(());
        }
        let cancellation = self
            .cleanup
            .state
            .register_alignment_cancellation(self.id())?;
        let manifest = model_store::manifest_by_id(assets::MODEL_KEY)
            .ok_or("Auxiliary alignment assets unavailable")?;
        let dir = model_store::installed_translation_model_dir(app, manifest)?;
        let session =
            AlignmentSession::load_installed(self.cleanup.state, &dir, &cancellation.cancellation)
                .map_err(|e| {
                    if cancellation.cancellation.is_cancelled() {
                        "Translation cancelled".into()
                    } else {
                        format!("Alignment session failed: {e:?}")
                    }
                })?;
        self.check_cancelled()?;
        self.alignment = Some(session);
        self.alignment_cancel = Some(cancellation);
        Ok(())
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
