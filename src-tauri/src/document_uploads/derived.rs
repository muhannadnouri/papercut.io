//! Storage bridge for generated HTML documents such as translated variants.

use std::fs;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, Transaction};
use tauri::Runtime;

use super::html::{parsed_html_document, sanitize_html};
use super::parsed::{ParsedDocument, ParsedDocumentAsset};
use super::pipeline;
use super::reader_assets::{owned_assets, snapshot_assets};
use super::storage::{
    directory_size, upload_dir, upload_url, validate_upload_id, StoredSourceKind,
};
use super::store::{
    delete_document_rows_in_transaction, open_db, upsert_document_in_transaction,
    validate_upload_deletion_id, PdfTextStatus,
};
use super::types::UploadedDocumentSourceRequest;

const DERIVED_GENERATION_FILE: &str = ".generation";
const MAX_SOURCE_EVIDENCE_BYTES: usize = 120 * 1024 * 1024;

/// Open the shared upload database without exposing the store module.
pub(crate) fn open_document_uploads_db<R: Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<rusqlite::Connection, String> {
    open_db(app)
}

/// Read generated reader HTML through the same current sanitizer as the viewer.
pub(crate) fn read_uploaded_document_source<R: Runtime>(
    app: &tauri::AppHandle<R>,
    document_url: &str,
) -> Result<(String, Vec<ParsedDocumentAsset>), String> {
    // Paths alone are not a snapshot: keep promotion blocked until asset bytes are copied too.
    crate::native_tts::with_audiobook_reference_lock(|| {
        let source = pipeline::get_source_locked(
            app,
            UploadedDocumentSourceRequest {
                document_url: document_url.into(),
            },
        )?;
        let assets = snapshot_assets(&source.html, &source.asset_paths)?;
        Ok((source.html, assets))
    })
}

/// Stage and index a generated HTML document without exposing parser internals.
///
/// Existing files stay staged as a backup until upload/index rows and feature
/// metadata commit together. A failed replacement therefore restores the
/// previous usable variant instead of leaving a partial Library entry.
#[allow(clippy::too_many_arguments)]
#[cfg(test)]
pub(crate) fn persist_derived_document<R, F>(
    app: &tauri::AppHandle<R>,
    db: &mut Connection,
    id: &str,
    url: &str,
    title: &str,
    format: &str,
    view_html: String,
    assets: &[ParsedDocumentAsset],
    imported_at_ms: u128,
    persist_metadata: F,
) -> Result<bool, String>
where
    R: Runtime,
    F: FnOnce(&Transaction<'_>) -> Result<(), String>,
{
    persist_derived_document_with_evidence(
        app,
        db,
        id,
        url,
        title,
        format,
        view_html,
        None,
        assets,
        imported_at_ms,
        |tx, _| persist_metadata(tx),
    )
}

/// Keep source-only review assets in the same owned directory and transaction.
/// The callback binds feature evidence to the exact sanitized, indexed HTML.
#[allow(clippy::too_many_arguments)]
pub(crate) fn persist_derived_document_with_evidence<R, F>(
    app: &tauri::AppHandle<R>,
    db: &mut Connection,
    id: &str,
    url: &str,
    title: &str,
    format: &str,
    view_html: String,
    source_evidence: Option<&str>,
    assets: &[ParsedDocumentAsset],
    imported_at_ms: u128,
    persist_metadata: F,
) -> Result<bool, String>
where
    R: Runtime,
    F: FnOnce(&Transaction<'_>, &str) -> Result<(), String>,
{
    persist_guarded_derived_document_with_evidence(
        app,
        db,
        id,
        url,
        title,
        format,
        view_html,
        source_evidence,
        assets,
        imported_at_ms,
        |_| Ok(()),
        persist_metadata,
    )
}

/// Check caller-owned revision/ownership evidence before creating any replacement files.
/// The guard and metadata update share one immediate transaction under the reference lock.
#[allow(clippy::too_many_arguments)]
pub(crate) fn persist_guarded_derived_document_with_evidence<R, G, F>(
    app: &tauri::AppHandle<R>,
    db: &mut Connection,
    id: &str,
    url: &str,
    title: &str,
    format: &str,
    view_html: String,
    source_evidence: Option<&str>,
    assets: &[ParsedDocumentAsset],
    imported_at_ms: u128,
    preflight: G,
    persist_metadata: F,
) -> Result<bool, String>
where
    R: Runtime,
    G: FnOnce(&Transaction<'_>) -> Result<(), String>,
    F: FnOnce(&Transaction<'_>, &str) -> Result<(), String>,
{
    validate_upload_id(id)?;
    let dir = upload_dir(app, id)?;
    let parsed = prepare_derived_document(title, format, &view_html, source_evidence, assets)?;
    // Deleters use this same lock: protect admission/recovery/promotion as well as SQL.
    crate::native_tts::with_audiobook_reference_lock(|| {
        persist_guarded_derived_document_at(
            &dir,
            db,
            id,
            url,
            parsed,
            imported_at_ms,
            preflight,
            persist_metadata,
        )
    })
}

/// Recover the shared writer's interrupted replacement; caller owns the reference lock.
pub(crate) fn recover_derived_document_locked<R: Runtime>(
    app: &tauri::AppHandle<R>,
    db: &Connection,
    id: &str,
) -> Result<(), String> {
    validate_upload_id(id)?;
    let dir = upload_dir(app, id)?;
    let backup_dir = dir.with_file_name(format!(".{id}.replacing"));
    recover_interrupted_replacement(db, id, &dir, &backup_dir)
}

fn prepare_derived_document(
    title: &str,
    format: &str,
    html: &str,
    source_evidence: Option<&str>,
    assets: &[ParsedDocumentAsset],
) -> Result<ParsedDocument, String> {
    if let Some(source) = source_evidence {
        if source.len() > MAX_SOURCE_EVIDENCE_BYTES {
            return Err("Source review evidence exceeds the size limit".into());
        }
        if sanitize_html(source) != source {
            return Err("Source review evidence must be sanitized reader HTML".into());
        }
    }
    let mut parsed = parsed_html_document(title.into(), format, sanitize_html(html));
    parsed.assets = owned_assets(&parsed.view_html, source_evidence, assets)?;
    Ok(parsed)
}

#[cfg(test)]
fn persist_derived_document_at<F>(
    dir: &Path,
    db: &mut Connection,
    id: &str,
    url: &str,
    parsed: ParsedDocument,
    imported_at_ms: u128,
    persist_metadata: F,
) -> Result<bool, String>
where
    F: FnOnce(&Transaction<'_>, &str) -> Result<(), String>,
{
    persist_guarded_derived_document_at(
        dir,
        db,
        id,
        url,
        parsed,
        imported_at_ms,
        |_| Ok(()),
        persist_metadata,
    )
}

/// Keep filesystem rollback and feature metadata inside the same guarded write boundary.
#[allow(clippy::too_many_arguments)]
fn persist_guarded_derived_document_at<G, F>(
    dir: &Path,
    db: &mut Connection,
    id: &str,
    url: &str,
    parsed: ParsedDocument,
    imported_at_ms: u128,
    preflight: G,
    persist_metadata: F,
) -> Result<bool, String>
where
    G: FnOnce(&Transaction<'_>) -> Result<(), String>,
    F: FnOnce(&Transaction<'_>, &str) -> Result<(), String>,
{
    let bytes = parsed.view_html.len() as u64
        + parsed
            .assets
            .iter()
            .map(|asset| asset.bytes.len() as u64)
            .sum::<u64>();
    let staging_dir = dir.with_file_name(format!(".{id}.staging"));
    let backup_dir = dir.with_file_name(format!(".{id}.replacing"));
    recover_interrupted_replacement(db, id, dir, &backup_dir)?;
    let tx = db
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(db_err)?;
    preflight(&tx)?;
    if staging_dir.exists() {
        fs::remove_dir_all(&staging_dir).map_err(|err| {
            format!(
                "Failed to clear stale derived document staging directory {}: {err}",
                staging_dir.display()
            )
        })?;
    }
    fs::create_dir_all(&staging_dir).map_err(|err| {
        format!(
            "Failed to create derived document staging directory {}: {err}",
            staging_dir.display()
        )
    })?;
    if let Err(err) = fs::write(
        staging_dir.join(DERIVED_GENERATION_FILE),
        format!("{imported_at_ms}:{bytes}"),
    ) {
        let _ = fs::remove_dir_all(&staging_dir);
        return Err(format!(
            "Failed to write derived document generation marker: {err}"
        ));
    }
    if let Err(err) = fs::write(
        staging_dir.join(StoredSourceKind::Html.file_name()),
        parsed.view_html.as_bytes(),
    ) {
        let _ = fs::remove_dir_all(&staging_dir);
        return Err(format!("Failed to write derived document source: {err}"));
    }
    if let Err(error) = pipeline::write_reader_assets(&staging_dir, &parsed.assets) {
        let _ = fs::remove_dir_all(&staging_dir);
        return Err(error);
    }
    let replaced = dir.exists();
    if replaced {
        fs::rename(&dir, &backup_dir).map_err(|err| {
            let _ = fs::remove_dir_all(&staging_dir);
            format!(
                "Failed to stage existing derived document {}: {err}",
                dir.display()
            )
        })?;
    }
    if let Err(err) = fs::rename(&staging_dir, &dir) {
        let _ = fs::remove_dir_all(&staging_dir);
        return Err(restore_replaced_directory(
            &dir,
            &backup_dir,
            format!(
                "Failed to promote derived document directory {}: {err}",
                dir.display()
            ),
        ));
    }

    let transaction_result = (|| {
        upsert_document_in_transaction(
            &tx,
            id,
            url,
            &parsed,
            None,
            StoredSourceKind::Html,
            imported_at_ms,
            bytes,
            PdfTextStatus::Ready,
        )?;
        persist_metadata(&tx, &parsed.view_html)?;
        tx.commit().map_err(db_err)
    })();
    if let Err(error) = transaction_result {
        return Err(restore_replaced_directory(&dir, &backup_dir, error));
    }

    if backup_dir.exists() {
        if let Err(error) = fs::remove_dir_all(&backup_dir) {
            log::warn!(
                "Derived document was stored, but its replacement backup could not be removed {}: {error}",
                backup_dir.display()
            );
        }
    }
    Ok(replaced)
}

/// Delete only after feature metadata proves ownership inside the write transaction.
/// Missing provenance is a no-op, including unowned live/staged storage.
pub(crate) fn delete_derived_document<R, F>(
    app: &tauri::AppHandle<R>,
    id: &str,
    delete_metadata: F,
) -> Result<Option<u64>, String>
where
    R: Runtime,
    F: FnOnce(&Transaction<'_>) -> Result<bool, String>,
{
    validate_upload_id(id)?;
    crate::native_tts::with_audiobook_reference_lock(|| {
        let mut db = open_db(app)?;
        // Keep ownership stable until the staged files and all dependent rows commit.
        let tx = db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(db_err)?;
        validate_upload_deletion_id(&tx, id)?;
        if !delete_metadata(&tx)? {
            return Ok(None);
        }
        if crate::native_tts::document_has_audiobook_reference(
            app,
            &upload_url(id, StoredSourceKind::Html),
        )? {
            return Err("Delete the saved audio that uses this document before removing it from the Library".into());
        }
        let dir = upload_dir(app, id)?;
        let retired_bytes = prepare_owned_document_deletion(&tx, id, &dir)?;
        pipeline::delete_stored_document(&dir, id, || {
            delete_document_rows_in_transaction(&tx, id)?;
            tx.commit().map_err(db_err)
        })
        .map(|bytes| Some(retired_bytes + bytes))
    })
}

/// Reconcile publication scratch while ownership and the reference lock still hold.
/// Only uncommitted/obsolete files are retired before the compensating deletion.
pub(super) fn prepare_owned_document_deletion(
    db: &Connection,
    id: &str,
    dir: &Path,
) -> Result<u64, String> {
    pipeline::recover_stored_document_deletion(dir, id)?;
    let staging = dir.with_file_name(format!(".{id}.staging"));
    let backup = dir.with_file_name(format!(".{id}.replacing"));
    let before = directory_size(dir)? + directory_size(&staging)? + directory_size(&backup)?;
    recover_interrupted_replacement(db, id, dir, &backup)?;
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|err| {
            format!(
                "Failed to clear uncommitted derived document staging {}: {err}",
                staging.display()
            )
        })?;
    }
    Ok(before - directory_size(dir)?)
}

/// Recover a replacement interrupted before or after its SQLite commit.
///
/// The live generation marker mirrors the upload row timestamp and size. A
/// match means the replacement committed; otherwise the previous files win.
fn recover_interrupted_replacement(
    db: &Connection,
    id: &str,
    dir: &Path,
    backup_dir: &Path,
) -> Result<(), String> {
    if !backup_dir.exists() {
        return Ok(());
    }
    let stored_generation = db
        .query_row(
            "SELECT imported_at_ms, bytes FROM uploaded_documents WHERE id = ?1",
            [id],
            |row| {
                Ok(format!(
                    "{}:{}",
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?
                ))
            },
        )
        .optional()
        .map_err(db_err)?
        .ok_or("Interrupted derived document replacement has no current upload row")?;
    let backup_generation = fs::read_to_string(backup_dir.join(DERIVED_GENERATION_FILE)).ok();
    if !dir.exists() {
        // Missing committed output is not proof that old backup files match newer metadata.
        if backup_generation.as_deref() != Some(stored_generation.as_str()) {
            return Err(
                "Interrupted derived document replacement has no matching live generation".into(),
            );
        }
        return fs::rename(backup_dir, dir).map_err(|err| {
            format!(
                "Failed to recover interrupted derived document replacement {}: {err}",
                backup_dir.display()
            )
        });
    }

    let live_generation = fs::read_to_string(dir.join(DERIVED_GENERATION_FILE)).ok();
    if live_generation.as_deref() == Some(stored_generation.as_str()) {
        fs::remove_dir_all(backup_dir).map_err(|err| {
            format!(
                "Failed to clear committed derived document backup {}: {err}",
                backup_dir.display()
            )
        })
    } else {
        if backup_generation
            .as_deref()
            .is_some_and(|generation| generation != stored_generation)
        {
            return Err(
                "Interrupted derived document backup does not match the current upload generation"
                    .into(),
            );
        }
        fs::remove_dir_all(dir).map_err(|err| {
            format!(
                "Failed to discard interrupted derived document replacement {}: {err}",
                dir.display()
            )
        })?;
        fs::rename(backup_dir, dir).map_err(|err| {
            format!(
                "Failed to restore derived document backup {}: {err}",
                backup_dir.display()
            )
        })
    }
}

fn restore_replaced_directory(dir: &Path, backup_dir: &Path, error: String) -> String {
    let remove_error = dir
        .exists()
        .then(|| fs::remove_dir_all(dir).err())
        .flatten();
    let restore_error = backup_dir
        .exists()
        .then(|| fs::rename(backup_dir, dir).err())
        .flatten();
    match (remove_error, restore_error) {
        (None, None) => error,
        (remove_error, restore_error) => format!(
            "{error}; failed to restore previous derived document (remove: {}; restore: {})",
            remove_error
                .map(|value| value.to_string())
                .unwrap_or_else(|| "ok".into()),
            restore_error
                .map(|value| value.to_string())
                .unwrap_or_else(|| "ok".into())
        ),
    }
}

fn db_err(err: rusqlite::Error) -> String {
    format!("Document database error: {err}")
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use kuchikiki::traits::TendrilSink;
    use rusqlite::{params, Connection};

    use super::{recover_interrupted_replacement, DERIVED_GENERATION_FILE};

    #[test]
    fn review_original_survives_reopen_but_only_translation_enters_the_search_index() {
        use super::{
            persist_derived_document_at, prepare_derived_document, sanitize_html,
            ParsedDocumentAsset,
        };
        use crate::document_uploads::store::open_db_in;
        let root = test_root("translation-review");
        let mut db = open_db_in(&root).unwrap();
        let asset =
            ParsedDocumentAsset::new("image/png", b"retained original image".to_vec()).unwrap();
        let html = format!("<article><h2 data-papercut-translation-review='original' id='heading'>Titre original</h2><h2 data-papercut-translation-review='label'>Translation — formatting needs review</h2><h2>English heading</h2><p id='ref' data-papercut-translation-review='original'><strong>Original <em>emphasis</em></strong><a href='#note' role='doc-noteref'>1</a><img data-papercut-asset='{}'></p><p data-papercut-translation-review='label'>Translation</p><p>Translated meaning.</p><p id='note' data-papercut-translation-review='original'>Original note<a href='#ref' role='doc-backlink'>Back</a></p><p>Translated note.</p></article>", asset.file_name);
        let parsed =
            prepare_derived_document("English", "html", &html, None, &[asset.clone()]).unwrap();
        assert_eq!(
            parsed
                .sections
                .iter()
                .map(|s| s.text.as_str())
                .collect::<Vec<_>>(),
            ["English heading", "Translated meaning.", "Translated note."]
        );
        assert!(parsed
            .sections
            .iter()
            .all(|s| s.heading.as_deref() == Some("English heading")));
        let live = root.join("variant");
        persist_derived_document_at(
            &live,
            &mut db,
            "variant",
            "/uploads/variant.html",
            parsed,
            1,
            |_, _| Ok(()),
        )
        .unwrap();
        let saved = fs::read_to_string(live.join("source.html")).unwrap();
        let reopened =
            prepare_derived_document("English", "html", &saved, None, &[asset.clone()]).unwrap();
        assert_eq!(
            reopened
                .sections
                .iter()
                .map(|s| s.text.as_str())
                .collect::<Vec<_>>(),
            ["English heading", "Translated meaning.", "Translated note."]
        );
        let dom = kuchikiki::parse_html()
            .one(sanitize_html(&saved))
            .document_node;
        for id in ["heading", "ref", "note"] {
            assert_eq!(dom.select(&format!("#{id}")).unwrap().count(), 1);
        }
        assert_eq!(
            dom.select("#ref > strong > em")
                .unwrap()
                .next()
                .unwrap()
                .text_contents(),
            "emphasis"
        );
        assert_eq!(
            dom.select("[data-papercut-translation-review='original'][data-papercut-section]")
                .unwrap()
                .count(),
            0
        );
        assert_eq!(dom.select("[data-papercut-section]").unwrap().count(), 3);
        let rows = db
            .prepare(
                "SELECT text FROM uploaded_sections WHERE document_id='variant' ORDER BY ordinal",
            )
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            rows,
            ["English heading", "Translated meaning.", "Translated note."]
        );
        assert_eq!(
            fs::read(live.join("assets").join(&asset.file_name)).unwrap(),
            asset.bytes
        );
        drop(db);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn saved_reader_retains_protection_markers_and_formatting_through_the_real_preparation_path() {
        use super::{persist_derived_document_at, prepare_derived_document, sanitize_html};
        use crate::document_uploads::store::open_db_in;
        let root = test_root("metadata");
        let mut db = open_db_in(&root).unwrap();
        let html = r#"<p translate="no">user_id</p><p id="r"><strong>Translated<a role="doc-noteref" href='#n'>1</a>.</strong></p><p id="n" role="doc-endnote">Note<a role="doc-backlink" href='#r'>Back</a></p>"#;
        let parsed = prepare_derived_document("English", "html", html, None, &[]).unwrap();
        let expected = parsed.view_html.clone();
        let live = root.join("variant");
        persist_derived_document_at(
            &live,
            &mut db,
            "variant",
            "/uploads/variant.html",
            parsed,
            1,
            |_, _| Ok(()),
        )
        .unwrap();
        let saved = fs::read_to_string(live.join("source.html")).unwrap();
        assert_eq!(saved, expected);
        let reader = sanitize_html(&saved);
        assert!(reader.contains("translate=\"no\""));
        assert!(reader.contains("role=\"doc-noteref\""));
        assert!(reader.contains("role=\"doc-backlink\""));
        assert!(reader.contains("<strong>Translated"));
        let rows: Vec<(String, String)> = db.prepare("SELECT text,body_ranges FROM uploaded_sections WHERE document_id='variant' ORDER BY ordinal").unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?))).unwrap().collect::<Result<_,_>>().unwrap();
        assert_eq!(
            rows.iter().map(|row| row.0.as_str()).collect::<Vec<_>>(),
            ["user_id", "Translated1.", "NoteBack"]
        );
        assert_eq!(rows[2].1, "[]");
        drop(db);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn derived_assets_survive_source_deletion_and_failed_replacement_rolls_back() {
        use super::{
            parsed_html_document, persist_derived_document_at, prepare_derived_document,
            snapshot_assets, ParsedDocumentAsset,
        };
        use crate::document_uploads::{pipeline::stored_reader_asset_paths, store::open_db_in};
        let root = test_root("asset-rollback");
        let mut db = open_db_in(&root).unwrap();
        let source = root.join("source");
        fs::create_dir_all(source.join("assets")).unwrap();
        let asset =
            ParsedDocumentAsset::new("image/png", b"original image bytes".to_vec()).unwrap();
        fs::write(source.join("assets").join(&asset.file_name), &asset.bytes).unwrap();
        let html = format!("<article><h1>Chapter</h1><div>Hello <a href='#note'>note</a><img data-papercut-asset='{}'></div><aside role=doc-endnotes><p id=note>Note body</p></aside></article>", asset.file_name);
        let paths = stored_reader_asset_paths(&source).unwrap();
        let assets = snapshot_assets(&html, &paths).unwrap();
        let parsed = prepare_derived_document("English", "html", &html, None, &assets).unwrap();
        let expected_html = parsed.view_html.clone();
        let expected_bytes = expected_html.len() + asset.bytes.len();
        let live = root.join("variant");
        assert!(!persist_derived_document_at(
            &live,
            &mut db,
            "variant",
            "/uploads/variant.html",
            parsed,
            1,
            |_, _| Ok(())
        )
        .unwrap());
        fs::remove_dir_all(source).unwrap();
        assert_eq!(
            fs::read(live.join("assets").join(&asset.file_name)).unwrap(),
            asset.bytes
        );
        assert_eq!(
            fs::read_to_string(live.join("source.html")).unwrap(),
            expected_html
        );
        let sections: Vec<(String, String)> = db.prepare("SELECT text, body_ranges FROM uploaded_sections WHERE document_id='variant' ORDER BY ordinal").unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?))).unwrap().collect::<Result<_,_>>().unwrap();
        assert_eq!(sections.len(), 3);
        assert_eq!(sections[0], ("Chapter".into(), "[]".into()));
        assert_eq!(sections[1].0, "Hello note");
        assert_ne!(sections[1].1, "[]");
        assert_eq!(sections[2], ("Note body".into(), "[]".into()));
        let bytes: i64 = db
            .query_row(
                "SELECT bytes FROM uploaded_documents WHERE id='variant'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(bytes as usize, expected_bytes);
        let replacement = parsed_html_document(
            "Replacement".into(),
            "html",
            "<p>Replacement text</p>".into(),
        );
        assert!(persist_derived_document_at(
            &live,
            &mut db,
            "variant",
            "/uploads/variant.html",
            replacement,
            2,
            |_, _| Err("metadata failure".into())
        )
        .is_err());
        assert_eq!(
            fs::read_to_string(live.join("source.html")).unwrap(),
            expected_html
        );
        assert_eq!(
            fs::read(live.join("assets").join(&asset.file_name)).unwrap(),
            asset.bytes
        );
        let indexed: String = db
            .query_row(
                "SELECT text FROM uploaded_sections WHERE document_id='variant' AND ordinal=1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(indexed, "Hello note");
        assert!(!root.join(".variant.staging").exists());
        assert!(!root.join(".variant.replacing").exists());
        let replacement = parsed_html_document(
            "New edition".into(),
            "html",
            "<h1>New chapter</h1><p>New body</p>".into(),
        );
        assert!(persist_derived_document_at(
            &live,
            &mut db,
            "variant",
            "/uploads/variant.html",
            replacement,
            3,
            |_, _| Ok(())
        )
        .unwrap());
        assert!(!live.join("assets").exists());
        let indexed: String = db
            .query_row(
                "SELECT text FROM uploaded_sections WHERE document_id='variant' AND ordinal=1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(indexed, "New body");
        drop(db);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn source_only_evidence_assets_are_owned_once_and_rollback_with_the_snapshot() {
        use super::{persist_derived_document_at, prepare_derived_document, sanitize_html};
        use crate::document_uploads::{
            pipeline::stored_reader_asset_paths, reader_assets::snapshot_assets, store::open_db_in,
            ParsedDocumentAsset,
        };
        let root = test_root("review-evidence-assets");
        let mut db = open_db_in(&root).unwrap();
        db.execute_batch(
            "CREATE TABLE review_evidence (html TEXT NOT NULL, published TEXT NOT NULL)",
        )
        .unwrap();
        let source_dir = root.join("original");
        fs::create_dir_all(source_dir.join("assets")).unwrap();
        let source_only = ParsedDocumentAsset::new("image/png", b"source only".to_vec()).unwrap();
        let shared = ParsedDocumentAsset::new("image/png", b"shared".to_vec()).unwrap();
        for asset in [&source_only, &shared] {
            fs::write(
                source_dir.join("assets").join(&asset.file_name),
                &asset.bytes,
            )
            .unwrap();
        }
        let source = sanitize_html(&format!(
            "<p id='ref'>Original<a role='doc-noteref' href='#note'>1</a><img data-papercut-asset='{}'><img data-papercut-asset='{}'></p><p id='note'>Original note<a role='doc-backlink' href='#ref'>Back</a></p>",
            source_only.file_name, shared.file_name
        ));
        let assets =
            snapshot_assets(&source, &stored_reader_asset_paths(&source_dir).unwrap()).unwrap();
        let published = format!(
            "<p>English meaning.<img data-papercut-asset='{}'></p>",
            shared.file_name
        );
        let parsed =
            prepare_derived_document("English", "html", &published, Some(&source), &assets)
                .unwrap();
        let expected = parsed.view_html.clone();
        assert_eq!(parsed.assets.len(), 2);
        let live = root.join("variant");
        persist_derived_document_at(
            &live,
            &mut db,
            "variant",
            "/uploads/variant.html",
            parsed,
            1,
            |tx, html| {
                assert_eq!(html, expected);
                tx.execute(
                    "INSERT INTO review_evidence VALUES (?1, ?2)",
                    [&source, html],
                )
                .unwrap();
                Ok(())
            },
        )
        .unwrap();
        fs::remove_dir_all(source_dir).unwrap();
        drop(db);
        let mut db = open_db_in(&root).unwrap();
        assert_eq!(
            db.query_row("SELECT html, published FROM review_evidence", [], |row| Ok(
                (row.get::<_, String>(0)?, row.get::<_, String>(1)?)
            ))
            .unwrap(),
            (source.clone(), expected.clone())
        );
        assert_eq!(stored_reader_asset_paths(&live).unwrap().len(), 2);
        for asset in [&source_only, &shared] {
            assert_eq!(
                fs::read(live.join("assets").join(&asset.file_name)).unwrap(),
                asset.bytes
            );
        }
        assert_eq!(
            db.query_row(
                "SELECT text FROM uploaded_sections WHERE document_id='variant'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            "English meaning."
        );
        let bytes: i64 = db
            .query_row(
                "SELECT bytes FROM uploaded_documents WHERE id='variant'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            bytes as usize,
            expected.len() + source_only.bytes.len() + shared.bytes.len()
        );

        let replacement_asset =
            ParsedDocumentAsset::new("image/png", b"replacement".to_vec()).unwrap();
        let replacement_source = sanitize_html(&format!(
            "<img data-papercut-asset='{}'>",
            replacement_asset.file_name
        ));
        let replacement = prepare_derived_document(
            "Replacement",
            "html",
            "<p>Changed English.</p>",
            Some(&replacement_source),
            &[replacement_asset.clone()],
        )
        .unwrap();
        assert!(persist_derived_document_at(
            &live,
            &mut db,
            "variant",
            "/uploads/variant.html",
            replacement,
            2,
            |tx, html| {
                tx.execute(
                    "UPDATE review_evidence SET html=?1, published=?2",
                    [&replacement_source, html],
                )
                .unwrap();
                Err("review snapshot failure".into())
            },
        )
        .unwrap_err()
        .contains("review snapshot failure"));
        assert_eq!(
            fs::read_to_string(live.join("source.html")).unwrap(),
            expected
        );
        assert_eq!(
            db.query_row("SELECT html FROM review_evidence", [], |row| row
                .get::<_, String>(0))
                .unwrap(),
            source
        );
        assert_eq!(stored_reader_asset_paths(&live).unwrap().len(), 2);
        assert!(!live
            .join("assets")
            .join(replacement_asset.file_name)
            .exists());
        assert_eq!(
            db.query_row(
                "SELECT text FROM uploaded_sections WHERE document_id='variant'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            "English meaning."
        );
        drop(db);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn review_source_assets_require_sanitized_bounded_evidence_and_verified_bytes() {
        use super::{
            prepare_derived_document, sanitize_html, ParsedDocumentAsset, MAX_SOURCE_EVIDENCE_BYTES,
        };
        let mut asset = ParsedDocumentAsset::new("image/png", b"original".to_vec()).unwrap();
        let source = sanitize_html(&format!("<img data-papercut-asset='{}'>", asset.file_name));
        let error = |source: &str, assets: &[ParsedDocumentAsset]| {
            prepare_derived_document("English", "html", "<p>English.</p>", Some(source), assets)
                .err()
                .expect("invalid source evidence")
        };
        assert!(error(&source, &[]).contains("missing"));
        asset.bytes = b"forged".to_vec();
        assert!(error(&source, &[asset]).contains("hash"));
        assert!(error("<script>alert(1)</script>", &[]).contains("sanitized"));
        assert!(error("<img data-papercut-asset='../private'>", &[]).contains("sanitized"));
        assert!(error(&"x".repeat(MAX_SOURCE_EVIDENCE_BYTES + 1), &[]).contains("size limit"));
    }

    #[test]
    fn guarded_admission_reserves_sqlite_before_creating_any_replacement_files() {
        use super::{persist_guarded_derived_document_at, prepare_derived_document};
        use crate::document_uploads::store::open_db_in;
        let root = test_root("guarded-admission");
        let mut db = open_db_in(&root).unwrap();
        let competing = Connection::open(root.join("search.sqlite3")).unwrap();
        competing.busy_timeout(std::time::Duration::ZERO).unwrap();
        let live = root.join("variant");
        let parsed = prepare_derived_document("English", "html", "<p>New.</p>", None, &[]).unwrap();
        let error = persist_guarded_derived_document_at(
            &live,
            &mut db,
            "variant",
            "/uploads/variant.html",
            parsed,
            2,
            |tx| {
                assert!(!tx.is_autocommit());
                assert!(!live.exists());
                assert!(!root.join(".variant.staging").exists());
                assert!(!root.join(".variant.replacing").exists());
                assert!(matches!(
                    competing.execute("DELETE FROM uploaded_documents WHERE id='variant'", []),
                    Err(rusqlite::Error::SqliteFailure(error, _))
                        if error.code == rusqlite::ErrorCode::DatabaseBusy
                ));
                Err("stale or deleted variant".into())
            },
            |_, _| panic!("rejected admission must not persist metadata"),
        )
        .unwrap_err();
        assert!(error.contains("stale or deleted variant"));
        assert!(!live.exists());
        assert!(!root.join(".variant.staging").exists());
        assert!(!root.join(".variant.replacing").exists());
        assert_eq!(
            db.query_row("SELECT count(*) FROM uploaded_documents", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
        // The failed guard releases its reservation rather than blocking the next edit/delete.
        competing
            .execute("DELETE FROM uploaded_documents WHERE id='variant'", [])
            .unwrap();
        drop(competing);
        drop(db);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn guarded_index_and_metadata_failures_restore_every_previous_generation_owner() {
        use super::{
            persist_derived_document_at, persist_guarded_derived_document_at,
            prepare_derived_document,
        };
        use crate::document_uploads::store::open_db_in;
        let root = test_root("guarded-rollback");
        let mut db = open_db_in(&root).unwrap();
        db.execute_batch("CREATE TABLE feature_receipt (state TEXT NOT NULL); INSERT INTO feature_receipt VALUES ('old');").unwrap();
        let live = root.join("variant");
        let original =
            prepare_derived_document("Original", "html", "<p>Previous English.</p>", None, &[])
                .unwrap();
        let old_html = original.view_html.clone();
        persist_derived_document_at(
            &live,
            &mut db,
            "variant",
            "/uploads/variant.html",
            original,
            1,
            |_, _| Ok(()),
        )
        .unwrap();
        for fail_index in [true, false] {
            if fail_index {
                db.execute_batch("CREATE TRIGGER fail_derived_index BEFORE INSERT ON uploaded_sections BEGIN SELECT RAISE(ABORT, 'index failure'); END;").unwrap();
            }
            let replacement =
                prepare_derived_document("New", "html", "<p>Replacement English.</p>", None, &[])
                    .unwrap();
            let error = persist_guarded_derived_document_at(
                &live,
                &mut db,
                "variant",
                "/uploads/variant.html",
                replacement,
                2,
                |tx| {
                    assert_eq!(
                        tx.query_row("SELECT state FROM feature_receipt", [], |row| row
                            .get::<_, String>(0))
                            .unwrap(),
                        "old"
                    );
                    assert_eq!(
                        fs::read_to_string(live.join("source.html")).unwrap(),
                        old_html
                    );
                    Ok(())
                },
                |tx, _| {
                    assert!(
                        !fail_index,
                        "index failure must happen before feature mutation"
                    );
                    tx.execute("UPDATE feature_receipt SET state='new'", [])
                        .unwrap();
                    Err("metadata failure".into())
                },
            )
            .unwrap_err();
            assert!(error.contains(if fail_index {
                "index failure"
            } else {
                "metadata failure"
            }));
            assert_eq!(
                fs::read_to_string(live.join("source.html")).unwrap(),
                old_html
            );
            assert_eq!(
                db.query_row(
                    "SELECT imported_at_ms FROM uploaded_documents WHERE id='variant'",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
                1
            );
            assert_eq!(
                db.query_row(
                    "SELECT text FROM uploaded_sections WHERE document_id='variant'",
                    [],
                    |row| row.get::<_, String>(0)
                )
                .unwrap(),
                "Previous English."
            );
            assert_eq!(
                db.query_row("SELECT state FROM feature_receipt", [], |row| row
                    .get::<_, String>(0))
                    .unwrap(),
                "old"
            );
            assert!(!root.join(".variant.staging").exists());
            assert!(!root.join(".variant.replacing").exists());
            if fail_index {
                db.execute_batch("DROP TRIGGER fail_derived_index").unwrap();
            }
        }
        drop(db);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recovery_never_restores_old_files_under_committed_or_deleted_metadata() {
        let root = test_root("missing-live-recovery");
        let live = root.join("variant");
        let backup = root.join(".variant.replacing");
        write_generation(&backup, "1:10", "old");
        recover_interrupted_replacement(&database("1:10"), "variant", &live, &backup).unwrap();
        assert_eq!(fs::read_to_string(live.join("source.html")).unwrap(), "old");
        assert!(!backup.exists());
        fs::rename(&live, &backup).unwrap();
        assert!(
            recover_interrupted_replacement(&database("2:20"), "variant", &live, &backup)
                .unwrap_err()
                .contains("no matching live generation")
        );
        assert!(!live.exists());
        assert!(backup.exists());
        let deleted = database("1:10");
        deleted
            .execute("DELETE FROM uploaded_documents", [])
            .unwrap();
        assert!(
            recover_interrupted_replacement(&deleted, "variant", &live, &backup)
                .unwrap_err()
                .contains("no current upload row")
        );
        assert!(!live.exists());
        assert_eq!(
            fs::read_to_string(backup.join("source.html")).unwrap(),
            "old"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reader_waits_for_failed_promotion_and_recovers_interrupted_output_before_serving() {
        use super::{
            persist_derived_document, persist_guarded_derived_document_with_evidence,
            UploadedDocumentSourceRequest,
        };
        use crate::document_uploads::{pipeline, store::open_db_in};
        use std::sync::mpsc;
        use std::time::Duration;
        let root = test_root("reader-publication");
        fs::create_dir_all(&root).unwrap();
        let mut context = tauri::test::mock_context(tauri::test::noop_assets());
        context.config_mut().identifier = root.to_str().unwrap().into();
        let app = tauri::test::mock_builder().build(context).unwrap();
        let uploads = root.join("document_uploads");
        let mut db = open_db_in(&uploads).unwrap();
        let live = uploads.join("abc123");
        persist_derived_document(
            app.handle(),
            &mut db,
            "abc123",
            "/uploads/abc123.html",
            "Old",
            "html",
            "<p>Committed English.</p>".into(),
            &[],
            1,
            |_| Ok(()),
        )
        .unwrap();
        let original = fs::read_to_string(live.join("source.html")).unwrap();
        let (started_tx, started_rx) = mpsc::channel();
        let (finished_tx, finished_rx) = mpsc::channel();
        let mut reader = None;
        let error = persist_guarded_derived_document_with_evidence(
            app.handle(),
            &mut db,
            "abc123",
            "/uploads/abc123.html",
            "New",
            "html",
            "<p>Uncommitted English.</p>".into(),
            None,
            &[],
            2,
            |_| Ok(()),
            |_, _| {
                assert!(fs::read_to_string(live.join("source.html"))
                    .unwrap()
                    .contains("Uncommitted English."));
                let handle = app.handle().clone();
                reader = Some(std::thread::spawn(move || {
                    started_tx.send(()).unwrap();
                    finished_tx
                        .send(pipeline::get_source(
                            &handle,
                            UploadedDocumentSourceRequest {
                                document_url: "/uploads/abc123.html".into(),
                            },
                        ))
                        .unwrap();
                }));
                started_rx.recv().unwrap();
                assert!(matches!(
                    finished_rx.recv_timeout(Duration::from_millis(100)),
                    Err(mpsc::RecvTimeoutError::Timeout)
                ));
                Err("publication failed".into())
            },
        )
        .unwrap_err();
        assert!(error.contains("publication failed"));
        reader.unwrap().join().unwrap();
        let served = finished_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap();
        assert!(served.html.contains("Committed English."));
        assert!(!served.html.contains("Uncommitted English."));
        assert_eq!(
            fs::read_to_string(live.join("source.html")).unwrap(),
            original
        );

        // A previous process can die after promotion; the next ordinary read repairs from SQLite.
        let backup = uploads.join(".abc123.replacing");
        fs::rename(&live, &backup).unwrap();
        write_generation(&live, "2:999", "<p>Interrupted English.</p>");
        let served = pipeline::get_source(
            app.handle(),
            UploadedDocumentSourceRequest {
                document_url: "/uploads/abc123.html".into(),
            },
        )
        .unwrap();
        assert!(served.html.contains("Committed English."));
        assert!(!served.html.contains("Interrupted English."));
        assert_eq!(
            fs::read_to_string(live.join("source.html")).unwrap(),
            original
        );
        assert!(!backup.exists());
        drop(db);
        drop(app);
        fs::remove_dir_all(root).unwrap();
    }

    fn test_root(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "papercut-derived-{name}-{}-{nonce}",
            std::process::id()
        ))
    }

    fn database(generation: &str) -> Connection {
        let db = Connection::open_in_memory().expect("database");
        db.execute_batch(
            "CREATE TABLE uploaded_documents (
                id TEXT PRIMARY KEY,
                imported_at_ms INTEGER NOT NULL,
                bytes INTEGER NOT NULL
            );",
        )
        .expect("schema");
        let (timestamp, bytes) = generation.split_once(':').expect("generation");
        db.execute(
            "INSERT INTO uploaded_documents (id, imported_at_ms, bytes) VALUES ('variant', ?1, ?2)",
            params![
                timestamp.parse::<i64>().expect("timestamp"),
                bytes.parse::<i64>().expect("bytes")
            ],
        )
        .expect("row");
        db
    }

    fn write_generation(dir: &std::path::Path, generation: &str, content: &str) {
        fs::create_dir_all(dir).expect("directory");
        fs::write(dir.join(DERIVED_GENERATION_FILE), generation).expect("marker");
        fs::write(dir.join("source.html"), content).expect("content");
    }

    #[test]
    fn replacement_recovery_follows_the_committed_database_generation() {
        let root = test_root("recovery");
        let live = root.join("variant");
        let backup = root.join(".variant.replacing");

        write_generation(&live, "2:20", "new");
        write_generation(&backup, "1:10", "old");
        recover_interrupted_replacement(&database("1:10"), "variant", &live, &backup)
            .expect("restore pre-commit backup");
        assert_eq!(
            fs::read_to_string(live.join("source.html")).expect("restored content"),
            "old"
        );
        assert!(!backup.exists());

        write_generation(&backup, "1:10", "old");
        write_generation(&live, "2:20", "new");
        recover_interrupted_replacement(&database("2:20"), "variant", &live, &backup)
            .expect("keep committed replacement");
        assert_eq!(
            fs::read_to_string(live.join("source.html")).expect("committed content"),
            "new"
        );
        assert!(!backup.exists());

        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn deletion_retires_every_interrupted_publication_phase_and_rolls_back_to_committed_files() {
        use super::{directory_size, pipeline, prepare_owned_document_deletion};
        for phase in ["staging", "backup", "promoted", "committed", "deleting"] {
            for fail_delete in [false, true] {
                let root = test_root(&format!("delete-publication-{phase}-{fail_delete}"));
                let live = root.join("variant");
                let staging = root.join(".variant.staging");
                let backup = root.join(".variant.replacing");
                let deleting = root.join(".variant.deleting");
                let db = database(if phase == "committed" { "2:20" } else { "1:10" });
                match phase {
                    "staging" => {
                        write_generation(&live, "1:10", "old");
                        write_generation(&staging, "2:20", "new");
                    }
                    "backup" => {
                        write_generation(&backup, "1:10", "old");
                        write_generation(&staging, "2:20", "new");
                    }
                    "promoted" | "committed" => {
                        write_generation(&backup, "1:10", "old");
                        write_generation(&live, "2:20", "new");
                    }
                    "deleting" => {
                        write_generation(&deleting, "1:10", "old");
                        write_generation(&staging, "2:20", "new");
                    }
                    _ => unreachable!(),
                }
                let expected_bytes = [&live, &staging, &backup, &deleting]
                    .iter()
                    .map(|path| directory_size(path).unwrap())
                    .sum::<u64>();
                let retired = prepare_owned_document_deletion(&db, "variant", &live).unwrap();
                let expected = if phase == "committed" { "new" } else { "old" };
                assert_eq!(
                    fs::read_to_string(live.join("source.html")).unwrap(),
                    expected
                );
                assert!(!staging.exists());
                assert!(!backup.exists());
                let result = pipeline::delete_stored_document(&live, "variant", || {
                    assert_eq!(
                        fs::read_to_string(deleting.join("source.html")).unwrap(),
                        expected
                    );
                    if fail_delete {
                        Err("injected deletion failure".into())
                    } else {
                        db.execute("DELETE FROM uploaded_documents", []).unwrap();
                        Ok(())
                    }
                });
                if fail_delete {
                    assert_eq!(result.unwrap_err(), "injected deletion failure");
                    assert_eq!(
                        fs::read_to_string(live.join("source.html")).unwrap(),
                        expected
                    );
                    assert_eq!(
                        db.query_row("SELECT count(*) FROM uploaded_documents", [], |row| row
                            .get::<_, usize>(
                            0
                        ))
                        .unwrap(),
                        1
                    );
                    assert_eq!(
                        prepare_owned_document_deletion(&db, "variant", &live).unwrap(),
                        0
                    );
                    pipeline::delete_stored_document(&live, "variant", || {
                        db.execute("DELETE FROM uploaded_documents", []).unwrap();
                        Ok(())
                    })
                    .unwrap();
                } else {
                    assert_eq!(retired + result.unwrap(), expected_bytes);
                }
                for path in [&live, &staging, &backup, &deleting] {
                    assert!(!path.exists(), "{phase}: {}", path.display());
                }
                // Reopening recovery after successful deletion cannot resurrect any generation.
                recover_interrupted_replacement(&db, "variant", &live, &backup).unwrap();
                assert!(!live.exists());
                fs::remove_dir_all(root).unwrap();
            }
        }
    }

    #[test]
    fn deletion_scratch_cleanup_failure_keeps_the_committed_document_retryable() {
        use super::prepare_owned_document_deletion;
        let root = test_root("delete-publication-cleanup-failure");
        let live = root.join("variant");
        let staging = root.join(".variant.staging");
        let db = database("1:10");
        write_generation(&live, "1:10", "old");
        fs::write(&staging, "not a directory").unwrap();
        assert!(prepare_owned_document_deletion(&db, "variant", &live)
            .unwrap_err()
            .contains("Failed to clear uncommitted"));
        assert_eq!(fs::read_to_string(live.join("source.html")).unwrap(), "old");
        assert_eq!(
            db.query_row("SELECT count(*) FROM uploaded_documents", [], |row| row
                .get::<_, usize>(
                0
            ))
            .unwrap(),
            1
        );
        fs::remove_file(&staging).unwrap();
        write_generation(&staging, "2:20", "new");
        assert!(prepare_owned_document_deletion(&db, "variant", &live).unwrap() > 0);
        assert_eq!(fs::read_to_string(live.join("source.html")).unwrap(), "old");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn deletion_rejects_publication_case_aliases_before_recovering_or_removing_files() {
        use super::prepare_owned_document_deletion;
        for alias in [".Variant.staging", ".Variant.replacing"] {
            let root = test_root("delete-publication-case-alias");
            let live = root.join("variant");
            let deleting = root.join(".variant.deleting");
            let unowned = root.join(alias);
            let db = database("1:10");
            write_generation(&deleting, "1:10", "old");
            write_generation(&unowned, "2:20", "sentinel");
            assert!(prepare_owned_document_deletion(&db, "variant", &live)
                .unwrap_err()
                .contains("letter case"));
            assert!(!live.exists());
            assert_eq!(
                fs::read_to_string(deleting.join("source.html")).unwrap(),
                "old"
            );
            assert_eq!(
                fs::read_to_string(unowned.join("source.html")).unwrap(),
                "sentinel"
            );
            fs::remove_dir_all(root).unwrap();
        }
    }
}
