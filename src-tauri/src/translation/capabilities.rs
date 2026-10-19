//! Capability and model-status reporting for the Translation UI.
//!
//! These are read-only views over compiled engines, the model catalog and
//! on-disk state; the frontend narrows worker availability for its WebView.

use super::config::{
    DEFAULT_BATCH_SEGMENT_LIMIT, DEFAULT_MAX_SEGMENT_CHARS, DEFAULT_TRANSLATION_QUALITY_MODE,
    TRANSLATION_BACKEND_CTRANSLATE2, TRANSLATION_BACKEND_LLAMA_CPP, TRANSLATION_BACKEND_MULTI,
    TRANSLATION_BACKEND_UNAVAILABLE,
};
use super::hy_mt2::hy_mt2_hardware_acceleration;
use super::model_store::{directory_size, manifest_by_id, resolve_translation_model_dir};
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
    let worker = cfg!(feature = "native-translation");
    let models = planned_models();
    let available = models.iter().any(|model| model.available);
    let native_backend = match (ctranslate2, llama) {
        (true, true) => TRANSLATION_BACKEND_MULTI,
        (true, false) => TRANSLATION_BACKEND_CTRANSLATE2,
        (false, true) => TRANSLATION_BACKEND_LLAMA_CPP,
        (false, false) => "",
    };
    let backend = [native_backend, if worker { "bergamot-worker" } else { "" }]
        .into_iter()
        .filter(|name| !name.is_empty())
        .collect::<Vec<_>>()
        .join("+");
    TranslationCapabilities {
        available,
        backend: if backend.is_empty() {
            TRANSLATION_BACKEND_UNAVAILABLE.into()
        } else {
            backend
        },
        reason: if worker {
            "Translation engines are compiled; Bergamot also requires a compatible foreground WebView. Model files must be installed separately.".into()
        } else {
            format!("{NOT_IMPLEMENTED} Planned defaults: max {DEFAULT_MAX_SEGMENT_CHARS} chars/segment, {DEFAULT_BATCH_SEGMENT_LIMIT} segments/batch.")
        },
        platform: std::env::consts::OS.into(),
        default_quality_mode: DEFAULT_TRANSLATION_QUALITY_MODE.into(),
        hardware_acceleration: hy_mt2_hardware_acceleration(),
        models,
    }
}

/// Return install status for one catalog model.
///
/// This bridges planning and runtime: non-downloadable catalog entries explain
/// why they cannot run yet, while pinned manifests can report real disk state.
pub(super) fn translation_model_status<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    state: &TranslationState,
    request: TranslationModelStatusRequest,
) -> TranslationModelStatus {
    let Some(manifest) = manifest_by_id(&request.model_id) else {
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

    let model = find_planned_model(&request.model_id);
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
            message: if model.is_some_and(|model| model.engine == "bergamot") {
                "Bergamot assets verified; worker readiness and document translation are separate checks".into()
            } else {
                "Offline translation model installed".into()
            },
        },
        Err(_) => TranslationModelStatus {
            model_id: manifest.model_id.into(),
            installed: false,
            installing,
            model_dir: None,
            source_url: manifest.source_url.into(),
            source_label: model
                .map(|model| format!("{} ({})", model.name, model.manifest_state))
                .unwrap_or_else(|| manifest.source_label.into()),
            archive_bytes: manifest.total_bytes(),
            installed_bytes: 0,
            sha256: String::new(),
            message: if manifest.files.is_empty() {
                format!("{NOT_IMPLEMENTED} This candidate is not downloadable until source URL, checksum, license, required files, and platform gates are reviewed.")
            } else if model.is_some_and(|model| model.engine_available()) {
                "Translation model is installable. Install it before starting a translation job."
                    .into()
            } else {
                format!(
                    "{NOT_IMPLEMENTED} The file manifest is pinned, but the {} engine is not compiled in this build.",
                    model.map(|model| model.engine).unwrap_or("unavailable")
                )
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run in both minimal and engine-enabled builds: pinned files alone must not imply execution support.
    #[test]
    fn catalog_gates_each_engine_without_changing_native_order_or_admitting_candidates() {
        let caps = translation_capabilities();
        assert_eq!(caps.models[0].id, "opus-mt-es-en-ctranslate2");
        assert_eq!(caps.models[1].id, "opus-mt-fr-en-ctranslate2");
        assert_eq!(
            caps.available,
            caps.models.iter().any(|model| model.available)
        );
        for model in caps.models {
            let expected = match model.engine.as_str() {
                "ctranslate2" => cfg!(feature = "native-translation-ctranslate2"),
                "llama.cpp" => cfg!(feature = "native-translation-llama"),
                "bergamot" => cfg!(feature = "native-translation"),
                _ => false,
            } && model.manifest_state == "pinned-file-manifest";
            assert_eq!(model.available, expected, "{}", model.id);
            if model.engine == "bergamot" {
                assert!(model.name.contains("Experimental"));
                assert!(model.recommended_platforms.is_empty());
                assert_eq!(model.target_languages, ["en"]);
            }
        }
        assert_eq!(
            caps.backend.contains("bergamot-worker"),
            cfg!(feature = "native-translation")
        );
    }
}
