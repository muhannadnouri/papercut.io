//! Native sherpa-onnx engine implementation (compiled with `native-tts-core`).
//!
//! Submodules, with dependencies pointing downward:
//!
//! ```text
//! { model, synth, save, cache, bundle } -> { paths, config, text_normalization } -> super::types
//! save -> { synth, cache }   bundle -> { cache, save, paths }   synth -> cache
//! ```
//!
//! - [`config`]: pinned model metadata, event names, bundle format constants.
//! - [`paths`]: app-data paths, ids, hashing, filesystem accounting.
//! - [`synth`]: the sherpa engine handle and chunk synthesis.
//! - [`text_normalization`]: shared cleanup plus English-only synthesis rewrites.
//! - [`cache`]: native audiobook directory scanning and WAV parsing.
//! - [`save`]: long-running native audiobook save jobs and progress events.
//! - [`bundle`]: audiobook export/import bundle format, plus delete.
//! - [`model`]: voice-model download / verify / extract / status / capabilities.
//!
//! The functions re-exported below are the backend surface consumed by
//! [`super::commands`]; they mirror the `super::stub` fallbacks one-for-one.

mod bundle;
mod cache;
mod config;
mod file_commit;
mod model;
mod models;
mod paths;
mod playback;
mod preprocess;
mod save;
mod synth;
mod text_normalization;

pub(crate) use synth::SherpaTtsEngine;

pub(crate) use bundle::{
    delete_audiobook_native, export_audiobook_native, get_imported_audiobook_metadata,
    get_imported_audiobook_source, import_audiobook_native,
};
pub(crate) use cache::{get_native_audiobook_chunk, native_audiobook_status};
pub(crate) use model::{install_model, model_status, native_capabilities};
pub(crate) use playback::prepare_native_audiobook_playback;
pub(crate) use save::{cancel_audiobook_save, save_audiobook_native};
