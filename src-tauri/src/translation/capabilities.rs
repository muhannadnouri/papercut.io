//! Capability and model-status reporting for the Translation UI.
//!
//! These are read-only views over the feature flag, planned model catalog, and
//! on-disk install state; job execution lives in `runner`.

use super::config::{
    DEFAULT_BATCH_SEGMENT_LIMIT, DEFAULT_MAX_SEGMENT_CHARS, DEFAULT_TRANSLATION_QUALITY_MODE,
    TRANSLATION_BACKEND_CTRANSLATE2, TRANSLATION_BACKEND_LLAMA_CPP, TRANSLATION_BACKEND_MULTI,
    TRANSLATION_BACKEND_UNAVAILABLE,
};
use super::hy_mt2::hy_mt2_hardware_acceleration;
use super::model_store::{directory_size, manifest_for, resolve_translation_model_dir};
use super::models::{find_planned_model, planned_models};
use super::state::TranslationState;
use super::types::{
    TranslationCapabilities, TranslationModelStatus, TranslationModelStatusRequest,
};

pub(super) const NOT_IMPLEMENTED: &str =
    "Offline translation is planned but not implemented in this build.";

/// Report translation capability shape even when a backend is unavailable.
///
/// The UI uses this stable payload to render model choices and feature gates
/// before every platform has native inference support.
pub(super) fn translation_capabilities() -> TranslationCapabilities {
    let ctranslate2 = cfg!(feature = "native-translation-ctranslate2");
    let llama = cfg!(feature = "native-translation-llama");
    let available = ctranslate2 || llama;
    TranslationCapabilities {
        available,
        backend: match (ctranslate2, llama) {
            (true, true) => TRANSLATION_BACKEND_MULTI,
            (true, false) => TRANSLATION_BACKEND_CTRANSLATE2,
            (false, true) => TRANSLATION_BACKEND_LLAMA_CPP,
            (false, false) => TRANSLATION_BACKEND_UNAVAILABLE,
        }
        .into(),
        reason: translation_capability_reason(ctranslate2, llama),
        platform: std::env::consts::OS.into(),
        default_quality_mode: DEFAULT_TRANSLATION_QUALITY_MODE.into(),
        hardware_acceleration: hy_mt2_hardware_acceleration(),
        models: planned_models(),
    }
}

/// Return install status for one catalog model.
///
/// This bridges planning and runtime: non-downloadable catalog entries explain
/// why they cannot run yet, while pinned manifests can report real disk state.
pub(super) fn translation_model_status<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    state: &tauri::State<'_, TranslationState>,
    request: TranslationModelStatusRequest,
) -> TranslationModelStatus {
    let Some(model) = find_planned_model(&request.model_id) else {
        return TranslationModelStatus {
            model_id: request.model_id,
            installed: false,
            installing: false,
            model_dir: None,
            source_url: String::new(),
            source_label: "Unknown offline translation model".into(),
            archive_bytes: 0,
            installed_bytes: 0,
            sha256: String::new(),
            message: "Translation model is not in the planned catalog.".into(),
        };
    };

    let manifest = manifest_for(model);
    let installing = state.model_operation_active(manifest.directory_name);
    match resolve_translation_model_dir(app, manifest) {
        Ok(model_dir) => TranslationModelStatus {
            model_id: manifest.model_id.into(),
            installed: true,
            installing,
            model_dir: Some(model_dir.display().to_string()),
            source_url: manifest.source_url.into(),
            source_label: manifest.source_label.into(),
            archive_bytes: manifest.total_bytes(),
            installed_bytes: directory_size(&model_dir).unwrap_or(0),
            sha256: String::new(),
            message: "Offline translation model installed".into(),
        },
        Err(_) => TranslationModelStatus {
            model_id: manifest.model_id.into(),
            installed: false,
            installing,
            model_dir: None,
            source_url: manifest.source_url.into(),
            source_label: format!("{} ({})", model.name, model.manifest_state),
            archive_bytes: manifest.total_bytes(),
            installed_bytes: 0,
            sha256: String::new(),
            message: if manifest.files.is_empty() {
                format!(
                    "{NOT_IMPLEMENTED} This candidate is not downloadable until source URL, checksum, license, required files, and platform gates are reviewed."
                )
            } else if model_engine_available(model) {
                "Translation model is installable. Install it before starting a translation job."
                    .into()
            } else {
                format!(
                    "{NOT_IMPLEMENTED} The file manifest is pinned, but the native {} engine is not compiled in this build.",
                    model.engine
                )
            },
        },
    }
}

fn translation_capability_reason(ctranslate2: bool, llama: bool) -> String {
    let limits = format!(
        "max {DEFAULT_MAX_SEGMENT_CHARS} chars/segment, {DEFAULT_BATCH_SEGMENT_LIMIT} segments/batch"
    );
    match (ctranslate2, llama) {
        (true, true) => {
            format!("OPUS-MT and HY-MT2 offline translation are available; {limits}.")
        }
        (true, false) => format!("OPUS-MT offline translation is available; {limits}."),
        (false, true) => format!("HY-MT2 offline translation is available; {limits}."),
        (false, false) => format!("{NOT_IMPLEMENTED} Planned defaults: {limits}."),
    }
}

fn model_engine_available(model: super::models::TranslationModelDefinition) -> bool {
    match model.engine {
        "ctranslate2" => cfg!(feature = "native-translation-ctranslate2"),
        "llama.cpp" => cfg!(feature = "native-translation-llama"),
        _ => false,
    }
}
