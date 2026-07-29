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

/// Return planned offline translation capabilities and candidate catalog entries.
#[tauri::command]
pub fn translation_capabilities() -> TranslationCapabilities {
    translation_capabilities_backend()
}

/// Return install status for a planned translation model.
#[tauri::command]
pub fn translation_model_status<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, TranslationState>,
    request: TranslationModelStatusRequest,
) -> TranslationModelStatus {
    translation_model_status_backend(&app, &state, request)
}

/// Download and verify a pinned translation model manifest.
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
