//! Publish a checked formatting draft through the shared upload writer, never a second file owner.
use std::cell::Cell;

use rusqlite::{params, Connection};
use tauri::Runtime;

use super::*;
use crate::translation::{review, review_store};

/// Prepare from native evidence; recheck its revision/file under the writer's promotion lock.
pub(in crate::translation) fn finish_translation_review<R: Runtime>(
    app: &tauri::AppHandle<R>,
    expected: &review::Expected,
) -> Result<TranslatedDocumentInfo, String> {
    let id = &expected.variant_id;
    validate_review_id(id)?;
    let prepared = crate::native_tts::with_audiobook_reference_lock(|| {
        let db = open_translation_db(app)?;
        crate::document_uploads::recover_derived_document_locked(app, &db, id)?;
        let snapshot = checked_snapshot(&db, expected)?;
        verify_review_baseline(app, &db, id, &snapshot)?;
        let info = document_info(&db, id)?;
        if already_published(&db, &snapshot)? {
            return Ok::<_, String>(Err(info));
        }
        let html = snapshot
            .publication(expected)
            .map_err(review_store::review_error)?;
        let assets = crate::document_uploads::snapshot_assets_with_evidence(
            &html,
            Some(&snapshot.source_html),
            &review_assets(app, id)?,
        )?;
        let imported: i64 = db
            .query_row(
                "SELECT imported_at_ms FROM uploaded_documents WHERE id=?1",
                [id],
                |row| row.get(0),
            )
            .map_err(db_err)?;
        // Recovery markers must distinguish equal-size publications in the same millisecond.
        let at = now_ms()?.max(
            imported
                .checked_add(1)
                .ok_or("Publication timestamp exhausted")? as u128,
        );
        if at > i64::MAX as u128 || imported < 0 {
            return Err("Publication timestamp exceeds its storage bound".into());
        }
        Ok(Ok((db, snapshot, html, assets, info, imported, at)))
    })?;
    let (mut db, snapshot, html, assets, mut info, imported, at) = match prepared {
        Ok(prepared) => prepared,
        Err(info) => return Ok(info),
    };
    let duplicate = Cell::new(false);
    let result = crate::document_uploads::persist_guarded_derived_document_with_evidence(
        app,
        &mut db,
        id,
        &info.document_url,
        &info.title,
        "html",
        html,
        Some(&snapshot.source_html),
        &assets,
        at,
        |tx| {
            let current = checked_snapshot(tx, expected)?;
            if current.document != snapshot.document {
                return Err(review_store::review_error(review::Error::Stale));
            }
            verify_review_baseline(app, tx, id, &current)?;
            if already_published(tx, &current)? {
                duplicate.set(true);
                return Err("Formatting review was already published".into());
            }
            let current_imported: i64 = tx
                .query_row(
                    "SELECT imported_at_ms FROM uploaded_documents WHERE id=?1",
                    [id],
                    |row| row.get(0),
                )
                .map_err(db_err)?;
            if current_imported != imported {
                return Err(review_store::review_error(review::Error::Stale));
            }
            Ok(())
        },
        |tx, stored_html| {
            review_store::record_publication_in(tx, expected, &review::fingerprint(stored_html))?;
            let changed = tx
                .execute(
                    "UPDATE translated_documents SET status='ready', updated_at_ms=?2 WHERE id=?1",
                    params![id, at as i64],
                )
                .map_err(db_err)?;
            if changed != 1 {
                return Err(review_store::review_error(review::Error::Missing));
            }
            Ok(())
        },
    );
    // Another Finish may win while preparation runs. Re-read it; never turn a stale edit into success.
    if duplicate.get() {
        return crate::native_tts::with_audiobook_reference_lock(|| {
            let db = open_translation_db(app)?;
            crate::document_uploads::recover_derived_document_locked(app, &db, id)?;
            let current = checked_snapshot(&db, expected)?;
            verify_review_baseline(app, &db, id, &current)?;
            if !already_published(&db, &current)? {
                return Err(review_store::review_error(review::Error::Stale));
            }
            document_info(&db, id)
        });
    }
    result?;
    info.status = "ready".into();
    info.updated_at_ms = at;
    Ok(info)
}

/// A draft revision is the compare-and-swap identity; missing rows must never become an upsert.
fn checked_snapshot(
    db: &Connection,
    expected: &review::Expected,
) -> Result<review::Snapshot, String> {
    let snapshot = review_store::load_in(db, &expected.variant_id)?
        .ok_or_else(|| review_store::review_error(review::Error::Missing))?;
    if snapshot.document.expected() != expected {
        return Err(review_store::review_error(review::Error::Stale));
    }
    Ok(snapshot)
}

fn already_published(db: &Connection, snapshot: &review::Snapshot) -> Result<bool, String> {
    Ok(review_store::publication_in(db, snapshot)?
        .is_some_and(|receipt| receipt.revision == snapshot.document.expected().revision))
}

/// Read the response before mutation so returning success requires no fallible query after commit.
fn document_info(db: &Connection, id: &str) -> Result<TranslatedDocumentInfo, String> {
    db.query_row(
        "SELECT id,source_path,source_document_url,title,source_language,target_language,
         model_id,status,created_at_ms,updated_at_ms FROM translated_documents WHERE id=?1",
        [id],
        |row| {
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
        },
    )
    .map_err(db_err)
}
