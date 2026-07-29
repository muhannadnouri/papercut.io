//! Translation feature constants.
//!
//! These are conservative job limits shared by planning, cache identity, and
//! native inference. Keeping them centralized prevents each engine from picking
//! incompatible chunk/job defaults.

pub(crate) const TRANSLATION_BACKEND_UNAVAILABLE: &str = "translation-unavailable";
pub(crate) const TRANSLATION_BACKEND_CTRANSLATE2: &str = "ctranslate2";
pub(crate) const TRANSLATION_BACKEND_LLAMA_CPP: &str = "llama.cpp";
pub(crate) const TRANSLATION_BACKEND_MULTI: &str = "ctranslate2+llama.cpp";
pub(crate) const TRANSLATION_MODEL_INSTALL_PROGRESS_EVENT: &str =
    "translation-model-install-progress";
pub(crate) const TRANSLATION_JOB_PROGRESS_EVENT: &str = "translation-progress";
pub(crate) const DEFAULT_TRANSLATION_QUALITY_MODE: &str = "balanced";
pub(crate) const DEFAULT_MAX_SEGMENT_CHARS: usize = 900;
pub(crate) const DEFAULT_BATCH_SEGMENT_LIMIT: usize = 16;
