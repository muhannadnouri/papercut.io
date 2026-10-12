//! Independent conceptual retrieval. Never changes literal modes or invents occurrence evidence.

use crate::document_uploads::search::query::validate_search_query;
#[cfg(any(test, feature = "experimental-search-embeddings-core"))]
use crate::document_uploads::search::query::{fts_phrase_queries, fts_terms, normalize_exact_text};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QueryRequest {
    query: String,
    #[serde(default)]
    exact_phrases: Vec<String>,
    // None means all uploads; an explicitly empty scope means no uploads.
    document_urls: Option<Vec<String>>,
    limit: Option<usize>,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "kebab-case")]
#[cfg_attr(
    not(any(test, feature = "experimental-search-embeddings-core")),
    allow(dead_code)
)]
pub(crate) enum Availability {
    #[default]
    MissingIndex,
    IncompatibleIndex,
    Partial,
    Ready,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QueryResponse {
    availability: Availability,
    results: Vec<ConceptualResult>,
    indexed_documents: usize,
    eligible_documents: usize,
    scanned_passages: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConceptualResult {
    document_id: String,
    url: String,
    title: String,
    format: String,
    score: f64,
    provenance: &'static str,
    section_index: usize,
    page_index: Option<usize>,
    section_title: Option<String>,
    byte_start: usize,
    byte_end: usize,
    text: String,
    context: Vec<SourceContext>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceContext {
    section_index: usize,
    page_index: Option<usize>,
    section_title: Option<String>,
    byte_start: usize,
    byte_end: usize,
    text: String,
}

/// Read nearby body spans in the same snapshot; they are context, never scored or embedded together.
#[cfg(any(test, feature = "experimental-search-embeddings-core"))]
fn nearby_context(
    tx: &rusqlite::Transaction<'_>,
    candidate: &Candidate,
    mut check: impl FnMut() -> Result<(), String>,
) -> Result<Vec<SourceContext>, String> {
    if !candidate.main_content {
        return Ok(Vec::new());
    }
    let mut context = Vec::new();
    // shortcut: four source blocks in each direction, not inferred chapters; revisit with judged context failures.
    for before in [true, false] {
        let order = if before { "DESC" } else { "ASC" };
        let comparison = if before { "<=" } else { ">=" };
        let mut stmt = tx
            .prepare(&format!(
                "SELECT ordinal,heading,text,page_index,body_ranges FROM uploaded_sections \
             WHERE document_id=?1 AND ordinal {comparison} ?2 ORDER BY ordinal {order} LIMIT 5"
            ))
            .map_err(db_err)?;
        let mut rows = stmt
            .query(params![candidate.id, candidate.section])
            .map_err(db_err)?;
        let mut remaining = 4096;
        let mut spans = 0;
        while let Some(row) = rows.next().map_err(db_err)? {
            check()?;
            if remaining == 0 || spans == 5 {
                break;
            }
            let ordinal: usize = row.get(0).map_err(db_err)?;
            let heading: Option<String> = row.get(1).map_err(db_err)?;
            let text: String = row.get(2).map_err(db_err)?;
            let page: Option<usize> = row.get(3).map_err(db_err)?;
            let encoded: Option<String> = row.get(4).map_err(db_err)?;
            let mut ranges = super::super::search_content::ranges(&text, encoded.as_deref())?;
            // An auxiliary-only block is a boundary, not a bridge into another part of the book.
            if ranges.is_empty() {
                break;
            }
            if ordinal == candidate.section {
                ranges.retain(|range| range.start <= candidate.start && candidate.end <= range.end);
                for range in &mut ranges {
                    if before {
                        range.end = candidate.start;
                    } else {
                        range.start = candidate.end;
                    }
                }
            }
            if before {
                ranges.reverse();
            }
            for mut range in ranges {
                check()?;
                if remaining == 0 || spans == 5 {
                    break;
                }
                if range.is_empty() {
                    continue;
                }
                let mut chars = text[range.clone()].chars();
                let mut count = 0;
                let mut bytes = 0;
                for _ in 0..2048.min(remaining) {
                    let Some(ch) = (if before {
                        chars.next_back()
                    } else {
                        chars.next()
                    }) else {
                        break;
                    };
                    count += 1;
                    bytes += ch.len_utf8();
                }
                if before {
                    range.start = range.end - bytes;
                } else {
                    range.end = range.start + bytes;
                }
                context.push(SourceContext {
                    section_index: ordinal,
                    page_index: page,
                    section_title: heading.clone(),
                    byte_start: range.start,
                    byte_end: range.end,
                    text: text[range].into(),
                });
                remaining -= count;
                spans += 1;
            }
        }
    }
    context.sort_by_key(|span| (span.section_index, span.byte_start));
    Ok(context)
}

impl QueryRequest {
    fn validate(&self) -> Result<(), String> {
        validate_search_query(&self.query, &self.exact_phrases)?;
        if self.limit.is_some_and(|limit| !(1..=50).contains(&limit)) {
            return Err("Conceptual result limit must be between 1 and 50".into());
        }
        if self.query.trim().is_empty()
            && self
                .exact_phrases
                .iter()
                .all(|phrase| phrase.trim().is_empty())
        {
            return Err("Conceptual search requires a nonempty query".into());
        }
        // Bound IPC scope memory without silently searching a shortened selection.
        if self.document_urls.as_ref().is_some_and(|urls| {
            urls.len() > 10_000
                || urls
                    .iter()
                    .any(|url| url.len() > 2048 || url.trim().is_empty())
        }) {
            return Err("Invalid or oversized conceptual document scope".into());
        }
        Ok(())
    }
}

/// Explicit cached-provider query only: no downloads, activation, schema creation or index writes.
#[tauri::command]
pub(crate) async fn search_semantic_query<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    state: tauri::State<'_, super::SemanticIndexState>,
    provider: tauri::State<'_, crate::search_embeddings::SearchEmbeddingState>,
    request: QueryRequest,
) -> Result<QueryResponse, String> {
    request.validate()?;
    #[cfg(not(feature = "experimental-search-embeddings-core"))]
    {
        let _ = (app, state, provider);
        Err("Experimental embeddings are not compiled in this build".into())
    }
    #[cfg(feature = "experimental-search-embeddings-core")]
    {
        let guard = state.begin_query()?;
        let provider = provider.inner().clone();
        let version = provider.index_generation();
        let start = std::time::Instant::now();
        tauri::async_runtime::spawn_blocking(move || {
            let _guard = guard;
            if provider.index_generation() != version {
                return Err("Semantic model disabled; retry query".into());
            }
            let (encoder, dimensions) = provider.index_encoder_identity()?;
            let identity = super::store::Identity::new(&encoder, dimensions)?;
            let path = crate::document_uploads::storage::uploads_root(&app)?.join("search.sqlite3");
            if !path.exists() {
                return Ok(QueryResponse::default());
            }
            let mut db = rusqlite::Connection::open_with_flags(
                path,
                rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
            )
            .map_err(crate::document_uploads::store::db_err)?;
            db.busy_timeout(std::time::Duration::from_secs(5))
                .map_err(crate::document_uploads::store::db_err)?;
            let response = retrieve(
                &mut db,
                &identity,
                &request,
                |input| provider.index_encode(&encoder, input),
                || {
                    if provider.index_generation() != version {
                        return Err("Semantic model disabled; retry query".into());
                    }
                    // shortcut: exact streaming scan has a ten-second budget; benchmark an ANN index only if measured library sizes need it.
                    if start.elapsed() > std::time::Duration::from_secs(10) {
                        return Err(
                            "Conceptual query timed out; lexical search remains available".into(),
                        );
                    }
                    Ok(())
                },
            )?;
            if provider.index_generation() != version
                || provider.index_encoder_identity()?.0 != encoder
            {
                return Err("Semantic model changed; retry query".into());
            }
            Ok(response)
        })
        .await
        .map_err(|err| format!("Conceptual query task failed: {err}"))?
    }
}

#[cfg(any(test, feature = "experimental-search-embeddings-core"))]
use crate::document_uploads::store::db_err;
#[cfg(any(test, feature = "experimental-search-embeddings-core"))]
use rusqlite::{params, Connection};

#[cfg(any(test, feature = "experimental-search-embeddings-core"))]
fn metadata(db: &Connection) -> Result<(i64, String, i64), String> {
    db.query_row(
        "SELECT schema_version,identity,generation FROM upload_semantic_metadata WHERE id=1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )
    .map_err(db_err)
}

#[cfg(any(test, feature = "experimental-search-embeddings-core"))]
struct Candidate {
    id: String,
    url: String,
    title: String,
    format: String,
    score: f64,
    section: usize,
    start: usize,
    end: usize,
    hash: String,
    main_content: bool,
}

/// Scope and authored quotes constrain the whole scan before top-k selection, avoiding starvation.
/// Keep one best passage per document, one vector at a time, and at most 50 document candidates.
#[cfg(any(test, feature = "experimental-search-embeddings-core"))]
pub(super) fn retrieve(
    db: &mut Connection,
    identity: &super::store::Identity,
    request: &QueryRequest,
    mut encode: impl FnMut(&str) -> Result<Vec<f32>, String>,
    mut check: impl FnMut() -> Result<(), String>,
) -> Result<QueryResponse, String> {
    use super::store;
    use sha2::{Digest, Sha256};
    request.validate()?;
    check()?;
    let tx = db.transaction().map_err(db_err)?;
    let mut response = QueryResponse::default();
    if !store::exists(&tx)? {
        return Ok(response);
    }
    let original = metadata(&tx)?;
    if original.0 != store::SCHEMA_VERSION || original.1 != identity.key {
        response.availability = Availability::IncompatibleIndex;
        return Ok(response);
    }
    let scope = request
        .document_urls
        .as_ref()
        .map(|urls| urls.iter().collect::<std::collections::HashSet<_>>());
    let phrases = request
        .exact_phrases
        .iter()
        .filter(|phrase| !fts_terms(phrase, 128).is_empty())
        .map(|phrase| normalize_exact_text(phrase))
        .collect::<Vec<_>>();
    // Preserve authored hyphens for the existing FTS aliases; normalize only the source check.
    let phrase_queries = fts_phrase_queries(&request.exact_phrases);
    let limit = request.limit.unwrap_or(50);
    let input = format!(
        "query: {} {}",
        request.query.trim(),
        request.exact_phrases.join(" ")
    );
    let mut vector = None;
    let mut candidates: Vec<Candidate> = Vec::new();
    {
        // Coverage is scoped library readiness, not the number of query/phrase matches.
        // Uncorrelated IN subqueries reuse common-phrase postings instead of rescanning them per document.
        let phrase_predicate = if phrase_queries.is_empty() {
            "1".to_owned()
        } else {
            vec!["d.id IN(SELECT document_id FROM uploaded_document_fts WHERE uploaded_document_fts MATCH ?)"; phrase_queries.len()].join(" AND ")
        };
        let mut documents = tx.prepare(&format!("SELECT d.id,d.url,d.title,d.format,i.ready,({phrase_predicate}) FROM uploaded_documents d
            LEFT JOIN upload_semantic_documents i ON i.document_id=d.id WHERE d.text_status<>'processing'
            AND EXISTS(SELECT 1 FROM uploaded_sections s WHERE s.document_id=d.id AND length(trim(s.text))>0) ORDER BY d.id")).map_err(db_err)?;
        let mut rows = documents
            .query(rusqlite::params_from_iter(
                phrase_queries
                    .iter()
                    .map(|phrase| format!("{{text}} : ({phrase})")),
            ))
            .map_err(db_err)?;
        while let Some(row) = rows.next().map_err(db_err)? {
            check()?;
            let id: String = row.get(0).map_err(db_err)?;
            let url: String = row.get(1).map_err(db_err)?;
            if scope.as_ref().is_some_and(|scope| !scope.contains(&url)) {
                continue;
            }
            response.eligible_documents += 1;
            if !row
                .get::<_, Option<bool>>(4)
                .map_err(db_err)?
                .unwrap_or(false)
            {
                continue;
            }
            response.indexed_documents += 1;
            // FTS body-token boundaries reject substrings/title-only hits; authored verification rejects stems.
            let phrase_candidate: bool = row.get(5).map_err(db_err)?;
            if !phrase_candidate
                || (!phrases.is_empty()
                    && !crate::document_uploads::search::document_contains_exact_phrases(
                        &tx, &id, &phrases,
                    )?)
            {
                continue;
            }
            if vector.is_none() {
                let encoded = encode(&input)?;
                store::validate(&encoded, identity.dimensions)?;
                vector = Some(encoded);
            }
            let title: String = row.get(2).map_err(db_err)?;
            let format: String = row.get(3).map_err(db_err)?;
            let mut passages = tx.prepare("SELECT p.section_ordinal,p.byte_start,p.byte_end,p.content_hash,p.vector,p.vector_hash,s.text,s.body_ranges
                FROM upload_semantic_passages p JOIN uploaded_sections s ON s.document_id=p.document_id AND s.ordinal=p.section_ordinal
                WHERE p.document_id=?1 ORDER BY p.section_ordinal,p.byte_start,p.byte_end").map_err(db_err)?;
            let mut passages = passages.query([&id]).map_err(db_err)?;
            let mut best = None;
            let mut section_ranges = None;
            while let Some(passage) = passages.next().map_err(db_err)? {
                check()?;
                let bytes: Vec<u8> = passage.get(4).map_err(db_err)?;
                let checksum: String = passage.get(5).map_err(db_err)?;
                let values = store::decode(&bytes, identity.dimensions, &checksum)?;
                let score = values
                    .iter()
                    .zip(vector.as_ref().unwrap())
                    .map(|(a, b)| f64::from(*a) * f64::from(*b))
                    .sum::<f64>()
                    .clamp(-1.0, 1.0);
                response.scanned_passages += 1;
                let section: usize = passage.get(0).map_err(db_err)?;
                let start: usize = passage.get(1).map_err(db_err)?;
                let end: usize = passage.get(2).map_err(db_err)?;
                if section_ranges
                    .as_ref()
                    .is_none_or(|(ordinal, _)| *ordinal != section)
                {
                    let text: String = passage.get(6).map_err(db_err)?;
                    let encoded: Option<String> = passage.get(7).map_err(db_err)?;
                    section_ranges = Some((
                        section,
                        super::super::search_content::ranges(&text, encoded.as_deref())?,
                    ));
                }
                let main_content = section_ranges
                    .as_ref()
                    .unwrap()
                    .1
                    .iter()
                    .any(|range| range.start <= start && end <= range.end);
                // SQL source order resolves equal passage scores without extra overlap votes.
                // Auxiliary passages remain available when a document has no main content.
                // shortcut: body always wins; revisit with judged citation queries before blending roles.
                if best.as_ref().is_none_or(|old: &Candidate| {
                    main_content > old.main_content
                        || (main_content == old.main_content && score > old.score)
                }) {
                    best = Some(Candidate {
                        id: id.clone(),
                        url: url.clone(),
                        title: title.clone(),
                        format: format.clone(),
                        score,
                        section,
                        start,
                        end,
                        hash: passage.get(3).map_err(db_err)?,
                        main_content,
                    });
                }
            }
            let best = best.ok_or("Ready semantic document has no indexed passages")?;
            let position = candidates.partition_point(|old| {
                old.score > best.score || (old.score == best.score && old.id < best.id)
            });
            if position < limit {
                candidates.insert(position, best);
                candidates.truncate(limit);
            }
        }
    }
    for candidate in candidates {
        check()?;
        let (heading, text, page): (Option<String>, String, Option<usize>) = tx.query_row(
            "SELECT heading,text,page_index FROM uploaded_sections WHERE document_id=?1 AND ordinal=?2",
            params![candidate.id,candidate.section], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?))).map_err(db_err)?;
        let body = text
            .get(candidate.start..candidate.end)
            .filter(|body| !body.is_empty() && body.chars().count() <= 4096)
            .ok_or("Invalid semantic UTF-8 source span")?;
        let hash = format!(
            "{:x}",
            Sha256::digest(
                store::passage_input(&candidate.title, heading.as_deref(), body).as_bytes()
            )
        );
        if hash != candidate.hash {
            return Err("Semantic source content changed; rebuild index".into());
        }
        let context = nearby_context(&tx, &candidate, &mut check)?;
        response.results.push(ConceptualResult {
            document_id: candidate.id,
            url: candidate.url,
            title: candidate.title,
            format: candidate.format,
            score: candidate.score,
            provenance: "conceptual",
            section_index: candidate.section,
            page_index: page,
            section_title: heading,
            byte_start: candidate.start,
            byte_end: candidate.end,
            text: body.into(),
            context,
        });
    }
    response.availability = if response.indexed_documents < response.eligible_documents {
        Availability::Partial
    } else {
        Availability::Ready
    };
    tx.commit().map_err(db_err)?;
    // Check a fresh snapshot; an edit/removal during scoring invalidates the whole response.
    let fresh = db.transaction().map_err(db_err)?;
    if metadata(&fresh)? != original {
        return Err("Semantic library changed; retry query".into());
    }
    check()?;
    Ok(response)
}
