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
    delete_derived_document, open_document_uploads_db, persist_derived_document_with_evidence,
    validate_upload_id,
};

use super::formatting::FormattingPolicy;
use super::hash::StableHasher;
use super::quality::validate_translated_output;
use super::render::render_translated_html;
use super::source::TranslationSourceDocument;
use super::types::{
    TranslatedDocumentInfo, TranslationDeleteResponse, TranslationGlossaryEntry,
    TranslationRepairMode,
};

const TRANSLATION_SCHEMA_VERSION: &str = "3";

#[path = "review_publication.rs"]
mod publication;
pub(super) use publication::finish_translation_review;
#[cfg(test)]
pub(super) use publication::{prepare_publication, publish_prepared};

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
    /// Retained job semantics, independent of whether result fragments contain markup or strict receipts.
    pub(crate) formatting: FormattingPolicy,
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
    pub(crate) source_heading: Option<String>,
    pub(crate) source_ordinal: usize,
    pub(crate) is_heading: bool,
    pub(crate) text: String,
    pub(crate) fragments: Vec<PersistTranslationFragment>,
}

#[derive(Debug, Clone)]
pub(crate) struct PersistTranslationFragment {
    pub(crate) needs_review: bool,
    pub(crate) source_start: usize,
    pub(crate) source_end: usize,
    pub(crate) source_text: String,
    pub(crate) text: String,
    pub(crate) inline_phrases: Vec<PersistTranslationInlinePhrase>,
    pub(crate) markup: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct PersistTranslationInlinePhrase {
    pub(crate) source_text: String,
    pub(crate) text: String,
}

/// Store a completed translation as its own reader/search document.
///
/// Rendering retains source-owned shells and escapes translated prose before insertion through
/// `document_uploads` so Find, search, viewing, and TTS see the same
/// contract as imported HTML/EPUB. The translation row only records provenance
/// and delete/list metadata. `on_validated` runs after quality checks but before
/// durable writes so the caller can report the saving phase accurately.
pub(crate) fn persist_translated_document<R: Runtime>(
    app: &tauri::AppHandle<R>,
    request: PersistTranslationRequest,
    on_validated: impl FnOnce() -> Result<(), String>,
) -> Result<TranslatedDocumentInfo, String> {
    if request.translated_sections.is_empty() {
        return Err("Translation produced no sections to store".into());
    }

    let now = now_ms()?;
    let generation = (request.formatting == FormattingPolicy::SentenceReview)
        .then(new_review_generation)
        .transpose()?;
    // A new assisted generation must never overwrite old review ranges, annotations or saved audio.
    let id = generation.as_ref().map_or_else(
        || translated_document_id(&request),
        |generation| format!("{}{generation}", translated_document_id(&request)),
    );
    let document_url = format!("/uploads/{id}.html");
    let title = format!(
        "{} ({})",
        request.source.title,
        format_language_label(&request.target_language)
    );
    let view_html = render_translated_html(&title, &request)?;
    validate_translated_output(
        &view_html,
        &request.source.blocks,
        &request.translated_sections,
        &request.glossary,
        &request.target_language,
    )?;
    let engine_id = super::models::find_planned_model(&request.model_id)
        .map(|model| model.engine)
        .ok_or("Unknown translation engine for stored variant")?;
    on_validated()?;
    let mut settings = serde_json::json!({
        "providerIdentity": super::bergamot::assets::provider_identity(&request.model_id),
        "jobId": request.job_id,
        "qualityMode": request.quality_mode,
        "repairMode": request.repair_mode.label(),
        "sourceFormat": request.source.format,
        "glossaryEntries": request.glossary.len(),
    });
    if let Some(report) = &request.source.source_formatting {
        settings["sourceFormatting"] = serde_json::to_value(report)
            .map_err(|_| "Failed to serialize translation source formatting".to_string())?;
    }
    if request.formatting == FormattingPolicy::SentenceReview {
        settings["formattingPolicy"] = super::formatting::SENTENCE_REVIEW_POLICY.into();
    }
    let review_sections = request
        .translated_sections
        .iter()
        .filter(|section| {
            section
                .fragments
                .iter()
                .any(|fragment| fragment.needs_review)
        })
        .count();
    let status = if review_sections > 0 {
        "needs-review"
    } else {
        "ready"
    };
    if request.formatting == FormattingPolicy::SentenceReview {
        settings["formattingReviewSections"] = review_sections.into();
    }
    let settings_json = settings.to_string();
    let glossary_hash = translation_glossary_hash(&request.glossary);
    let mut db = open_translation_db(app)?;
    let source_evidence = generation
        .as_ref()
        .map(|_| crate::document_uploads::sanitize_html(&request.source.view_html));
    let replaced = persist_derived_document_with_evidence(
        app,
        &mut db,
        &id,
        &document_url,
        &title,
        "html",
        view_html,
        source_evidence.as_deref(),
        &request.source.assets,
        now,
        |tx, stored_html| {
            tx.execute(
            "INSERT INTO translated_documents \
             (id, source_document_id, source_document_url, title, source_language, target_language, \
              model_id, engine_id, quality_mode, settings_json, glossary_hash, status, source_path, \
              created_at_ms, updated_at_ms) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?14, ?8, ?9, ?10, ?15, ?11, ?12, ?13) \
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
                engine_id,
                status,
            ],
        )
        .map_err(db_err)?;
            if let Some(generation) = generation {
                let snapshot =
                    super::review::Snapshot::capture(&request, id.clone(), generation, stored_html)
                        .map_err(super::review_store::review_error)?;
                super::review_store::insert_in_transaction(tx, &snapshot)?;
            }
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
        status: status.into(),
        created_at_ms: now,
        updated_at_ms: now,
    })
}

/// Generation tokens belong to the saved result, not the retryable inference job/cache identity.
fn new_review_generation() -> Result<String, String> {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes)
        .map_err(|_| "Cannot create formatting review generation".to_string())?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Draft previews are separate artifacts; neither endpoint changes reader HTML, search rows or audio.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TranslationReviewDraft {
    document: super::review::Document,
    source_preview_html: String,
    preview_html: String,
    asset_paths: std::collections::HashMap<String, String>,
}

/// Legacy results without generation-owned evidence stay readable but cannot acquire guessed edit ranges.
pub(super) fn load_translation_review<R: Runtime>(
    app: &tauri::AppHandle<R>,
    id: &str,
) -> Result<Option<TranslationReviewDraft>, String> {
    validate_review_id(id)?;
    crate::native_tts::with_audiobook_reference_lock(|| {
        let db = open_translation_db(app)?;
        crate::document_uploads::recover_derived_document_locked(app, &db, id)?;
        let Some(snapshot) = super::review_store::load_in(&db, id)? else {
            return Ok(None);
        };
        verify_review_baseline(app, &db, id, &snapshot)?;
        review_draft(snapshot, review_assets(app, id)?).map(Some)
    })
}

/// The shared writer/deletion lock protects the file precondition; SQLite protects the draft revision.
pub(super) fn save_translation_review<R: Runtime>(
    app: &tauri::AppHandle<R>,
    request: &super::review::EditRequest,
) -> Result<TranslationReviewDraft, String> {
    let id = &request.expected.variant_id;
    validate_review_id(id)?;
    crate::native_tts::with_audiobook_reference_lock(|| {
        let mut db = open_translation_db(app)?;
        crate::document_uploads::recover_derived_document_locked(app, &db, id)?;
        // Prepare the entire response in the edit transaction; failure leaves the same revision retryable.
        super::review_store::apply_prepared_in(&mut db, request, |tx, snapshot| {
            verify_review_baseline(app, tx, id, &snapshot)?;
            review_draft(snapshot, review_assets(app, id)?)
        })
    })
}

fn validate_review_id(id: &str) -> Result<(), String> {
    if id.len() > 128 {
        return Err("Formatting review identity is invalid".into());
    }
    validate_upload_id(id)
}

/// Read only the native-owned canonical file, bounded before allocation; supplied paths never reach storage.
fn verify_review_baseline<R: Runtime>(
    app: &tauri::AppHandle<R>,
    db: &rusqlite::Connection,
    id: &str,
    snapshot: &super::review::Snapshot,
) -> Result<(), String> {
    use std::io::Read;
    let path = crate::document_uploads::upload_source_path(
        app,
        id,
        crate::document_uploads::StoredSourceKind::Html,
    )?;
    let metadata = std::fs::symlink_metadata(&path)
        .map_err(|_| "Formatting review published document is missing".to_string())?;
    if !metadata.is_file() || metadata.len() > super::review::MAX_SNAPSHOT_BYTES as u64 {
        return Err("Formatting review published document exceeds its bound".into());
    }
    let mut html = String::new();
    std::fs::File::open(path)
        .map_err(|_| "Cannot read formatting review published document".to_string())?
        .take(super::review::MAX_SNAPSHOT_BYTES as u64 + 1)
        .read_to_string(&mut html)
        .map_err(|_| "Cannot read formatting review published document".to_string())?;
    let matches = match super::review_store::publication_in(db, snapshot)? {
        Some(receipt) => super::review::fingerprint(&html) == receipt.hash,
        None => html == snapshot.baseline_html,
    };
    if !matches {
        return Err(super::review_store::review_error(
            super::review::Error::Evidence,
        ));
    }
    Ok(())
}

fn review_assets<R: Runtime>(
    app: &tauri::AppHandle<R>,
    id: &str,
) -> Result<std::collections::HashMap<String, String>, String> {
    crate::document_uploads::stored_reader_asset_paths(&crate::document_uploads::upload_dir(
        app, id,
    )?)
}

fn review_draft(
    snapshot: super::review::Snapshot,
    asset_paths: std::collections::HashMap<String, String>,
) -> Result<TranslationReviewDraft, String> {
    let (source_preview_html, preview_html) = snapshot
        .previews()
        .map_err(super::review_store::review_error)?;
    Ok(TranslationReviewDraft {
        document: snapshot.document,
        source_preview_html,
        preview_html,
        asset_paths,
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
    validate_upload_id(id)?;
    // Bootstrap the translation schema before the shared derived-document
    // helper opens its own connection and transaction.
    drop(open_translation_db(app)?);
    let bytes_freed = delete_derived_document(app, id, |tx| {
        tx.execute("DELETE FROM translated_documents WHERE id = ?1", [id])
            .map(|rows| rows > 0)
            .map_err(db_err)
    })?;

    Ok(TranslationDeleteResponse {
        id: id.into(),
        deleted: bytes_freed.is_some(),
        bytes_freed: bytes_freed.unwrap_or(0),
        message: if bytes_freed.is_some() {
            "Deleted translated document variant".into()
        } else {
            "Translated document variant was not found".into()
        },
    })
}

pub(super) fn open_translation_db<R: Runtime>(
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
pub(super) fn ensure_translation_schema(db: &rusqlite::Connection) -> Result<(), String> {
    db.execute_batch("PRAGMA foreign_keys = ON;")
        .map_err(db_err)?;
    // Check and advance together; a newer schema must reject instead of being silently downgraded.
    let tx = rusqlite::Transaction::new_unchecked(db, rusqlite::TransactionBehavior::Immediate)
        .map_err(db_err)?;
    let version: Option<String> = tx
        .query_row(
            "SELECT value FROM upload_schema_metadata WHERE key = 'translation_schema_version'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(db_err)?;
    if !matches!(version.as_deref(), None | Some("1") | Some("2") | Some("3")) {
        return Err("Unsupported translation schema version; a newer app may be required".into());
    }
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS translated_documents (
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
           ON translated_documents(status, updated_at_ms);
         CREATE TABLE IF NOT EXISTS translation_review (
           variant_id TEXT PRIMARY KEY NOT NULL,
           generation TEXT NOT NULL UNIQUE,
           revision INTEGER NOT NULL CHECK(revision >= 0 AND revision <= 9007199254740991),
           snapshot_json TEXT NOT NULL,
           document_json TEXT NOT NULL,
           published_revision INTEGER,
           published_hash TEXT,
           FOREIGN KEY(variant_id) REFERENCES translated_documents(id) ON DELETE CASCADE
         );",
    )
    .map_err(db_err)?;
    // v2 evidence/drafts remain byte-identical; absent receipts still require their original baseline.
    let columns = tx
        .prepare("PRAGMA table_info(translation_review)")
        .map_err(db_err)?
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(db_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(db_err)?;
    for (column, kind) in [
        ("published_revision", "INTEGER"),
        ("published_hash", "TEXT"),
    ] {
        if !columns.iter().any(|name| name == column) {
            tx.execute_batch(&format!(
                "ALTER TABLE translation_review ADD COLUMN {column} {kind}"
            ))
            .map_err(db_err)?;
        }
    }
    tx.execute(
        "INSERT INTO upload_schema_metadata(key, value) VALUES ('translation_schema_version', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [TRANSLATION_SCHEMA_VERSION],
    )
    .map_err(db_err)?;
    tx.commit().map_err(db_err)
}

/// Stable settings prefix; legacy variants retain this ID, assisted generations append a unique token.
///
/// Job/cache identity is independent of review identity: a retry may reuse inference
/// while saving a separate assisted result without destroying an earlier draft or audio.
fn translated_document_id(request: &PersistTranslationRequest) -> String {
    let mut hasher = StableHasher::new();
    hasher.write_str(&request.source.document_id);
    hasher.write_str(&request.source_language);
    hasher.write_str(&request.target_language);
    hasher.write_str(&request.model_id);
    hasher.write_str(&request.quality_mode);
    hasher.write_str(request.repair_mode.label());
    hash_glossary_entries(&request.glossary, &mut hasher);
    if request.formatting == FormattingPolicy::SentenceReview {
        hasher.write_str(super::formatting::SENTENCE_REVIEW_POLICY);
    }
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
        assert_eq!(version, "3");
    }

    #[test]
    fn schema_v1_migration_preserves_legacy_variant_and_audio_rows() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE upload_schema_metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL);
             CREATE TABLE uploaded_documents(id TEXT PRIMARY KEY);
             CREATE TABLE audio(id TEXT PRIMARY KEY,document_url TEXT NOT NULL,payload BLOB NOT NULL);
             INSERT INTO audio VALUES('old-audio','/uploads/a1b2.html',x'010203');",
        ).unwrap();
        ensure_translation_schema(&db).unwrap();
        db.execute_batch(
            "DROP TABLE translation_review;
             UPDATE upload_schema_metadata SET value='1' WHERE key='translation_schema_version';
             INSERT INTO translated_documents(id,source_document_url,title,source_language,target_language,
               model_id,engine_id,quality_mode,settings_json,status,source_path,created_at_ms,updated_at_ms)
             VALUES('a1b2','/uploads/source1.html','Legacy','fr','en','old-model','old-engine','balanced',
               '{}','ready','/uploads/a1b2.html',1,2);",
        ).unwrap();
        ensure_translation_schema(&db).unwrap();
        assert_eq!(
            db.query_row(
                "SELECT title,source_path,created_at_ms,updated_at_ms FROM translated_documents",
                [],
                |r| Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?
                ))
            )
            .unwrap(),
            ("Legacy".into(), "/uploads/a1b2.html".into(), 1, 2)
        );
        assert_eq!(
            db.query_row("SELECT payload FROM audio", [], |r| r.get::<_, Vec<u8>>(0))
                .unwrap(),
            vec![1, 2, 3]
        );
        assert_eq!(
            db.query_row("SELECT count(*) FROM translation_review", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0,
            "legacy variants must not invent evidence from a mutable original"
        );
    }

    #[test]
    fn future_or_invalid_schema_version_is_never_downgraded_or_partially_bootstrapped() {
        for version in ["4", "broken", "0"] {
            let db = Connection::open_in_memory().unwrap();
            db.execute_batch("CREATE TABLE upload_schema_metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL); CREATE TABLE uploaded_documents(id TEXT PRIMARY KEY);").unwrap();
            db.execute(
                "INSERT INTO upload_schema_metadata VALUES('translation_schema_version',?1)",
                [version],
            )
            .unwrap();
            assert!(ensure_translation_schema(&db).is_err());
            assert_eq!(
                db.query_row("SELECT value FROM upload_schema_metadata", [], |r| r
                    .get::<_, String>(0))
                    .unwrap(),
                version
            );
            assert_eq!(db.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('translated_documents','translation_review')", [], |r| r.get::<_,i64>(0)).unwrap(), 0);
        }
    }

    #[test]
    fn v2_publication_migration_preserves_evidence_drafts_and_rolls_back_failed_alters() {
        for fail in [false, true] {
            let db = Connection::open_in_memory().unwrap();
            db.execute_batch(
                "CREATE TABLE upload_schema_metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL);
                CREATE TABLE uploaded_documents(id TEXT PRIMARY KEY);",
            )
            .unwrap();
            ensure_translation_schema(&db).unwrap();
            db.execute_batch("DROP TABLE translation_review;
                CREATE TABLE translation_review(variant_id TEXT PRIMARY KEY,generation TEXT NOT NULL,
                    revision INTEGER NOT NULL,snapshot_json TEXT NOT NULL,document_json TEXT NOT NULL);
                INSERT INTO translation_review VALUES('a1b2','generation',7,'immutable exact bytes','draft exact bytes');
                UPDATE upload_schema_metadata SET value='2' WHERE key='translation_schema_version';").unwrap();
            if fail {
                db.execute_batch(
                    "CREATE TRIGGER fail_migration BEFORE UPDATE ON upload_schema_metadata
                    BEGIN SELECT RAISE(ABORT,'injected migration failure'); END;",
                )
                .unwrap();
            }
            assert_eq!(ensure_translation_schema(&db).is_err(), fail);
            let version: String = db.query_row("SELECT value FROM upload_schema_metadata WHERE key='translation_schema_version'", [], |r| r.get(0)).unwrap();
            assert_eq!(version, if fail { "2" } else { "3" });
            let columns: Vec<String> = db
                .prepare("PRAGMA table_info(translation_review)")
                .unwrap()
                .query_map([], |r| r.get(1))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            assert_eq!(columns.iter().any(|c| c == "published_hash"), !fail);
            assert_eq!(columns.iter().any(|c| c == "published_revision"), !fail);
            let preserved: (i64, String, String) = db
                .query_row(
                    "SELECT revision,snapshot_json,document_json FROM translation_review",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .unwrap();
            assert_eq!(
                preserved,
                (
                    7,
                    "immutable exact bytes".into(),
                    "draft exact bytes".into()
                )
            );
            if !fail {
                assert_eq!(db.query_row("SELECT published_revision IS NULL AND published_hash IS NULL FROM translation_review", [], |r| r.get::<_, bool>(0)).unwrap(), true);
            }
        }
    }

    #[test]
    fn failed_migration_rolls_back_table_creation_and_keeps_the_prior_version() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE upload_schema_metadata(key TEXT PRIMARY KEY,value TEXT NOT NULL);
             CREATE TABLE uploaded_documents(id TEXT PRIMARY KEY);
             INSERT INTO upload_schema_metadata VALUES('translation_schema_version','1');
             CREATE TRIGGER reject_version BEFORE UPDATE ON upload_schema_metadata
               BEGIN SELECT RAISE(ABORT,'migration failure'); END;",
        )
        .unwrap();
        assert!(ensure_translation_schema(&db).is_err());
        assert_eq!(
            db.query_row("SELECT value FROM upload_schema_metadata", [], |r| r
                .get::<_, String>(0))
                .unwrap(),
            "1"
        );
        assert_eq!(db.query_row("SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN ('translated_documents','translation_review')", [], |r| r.get::<_,i64>(0)).unwrap(), 0);
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

    /// Formatting policy remains internal; existing readers/audio keep the exact same variant URLs.
    #[test]
    fn explicit_formatting_keeps_legacy_and_sentence_variant_ids() {
        let mut request = persist_request("job-a");
        assert_eq!(super::translated_document_id(&request), "801aff7d9ec0e5a2");
        request.formatting = crate::translation::formatting::FormattingPolicy::SentenceReview;
        assert_eq!(super::translated_document_id(&request), "3e1df242d709e97e");
        request.job_id = "retry".into();
        assert_eq!(super::translated_document_id(&request), "3e1df242d709e97e");
    }

    fn persist_request(job_id: &str) -> PersistTranslationRequest {
        PersistTranslationRequest {
            formatting: crate::translation::formatting::FormattingPolicy::Provider,

            source: TranslationSourceDocument {
                document_id: "source1".into(),
                document_url: "/uploads/source1.html".into(),
                title: "Livre".into(),
                format: "html".into(),
                view_html: String::new(),
                source_formatting: None,
                assets: Default::default(),
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
    fn refuses_storage_render_without_the_source_dom() {
        let request = PersistTranslationRequest {
            formatting: crate::translation::formatting::FormattingPolicy::Provider,

            source: TranslationSourceDocument {
                document_id: "source1".into(),
                document_url: "/uploads/source1.html".into(),
                title: "Livre".into(),
                format: "html".into(),
                view_html: String::new(),
                source_formatting: None,
                assets: Default::default(),
                blocks: vec![
                    TranslationSourceBlock {
                        ordinal: 0,
                        is_heading: true,
                        heading: Some("Chapitre".into()),
                        text: "Chapitre".into(),
                    },
                    TranslationSourceBlock {
                        ordinal: 1,
                        is_heading: false,
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
                    source_heading: Some("Chapitre".into()),
                    source_ordinal: 0,
                    is_heading: true,
                    text: "Chapter".into(),
                    fragments: Vec::new(),
                },
                PersistTranslationSection {
                    source_heading: Some("Chapitre".into()),
                    source_ordinal: 1,
                    is_heading: false,
                    text: "Hello".into(),
                    fragments: Vec::new(),
                },
            ],
        };

        let html = render_translated_html("Livre (English translation)", &request);
        assert!(html.is_err());
    }
}
