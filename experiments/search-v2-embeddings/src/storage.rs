//! Opt-in experimental index; never opens the installed app database.

use super::{
    check_vectors, new_private_file, rank_vectors, source_evidence, Corpus, Document, Passage,
    RankedHit,
};
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{error::Error, path::Path};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;
const APPLICATION_ID: i64 = 0x50535632;

pub struct Store(Connection);

pub struct Snapshot {
    pub corpus: Corpus,
    pub chunks: Vec<Passage>,
    pub corpus_sha256: String,
    pub key: String,
}

#[derive(Default, Serialize)]
pub struct Changes {
    pub embedded: usize,
    pub reused: usize,
    pub deleted: usize,
}

fn decode(bytes: &[u8]) -> Result<Vec<f32>> {
    if bytes.len() != 384 * 4 {
        return Err("invalid stored vector length".into());
    }
    let vector = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .copied()
        .map(f32::from_le_bytes)
        .collect::<Vec<_>>();
    check_vectors(std::slice::from_ref(&vector))?;
    Ok(vector)
}

impl Store {
    pub fn open(path: &Path, model: &str) -> Result<Self> {
        let new = !path.try_exists()?;
        if new {
            drop(new_private_file(path)?);
        }
        let mut conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        conn.busy_timeout(std::time::Duration::from_secs(2))?;
        conn.execute_batch("PRAGMA cache_size=-2048;")?;
        if new {
            let tx = conn.transaction()?;
            tx.execute_batch(
                "CREATE TABLE model (identity TEXT NOT NULL, dimension INTEGER NOT NULL CHECK(dimension=384));
                 CREATE TABLE passages (
                    id TEXT PRIMARY KEY, ordinal INTEGER NOT NULL,
                    document_id TEXT NOT NULL, section_index INTEGER NOT NULL,
                    page_index INTEGER, byte_start INTEGER NOT NULL, byte_end INTEGER NOT NULL,
                    body_tokens INTEGER NOT NULL, content_hash TEXT NOT NULL,
                    vector BLOB NOT NULL CHECK(length(vector)=1536));
                 CREATE INDEX passage_order ON passages(ordinal);
                 PRAGMA user_version=1;"
            )?;
            tx.pragma_update(None, "application_id", APPLICATION_ID)?;
            tx.execute("INSERT INTO model VALUES (?1,384)", [model])?;
            tx.commit()?;
        }
        Self::check_schema(&conn, false)?;
        let store = Self(conn);
        store.check_model(model)?;
        Ok(store)
    }

    fn check_schema(conn: &Connection, query_only: bool) -> Result<()> {
        let app: i64 = conn.query_row("PRAGMA application_id", [], |row| row.get(0))?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if app != APPLICATION_ID || !matches!(version, 1 | 2) || (query_only && version != 2) {
            return Err("not a supported experimental source/vector index; synchronize a separate copy first".into());
        }
        Ok(())
    }

    pub fn read_only(path: &Path) -> Result<Self> {
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        conn.busy_timeout(std::time::Duration::from_secs(2))?;
        // Pin metadata, vectors and source spans to one committed snapshot during concurrent updates.
        conn.execute_batch("PRAGMA cache_size=-2048; PRAGMA query_only=ON; BEGIN;")?;
        Self::check_schema(&conn, true)?;
        Ok(Self(conn))
    }

    pub fn check_model(&self, model: &str) -> Result<()> {
        let identities: Vec<(String, usize)> = self
            .0
            .prepare("SELECT identity,dimension FROM model")?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<std::result::Result<_, _>>()?;
        if identities != [(model.to_owned(), 384)] {
            return Err("incompatible SQLite model; choose a new external index path".into());
        }
        Ok(())
    }

    pub fn snapshot(&self) -> Result<Snapshot> {
        // ponytail: retain O(passages) metadata for the shared reducer; stream metadata too if measured memory requires it.
        let json: String = self.0.query_row(
            "SELECT metadata FROM source_snapshot WHERE id=1",
            [],
            |row| row.get(0),
        )?;
        let value: serde_json::Value = serde_json::from_str(&json)?;
        let corpus: Corpus = serde_json::from_value(value["corpus"].clone())?;
        let chunks: Vec<Passage> = serde_json::from_value(value["chunks"].clone())?;
        if corpus
            .documents
            .iter()
            .any(|doc| doc.sections.iter().any(|section| !section.text.is_empty()))
            || chunks.iter().any(|chunk| {
                !chunk.text.is_empty()
                    || chunk.end <= chunk.start
                    || corpus
                        .documents
                        .get(chunk.document)
                        .and_then(|doc| doc.sections.get(chunk.section))
                        .is_none()
            })
        {
            return Err("invalid stored source metadata".into());
        }
        let hash = |field: &str| -> Result<String> {
            let text = value[field].as_str().ok_or("missing snapshot hash")?;
            if text.len() != 64
                || !text
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            {
                return Err("invalid snapshot hash".into());
            }
            Ok(text.to_owned())
        };
        Ok(Snapshot {
            corpus,
            chunks,
            corpus_sha256: hash("corpusSha256")?,
            key: hash("key")?,
        })
    }

    pub fn source_hit(
        &self,
        corpus: &Corpus,
        chunks: &[Passage],
        ordinal: usize,
        score: f32,
    ) -> Result<serde_json::Value> {
        let chunk = chunks.get(ordinal).ok_or("missing source ordinal")?;
        let document = &corpus.documents[chunk.document];
        let section = &document.sections[chunk.section];
        let (id, section_index, page, start, end, hash, text): (String, usize, Option<usize>, usize, usize, String, String) = self.0.query_row(
            "SELECT p.document_id,p.section_index,p.page_index,p.byte_start,p.byte_end,p.content_hash,
             s.text
             FROM passages p JOIN sources s ON s.id=p.id
             WHERE p.ordinal=?1", [ordinal], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?)))?;
        let embedded_text = format!(
            "passage: {}. {}. {}",
            document.title,
            section.heading.as_deref().unwrap_or(""),
            text
        );
        if id != document.id
            || section_index != chunk.section
            || page != section.page_index
            || start != chunk.start
            || end != chunk.end
            || text.len() != end - start
            || hash != format!("{:x}", Sha256::digest(embedded_text.as_bytes()))
        {
            return Err("winning evidence does not match indexed source".into());
        }
        Ok(source_evidence(document, section, chunk, &text, score))
    }

    pub fn sync(
        &mut self,
        corpus: &Corpus,
        chunks: &[Passage],
        batch_size: usize,
        mut embed: impl FnMut(&[Passage]) -> Result<Vec<Vec<f32>>>,
        mut cancel: impl FnMut() -> Result<()>,
    ) -> Result<Changes> {
        if !(1..=16).contains(&batch_size) {
            return Err("invalid indexing batch size".into());
        }
        cancel()?;
        // ponytail: one snapshot transaction/one writer; per-document jobs only if indexing lock time matters.
        let tx = self
            .0
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        tx.execute_batch(
            "CREATE TEMP TABLE IF NOT EXISTS live (id TEXT PRIMARY KEY); DELETE FROM live;
             CREATE TABLE IF NOT EXISTS source_snapshot (id INTEGER PRIMARY KEY CHECK(id=1),metadata TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS sources (id TEXT PRIMARY KEY,text TEXT NOT NULL);",
        )?;
        let mut changes = Changes::default();
        for (batch_index, batch) in chunks.chunks(batch_size).enumerate() {
            cancel()?;
            let mut missing = Vec::new();
            let mut pending = Vec::new();
            for (offset, chunk) in batch.iter().enumerate() {
                let document = &corpus.documents[chunk.document];
                // Position-independent document identity; page/heading/source edits update their actual locators.
                let id = format!(
                    "{:x}",
                    Sha256::digest(serde_json::to_vec(&(
                        &document.id,
                        chunk.section,
                        chunk.start,
                        chunk.end
                    ))?)
                );
                let hash = format!("{:x}", Sha256::digest(chunk.text.as_bytes()));
                tx.execute("INSERT INTO live VALUES (?1)", [&id])?;
                let source = document.sections[chunk.section]
                    .text
                    .get(chunk.start..chunk.end)
                    .ok_or("invalid indexing source span")?;
                tx.execute("INSERT INTO sources VALUES (?1,?2) ON CONFLICT(id) DO UPDATE SET text=excluded.text WHERE sources.text IS NOT excluded.text",params![id,source])?;
                let stored: Option<(String, Vec<u8>)> = tx
                    .query_row(
                        "SELECT content_hash,vector FROM passages WHERE id=?1",
                        [&id],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .optional()?;
                let ordinal = batch_index * batch_size + offset;
                let vector = match stored {
                    Some((old_hash, bytes)) if old_hash == hash => {
                        decode(&bytes)?;
                        changes.reused += 1;
                        Some(bytes)
                    }
                    _ => None,
                };
                if let Some(bytes) = vector {
                    Self::put(&tx, document, chunk, ordinal, &id, &hash, &bytes)?;
                } else {
                    missing.push(chunk.clone());
                    pending.push((ordinal, id, hash));
                }
            }
            if !missing.is_empty() {
                cancel()?;
                let vectors = embed(&missing)?;
                if vectors.len() != missing.len() {
                    return Err("wrong SQLite indexing vector count".into());
                }
                check_vectors(&vectors)?;
                for ((chunk, (ordinal, id, hash)), vector) in
                    missing.iter().zip(pending).zip(vectors)
                {
                    let doc = &corpus.documents[chunk.document];
                    let bytes = vector
                        .iter()
                        .flat_map(|value| value.to_le_bytes())
                        .collect::<Vec<_>>();
                    Self::put(&tx, doc, chunk, ordinal, &id, &hash, &bytes)?;
                    changes.embedded += 1;
                }
            }
        }
        changes.deleted = tx.execute(
            "DELETE FROM passages WHERE id NOT IN (SELECT id FROM live)",
            [],
        )?;
        cancel()?;
        tx.execute(
            "DELETE FROM sources WHERE id NOT IN (SELECT id FROM live)",
            [],
        )?;
        let documents = corpus.documents.iter().map(|doc| serde_json::json!({"id":doc.id,"title":doc.title,"url":doc.url,"sections":doc.sections.iter().map(|part| serde_json::json!({"heading":part.heading,"page_index":part.page_index,"text":""})).collect::<Vec<_>>()})).collect::<Vec<_>>();
        let metadata_chunks = chunks.iter().map(|part| serde_json::json!({"document":part.document,"section":part.section,"start":part.start,"end":part.end,"text":"","body_tokens":part.body_tokens})).collect::<Vec<_>>();
        let model: String = tx.query_row("SELECT identity FROM model", [], |row| row.get(0))?;
        let mut key = Sha256::new();
        key.update(model.as_bytes());
        key.update(serde_json::to_vec(chunks)?);
        let metadata = serde_json::to_string(
            &serde_json::json!({"corpusSha256":corpus.corpus_sha256,"key":format!("{:x}",key.finalize()),"corpus":{"documents":documents,"queries":[],"snapshotSha256":corpus.snapshot_sha256},"chunks":metadata_chunks}),
        )?;
        tx.execute("INSERT INTO source_snapshot VALUES (1,?1) ON CONFLICT(id) DO UPDATE SET metadata=excluded.metadata WHERE source_snapshot.metadata IS NOT excluded.metadata",[metadata])?;
        if tx.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))? != 2 {
            tx.execute_batch("PRAGMA user_version=2;")?;
        }
        cancel()?;
        tx.commit()?;
        Ok(changes)
    }

    fn put(
        tx: &rusqlite::Transaction<'_>,
        document: &Document,
        chunk: &Passage,
        ordinal: usize,
        id: &str,
        hash: &str,
        bytes: &[u8],
    ) -> Result<()> {
        tx.execute(
            "INSERT INTO passages VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)
             ON CONFLICT(id) DO UPDATE SET ordinal=excluded.ordinal,document_id=excluded.document_id,
               section_index=excluded.section_index,page_index=excluded.page_index,byte_start=excluded.byte_start,
               byte_end=excluded.byte_end,body_tokens=excluded.body_tokens,content_hash=excluded.content_hash,vector=excluded.vector
             WHERE (passages.ordinal,passages.document_id,passages.section_index,passages.page_index,
                    passages.byte_start,passages.byte_end,passages.body_tokens,passages.content_hash,passages.vector)
               IS NOT (excluded.ordinal,excluded.document_id,excluded.section_index,excluded.page_index,
                    excluded.byte_start,excluded.byte_end,excluded.body_tokens,excluded.content_hash,excluded.vector)",
            params![id,ordinal,document.id,chunk.section,document.sections[chunk.section].page_index,chunk.start,chunk.end,chunk.body_tokens,hash,bytes],
        )?;
        Ok(())
    }

    pub fn rank(
        &self,
        corpus: &Corpus,
        chunks: &[Passage],
        query: &[f32],
        scope: &[String],
        cancel: impl FnMut() -> Result<()>,
    ) -> Result<Vec<RankedHit>> {
        let count: usize = self
            .0
            .query_row("SELECT count(*) FROM passages", [], |row| row.get(0))?;
        if count != chunks.len() {
            return Err("SQLite index has not been synchronized with this snapshot".into());
        }
        let mut statement = self
            .0
            .prepare("SELECT ordinal,document_id,section_index,page_index,byte_start,byte_end,body_tokens,vector FROM passages ORDER BY ordinal")?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, usize>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, usize>(2)?,
                row.get::<_, Option<usize>>(3)?,
                row.get::<_, usize>(4)?,
                row.get::<_, usize>(5)?,
                row.get::<_, usize>(6)?,
                row.get::<_, Vec<u8>>(7)?,
            ))
        })?;
        let vectors = rows.enumerate().map(|(expected, row)| {
            let (ordinal, id, section, page, start, end, body_tokens, bytes) = row?;
            let chunk = chunks.get(expected).ok_or("invalid SQLite passage order")?;
            let doc = &corpus.documents[chunk.document];
            if ordinal != expected
                || id != doc.id
                || section != chunk.section
                || page != doc.sections[section].page_index
                || start != chunk.start
                || end != chunk.end
                || body_tokens != chunk.body_tokens
            {
                return Err("invalid SQLite passage metadata".into());
            }
            Ok((ordinal, decode(&bytes)?))
        });
        rank_vectors(corpus, chunks, vectors, query, scope, cancel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_snapshot_keeps_utf8_evidence_and_does_not_read_other_sources() {
        assert!(
            serde_json::from_value::<Corpus>(serde_json::json!({"queries":[]})).is_err(),
            "indexing must require an explicit document snapshot"
        );
        assert!(serde_json::from_value::<Corpus>(serde_json::json!({"documents":[{"id":"a","title":"Guide","sections":[{}]}],"queries":[]})).is_err(), "missing authored text must not silently become empty");
        assert!(
            serde_json::from_value::<super::super::QueryInput>(
                serde_json::json!({"corpusSha256":"c".repeat(64),"queries":[],"documents":[]})
            )
            .is_err(),
            "query-only input must not admit source documents"
        );
        let path = std::env::temp_dir().join(format!(
            "papercut-source-check-{}-{}.sqlite3",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        assert!(!path.exists());
        let mut corpus: Corpus = serde_json::from_value(serde_json::json!({"documents":[
            {"id":"a","title":"Navigation","url":"/uploads/a.pdf","sections":[
                {"text":"Index"},
                {"heading":"Direction","text":"Intro: 😀 compass <north> اتجاه tail","page_index":3}]},
            {"id":"b","title":"Archive","sections":[{"text":"Storage shelves."}]}
        ],"queries":[],"snapshotSha256":"snapshot"})).unwrap();
        corpus.corpus_sha256 = Some("c".repeat(64));
        let chunks = corpus
            .documents
            .iter()
            .enumerate()
            .flat_map(|(document, doc)| {
                doc.sections.iter().enumerate().map(move |(section, part)| {
                    let start = if section == 1 { "Intro: ".len() } else { 0 };
                    let end = if section == 1 {
                        part.text.len() - " tail".len()
                    } else {
                        part.text.len()
                    };
                    Passage {
                        document,
                        section,
                        start,
                        end,
                        text: format!(
                            "passage: {}. {}. {}",
                            doc.title,
                            part.heading.as_deref().unwrap_or(""),
                            &part.text[start..end]
                        ),
                        body_tokens: if section == 1 { 32 } else { 1 },
                    }
                })
            })
            .collect::<Vec<_>>();
        let vectors = (0..3)
            .map(|index| {
                let mut vector = vec![0.0; 384];
                if index == 1 {
                    vector[0] = 0.8;
                    vector[1] = 0.6;
                } else {
                    vector[usize::from(index == 2)] = 1.0;
                }
                vector
            })
            .collect::<Vec<_>>();
        let mut store = Store::open(&path, "model").unwrap();
        assert!(
            Store::read_only(&path).is_err(),
            "unsynchronized v1 index cannot serve sources"
        );
        let stats = store
            .sync(&corpus, &chunks, 16, |_| Ok(vectors.clone()), || Ok(()))
            .unwrap();
        assert_eq!(stats.embedded, 3);
        let before = std::fs::read(&path).unwrap();
        let stats = store
            .sync(
                &corpus,
                &chunks,
                16,
                |_| panic!("no repeated inference"),
                || Ok(()),
            )
            .unwrap();
        assert_eq!(stats.reused, 3);
        assert!(
            std::fs::read(&path).unwrap() == before,
            "unchanged restart must not rewrite sources or metadata"
        );
        corpus.documents[0].sections[1].page_index = Some(9);
        let mut steps = 0;
        assert!(store
            .sync(
                &corpus,
                &chunks,
                16,
                |_| panic!("locator-only update must reuse"),
                || {
                    steps += 1;
                    if steps == 4 {
                        Err("cancel before source snapshot publication".into())
                    } else {
                        Ok(())
                    }
                }
            )
            .is_err());
        assert!(
            std::fs::read(&path).unwrap() == before,
            "source/metadata publication must roll back with vectors"
        );
        corpus.documents[0].sections[1].page_index = Some(3);
        let mut query = vec![0.0; 384];
        query[0] = 1.0;
        let expected = super::super::source_hit(&corpus, &chunks[1], 0.8);
        let baseline = super::super::rank(&corpus, &chunks, &vectors, &query, &[]);
        drop(store);
        let writer = Connection::open(&path).unwrap();
        writer.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
        // Deliberately broken nonwinning source is neither loaded nor used to construct winning evidence.
        writer.execute("UPDATE sources SET text='broken unrelated text' WHERE id=(SELECT id FROM passages WHERE ordinal=2)",[]).unwrap();
        let reader = Store::read_only(&path).unwrap();
        reader.check_model("model").unwrap();
        assert!(reader.check_model("other").is_err());
        let snapshot = reader.snapshot().unwrap();
        assert!(snapshot
            .corpus
            .documents
            .iter()
            .all(|doc| doc.sections.iter().all(|part| part.text.is_empty())));
        assert!(snapshot.chunks.iter().all(|part| part.text.is_empty()));
        let disk = reader
            .rank(&snapshot.corpus, &snapshot.chunks, &query, &[], || Ok(()))
            .unwrap();
        assert_eq!(
            disk.iter()
                .map(|hit| (hit.passage, hit.score, hit.evidence))
                .collect::<Vec<_>>(),
            baseline
                .iter()
                .map(|hit| (hit.passage, hit.score, hit.evidence))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            reader
                .source_hit(
                    &snapshot.corpus,
                    &snapshot.chunks,
                    disk[0].evidence.0,
                    disk[0].evidence.1
                )
                .unwrap(),
            expected
        );
        let combined = super::super::semantic_evidence(&disk[0], |index, score| {
            reader.source_hit(&snapshot.corpus, &snapshot.chunks, index, score)
        })
        .unwrap();
        assert_eq!(combined["passages"][0]["excerpt"], expected["excerpt"]);
        assert_eq!(combined["passages"][0]["pageIndex"], 3);
        assert_eq!(combined["passages"][1]["excerpt"], "Index");
        assert_eq!(combined["passages"][1]["cosine"], 1.0);
        assert!(reader
            .source_hit(&snapshot.corpus, &snapshot.chunks, 2, 0.0)
            .is_err());
        assert!(
            reader.0.execute("DELETE FROM passages", []).is_err(),
            "query-only connection forbids writes"
        );
        writer.execute("UPDATE sources SET text='changed winning text' WHERE id=(SELECT id FROM passages WHERE ordinal=1)",[]).unwrap();
        assert_eq!(
            reader
                .source_hit(&snapshot.corpus, &snapshot.chunks, 1, 0.8)
                .unwrap(),
            expected,
            "reader remains on its original committed snapshot"
        );
        drop(reader);
        let reader = Store::read_only(&path).unwrap();
        assert!(
            reader
                .source_hit(&snapshot.corpus, &snapshot.chunks, 1, 0.8)
                .is_err(),
            "new snapshot detects altered source hash"
        );
        drop(reader);
        writer
            .execute("UPDATE passages SET body_tokens=999 WHERE ordinal=0", [])
            .unwrap();
        let reader = Store::read_only(&path).unwrap();
        assert!(
            reader
                .rank(&snapshot.corpus, &snapshot.chunks, &query, &[], || Ok(()))
                .is_err(),
            "scan metadata must match the bound snapshot"
        );
        drop(reader);
        drop(writer);
        for suffix in ["", "-wal", "-shm"] {
            let file = std::path::PathBuf::from(format!("{}{suffix}", path.display()));
            if file.exists() {
                std::fs::remove_file(file).unwrap();
            }
        }
    }

    #[test]
    fn sqlite_restart_changes_rollback_and_streamed_ranking() {
        assert!(decode(&[0; 4]).is_err());
        assert!(decode(&[0; 1536]).is_err());
        let path = std::env::temp_dir().join(format!(
            "papercut-sqlite-check-{}.sqlite3",
            std::process::id()
        ));
        assert!(!path.exists());
        let mut corpus: Corpus = serde_json::from_value(serde_json::json!({"documents":[
            {"id":"a","title":"Navigation","sections":[{"text":"Compass source.","page_index":2}]},
            {"id":"b","title":"Storage","sections":[{"text":"Archive source."}]}
        ],"queries":[]}))
        .unwrap();
        let make = |corpus: &Corpus| {
            corpus
                .documents
                .iter()
                .enumerate()
                .map(|(document, doc)| Passage {
                    document,
                    section: 0,
                    start: 0,
                    end: doc.sections[0].text.len(),
                    text: doc.sections[0].text.clone(),
                    body_tokens: 32,
                })
                .collect::<Vec<_>>()
        };
        let mut chunks = make(&corpus);
        let embed = |batch: &[Passage]| {
            Ok(batch
                .iter()
                .map(|p| {
                    let mut v = vec![0.0; 384];
                    v[usize::from(p.text.starts_with("Archive"))] = 1.0;
                    v
                })
                .collect::<Vec<_>>())
        };
        let mut store = Store::open(&path, "test-model-and-policy").unwrap();
        let stats = store.sync(&corpus, &chunks, 16, embed, || Ok(())).unwrap();
        assert_eq!((stats.embedded, stats.reused, stats.deleted), (2, 0, 0));
        let mut q = vec![0.0; 384];
        q[0] = 1.0;
        let baseline = store.rank(&corpus, &chunks, &q, &[], || Ok(())).unwrap();
        let expected = super::super::rank(&corpus, &chunks, &embed(&chunks).unwrap(), &q, &[]);
        assert_eq!(
            baseline
                .iter()
                .map(|h| (h.passage, h.score, h.evidence))
                .collect::<Vec<_>>(),
            expected
                .iter()
                .map(|h| (h.passage, h.score, h.evidence))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            store
                .rank(&corpus, &chunks, &q, &["b".into()], || Ok(()))
                .unwrap()[0]
                .passage,
            1
        );
        drop(store);
        assert!(Store::open(&path, "different-model-or-policy").is_err());
        let mut store = Store::open(&path, "test-model-and-policy").unwrap();
        let stats = store
            .sync(
                &corpus,
                &chunks,
                1,
                |_| panic!("unchanged source must reuse"),
                || Ok(()),
            )
            .unwrap();
        assert_eq!((stats.embedded, stats.reused, stats.deleted), (0, 2, 0));

        // Reorder documents and change only a page locator: stable IDs/content reuse both vectors.
        corpus.documents.reverse();
        corpus.documents[1].sections[0].page_index = Some(5);
        chunks = make(&corpus);
        let stats = store
            .sync(
                &corpus,
                &chunks,
                16,
                |_| panic!("locator-only change must reuse"),
                || Ok(()),
            )
            .unwrap();
        assert_eq!(stats.reused, 2);
        let hit = store.rank(&corpus, &chunks, &q, &[], || Ok(())).unwrap();
        assert_eq!(
            super::super::source_hit(&corpus, &chunks[hit[0].evidence.0], hit[0].evidence.1)
                ["pageIndex"],
            5
        );
        let saved: Vec<(String, Vec<u8>)> = store
            .0
            .prepare("SELECT id,vector FROM passages ORDER BY id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        corpus.documents[0].sections[0].text = "Updated archive source.".into();
        chunks = make(&corpus);
        assert!(store
            .sync(
                &corpus,
                &chunks,
                1,
                |_| Err("inference failed".into()),
                || Ok(())
            )
            .is_err());
        let mut calls = 0;
        assert!(store
            .sync(&corpus, &chunks, 1, embed, || {
                calls += 1;
                if calls >= 5 {
                    Err("cancelled".into())
                } else {
                    Ok(())
                }
            })
            .is_err());
        let after: Vec<(String, Vec<u8>)> = store
            .0
            .prepare("SELECT id,vector FROM passages ORDER BY id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        assert_eq!(
            after, saved,
            "failed/cancelled snapshots must not partially commit"
        );
        let stats = store.sync(&corpus, &chunks, 1, embed, || Ok(())).unwrap();
        assert_eq!((stats.embedded, stats.reused, stats.deleted), (1, 1, 1));
        corpus.documents.remove(0);
        chunks = make(&corpus);
        let stats = store.sync(&corpus, &chunks, 1, embed, || Ok(())).unwrap();
        assert_eq!((stats.embedded, stats.reused, stats.deleted), (0, 1, 1));
        let mut steps = 0;
        assert!(store
            .rank(&corpus, &chunks, &q, &[], || {
                steps += 1;
                if steps > 1 {
                    Err("cancelled".into())
                } else {
                    Ok(())
                }
            })
            .is_err());
        let mut corrupt = vec![0u8; 1536];
        corrupt[..4].copy_from_slice(&f32::NAN.to_le_bytes());
        store
            .0
            .execute("UPDATE passages SET vector=?1", [corrupt])
            .unwrap();
        assert!(store.rank(&corpus, &chunks, &q, &[], || Ok(())).is_err());
        assert!(store.sync(&corpus, &chunks, 1, embed, || Ok(())).is_err());
        let empty: Corpus =
            serde_json::from_value(serde_json::json!({"documents":[],"queries":[]})).unwrap();
        let stats = store
            .sync(
                &empty,
                &[],
                1,
                |_| panic!("empty snapshot must not embed"),
                || Ok(()),
            )
            .unwrap();
        assert_eq!(stats.deleted, 1);
        assert!(store
            .rank(&empty, &[], &q, &[], || Ok(()))
            .unwrap()
            .is_empty());
        drop(store);
        std::fs::remove_file(&path).unwrap();
        std::fs::write(&path, b"unrelated file").unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(Store::open(&path, "test").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        std::fs::remove_file(path).unwrap();
    }
}
