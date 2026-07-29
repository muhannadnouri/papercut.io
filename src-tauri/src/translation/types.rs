//! Serde DTOs crossing the offline-translation Tauri boundary.
//!
//! Keep this module as the leaf of the translation tree. Commands, stubs, and
//! future native engines should share these structs instead of inventing
//! frontend-specific shapes in each layer.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Stable command error code plus the original diagnostic detail.
pub(crate) struct TranslationCommandError {
    pub(crate) code: &'static str,
    pub(crate) message: String,
}

impl TranslationCommandError {
    pub(crate) fn new(message: String) -> Self {
        Self {
            code: translation_error_code(&message),
            message,
        }
    }
}

impl From<String> for TranslationCommandError {
    fn from(message: String) -> Self {
        Self::new(message)
    }
}

/// Classify expected user-actionable failures without changing internal error types.
fn translation_error_code(message: &str) -> &'static str {
    let message = message.to_ascii_lowercase();
    if message.contains("cancelled") {
        return "operation-cancelled";
    }
    if message.contains("not compiled") || message.contains("only available in") {
        return "translation-unavailable";
    }
    if message.contains("must be installed before translation")
        || message.contains("offline translation model") && message.contains("is not installed")
    {
        return "model-not-installed";
    }
    if message.contains("not in the planned catalog")
        || message.contains("planning candidate")
        || message.contains("does not support")
    {
        return "unsupported-translation-option";
    }
    if message.contains("already in progress") || message.contains("already being translated") {
        return "operation-in-progress";
    }
    if message.contains("source document was not found") {
        return "source-not-found";
    }
    if message.contains("no translatable text")
        || message.contains("no translatable sections")
        || message.contains("produced no sections")
    {
        return "no-translatable-text";
    }
    if message.contains("translation quality check failed") {
        return "quality-check-failed";
    }
    if message.contains("translated document variant was not found") {
        return "translated-document-not-found";
    }
    "translation-failed"
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TranslationCapabilities {
    pub(crate) available: bool,
    pub(crate) backend: String,
    pub(crate) reason: String,
    pub(crate) platform: String,
    pub(crate) default_quality_mode: String,
    pub(crate) models: Vec<TranslationModelInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TranslationModelInfo {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) engine: String,
    pub(crate) tier: String,
    pub(crate) manifest_state: String,
    pub(crate) source_languages: Vec<String>,
    pub(crate) target_languages: Vec<String>,
    pub(crate) default_quality_mode: String,
    pub(crate) recommended_platforms: Vec<String>,
    pub(crate) license_notes: String,
    pub(crate) size_notes: String,
    pub(crate) notes: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TranslationModelStatus {
    pub(crate) model_id: String,
    pub(crate) installed: bool,
    pub(crate) installing: bool,
    pub(crate) model_dir: Option<String>,
    pub(crate) source_url: String,
    pub(crate) source_label: String,
    pub(crate) archive_bytes: u64,
    pub(crate) installed_bytes: u64,
    pub(crate) sha256: String,
    pub(crate) message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TranslationModelStatusRequest {
    pub(crate) model_id: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TranslationModelInstallProgress {
    pub(crate) model_id: String,
    pub(crate) status: String,
    pub(crate) message: String,
    pub(crate) downloaded_bytes: u64,
    pub(crate) total_bytes: u64,
    pub(crate) percent: u8,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TranslationModelInstallResponse {
    pub(crate) model_id: String,
    pub(crate) model_dir: String,
    pub(crate) bytes: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TranslationStartRequest {
    #[serde(default)]
    pub(crate) job_id: Option<String>,
    pub(crate) document_url: String,
    pub(crate) source_language: String,
    pub(crate) target_language: String,
    pub(crate) model_id: String,
    pub(crate) quality_mode: String,
    #[serde(default)]
    pub(crate) repair_mode: TranslationRepairMode,
    #[serde(default)]
    pub(crate) glossary: Vec<TranslationGlossaryEntry>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum TranslationRepairMode {
    Off,
    Chapter,
}

impl Default for TranslationRepairMode {
    fn default() -> Self {
        Self::Off
    }
}

impl TranslationRepairMode {
    /// Stable label used in cache identity and stored provenance metadata.
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Chapter => "chapter",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TranslationGlossaryEntry {
    pub(crate) source: String,
    pub(crate) target: String,
    #[serde(default)]
    pub(crate) note: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TranslationStartResponse {
    pub(crate) job_id: String,
    pub(crate) status: String,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TranslationJobProgress {
    pub(crate) job_id: String,
    pub(crate) status: String,
    pub(crate) message: String,
    pub(crate) model_id: String,
    pub(crate) elapsed_ms: u64,
    /// Source heading of the section currently being translated, when known.
    pub(crate) current_heading: Option<String>,
    pub(crate) completed_segments: usize,
    pub(crate) total_segments: usize,
    pub(crate) cached_segments: usize,
    pub(crate) translated_segments: usize,
    pub(crate) reused_segments_in_batch: usize,
    pub(crate) completed_batches: usize,
    pub(crate) total_batches: usize,
    pub(crate) percent: u8,
    pub(crate) preview: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TranslationCancelRequest {
    pub(crate) job_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TranslatedDocumentInfo {
    pub(crate) id: String,
    pub(crate) document_url: String,
    pub(crate) source_document_url: String,
    pub(crate) title: String,
    pub(crate) source_language: String,
    pub(crate) target_language: String,
    pub(crate) model_id: String,
    pub(crate) status: String,
    pub(crate) created_at_ms: u128,
    pub(crate) updated_at_ms: u128,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TranslationDeleteRequest {
    pub(crate) id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TranslationDeleteResponse {
    pub(crate) id: String,
    pub(crate) deleted: bool,
    pub(crate) bytes_freed: u64,
    pub(crate) message: String,
}

#[cfg(test)]
mod tests {
    use super::translation_error_code;

    #[test]
    fn expected_translation_errors_have_stable_codes() {
        assert_eq!(
            translation_error_code("Offline translation model opus is not installed"),
            "model-not-installed"
        );
        assert_eq!(
            translation_error_code("Source document has no translatable sections"),
            "no-translatable-text"
        );
        assert_eq!(
            translation_error_code("Translation quality check failed: output was empty"),
            "quality-check-failed"
        );
        assert_eq!(
            translation_error_code("unexpected database detail"),
            "translation-failed"
        );
    }
}
