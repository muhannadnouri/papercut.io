//! SQLite and filesystem storage for translated document variants.
//!
//! Translation variants live beside uploaded documents because they should be
//! searchable/openable through the same runtime-library path later. This module
//! owns only translation metadata and variant directories; it never mutates the
//! original uploaded document rows or source files.

use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, OptionalExtension};
use tauri::Runtime;

use crate::document_uploads::{
    delete_derived_document, open_document_uploads_db, persist_derived_document,
    DerivedDocumentSection,
};

use super::hash::StableHasher;
use super::quality::validate_translated_output;
use super::render::render_translated_html;
use super::source::TranslationSourceDocument;
use super::types::{
    TranslatedDocumentInfo, TranslationDeleteResponse, TranslationGlossaryEntry,
    TranslationRepairMode,
};

const TRANSLATION_SCHEMA_VERSION: &str = "1";

pub(super) fn list_translated_documents<R: Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<Vec<TranslatedDocumentInfo>, String> {
    let db = open_translation_db(app)?;
    let mut stmt = db
        .prepare(
            "SELECT id, source_path, source_document_url, title, source_language, target_language, \
                    model_id, status, created_at_ms, updated_at_ms \
             FROM translated_documents ORDER BY updated_at_ms DESC",
        )
        .map_err(db_err)?;
    let rows = stmt
        .query_map([], |row| {
            Ok(TranslatedDocumentInfo {
                id: row.get(0)?,
                document_url: row.get(1)?,
                source_document_url: row.get(2)?,
                title: row.get(3)?,
                source_language: row.get(4)?,
                target_language: row.get(5)?,
                model_id: row.get(6)?,
                status: row.get(7)?,
                created_at_ms: row.get::<_, i64>(8)? as u128,
                updated_at_ms: row.get::<_, i64>(9)? as u128,
            })
        })
        .map_err(db_err)?;

    rows.collect::<Result<Vec<_>, _>>().map_err(db_err)
}

pub(crate) struct PersistTranslationRequest {
    pub(crate) source: TranslationSourceDocument,
    pub(crate) source_language: String,
    pub(crate) target_language: String,
    pub(crate) model_id: String,
    pub(crate) quality_mode: String,
    pub(crate) repair_mode: TranslationRepairMode,
    pub(crate) job_id: String,
    pub(crate) glossary: Vec<TranslationGlossaryEntry>,
    pub(crate) translated_sections: Vec<PersistTranslationSection>,
}

#[derive(Debug, Clone)]
pub(crate) struct PersistTranslationSection {
    pub(crate) heading: Option<String>,
    pub(crate) source_heading: Option<String>,
    pub(crate) source_ordinal: usize,
    pub(crate) is_heading: bool,
    pub(crate) text: String,
    pub(crate) fragments: Vec<PersistTranslationFragment>,
}

#[derive(Debug, Clone)]
pub(crate) struct PersistTranslationFragment {
    pub(crate) source_start: usize,
    pub(crate) source_end: usize,
    pub(crate) source_text: String,
    pub(crate) text: String,
    pub(crate) inline_phrases: Vec<PersistTranslationInlinePhrase>,
}

#[derive(Debug, Clone)]
pub(crate) struct PersistTranslationInlinePhrase {
    pub(crate) source_text: String,
    pub(crate) text: String,
}

/// Store a completed translation as its own reader/search document.
///
/// The translated text is generated as escaped plain HTML, then inserted through
/// `document_uploads` so Find, search, viewing, and TTS see the same
/// contract as imported HTML/EPUB. The translation row only records provenance
/// and delete/list metadata.
pub(crate) fn persist_translated_document<R: Runtime>(
    app: &tauri::AppHandle<R>,
    request: PersistTranslationRequest,
) -> Result<TranslatedDocumentInfo, String> {
    if request.translated_sections.is_empty() {
        return Err("Translation produced no sections to store".into());
    }

    let now = now_ms()?;
    let id = translated_document_id(&request);
    let document_url = format!("/uploads/{id}.html");
    let title = format!(
        "{} ({})",
        request.source.title,
        format_language_label(&request.target_language)
    );
    let view_html = render_translated_html(&title, &request);
    validate_translated_output(
        &view_html,
        &request.source.blocks,
        &request.translated_sections,
        &request.glossary,
        &request.target_language,
    )?;
    let bytes = view_html.as_bytes().len() as u64;
    let sections = request
        .translated_sections
        .iter()
        .map(|section| DerivedDocumentSection {
            heading: section.heading.clone(),
            text: section.text.clone(),
        })
        .collect();
    let settings_json = serde_json::json!({
        "jobId": request.job_id,
        "qualityMode": request.quality_mode,
        "repairMode": request.repair_mode.label(),
        "sourceFormat": request.source.format,
        "glossaryEntries": request.glossary.len(),
    })
    .to_string();
    let glossary_hash = translation_glossary_hash(&request.glossary);
    let mut db = open_translation_db(app)?;
    let replaced = persist_derived_document(
        app,
        &mut db,
        &id,
        &document_url,
        &title,
        "html",
        view_html,
        sections,
        now,
        bytes,
        |tx| {
            tx.execute(
            "INSERT INTO translated_documents \
             (id, source_document_id, source_document_url, title, source_language, target_language, \
              model_id, engine_id, quality_mode, settings_json, glossary_hash, status, source_path, \
              created_at_ms, updated_at_ms) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'ctranslate2', ?8, ?9, ?10, 'ready', ?11, ?12, ?13) \
             ON CONFLICT(id) DO UPDATE SET \
               source_document_id = excluded.source_document_id, \
               source_document_url = excluded.source_document_url, \
               title = excluded.title, \
               source_language = excluded.source_language, \
               target_language = excluded.target_language, \
               model_id = excluded.model_id, \
               engine_id = excluded.engine_id, \
               quality_mode = excluded.quality_mode, \
               settings_json = excluded.settings_json, \
               status = excluded.status, \
               source_path = excluded.source_path, \
               updated_at_ms = excluded.updated_at_ms",
            params![
                id.as_str(),
                request.source.document_id.as_str(),
                request.source.document_url.as_str(),
                title.as_str(),
                request.source_language.as_str(),
                request.target_language.as_str(),
                request.model_id.as_str(),
                request.quality_mode.as_str(),
                settings_json.as_str(),
                glossary_hash.as_deref(),
                document_url.as_str(),
                now as i64,
                now as i64,
            ],
        )
        .map_err(db_err)?;
            Ok(())
        },
    )?;
    if replaced {
        log::info!(
            "translation: replaced translated variant {id} for identical document and settings"
        );
    }

    Ok(TranslatedDocumentInfo {
        id,
        document_url,
        source_document_url: request.source.document_url,
        title,
        source_language: request.source_language,
        target_language: request.target_language,
        model_id: request.model_id,
        status: "ready".into(),
        created_at_ms: now,
        updated_at_ms: now,
    })
}

/// Delete one translated variant without touching the original source document.
///
/// Translation variants are derived artifacts: deleting one should behave like
/// deleting saved audiobook audio, not like deleting the imported HTML/EPUB.
/// The row deletion and directory deletion are deliberately scoped to the
/// translation id. A source upload can outlive all variants, and a missing row
/// is treated as an idempotent no-op so UI cleanup can retry safely.
pub(super) fn delete_translated_document<R: Runtime>(
    app: &tauri::AppHandle<R>,
    id: &str,
) -> Result<TranslationDeleteResponse, String> {
    // Bootstrap the translation schema before the shared derived-document
    // helper opens its own connection and transaction.
    drop(open_translation_db(app)?);
    let mut deleted_rows = 0;
    let bytes_freed = delete_derived_document(app, id, |tx| {
        deleted_rows = tx
            .execute("DELETE FROM translated_documents WHERE id = ?1", [id])
            .map_err(db_err)?;
        Ok(())
    })?;

    Ok(TranslationDeleteResponse {
        id: id.into(),
        deleted: deleted_rows > 0,
        bytes_freed,
        message: if deleted_rows > 0 {
            "Deleted translated document variant".into()
        } else {
            "Translated document variant was not found".into()
        },
    })
}

fn open_translation_db<R: Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<rusqlite::Connection, String> {
    let db = open_document_uploads_db(app)?;
    ensure_translation_schema(&db)?;
    Ok(db)
}

/// Create translation tables inside the existing runtime upload/search DB.
///
/// Keeping variants in the upload database makes future joins cheap: translated
/// copies can point back to uploaded source documents, share delete/list flows,
/// and later promote their generated HTML into the same reader/search contract.
/// The schema is additive only; uploaded document rows and FTS rows are not
/// changed by this bootstrap.
fn ensure_translation_schema(db: &rusqlite::Connection) -> Result<(), String> {
    db.execute_batch(
        "PRAGMA foreign_keys = ON;
         INSERT OR REPLACE INTO upload_schema_metadata (key, value)
           VALUES ('translation_schema_version', '1');
         CREATE TABLE IF NOT EXISTS translated_documents (
           id TEXT PRIMARY KEY,
           source_document_id TEXT,
           source_document_url TEXT NOT NULL,
           title TEXT NOT NULL,
           source_language TEXT NOT NULL,
           target_language TEXT NOT NULL,
           model_id TEXT NOT NULL,
           engine_id TEXT NOT NULL,
           quality_mode TEXT NOT NULL,
           settings_json TEXT NOT NULL,
           glossary_hash TEXT,
           status TEXT NOT NULL,
           source_path TEXT NOT NULL,
           created_at_ms INTEGER NOT NULL,
           updated_at_ms INTEGER NOT NULL,
           FOREIGN KEY(source_document_id) REFERENCES uploaded_documents(id) ON DELETE SET NULL
         );
         CREATE INDEX IF NOT EXISTS translated_documents_source_idx
           ON translated_documents(source_document_id, target_language, updated_at_ms);
         CREATE INDEX IF NOT EXISTS translated_documents_status_idx
           ON translated_documents(status, updated_at_ms);",
    )
    .map_err(db_err)?;

    let version: Option<String> = db
        .query_row(
            "SELECT value FROM upload_schema_metadata WHERE key = 'translation_schema_version'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(db_err)?;
    if version.as_deref() != Some(TRANSLATION_SCHEMA_VERSION) {
        return Err("Unexpected translation schema version".into());
    }

    Ok(())
}

/// Deterministic variant identity from source document plus translation settings.
///
/// Deliberately excludes job id and timestamps: re-running the same document
/// with the same settings must produce the same id so the stored variant is
/// replaced rather than duplicated. Changing any setting (languages, model,
/// quality, repair mode, glossary) yields a separate variant.
fn translated_document_id(request: &PersistTranslationRequest) -> String {
    let mut hasher = StableHasher::new();
    hasher.write_str(&request.source.document_id);
    hasher.write_str(&request.source_language);
    hasher.write_str(&request.target_language);
    hasher.write_str(&request.model_id);
    hasher.write_str(&request.quality_mode);
    hasher.write_str(request.repair_mode.label());
    hash_glossary_entries(&request.glossary, &mut hasher);
    hasher.finish_hex()
}

/// Stored provenance hash; must stay stable across app/Rust upgrades so the
/// same glossary always records the same identity.
fn translation_glossary_hash(glossary: &[TranslationGlossaryEntry]) -> Option<String> {
    if glossary.is_empty() {
        return None;
    }
    let mut hasher = StableHasher::new();
    hash_glossary_entries(glossary, &mut hasher);
    Some(hasher.finish_hex())
}

fn hash_glossary_entries(glossary: &[TranslationGlossaryEntry], hasher: &mut StableHasher) {
    for entry in glossary {
        hasher.write_str(entry.source.trim());
        hasher.write_str(entry.target.trim());
        hasher.write_str(entry.note.as_deref().unwrap_or("").trim());
    }
}

fn format_language_label(language: &str) -> String {
    if language.eq_ignore_ascii_case("en") {
        "English translation".into()
    } else {
        format!("{} translation", language.to_uppercase())
    }
}

fn now_ms() -> Result<u128, String> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("System clock error: {err}"))?
        .as_millis())
}

fn db_err(err: rusqlite::Error) -> String {
    format!("Translation database error: {err}")
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::{ensure_translation_schema, PersistTranslationRequest, PersistTranslationSection};
    use crate::translation::render::render_translated_html;
    use crate::translation::source::{TranslationSourceBlock, TranslationSourceDocument};

    #[test]
    fn schema_bootstrap_is_idempotent() {
        let db = Connection::open_in_memory().expect("db");
        db.execute_batch(
            "CREATE TABLE upload_schema_metadata (
               key TEXT PRIMARY KEY,
               value TEXT NOT NULL
             );
             CREATE TABLE uploaded_documents (
               id TEXT PRIMARY KEY,
               url TEXT NOT NULL UNIQUE,
               title TEXT NOT NULL,
               format TEXT NOT NULL,
               imported_at_ms INTEGER NOT NULL,
               bytes INTEGER NOT NULL,
               sections INTEGER NOT NULL
             );",
        )
        .expect("base schema");

        ensure_translation_schema(&db).expect("first bootstrap");
        ensure_translation_schema(&db).expect("second bootstrap");

        let version: String = db
            .query_row(
                "SELECT value FROM upload_schema_metadata WHERE key = 'translation_schema_version'",
                [],
                |row| row.get(0),
            )
            .expect("version");
        assert_eq!(version, "1");
    }

    #[test]
    fn translated_document_id_is_stable_per_document_and_settings() {
        let first = persist_request("job-a");
        let second = persist_request("job-b");
        assert_eq!(
            super::translated_document_id(&first),
            super::translated_document_id(&second),
            "same document and settings must map to the same variant id"
        );

        let mut different_target = persist_request("job-a");
        different_target.target_language = "de".into();
        assert_ne!(
            super::translated_document_id(&first),
            super::translated_document_id(&different_target)
        );
    }

    fn persist_request(job_id: &str) -> PersistTranslationRequest {
        PersistTranslationRequest {
            source: TranslationSourceDocument {
                document_id: "source1".into(),
                document_url: "/uploads/source1.html".into(),
                title: "Livre".into(),
                format: "html".into(),
                view_html: String::new(),
                blocks: Vec::new(),
            },
            source_language: "fr".into(),
            target_language: "en".into(),
            model_id: "opus-mt-fr-en-ctranslate2".into(),
            quality_mode: "balanced".into(),
            repair_mode: Default::default(),
            job_id: job_id.into(),
            glossary: Vec::new(),
            translated_sections: Vec::new(),
        }
    }

    #[test]
    fn translated_html_preserves_section_order_and_heading_shape() {
        let request = PersistTranslationRequest {
            source: TranslationSourceDocument {
                document_id: "source1".into(),
                document_url: "/uploads/source1.html".into(),
                title: "Livre".into(),
                format: "html".into(),
                view_html: String::new(),
                blocks: vec![
                    TranslationSourceBlock {
                        ordinal: 0,
                        heading: Some("Chapitre".into()),
                        text: "Chapitre".into(),
                    },
                    TranslationSourceBlock {
                        ordinal: 1,
                        heading: Some("Chapitre".into()),
                        text: "Bonjour".into(),
                    },
                ],
            },
            source_language: "fr".into(),
            target_language: "en".into(),
            model_id: "opus-mt-fr-en-ctranslate2".into(),
            quality_mode: "balanced".into(),
            repair_mode: Default::default(),
            job_id: "job1".into(),
            glossary: Vec::new(),
            translated_sections: vec![
                PersistTranslationSection {
                    heading: Some("Chapitre".into()),
                    source_heading: Some("Chapitre".into()),
                    source_ordinal: 0,
                    is_heading: true,
                    text: "Chapter".into(),
                    fragments: Vec::new(),
                },
                PersistTranslationSection {
                    heading: Some("Chapitre".into()),
                    source_heading: Some("Chapitre".into()),
                    source_ordinal: 1,
                    is_heading: false,
                    text: "Hello".into(),
                    fragments: Vec::new(),
                },
            ],
        };

        let html = render_translated_html("Livre (English translation)", &request);

        assert!(html.contains("id=\"translation-section-1\""));
        assert!(html.contains("data-source-ordinal=\"0\""));
        assert!(html.contains("<h2 id=\"translation-section-1-heading\">Chapter</h2>"));
        assert!(html.contains("id=\"translation-section-2\""));
        assert!(html.contains("<p>Hello</p>"));
        assert!(!html.contains("<h2>Chapitre</h2>"));
    }
}
