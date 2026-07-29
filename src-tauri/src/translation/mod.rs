//! Offline document translation feature.
//!
//! Translation is intentionally separate from `document_uploads` and
//! `native_tts`: upload parsers keep producing safe source + section records,
//! while this feature consumes that stable contract and creates translated
//! document variants through a feature-gated native engine.

mod cache;
mod capabilities;
pub(crate) mod commands;
mod config;
mod ctranslate2;
mod engine;
mod hash;
mod html;
mod inline_markup;
mod job;
mod model_install;
mod model_store;
mod models;
mod quality;
mod render;
mod runner;
mod segment;
mod source;
mod state;
mod storage;
mod types;

pub use state::TranslationState;
