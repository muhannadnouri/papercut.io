//! Native sherpa-onnx engine implementation (compiled with `native-tts-core`).
//!
//! Submodules, with dependencies pointing downward:
//!
//! ```text
//! { model, synth, save, cache, bundle } -> { manifest, paths, config, text_normalization } -> super::types
//! save -> { synth, cache, prune }   bundle -> { cache, manifest, paths }   synth -> cache
//! ```
//!
//! - [`config`]: pinned model metadata, event names, bundle format constants.
//! - [`paths`]: app-data paths, ids, hashing, filesystem accounting.
//! - [`synth`]: the loaded engine slot, sherpa handle, and chunk synthesis.
//! - [`text_normalization`]: shared cleanup plus English-only synthesis rewrites.
//! - `manifest`: saved-audiobook manifest JSON and playback index validation.
//! - `prune`: saved-audiobook chunk/temp cleanup.
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
mod manifest;
mod model;
mod models;
mod paths;
mod playback;
mod preprocess;
mod prune;
mod save;
mod sidecar_probe;
mod silma_sidecar;
mod synth;
mod text_normalization;
mod wav_sink;

pub(crate) use synth::{preview_voice, LoadedTtsEngine};

pub(crate) use bundle::{
    delete_audiobook_native, export_audiobook_native, get_imported_audiobook_metadata,
    get_imported_audiobook_source, import_audiobook_native,
};
pub(crate) use cache::{get_native_audiobook_chunk, native_audiobook_status};
pub(crate) use manifest::{
    list_audiobook_transfer_payloads, list_saved_audiobooks, validate_transferred_audiobook,
};
pub(crate) use model::{install_model, model_status, native_capabilities};
pub(crate) use paths::{audiobooks_dir, imported_upload_dir, imported_upload_id_from_document_url};
pub(crate) use playback::prepare_native_audiobook_playback;
pub(crate) use save::{cancel_audiobook_save, save_audiobook_native};
pub(crate) use sidecar_probe::probe_silma_sidecar;
