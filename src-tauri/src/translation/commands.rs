//! Tauri command edge for offline translation.
//!
//! Commands are intentionally thin. Model installs, translation jobs, and
//! translated-document storage live behind backend modules so the Tauri command
//! surface stays stable as engines and packaging evolve.

use super::capabilities::{
    translation_capabilities as translation_capabilities_backend,
    translation_model_status as translation_model_status_backend,
};
use super::model_install::{
    install_translation_model as install_translation_model_backend,
    remove_translation_model as remove_translation_model_backend,
};
use super::runner::{
    cancel_translation as cancel_translation_backend,
    start_translation as start_translation_backend,
};
use super::state::TranslationState;
use super::storage::{
    delete_translated_document as delete_translated_document_storage,
    list_translated_documents as list_translated_documents_storage,
};
use super::types::{
    TranslatedDocumentInfo, TranslationCancelRequest, TranslationCapabilities,
    TranslationCommandError, TranslationDeleteRequest, TranslationDeleteResponse,
    TranslationModelInstallResponse, TranslationModelRemoveResponse, TranslationModelStatus,
    TranslationModelStatusRequest, TranslationStartRequest, TranslationStartResponse,
};

/// Only pinned, verified worker assets leave Rust, as binary IPC rather than JSON.
#[tauri::command]
pub async fn translation_bergamot_asset<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, TranslationState>,
    window: tauri::WebviewWindow<R>,
    source_language: String,
    asset: super::bergamot::assets::Asset,
) -> Result<tauri::ipc::Response, TranslationCommandError> {
    if window.label() != "main" {
        return Err(TranslationCommandError::new(
            "Bergamot assets require the main WebView".into(),
        ));
    }
    let manifest = super::bergamot::assets::manifest_for_language(&source_language)?;
    super::models::admit_provider(manifest.model_id)?;
    let claim = state.claim_job("bergamot-asset-read", manifest.directory_name)?;
    let lease = state.claim_model_read(manifest.directory_name)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _claims = (claim, lease);
        let dir = super::model_store::installed_translation_model_dir(&app, manifest)?;
        super::bergamot::assets::read_asset(&dir, manifest, asset).map(tauri::ipc::Response::new)
    })
    .await
    .map_err(|_| TranslationCommandError::new("Bergamot asset task failed".into()))?
    .map_err(TranslationCommandError::from)
}

/// Full document job using the shared Rust cache, DOM renderer and atomic writer.
#[tauri::command]
pub async fn translation_bergamot_document<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, TranslationState>,
    window: tauri::WebviewWindow<R>,
    request: TranslationStartRequest,
    on_request: tauri::ipc::Channel<super::bergamot::WorkerRequest>,
) -> Result<TranslationStartResponse, TranslationCommandError> {
    if window.label() != "main" {
        return Err(TranslationCommandError::new(
            "Bergamot worker requires the main WebView".into(),
        ));
    }
    let state = state.inner().clone();
    let owner = window.label().to_string();
    tauri::async_runtime::spawn_blocking(move || {
        super::runner::start_bergamot_translation(&app, &state, request, owner, move |message| {
            on_request
                .send(message)
                .map_err(|_| "Bergamot worker channel was lost".into())
        })
    })
    .await
    .map_err(|_| TranslationCommandError::new("Bergamot document task failed".into()))?
    .map_err(TranslationCommandError::from)
}

#[tauri::command]
pub fn translation_bergamot_reply<R: tauri::Runtime>(
    state: tauri::State<'_, TranslationState>,
    window: tauri::WebviewWindow<R>,
    reply: super::bergamot::WorkerReply,
) -> Result<bool, TranslationCommandError> {
    state
        .bergamot
        .reply(window.label(), reply)
        .map_err(TranslationCommandError::from)
}

/// Return planned offline translation capabilities and candidate catalog entries.
#[tauri::command]
pub fn translation_capabilities() -> TranslationCapabilities {
    translation_capabilities_backend()
}

/// Return install status for a catalog model or removal-only legacy resource.
#[tauri::command]
pub async fn translation_model_status<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, TranslationState>,
    request: TranslationModelStatusRequest,
) -> Result<TranslationModelStatus, TranslationCommandError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        translation_model_status_backend(&app, &state, request)
    })
    .await
    .map_err(|_| TranslationCommandError::new("Translation model status task failed".into()))
}

/// Install a trusted pinned model/auxiliary manifest, including verified bundled notices.
#[tauri::command]
pub async fn translation_install_model<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, TranslationState>,
    model_id: String,
) -> Result<TranslationModelInstallResponse, TranslationCommandError> {
    install_translation_model_backend(app, state, model_id)
        .await
        .map_err(TranslationCommandError::from)
}

/// Remove downloaded model files without deleting translated documents.
#[tauri::command]
pub async fn translation_remove_model<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, TranslationState>,
    model_id: String,
) -> Result<TranslationModelRemoveResponse, TranslationCommandError> {
    remove_translation_model_backend(app, state, model_id)
        .await
        .map_err(TranslationCommandError::from)
}

/// Start a document translation job when the selected backend is available.
#[tauri::command]
pub async fn translation_start<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, TranslationState>,
    request: TranslationStartRequest,
) -> Result<TranslationStartResponse, TranslationCommandError> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || start_translation_backend(&app, &state, request))
        .await
        .map_err(|err| {
            TranslationCommandError::new(format!("Translation start task failed: {err}"))
        })?
        .map_err(TranslationCommandError::from)
}

/// Request cancellation for a translation job.
#[tauri::command]
pub fn translation_cancel(
    state: tauri::State<'_, TranslationState>,
    request: TranslationCancelRequest,
) -> Result<(), TranslationCommandError> {
    cancel_translation_backend(state.inner(), request).map_err(TranslationCommandError::from)
}

/// List durable translated document variants.
///
/// Translated variants are durable generated documents, so listing them stays
/// separate from the normal upload list while sharing the same reader/search
/// storage underneath.
#[tauri::command]
pub async fn translation_list_documents<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
) -> Result<Vec<TranslatedDocumentInfo>, TranslationCommandError> {
    tauri::async_runtime::spawn_blocking(move || list_translated_documents_storage(&app))
        .await
        .map_err(|err| {
            TranslationCommandError::new(format!("Translation list task failed: {err}"))
        })?
        .map_err(TranslationCommandError::from)
}

/// Delete a translated document variant.
///
/// Delete is also real early because it defines an important safety boundary:
/// translated variants are disposable derived data, while original uploads stay
/// owned by `document_uploads`.
#[tauri::command]
pub async fn translation_delete_document<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    request: TranslationDeleteRequest,
) -> Result<TranslationDeleteResponse, TranslationCommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        delete_translated_document_storage(&app, &request.id)
    })
    .await
    .map_err(|err| TranslationCommandError::new(format!("Translation delete task failed: {err}")))?
    .map_err(TranslationCommandError::from)
}

/// Saved review evidence remains usable offline after the generating model or source is removed.
#[tauri::command]
pub async fn translation_review_load<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    window: tauri::WebviewWindow<R>,
    id: String,
) -> Result<Option<super::storage::TranslationReviewDraft>, TranslationCommandError> {
    if window.label() != "main" {
        return Err(TranslationCommandError::new(
            "Formatting review requires the main WebView".into(),
        ));
    }
    tauri::async_runtime::spawn_blocking(move || super::storage::load_translation_review(&app, &id))
        .await
        .map_err(|_| TranslationCommandError::new("Formatting review load task failed".into()))?
        .map_err(TranslationCommandError::from)
}

/// Decode the bounded edit contract before opening storage; clients supply no HTML or replacement text.
#[tauri::command]
pub async fn translation_review_save<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    window: tauri::WebviewWindow<R>,
    request: String,
) -> Result<super::storage::TranslationReviewDraft, TranslationCommandError> {
    if window.label() != "main" {
        return Err(TranslationCommandError::new(
            "Formatting review requires the main WebView".into(),
        ));
    }
    let request = super::review::decode_request(request.as_bytes())
        .map_err(super::review_store::review_error)
        .map_err(TranslationCommandError::from)?;
    tauri::async_runtime::spawn_blocking(move || {
        super::storage::save_translation_review(&app, &request)
    })
    .await
    .map_err(|_| TranslationCommandError::new("Formatting review save task failed".into()))?
    .map_err(TranslationCommandError::from)
}

/// Publish native approved ranges only; the main WebView cannot supply replacement HTML or text.
#[tauri::command]
pub async fn translation_review_finish<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    window: tauri::WebviewWindow<R>,
    request: String,
) -> Result<TranslatedDocumentInfo, TranslationCommandError> {
    if window.label() != "main" {
        return Err(TranslationCommandError::new(
            "Formatting review requires the main WebView".into(),
        ));
    }
    let expected = super::review::decode_finish(request.as_bytes())
        .map_err(super::review_store::review_error)
        .map_err(TranslationCommandError::from)?;
    tauri::async_runtime::spawn_blocking(move || {
        super::storage::finish_translation_review(&app, &expected)
    })
    .await
    .map_err(|_| TranslationCommandError::new("Formatting review publication task failed".into()))?
    .map_err(TranslationCommandError::from)
}

#[cfg(test)]
#[path = "admission_tests.rs"]
mod tests;
