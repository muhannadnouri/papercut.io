//! Durable assisted-formatting evidence and drafts in the existing upload database.
//! Saving a draft never writes the reader HTML, search index, MT cache or audio.

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

use super::review::{Document, EditRequest, Snapshot};

// JSON escaping can grow otherwise valid native snapshots; refuse oversized records before loading them.
const MAX_JSON_BYTES: usize = 256 * 1024 * 1024;

/// The derived-document writer inserts evidence in the same transaction as its variant metadata.
pub(super) fn insert_in_transaction(
    tx: &Transaction<'_>,
    snapshot: &Snapshot,
) -> Result<(), String> {
    snapshot.validate().map_err(review_error)?;
    let expected = snapshot.document.expected();
    if expected.revision != 0 {
        return Err("New formatting review evidence must start at revision zero".into());
    }
    let immutable = encode(snapshot)?;
    let draft = encode(&snapshot.document)?;
    tx.execute(
        "INSERT INTO translation_review(variant_id, generation, revision, snapshot_json, document_json)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            expected.variant_id,
            expected.generation,
            expected.revision as i64,
            immutable,
            draft,
        ],
    )
    .map_err(db_err)?;
    Ok(())
}

/// Read a generation without consulting its original file, model installation or inference runtime.
pub(super) fn load_in(db: &Connection, variant_id: &str) -> Result<Option<Snapshot>, String> {
    crate::document_uploads::validate_upload_id(variant_id)?;
    // CASE prevents oversized TEXT from being copied out of SQLite before its byte bound is checked.
    let stored: Option<(String, i64, Option<String>, Option<String>)> = db
        .query_row(
            "SELECT generation, revision,
                    CASE WHEN length(CAST(snapshot_json AS BLOB)) <= ?2 THEN snapshot_json END,
                    CASE WHEN length(CAST(document_json AS BLOB)) <= ?2 THEN document_json END
             FROM translation_review WHERE variant_id = ?1",
            params![variant_id, MAX_JSON_BYTES as i64],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(db_err)?;
    let Some((generation, revision, immutable, draft)) = stored else {
        return Ok(None);
    };
    let immutable = immutable.ok_or("Formatting review snapshot exceeds its storage bound")?;
    let draft = draft.ok_or("Formatting review draft exceeds its storage bound")?;
    let snapshot: Snapshot = serde_json::from_str(&immutable)
        .map_err(|_| "Invalid saved formatting review evidence".to_string())?;
    snapshot.validate().map_err(review_error)?;
    let document: Document = serde_json::from_str(&draft)
        .map_err(|_| "Invalid saved formatting review draft".to_string())?;
    let original = snapshot.document.expected();
    let expected = document.expected();
    if original.revision != 0
        || original.variant_id != variant_id
        || original.generation != generation
        || expected.variant_id != variant_id
        || expected.generation != generation
        || i64::try_from(expected.revision).ok() != Some(revision)
    {
        return Err("Saved formatting review generation or revision does not match".into());
    }
    snapshot
        .with_document(document)
        .map(Some)
        .map_err(review_error)
}

/// Serialize competing windows with SQLite, then conditionally advance only the loaded generation.
pub(super) fn apply_in(db: &mut Connection, request: &EditRequest) -> Result<Snapshot, String> {
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(db_err)?;
    let mut snapshot = load_in(&tx, &request.expected.variant_id)?
        .ok_or("Formatting review is unavailable for this saved translation")?;
    snapshot.apply(request).map_err(review_error)?;
    // An edit that cannot reconstruct safe source-owned HTML must never become a durable draft.
    snapshot.reconstruct().map_err(review_error)?;
    let draft = encode(&snapshot.document)?;
    let changed = tx
        .execute(
            "UPDATE translation_review SET revision = ?4, document_json = ?5
             WHERE variant_id = ?1 AND generation = ?2 AND revision = ?3",
            params![
                request.expected.variant_id,
                request.expected.generation,
                request.expected.revision as i64,
                snapshot.document.expected().revision as i64,
                draft,
            ],
        )
        .map_err(db_err)?;
    if changed != 1 {
        return Err("Formatting review changed; reload this saved translation".into());
    }
    tx.commit().map_err(db_err)?;
    Ok(snapshot)
}

fn encode(value: &impl serde::Serialize) -> Result<String, String> {
    let json = serde_json::to_string(value)
        .map_err(|_| "Failed to serialize formatting review evidence".to_string())?;
    if json.len() > MAX_JSON_BYTES {
        return Err("Formatting review exceeds its storage bound".into());
    }
    Ok(json)
}

pub(super) fn review_error(error: super::review::Error) -> String {
    use super::review::Error;
    match error {
        Error::Missing => "Formatting review is unavailable for this saved translation",
        Error::Version => "This formatting review requires a supported review version",
        Error::Model => "This translation model does not support assisted formatting review",
        Error::Identity => "Formatting review identity does not match this saved translation",
        Error::Limit => "Formatting review exceeds a supported bound",
        Error::Range => "Formatting selection is not a complete valid text range",
        Error::Locked => "Formatting selection crosses protected document content",
        Error::Stale => "Formatting review changed; reload this saved translation",
        Error::Evidence => "Saved formatting review evidence does not match",
        Error::Pending => "Formatting review still has unresolved obligations",
        Error::Revision => "Formatting review revision cannot be advanced",
        Error::Request => "Invalid formatting review request",
    }
    .into()
}

fn db_err(error: rusqlite::Error) -> String {
    format!("Formatting review database error: {error}")
}

#[cfg(test)]
mod tests;
