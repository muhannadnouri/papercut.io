//! Optional derived index in the upload database. No model or source text is stored here.

use std::ops::Range;

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use sha2::{Digest, Sha256};

pub(super) const SCHEMA_VERSION: i64 = 1;
// Change this whenever extraction, source normalization or passage construction changes.
const POLICY: &str = "uploaded-sections-v1/authored-v1/e5-prefix-v1/chunks-v1";
pub(crate) const CHUNK_TOKENS: usize = 320;
pub(crate) const OVERLAP_TOKENS: usize = 40;

pub(super) struct Identity {
    pub key: String,
    pub dimensions: usize,
}

impl Identity {
    pub fn new(encoder: &str, dimensions: usize) -> Result<Self, String> {
        if encoder.is_empty() || encoder.len() > 2048 || !(1..=4096).contains(&dimensions) {
            return Err("Invalid semantic index identity".into());
        }
        Ok(Self {
            key: serde_json::to_string(&(
                POLICY,
                encoder,
                dimensions,
                CHUNK_TOKENS,
                OVERLAP_TOKENS,
            ))
            .map_err(err)?,
            dimensions,
        })
    }
}

pub(super) struct Section {
    pub ordinal: usize,
    pub heading: Option<String>,
    pub text: String,
}

pub(super) struct Source {
    pub id: String,
    pub title: String,
    pub generation: i64,
    pub indexed: bool,
    pub sections: Vec<Section>,
}

#[derive(Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Coverage {
    pub indexed_documents: usize,
    pub eligible_documents: usize,
    pub passages: usize,
}

fn err(error: impl std::fmt::Display) -> String {
    format!("Semantic index: {error}")
}

pub(super) fn exists(db: &Connection) -> Result<bool, String> {
    db.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name='upload_semantic_metadata' AND type='table')",
        [], |row| row.get(0),
    ).map_err(err)
}

/// Removal retains empty tables/triggers; saved checkpoints exist independently of active coverage.
pub(super) fn has_stored_index(db: &Connection) -> Result<bool, String> {
    if !exists(db)? {
        return Ok(false);
    }
    db.query_row(
        "SELECT identity<>'' OR EXISTS(SELECT 1 FROM upload_semantic_passages) FROM upload_semantic_metadata WHERE id=1",
        [], |row| row.get(0),
    ).map_err(err)
}

/// Called only by an explicit indexing request, never by normal database startup.
pub(super) fn initialize(db: &mut Connection, identity: &Identity) -> Result<(), String> {
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(err)?;
    if exists(&tx)? {
        let version: i64 = tx
            .query_row(
                "SELECT schema_version FROM upload_semantic_metadata WHERE id=1",
                [],
                |row| row.get(0),
            )
            .map_err(err)?;
        if version != SCHEMA_VERSION {
            return Err(
                "Unsupported semantic index schema; lexical search remains available".into(),
            );
        }
    }
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS upload_semantic_metadata (
           id INTEGER PRIMARY KEY CHECK(id=1), schema_version INTEGER NOT NULL,
           identity TEXT NOT NULL, generation INTEGER NOT NULL DEFAULT 0);
         CREATE TABLE IF NOT EXISTS upload_semantic_documents (
           document_id TEXT PRIMARY KEY, generation INTEGER NOT NULL, ready INTEGER NOT NULL DEFAULT 0,
           FOREIGN KEY(document_id) REFERENCES uploaded_documents(id) ON DELETE CASCADE);
         CREATE TABLE IF NOT EXISTS upload_semantic_passages (
           document_id TEXT NOT NULL, section_ordinal INTEGER NOT NULL,
           byte_start INTEGER NOT NULL, byte_end INTEGER NOT NULL, content_hash TEXT NOT NULL,
           vector BLOB NOT NULL, vector_hash TEXT NOT NULL,
           PRIMARY KEY(document_id, section_ordinal, byte_start, byte_end),
           FOREIGN KEY(document_id) REFERENCES upload_semantic_documents(document_id) ON DELETE CASCADE,
           CHECK(byte_start>=0 AND byte_end>byte_start));
         CREATE INDEX IF NOT EXISTS upload_semantic_content_idx ON upload_semantic_passages(document_id,content_hash);
         CREATE TRIGGER IF NOT EXISTS upload_semantic_document_insert AFTER INSERT ON uploaded_documents BEGIN
           UPDATE upload_semantic_metadata SET generation=generation+1 WHERE id=1;
           INSERT INTO upload_semantic_documents(document_id,generation)
             VALUES(new.id,(SELECT generation FROM upload_semantic_metadata WHERE id=1));
         END;
         CREATE TRIGGER IF NOT EXISTS upload_semantic_document_update AFTER UPDATE OF title,url,text_status ON uploaded_documents
           WHEN (old.title,old.url,old.text_status) IS NOT (new.title,new.url,new.text_status) BEGIN
           UPDATE upload_semantic_metadata SET generation=generation+1 WHERE id=1;
           UPDATE upload_semantic_documents SET ready=0,generation=(SELECT generation FROM upload_semantic_metadata WHERE id=1)
             WHERE document_id=new.id;
         END;
         CREATE TRIGGER IF NOT EXISTS upload_semantic_section_insert AFTER INSERT ON uploaded_sections BEGIN
           UPDATE upload_semantic_metadata SET generation=generation+1 WHERE id=1;
           UPDATE upload_semantic_documents SET ready=0,generation=(SELECT generation FROM upload_semantic_metadata WHERE id=1)
             WHERE document_id=new.document_id;
         END;
         CREATE TRIGGER IF NOT EXISTS upload_semantic_section_delete AFTER DELETE ON uploaded_sections BEGIN
           UPDATE upload_semantic_metadata SET generation=generation+1 WHERE id=1;
           UPDATE upload_semantic_documents SET ready=0,generation=(SELECT generation FROM upload_semantic_metadata WHERE id=1)
             WHERE document_id=old.document_id;
         END;
         CREATE TRIGGER IF NOT EXISTS upload_semantic_section_update AFTER UPDATE ON uploaded_sections
           WHEN (old.document_id,old.ordinal,old.page_index,old.heading,old.text)
             IS NOT (new.document_id,new.ordinal,new.page_index,new.heading,new.text) BEGIN
           UPDATE upload_semantic_metadata SET generation=generation+1 WHERE id=1;
           UPDATE upload_semantic_documents SET ready=0,generation=(SELECT generation FROM upload_semantic_metadata WHERE id=1)
             WHERE document_id IN (old.document_id,new.document_id);
         END;",
    ).map_err(err)?;
    tx.execute("INSERT OR IGNORE INTO upload_semantic_metadata(id,schema_version,identity) VALUES(1,?1,?2)", params![SCHEMA_VERSION, identity.key]).map_err(err)?;
    tx.execute("INSERT OR IGNORE INTO upload_semantic_documents(document_id,generation) SELECT id,0 FROM uploaded_documents", []).map_err(err)?;
    let previous: String = tx
        .query_row(
            "SELECT identity FROM upload_semantic_metadata WHERE id=1",
            [],
            |row| row.get(0),
        )
        .map_err(err)?;
    if previous != identity.key {
        // An explicit build replaces incompatible derived vectors, never the lexical/source tables.
        tx.execute("DELETE FROM upload_semantic_passages", [])
            .map_err(err)?;
        tx.execute(
            "UPDATE upload_semantic_metadata SET generation=generation+1 WHERE id=1",
            [],
        )
        .map_err(err)?;
        tx.execute("UPDATE upload_semantic_documents SET ready=0,generation=(SELECT generation FROM upload_semantic_metadata WHERE id=1)", []).map_err(err)?;
        tx.execute(
            "UPDATE upload_semantic_metadata SET identity=?1 WHERE id=1",
            [&identity.key],
        )
        .map_err(err)?;
    }
    tx.commit().map_err(err)
}

pub(super) fn document_ids(db: &Connection) -> Result<Vec<String>, String> {
    let mut stmt = db.prepare("SELECT id FROM uploaded_documents d WHERE text_status<>'processing' AND EXISTS(SELECT 1 FROM uploaded_sections s WHERE s.document_id=d.id AND length(trim(s.text))>0) ORDER BY id").map_err(err)?;
    let ids = stmt
        .query_map([], |row| row.get(0))
        .map_err(err)?
        .collect::<Result<_, _>>()
        .map_err(err)?;
    Ok(ids)
}

/// Pin title, ordered sections and the invalidation generation to one read snapshot.
pub(super) fn source(db: &mut Connection, id: &str) -> Result<Option<Source>, String> {
    let tx = db.transaction().map_err(err)?;
    let metadata: Option<(String, i64, bool)> = tx.query_row(
        "SELECT d.title,i.generation,i.ready FROM uploaded_documents d JOIN upload_semantic_documents i ON i.document_id=d.id WHERE d.id=?1 AND d.text_status<>'processing'",
        [id], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
    ).optional().map_err(err)?;
    let Some((title, generation, indexed)) = metadata else {
        return Ok(None);
    };
    let mut stmt = tx.prepare("SELECT ordinal,heading,text FROM uploaded_sections WHERE document_id=?1 ORDER BY ordinal").map_err(err)?;
    let sections = stmt
        .query_map([id], |row| {
            Ok(Section {
                ordinal: row.get(0)?,
                heading: row.get(1)?,
                text: row.get(2)?,
            })
        })
        .map_err(err)?
        .collect::<Result<_, _>>()
        .map_err(err)?;
    Ok(Some(Source {
        id: id.into(),
        title,
        generation,
        indexed,
        sections,
    }))
}

/// Generations also reject delete/recreate and model-switch-away/back races with identical IDs.
fn check_current(db: &Connection, source: &Source, identity: &Identity) -> Result<(), String> {
    let current: Option<(String, i64)> = db.query_row(
        "SELECT m.identity,d.generation FROM upload_semantic_metadata m CROSS JOIN upload_semantic_documents d WHERE m.id=1 AND d.document_id=?1",
        [&source.id], |row| Ok((row.get(0)?,row.get(1)?)),
    ).optional().map_err(err)?;
    if current != Some((identity.key.clone(), source.generation)) {
        return Err("Semantic index source or model changed; retry indexing".into());
    }
    Ok(())
}

pub(super) fn decode(bytes: &[u8], dimensions: usize, checksum: &str) -> Result<Vec<f32>, String> {
    if bytes.len() != dimensions * 4 || format!("{:x}", Sha256::digest(bytes)) != checksum {
        return Err("Invalid semantic vector length/checksum".into());
    }
    let vector = bytes
        .chunks_exact(4)
        .map(|part| f32::from_le_bytes(part.try_into().unwrap()))
        .collect::<Vec<_>>();
    validate(&vector, dimensions)?;
    Ok(vector)
}

pub(super) fn validate(vector: &[f32], dimensions: usize) -> Result<(), String> {
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if vector.len() != dimensions
        || vector.iter().any(|value| !value.is_finite())
        || (norm - 1.0).abs() > 1e-4
    {
        return Err("Invalid semantic vector; expected finite unit-normalized values".into());
    }
    Ok(())
}

pub(crate) fn passage_input(title: &str, heading: Option<&str>, text: &str) -> String {
    format!("passage: {title}. {}. {text}", heading.unwrap_or(""))
}

/// Checkpoint one passage at a time, with no write lock held during native inference.
pub(super) fn synchronize(
    db: &mut Connection,
    identity: &Identity,
    source: &Source,
    mut ranges: impl FnMut(&str, Option<&str>, &str) -> Result<Vec<Range<usize>>, String>,
    mut encode: impl FnMut(&str) -> Result<Vec<f32>, String>,
    mut cancel: impl FnMut() -> Result<(), String>,
    mut progress: impl FnMut(bool),
) -> Result<(), String> {
    cancel()?;
    check_current(db, source, identity)?;
    if source.indexed {
        return Ok(());
    }
    let mut live = Vec::new();
    for section in &source.sections {
        cancel()?;
        if section.text.trim().is_empty() {
            continue;
        }
        let mut previous_start = None;
        let spans = ranges(&source.title, section.heading.as_deref(), &section.text)?;
        if spans.is_empty() {
            return Err("Readable semantic section produced no passages".into());
        }
        for span in spans {
            cancel()?;
            if span.start >= span.end
                || previous_start.is_some_and(|previous| span.start <= previous)
            {
                return Err("Invalid semantic passage order/span".into());
            }
            previous_start = Some(span.start);
            let body = section
                .text
                .get(span.clone())
                .ok_or("Invalid semantic UTF-8 source span")?;
            let input = passage_input(&source.title, section.heading.as_deref(), body);
            let hash = format!("{:x}", Sha256::digest(input.as_bytes()));
            let bytes: Option<(Vec<u8>, String)> = db.query_row(
                "SELECT vector,vector_hash FROM upload_semantic_passages WHERE document_id=?1 AND content_hash=?2 ORDER BY section_ordinal,byte_start LIMIT 1",
                params![source.id,hash], |row| Ok((row.get(0)?,row.get(1)?)),
            ).optional().map_err(err)?;
            let reused = bytes.is_some();
            let bytes: Vec<u8> = if let Some((bytes, checksum)) = bytes {
                decode(&bytes, identity.dimensions, &checksum)?;
                bytes
            } else {
                let vector = encode(&input)?;
                validate(&vector, identity.dimensions)?;
                vector
                    .iter()
                    .flat_map(|value| value.to_le_bytes())
                    .collect()
            };
            cancel()?;
            let tx = db
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(err)?;
            check_current(&tx, source, identity)?;
            tx.execute(
                "INSERT INTO upload_semantic_passages VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(document_id,section_ordinal,byte_start,byte_end) DO UPDATE SET content_hash=excluded.content_hash,vector=excluded.vector,vector_hash=excluded.vector_hash",
                params![source.id,section.ordinal,span.start,span.end,hash,bytes,format!("{:x}", Sha256::digest(&bytes))],
            ).map_err(err)?;
            tx.commit().map_err(err)?;
            live.push((section.ordinal, span.start, span.end));
            progress(reused);
        }
    }
    cancel()?;
    if live.is_empty() {
        return Err("Document has no readable semantic passages".into());
    }
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(err)?;
    check_current(&tx, source, identity)?;
    finish(&tx, source, &live)?;
    cancel()?;
    tx.commit().map_err(err)
}

/// Publish only after every span is checkpointed; prune obsolete spans in that same transaction.
fn finish(
    tx: &Transaction<'_>,
    source: &Source,
    live: &[(usize, usize, usize)],
) -> Result<(), String> {
    tx.execute_batch("CREATE TEMP TABLE IF NOT EXISTS semantic_live(section INTEGER,start INTEGER,end INTEGER,PRIMARY KEY(section,start,end)); DELETE FROM semantic_live;").map_err(err)?;
    for (section, start, end) in live {
        tx.execute(
            "INSERT INTO semantic_live VALUES(?1,?2,?3)",
            params![section, start, end],
        )
        .map_err(err)?;
    }
    tx.execute("DELETE FROM upload_semantic_passages WHERE document_id=?1 AND NOT EXISTS(SELECT 1 FROM semantic_live l WHERE l.section=section_ordinal AND l.start=byte_start AND l.end=byte_end)", [&source.id]).map_err(err)?;
    tx.execute(
        "UPDATE upload_semantic_documents SET ready=1 WHERE document_id=?1",
        [&source.id],
    )
    .map_err(err)?;
    Ok(())
}

/// Only fully synchronized, compatible documents count; partial checkpoints stay hidden.
pub(super) fn coverage(
    db: &mut Connection,
    identity: Option<&Identity>,
) -> Result<Coverage, String> {
    let tx = db.transaction().map_err(err)?;
    let db = &tx;
    let eligible_documents = document_ids(db)?.len();
    let mut coverage = Coverage {
        eligible_documents,
        ..Coverage::default()
    };
    if !exists(db)? {
        return Ok(coverage);
    }
    let (version, stored): (i64, String) = db
        .query_row(
            "SELECT schema_version,identity FROM upload_semantic_metadata WHERE id=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(err)?;
    if version != SCHEMA_VERSION || identity.is_none_or(|identity| identity.key != stored) {
        return Ok(coverage);
    }
    coverage.indexed_documents = db.query_row("SELECT count(*) FROM upload_semantic_documents i JOIN uploaded_documents d ON d.id=i.document_id WHERE i.ready=1 AND d.text_status<>'processing' AND EXISTS(SELECT 1 FROM uploaded_sections s WHERE s.document_id=d.id AND length(trim(s.text))>0)", [], |row| row.get(0)).map_err(err)?;
    coverage.passages = db.query_row("SELECT count(*) FROM upload_semantic_passages p JOIN upload_semantic_documents d ON d.document_id=p.document_id WHERE d.ready=1", [], |row| row.get(0)).map_err(err)?;
    Ok(coverage)
}

/// Retain source and lexical tables plus invalidation triggers for a later rebuild.
pub(super) fn remove(db: &mut Connection) -> Result<(), String> {
    if !exists(db)? {
        return Ok(());
    }
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(err)?;
    let version: i64 = tx
        .query_row(
            "SELECT schema_version FROM upload_semantic_metadata WHERE id=1",
            [],
            |row| row.get(0),
        )
        .map_err(err)?;
    if version != SCHEMA_VERSION {
        return Err("Unsupported semantic index schema".into());
    }
    tx.execute("DELETE FROM upload_semantic_passages", [])
        .map_err(err)?;
    tx.execute(
        "UPDATE upload_semantic_metadata SET generation=generation+1 WHERE id=1",
        [],
    )
    .map_err(err)?;
    tx.execute("UPDATE upload_semantic_documents SET ready=0,generation=(SELECT generation FROM upload_semantic_metadata WHERE id=1)", []).map_err(err)?;
    // Changing identity also rejects a stale writer from another connection.
    tx.execute(
        "UPDATE upload_semantic_metadata SET identity='' WHERE id=1",
        [],
    )
    .map_err(err)?;
    tx.commit().map_err(err)
}

#[cfg(test)]
pub(super) mod tests;
