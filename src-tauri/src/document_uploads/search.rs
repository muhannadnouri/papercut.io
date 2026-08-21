//! FTS5 query building and execution for uploaded documents.
//!
//! Read-only against the schema owned by [`super::store`]. Kept separate so
//! query/ranking behavior can evolve without touching the write path.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use rusqlite::{params, params_from_iter, types::Value, Connection, Row};
use tauri::Runtime;

use super::storage::{upload_reference_from_url, StoredSourceKind};
use super::store::{db_err, open_db};
use super::types::{
    UploadedDocumentConcordanceEntry, UploadedDocumentConcordanceRequest,
    UploadedDocumentConcordanceResponse, UploadedDocumentSearchLocation,
    UploadedDocumentSearchPassage, UploadedDocumentSearchRequest, UploadedDocumentSearchResponse,
    UploadedDocumentSearchResult, UploadedDocumentSearchStage, UploadedDocumentSearchTermMatch,
    UploadedPdfFindPage, UploadedPdfFindRequest, UploadedPdfFindResult,
};

const MAX_EVIDENCE_PASSAGES: usize = 3;
const OCCURRENCE_MAP_BINS: usize = 12;
const MAX_COMPARISON_TERMS: usize = 6;
const DEFAULT_CONCORDANCE_LIMIT: usize = 50;
const MAX_CONCORDANCE_LIMIT: usize = 100;

struct RankedDocumentCandidate {
    document_id: String,
    section_id: i64,
    section_index: usize,
    score: f64,
    imported_at_ms: i64,
    matching_sections: usize,
    section_count: usize,
    match_scope: &'static str,
    exact_evidence: Option<ExactPhraseEvidence>,
}

struct ExactPhraseEvidence {
    section_index: usize,
    page_index: Option<usize>,
    section_title: Option<String>,
    match_count: usize,
    excerpt: String,
}

struct SearchEvidence {
    passages: Vec<(f64, UploadedDocumentSearchPassage)>,
    locations: Vec<Option<UploadedDocumentSearchLocation>>,
}

/// Run an FTS5 MATCH query, joining hits back to their section and document and
/// returning BM25-ranked results with `<mark>`-highlighted snippets.
pub(crate) fn search_uploads<R: Runtime>(
    app: &tauri::AppHandle<R>,
    request: UploadedDocumentSearchRequest,
    mut progress: impl FnMut(UploadedDocumentSearchStage),
) -> Result<UploadedDocumentSearchResponse, String> {
    let search_started = Instant::now();
    let fuzzy_terms = fts_fuzzy_terms(&request.query);
    let fuzzy_queries = fuzzy_terms
        .iter()
        .map(|term| fts_alias_query(term))
        .collect::<Vec<_>>();
    let exact_phrases = request.exact_phrases.unwrap_or_default();
    let exact_queries = fts_phrase_queries(&exact_phrases);
    if fuzzy_queries.is_empty() && exact_queries.is_empty() {
        return Ok(empty_search_response());
    }
    let comparison_terms = comparison_terms(&fuzzy_terms, &fuzzy_queries, exact_phrases.is_empty());
    let mut queries = fuzzy_queries;
    queries.extend(exact_queries.iter().cloned());
    let query = fts_and_query(&queries);

    let db_started = Instant::now();
    let db = open_db(app)?;
    let db_ms = db_started.elapsed().as_millis();
    let limit = request.limit.unwrap_or(50).clamp(1, 100);
    let document_urls = request
        .document_urls
        .unwrap_or_default()
        .into_iter()
        .filter(|url| !url.trim().is_empty())
        .collect::<Vec<_>>();

    progress(UploadedDocumentSearchStage::FindingCandidates);
    let candidate_started = Instant::now();
    let mut section_candidates = document_candidates(&db, &query, &document_urls, None, "section")?;
    let section_document_ids = section_candidates
        .iter()
        .map(|candidate| candidate.document_id.clone())
        .collect::<HashSet<_>>();
    let mut document_candidates = if queries.len() > 1 {
        let mut document_ids = document_ids_matching_all_queries(&db, &queries, &document_urls)?;
        document_ids.retain(|id| !section_document_ids.contains(id));
        let or_query = fts_or_query(&queries);
        document_candidates(
            &db,
            &or_query,
            &document_urls,
            Some(&document_ids),
            "document",
        )?
    } else {
        Vec::new()
    };
    let candidate_documents = section_candidates.len() + document_candidates.len();
    let candidate_ms = candidate_started.elapsed().as_millis();

    let verification_started = Instant::now();
    if !exact_queries.is_empty() {
        progress(UploadedDocumentSearchStage::VerifyingPhrases);
        section_candidates =
            retain_exact_phrase_candidates(&db, section_candidates, &exact_phrases)?;
        document_candidates =
            retain_exact_phrase_candidates(&db, document_candidates, &exact_phrases)?;
    }
    let verification_ms = verification_started.elapsed().as_millis();

    let total_documents = section_candidates.len() + document_candidates.len();
    let total_matching_sections = section_candidates
        .iter()
        .chain(&document_candidates)
        .map(|candidate| candidate.matching_sections)
        .sum();
    section_candidates.truncate(limit);
    let remaining = limit.saturating_sub(section_candidates.len());
    document_candidates.truncate(remaining);
    let or_query = fts_or_query(&queries);
    progress(UploadedDocumentSearchStage::BuildingResults);
    let result_started = Instant::now();
    let exact_evidence_started = Instant::now();
    if !exact_queries.is_empty() {
        attach_exact_phrase_evidence(&db, &mut section_candidates, &exact_phrases)?;
        attach_exact_phrase_evidence(&db, &mut document_candidates, &exact_phrases)?;
    }
    let exact_evidence_ms = exact_evidence_started.elapsed().as_millis();
    let result_evidence_started = Instant::now();
    let mut results = search_results_for_candidates(&db, &query, &section_candidates)?;
    results.extend(search_results_for_candidates(
        &db,
        &or_query,
        &document_candidates,
    )?);
    let result_evidence_ms = result_evidence_started.elapsed().as_millis();
    let term_matches_started = Instant::now();
    attach_search_term_matches(&db, &comparison_terms, &mut results)?;
    let term_matches_ms = term_matches_started.elapsed().as_millis();
    let result_ms = result_started.elapsed().as_millis();
    if cfg!(debug_assertions) {
        log::info!(
            "[search] native performance summary terms={} exact_phrases={} scoped_documents={} \
             candidates={} matches={} matching_sections={} visible_results={} db_ms={} \
             candidate_ms={} verification_ms={} exact_evidence_ms={} result_evidence_ms={} \
             term_matches_ms={} result_ms={} total_ms={}",
            fuzzy_terms.len(),
            exact_phrases.len(),
            document_urls.len(),
            candidate_documents,
            total_documents,
            total_matching_sections,
            results.len(),
            db_ms,
            candidate_ms,
            verification_ms,
            exact_evidence_ms,
            result_evidence_ms,
            term_matches_ms,
            result_ms,
            search_started.elapsed().as_millis(),
        );
    }

    Ok(UploadedDocumentSearchResponse {
        results,
        total_documents,
        total_matching_sections,
    })
}

/// Return one bounded page of literal context lines from an uploaded document.
pub(crate) fn concordance_upload<R: Runtime>(
    app: &tauri::AppHandle<R>,
    request: UploadedDocumentConcordanceRequest,
) -> Result<UploadedDocumentConcordanceResponse, String> {
    let (document_id, _) = upload_reference_from_url(&request.document_url)?;
    let query = request.query.chars().take(512).collect::<String>();
    let normalized_query = normalize_exact_text(&query);
    if normalized_query.is_empty() {
        return Err("Concordance query is empty".to_string());
    }
    let db = open_db(app)?;
    document_concordance(
        &db,
        &document_id,
        &normalized_query,
        request.offset.unwrap_or(0),
        request
            .limit
            .unwrap_or(DEFAULT_CONCORDANCE_LIMIT)
            .clamp(1, MAX_CONCORDANCE_LIMIT),
    )
}

/// ponytail: each page rescans one indexed document to keep storage/schema
/// unchanged; persist occurrence offsets only if measured paging latency matters.
fn document_concordance(
    db: &Connection,
    document_id: &str,
    query: &str,
    offset: usize,
    limit: usize,
) -> Result<UploadedDocumentConcordanceResponse, String> {
    let mut stmt = db
        .prepare(
            "SELECT ordinal, page_index, heading, text FROM uploaded_sections \
             WHERE document_id = ?1 ORDER BY ordinal ASC",
        )
        .map_err(db_err)?;
    let rows = stmt
        .query_map(params![document_id], |row| {
            Ok((
                row.get::<_, i64>(0)? as usize,
                row.get::<_, Option<i64>>(1)?.map(|value| value as usize),
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(db_err)?;
    let mut total_matches = 0usize;
    let mut entries = Vec::new();
    for row in rows {
        let (section_index, page_index, section_title, text) = row.map_err(db_err)?;
        let display = normalize_exact_display(&text);
        let normalized = display.to_lowercase();
        for (section_occurrence_index, (match_start, _)) in
            normalized.match_indices(query).enumerate()
        {
            let occurrence_index = total_matches;
            total_matches = total_matches.saturating_add(1);
            if occurrence_index < offset || entries.len() >= limit {
                continue;
            }
            entries.push(UploadedDocumentConcordanceEntry {
                occurrence_index,
                section_occurrence_index,
                excerpt: highlighted_exact_excerpt(&display, &normalized, query, match_start),
                section_title: section_title.clone(),
                section_index,
                page_index,
            });
        }
    }
    let shown_end = offset.saturating_add(entries.len());
    Ok(UploadedDocumentConcordanceResponse {
        total_matches,
        entries,
        next_offset: (shown_end < total_matches).then_some(shown_end),
    })
}

fn empty_search_response() -> UploadedDocumentSearchResponse {
    UploadedDocumentSearchResponse {
        results: Vec::new(),
        total_documents: 0,
        total_matching_sections: 0,
    }
}

/// Find literal text across one PDF's already-indexed page rows.
///
/// This stays off the WebView thread and returns only per-page counts, so a
/// long OCR book does not require reparsing sidecars or transferring all text
/// and geometry over IPC on each query.
pub(crate) fn find_pdf_text<R: Runtime>(
    app: &tauri::AppHandle<R>,
    request: UploadedPdfFindRequest,
) -> Result<UploadedPdfFindResult, String> {
    let (document_id, source_kind) = upload_reference_from_url(&request.document_url)?;
    if source_kind != StoredSourceKind::Pdf {
        return Err("Document is not an uploaded PDF".into());
    }

    let query = request.query.chars().take(512).collect::<String>();
    let needle = normalize_exact_text(&query);
    if needle.is_empty() {
        return Ok(UploadedPdfFindResult {
            match_count: 0,
            pages: Vec::new(),
        });
    }

    let db = open_db(app)?;
    let mut stmt = db
        .prepare(
            "SELECT page_index, text FROM uploaded_sections \
             WHERE document_id = ?1 AND page_index IS NOT NULL ORDER BY ordinal ASC",
        )
        .map_err(db_err)?;
    let rows = stmt
        .query_map(params![document_id], |row| {
            Ok((row.get::<_, usize>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(db_err)?;
    let mut pages = Vec::new();
    let mut match_count = 0usize;
    for row in rows {
        let (page_index, text) = row.map_err(db_err)?;
        let page_matches = count_normalized_matches(&text, &needle);
        if page_matches == 0 {
            continue;
        }
        match_count = match_count.saturating_add(page_matches);
        pages.push(UploadedPdfFindPage {
            page_index,
            match_count: page_matches,
        });
    }

    Ok(UploadedPdfFindResult { match_count, pages })
}

fn count_normalized_matches(text: &str, normalized_query: &str) -> usize {
    if normalized_query.is_empty() {
        return 0;
    }
    normalize_exact_text(text).matches(normalized_query).count()
}

/// FTS narrows the candidate set efficiently, then this verifies Papercut's
/// literal, whitespace-normalized phrase semantics without reading every PDF.
#[cfg(test)]
fn retain_exact_phrase_hits(
    db: &Connection,
    results: Vec<UploadedDocumentSearchResult>,
    phrases: &[String],
) -> Result<Vec<UploadedDocumentSearchResult>, String> {
    let normalized_phrases = phrases
        .iter()
        .filter(|phrase| !fts_terms(phrase, 128).is_empty())
        .map(|phrase| normalize_exact_text(phrase))
        .collect::<Vec<_>>();
    let mut verdicts = HashMap::new();
    let mut verified = Vec::new();

    for result in results {
        let matches = match verdicts.get(&result.document_id) {
            Some(matches) => *matches,
            None => {
                let matches =
                    document_contains_exact_phrases(db, &result.document_id, &normalized_phrases)?;
                verdicts.insert(result.document_id.clone(), matches);
                matches
            }
        };
        if matches {
            verified.push(result);
        }
    }
    Ok(verified)
}

fn retain_exact_phrase_candidates(
    db: &Connection,
    candidates: Vec<RankedDocumentCandidate>,
    phrases: &[String],
) -> Result<Vec<RankedDocumentCandidate>, String> {
    let normalized_phrases = phrases
        .iter()
        .filter(|phrase| !fts_terms(phrase, 128).is_empty())
        .map(|phrase| normalize_exact_text(phrase))
        .collect::<Vec<_>>();
    let mut verified = Vec::new();
    for candidate in candidates {
        if document_contains_exact_phrases(db, &candidate.document_id, &normalized_phrases)? {
            verified.push(candidate);
        }
    }
    Ok(verified)
}

/// Stream indexed sections in reading order and stop once every phrase has
/// appeared; memory stays bounded even when a candidate PDF has many pages.
fn document_contains_exact_phrases(
    db: &Connection,
    document_id: &str,
    phrases: &[String],
) -> Result<bool, String> {
    if phrases.is_empty() {
        return Ok(false);
    }
    let mut stmt = db
        .prepare(
            "SELECT text FROM uploaded_sections \
             WHERE document_id = ?1 ORDER BY ordinal ASC",
        )
        .map_err(db_err)?;
    let rows = stmt
        .query_map(params![document_id], |row| row.get::<_, String>(0))
        .map_err(db_err)?;
    let mut remaining = phrases.to_vec();
    for row in rows {
        let normalized = normalize_exact_text(&row.map_err(db_err)?);
        remaining.retain(|phrase| !normalized.contains(phrase));
        if remaining.is_empty() {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Complete exact counts and display evidence only for the bounded visible set.
fn attach_exact_phrase_evidence(
    db: &Connection,
    candidates: &mut [RankedDocumentCandidate],
    phrases: &[String],
) -> Result<(), String> {
    let normalized_phrases = phrases
        .iter()
        .filter(|phrase| !fts_terms(phrase, 128).is_empty())
        .map(|phrase| normalize_exact_text(phrase))
        .collect::<Vec<_>>();
    for candidate in candidates {
        candidate.exact_evidence =
            document_exact_phrase_evidence(db, &candidate.document_id, &normalized_phrases)?;
        if candidate.exact_evidence.is_none() {
            return Err("Verified exact search candidate disappeared".to_string());
        }
    }
    Ok(())
}

/// Verify and count every phrase in one streaming section scan, retaining only
/// the first phrase's earliest source target and one bounded display excerpt.
fn document_exact_phrase_evidence(
    db: &Connection,
    document_id: &str,
    phrases: &[String],
) -> Result<Option<ExactPhraseEvidence>, String> {
    let Some(first_phrase) = phrases.first() else {
        return Ok(None);
    };
    let mut stmt = db
        .prepare(
            "SELECT ordinal, page_index, heading, text FROM uploaded_sections \
             WHERE document_id = ?1 ORDER BY ordinal ASC",
        )
        .map_err(db_err)?;
    let rows = stmt
        .query_map(params![document_id], |row| {
            Ok((
                row.get::<_, i64>(0)? as usize,
                row.get::<_, Option<i64>>(1)?.map(|value| value as usize),
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(db_err)?;
    let mut remaining = phrases.to_vec();
    let mut evidence = None;
    let mut match_count = 0usize;
    for row in rows {
        let (section_index, page_index, section_title, text) = row.map_err(db_err)?;
        let display = normalize_exact_display(&text);
        let normalized = display.to_lowercase();
        match_count = phrases.iter().fold(match_count, |count, phrase| {
            count.saturating_add(normalized.matches(phrase).count())
        });
        if evidence.is_none() && normalized.contains(first_phrase) {
            evidence = Some(ExactPhraseEvidence {
                section_index,
                page_index,
                section_title,
                match_count: 0,
                excerpt: exact_phrase_excerpt(&display, &normalized, first_phrase),
            });
        }
        remaining.retain(|phrase| !normalized.contains(phrase));
    }
    if !remaining.is_empty() {
        return Ok(None);
    }
    if let Some(evidence) = &mut evidence {
        evidence.match_count = match_count;
    }
    Ok(evidence)
}

fn exact_phrase_excerpt(display: &str, normalized: &str, phrase: &str) -> String {
    highlighted_exact_excerpt(
        display,
        normalized,
        phrase,
        normalized.find(phrase).unwrap_or(0),
    )
}

fn highlighted_exact_excerpt(
    display: &str,
    normalized: &str,
    phrase: &str,
    match_byte_index: usize,
) -> String {
    const CONTEXT: usize = 120;

    let match_start = normalized[..match_byte_index].chars().count();
    let characters = display.chars().collect::<Vec<_>>();
    let highlight_start = match_start.min(characters.len());
    let highlight_end = (highlight_start + phrase.chars().count()).min(characters.len());
    let start = highlight_start.saturating_sub(CONTEXT);
    let end = (highlight_end + CONTEXT).min(characters.len());
    let mut excerpt = String::new();
    if start > 0 {
        excerpt.push_str("… ");
    }
    excerpt.extend(characters[start..highlight_start].iter().copied());
    excerpt.push_str("<mark>");
    excerpt.extend(characters[highlight_start..highlight_end].iter().copied());
    excerpt.push_str("</mark>");
    excerpt.extend(characters[highlight_end..end].iter().copied());
    if end < characters.len() {
        excerpt.push_str(" …");
    }
    excerpt
}

#[cfg(test)]
fn search_section_hits(
    db: &Connection,
    query: &str,
    limit: i64,
    document_urls: &[String],
) -> Result<Vec<UploadedDocumentSearchResult>, String> {
    let candidates = document_candidates(db, query, document_urls, None, "section")?
        .into_iter()
        .take(limit.max(0) as usize)
        .collect::<Vec<_>>();
    search_results_for_candidates(db, query, &candidates)
}

fn document_candidates(
    db: &Connection,
    query: &str,
    document_urls: &[String],
    document_ids: Option<&[String]>,
    match_scope: &'static str,
) -> Result<Vec<RankedDocumentCandidate>, String> {
    if document_ids.is_some_and(|ids| ids.is_empty()) {
        return Ok(Vec::new());
    }

    let (url_scope_sql, mut values) = document_url_scope(document_urls);
    let (id_scope_sql, id_values) = document_id_scope(document_ids);
    let sql = format!(
        "SELECT uploaded_document_fts.document_id, \
                CAST(uploaded_document_fts.section_id AS INTEGER), s.ordinal, \
                bm25(uploaded_document_fts), d.imported_at_ms, d.sections \
         FROM uploaded_document_fts \
         JOIN uploaded_sections s ON s.id = uploaded_document_fts.section_id \
         JOIN uploaded_documents d ON d.id = uploaded_document_fts.document_id \
         WHERE uploaded_document_fts MATCH ? {url_scope_sql} {id_scope_sql}"
    );
    values.insert(0, Value::Text(query.to_string()));
    values.extend(id_values);

    let mut stmt = db.prepare(&sql).map_err(db_err)?;
    let rows = stmt
        .query_map(params_from_iter(values.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)? as usize,
                row.get::<_, f64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)? as usize,
            ))
        })
        .map_err(db_err)?;
    let mut candidates = HashMap::<String, RankedDocumentCandidate>::new();
    for row in rows {
        let (document_id, section_id, section_index, score, imported_at_ms, section_count) =
            row.map_err(db_err)?;
        if let Some(candidate) = candidates.get_mut(&document_id) {
            if score.total_cmp(&candidate.score).is_lt()
                || (score == candidate.score && section_index < candidate.section_index)
            {
                candidate.section_id = section_id;
                candidate.section_index = section_index;
                candidate.score = score;
            }
        } else {
            candidates.insert(
                document_id.clone(),
                RankedDocumentCandidate {
                    document_id,
                    section_id,
                    section_index,
                    score,
                    imported_at_ms,
                    matching_sections: 0,
                    section_count,
                    match_scope,
                    exact_evidence: None,
                },
            );
        }
    }
    drop(stmt);

    for (document_id, count) in matching_section_counts(db, query, document_urls, document_ids)? {
        if let Some(candidate) = candidates.get_mut(&document_id) {
            candidate.matching_sections = count;
        }
    }

    let mut candidates = candidates.into_values().collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        left.score
            .total_cmp(&right.score)
            .then_with(|| right.imported_at_ms.cmp(&left.imported_at_ms))
            .then_with(|| left.document_id.cmp(&right.document_id))
    });
    Ok(candidates)
}

fn matching_section_counts(
    db: &Connection,
    query: &str,
    document_urls: &[String],
    document_ids: Option<&[String]>,
) -> Result<HashMap<String, usize>, String> {
    let (url_scope_sql, mut values) = document_url_scope(document_urls);
    let (id_scope_sql, id_values) = document_id_scope(document_ids);
    let sql = format!(
        "SELECT uploaded_document_fts.document_id, COUNT(*) \
         FROM uploaded_document_fts \
         JOIN uploaded_documents d ON d.id = uploaded_document_fts.document_id \
         WHERE uploaded_document_fts MATCH ? {url_scope_sql} {id_scope_sql} \
         GROUP BY uploaded_document_fts.document_id"
    );
    values.insert(0, Value::Text(format!("{{heading text}} : ({query})")));
    values.extend(id_values);

    let mut stmt = db.prepare(&sql).map_err(db_err)?;
    let rows = stmt
        .query_map(params_from_iter(values.iter()), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
        })
        .map_err(db_err)?;
    rows.collect::<Result<HashMap<_, _>, _>>().map_err(db_err)
}

/// Find documents where every term or quoted phrase exists somewhere, even when
/// no single section contains all of them. This keeps broad book searches useful
/// without loading complete PDF text into the WebView.
#[cfg(test)]
fn search_cross_section_document_hits(
    db: &Connection,
    queries: &[String],
    limit: i64,
    document_urls: &[String],
    excluded_document_ids: &HashSet<String>,
) -> Result<Vec<UploadedDocumentSearchResult>, String> {
    if limit <= 0 {
        return Ok(Vec::new());
    }

    let mut candidate_ids = document_ids_matching_all_queries(db, queries, document_urls)?;
    candidate_ids.retain(|id| !excluded_document_ids.contains(id));

    let query = fts_or_query(queries);
    let candidates =
        document_candidates(db, &query, document_urls, Some(&candidate_ids), "document")?
            .into_iter()
            .take(limit as usize)
            .collect::<Vec<_>>();
    search_results_for_candidates(db, &query, &candidates)
}

fn document_ids_matching_all_queries(
    db: &Connection,
    queries: &[String],
    document_urls: &[String],
) -> Result<Vec<String>, String> {
    let mut intersection: Option<HashSet<String>> = None;

    for query in queries {
        let (scope_sql, mut values) = document_url_scope(document_urls);
        let sql = format!(
            "SELECT DISTINCT uploaded_document_fts.document_id \
             FROM uploaded_document_fts \
             JOIN uploaded_documents d ON d.id = uploaded_document_fts.document_id \
             WHERE uploaded_document_fts MATCH ? {scope_sql}"
        );
        values.insert(0, Value::Text(query.clone()));
        let mut stmt = db.prepare(&sql).map_err(db_err)?;
        let rows = stmt
            .query_map(params_from_iter(values.iter()), |row| {
                row.get::<_, String>(0)
            })
            .map_err(db_err)?;
        let ids = rows.collect::<Result<HashSet<_>, _>>().map_err(db_err)?;

        intersection = Some(match intersection {
            Some(current) => current.intersection(&ids).cloned().collect(),
            None => ids,
        });
        if intersection.as_ref().is_some_and(HashSet::is_empty) {
            return Ok(Vec::new());
        }
    }

    let mut ids = intersection
        .unwrap_or_default()
        .into_iter()
        .collect::<Vec<_>>();
    ids.sort();
    Ok(ids)
}

fn search_results_for_candidates(
    db: &Connection,
    query: &str,
    candidates: &[RankedDocumentCandidate],
) -> Result<Vec<UploadedDocumentSearchResult>, String> {
    if candidates.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders = (0..candidates.len())
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT d.id, d.url, d.title, s.ordinal, s.page_index, s.heading, \
                snippet(uploaded_document_fts, 4, '<mark>', '</mark>', '…', 18) AS excerpt, \
                CAST(uploaded_document_fts.section_id AS INTEGER) \
         FROM uploaded_document_fts \
         JOIN uploaded_sections s ON s.id = uploaded_document_fts.section_id \
         JOIN uploaded_documents d ON d.id = uploaded_document_fts.document_id \
         WHERE uploaded_document_fts MATCH ? \
           AND uploaded_document_fts.section_id IN ({placeholders})"
    );
    let mut values = vec![Value::Text(query.to_string())];
    values.extend(
        candidates
            .iter()
            .map(|candidate| Value::Integer(candidate.section_id)),
    );
    let candidate_by_section = candidates
        .iter()
        .map(|candidate| (candidate.section_id, candidate))
        .collect::<HashMap<_, _>>();
    let mut stmt = db.prepare(&sql).map_err(db_err)?;
    let rows = stmt
        .query_map(params_from_iter(values.iter()), |row| {
            let section_id = row.get::<_, i64>(7)?;
            let candidate = candidate_by_section
                .get(&section_id)
                .ok_or(rusqlite::Error::InvalidQuery)?;
            row_to_search_result(row, candidate)
        })
        .map_err(db_err)?;
    let mut result_by_document = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(db_err)?
        .into_iter()
        .map(|result| (result.document_id.clone(), result))
        .collect::<HashMap<_, _>>();

    let mut results = candidates
        .iter()
        .map(|candidate| {
            result_by_document
                .remove(&candidate.document_id)
                .ok_or_else(|| "Ranked search result disappeared".to_string())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if candidates
        .iter()
        .all(|candidate| candidate.exact_evidence.is_none())
    {
        attach_search_evidence(db, query, candidates, &mut results)?;
    }
    Ok(results)
}

fn row_to_search_result(
    row: &Row<'_>,
    candidate: &RankedDocumentCandidate,
) -> rusqlite::Result<UploadedDocumentSearchResult> {
    let document_id: String = row.get(0)?;
    let broad_section_index = row.get::<_, i64>(3)? as usize;
    let broad_page_index = row
        .get::<_, Option<i64>>(4)?
        .map(|page_index| page_index as usize);
    let broad_section_title = row.get::<_, Option<String>>(5)?;
    let broad_excerpt = row.get::<_, String>(6)?;
    let (section_index, page_index, section_title, excerpt, match_count) = candidate
        .exact_evidence
        .as_ref()
        .map(|evidence| {
            (
                evidence.section_index,
                evidence.page_index,
                evidence.section_title.clone(),
                evidence.excerpt.clone(),
                Some(evidence.match_count),
            )
        })
        .unwrap_or((
            broad_section_index,
            broad_page_index,
            broad_section_title,
            broad_excerpt,
            None,
        ));
    Ok(UploadedDocumentSearchResult {
        id: format!(
            "upload:{}:{document_id}:{section_index}",
            candidate.match_scope
        ),
        document_id,
        url: row.get(1)?,
        title: row.get(2)?,
        section_index,
        page_index,
        section_title: section_title.clone(),
        excerpt: excerpt.clone(),
        match_scope: candidate.match_scope.to_string(),
        matching_sections: candidate.matching_sections,
        match_count,
        passages: vec![UploadedDocumentSearchPassage {
            excerpt,
            section_title,
            section_index,
            page_index,
        }],
        match_locations: Vec::new(),
        term_matches: Vec::new(),
    })
}

/// Add bounded, source-linked evidence only for the visible document set.
/// One matching-section scan supplies both the three best snippets and a
/// twelve-bin distribution, so result payload and DOM size do not grow with a book.
fn attach_search_evidence(
    db: &Connection,
    query: &str,
    candidates: &[RankedDocumentCandidate],
    results: &mut [UploadedDocumentSearchResult],
) -> Result<(), String> {
    let placeholders = (0..candidates.len())
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT uploaded_document_fts.document_id, s.ordinal, s.page_index, s.heading, \
                snippet(uploaded_document_fts, 4, '<mark>', '</mark>', '…', 18), \
                bm25(uploaded_document_fts) \
         FROM uploaded_document_fts \
         JOIN uploaded_sections s ON s.id = uploaded_document_fts.section_id \
         WHERE uploaded_document_fts MATCH ? \
           AND uploaded_document_fts.document_id IN ({placeholders}) \
         ORDER BY uploaded_document_fts.document_id, s.ordinal"
    );
    let mut values = vec![Value::Text(format!("{{heading text}} : ({query})"))];
    values.extend(
        candidates
            .iter()
            .map(|candidate| Value::Text(candidate.document_id.clone())),
    );
    let candidate_by_document = candidates
        .iter()
        .map(|candidate| (candidate.document_id.as_str(), candidate))
        .collect::<HashMap<_, _>>();
    let mut evidence_by_document = candidates
        .iter()
        .map(|candidate| {
            (
                candidate.document_id.clone(),
                SearchEvidence {
                    passages: Vec::new(),
                    locations: (0..OCCURRENCE_MAP_BINS).map(|_| None).collect(),
                },
            )
        })
        .collect::<HashMap<_, _>>();

    let mut stmt = db.prepare(&sql).map_err(db_err)?;
    let rows = stmt
        .query_map(params_from_iter(values.iter()), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)? as usize,
                row.get::<_, Option<i64>>(2)?.map(|value| value as usize),
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, f64>(5)?,
            ))
        })
        .map_err(db_err)?;
    for row in rows {
        let (document_id, section_index, page_index, section_title, excerpt, score) =
            row.map_err(db_err)?;
        let Some(candidate) = candidate_by_document.get(document_id.as_str()) else {
            continue;
        };
        let Some(evidence) = evidence_by_document.get_mut(&document_id) else {
            continue;
        };
        let text = first_marked_text(&excerpt);

        evidence.passages.push((
            score,
            UploadedDocumentSearchPassage {
                excerpt,
                section_title,
                section_index,
                page_index,
            },
        ));
        evidence.passages.sort_by(|left, right| {
            left.0
                .total_cmp(&right.0)
                .then_with(|| left.1.section_index.cmp(&right.1.section_index))
        });
        evidence.passages.truncate(MAX_EVIDENCE_PASSAGES);

        let bin = (section_index.saturating_mul(OCCURRENCE_MAP_BINS)
            / candidate.section_count.max(1))
        .min(OCCURRENCE_MAP_BINS - 1);
        match &mut evidence.locations[bin] {
            Some(location) => location.match_count += 1,
            slot @ None => {
                *slot = Some(UploadedDocumentSearchLocation {
                    bin_index: bin,
                    section_index,
                    page_index,
                    match_count: 1,
                    text,
                });
            }
        }
    }

    for result in results {
        let Some(evidence) = evidence_by_document.remove(&result.document_id) else {
            continue;
        };
        if !evidence.passages.is_empty() {
            result.passages = evidence
                .passages
                .into_iter()
                .map(|(_, passage)| passage)
                .collect();
        }
        result.match_locations = evidence.locations.into_iter().flatten().collect();
    }
    Ok(())
}

fn first_marked_text(excerpt: &str) -> Option<String> {
    let start = excerpt.find("<mark>")? + "<mark>".len();
    let end = start + excerpt[start..].find("</mark>")?;
    let text = excerpt[start..end].trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// Add comparison counts only for small broad queries and visible results.
/// SQLite aggregates each term to one row per document, keeping IPC and Rust
/// memory bounded even when a common term occurs on many pages.
fn attach_search_term_matches(
    db: &Connection,
    terms: &[(String, String)],
    results: &mut [UploadedDocumentSearchResult],
) -> Result<(), String> {
    if terms.is_empty() || results.is_empty() {
        return Ok(());
    }

    for result in results.iter_mut() {
        result.term_matches = terms
            .iter()
            .map(|(term, _)| UploadedDocumentSearchTermMatch {
                term: term.clone(),
                matching_sections: 0,
                section_index: None,
                page_index: None,
                text: None,
            })
            .collect();
    }

    let result_indexes = results
        .iter()
        .enumerate()
        .map(|(index, result)| (result.document_id.clone(), index))
        .collect::<HashMap<_, _>>();
    let placeholders = (0..results.len())
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "WITH matches AS (\
           SELECT uploaded_document_fts.document_id, \
                  CAST(uploaded_document_fts.section_id AS INTEGER) AS section_id, \
                  s.ordinal, s.page_index, \
                  COUNT(*) OVER (PARTITION BY uploaded_document_fts.document_id) AS match_count, \
                  ROW_NUMBER() OVER (\
                    PARTITION BY uploaded_document_fts.document_id ORDER BY s.ordinal\
                  ) AS match_number \
           FROM uploaded_document_fts \
           JOIN uploaded_sections s ON s.id = uploaded_document_fts.section_id \
           WHERE uploaded_document_fts MATCH ? \
             AND uploaded_document_fts.document_id IN ({placeholders})\
         ) \
         SELECT matches.document_id, matches.ordinal, matches.page_index, matches.match_count, \
                snippet(uploaded_document_fts, -1, '<mark>', '</mark>', '…', 18) \
         FROM matches \
         JOIN uploaded_document_fts \
           ON CAST(uploaded_document_fts.section_id AS INTEGER) = matches.section_id \
         WHERE matches.match_number = 1 AND uploaded_document_fts MATCH ?"
    );

    let mut stmt = db.prepare(&sql).map_err(db_err)?;
    for (term_index, (_, query)) in terms.iter().enumerate() {
        let scoped_query = format!("{{heading text}} : ({query})");
        let mut values = vec![Value::Text(scoped_query.clone())];
        values.extend(
            results
                .iter()
                .map(|result| Value::Text(result.document_id.clone())),
        );
        values.push(Value::Text(scoped_query));
        let rows = stmt
            .query_map(params_from_iter(values.iter()), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)? as usize,
                    row.get::<_, Option<i64>>(2)?.map(|value| value as usize),
                    row.get::<_, i64>(3)? as usize,
                    row.get::<_, String>(4)?,
                ))
            })
            .map_err(db_err)?;
        for row in rows {
            let (document_id, section_index, page_index, matching_sections, excerpt) =
                row.map_err(db_err)?;
            let Some(result_index) = result_indexes.get(&document_id) else {
                continue;
            };
            let term_match = &mut results[*result_index].term_matches[term_index];
            term_match.matching_sections = matching_sections;
            term_match.section_index = Some(section_index);
            term_match.page_index = page_index;
            term_match.text = first_marked_text(&excerpt);
        }
    }
    Ok(())
}

fn document_url_scope(document_urls: &[String]) -> (String, Vec<Value>) {
    if document_urls.is_empty() {
        return (String::new(), Vec::new());
    }
    let placeholders = (0..document_urls.len())
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(", ");
    (
        format!("AND d.url IN ({placeholders})"),
        document_urls.iter().cloned().map(Value::Text).collect(),
    )
}

fn document_id_scope(document_ids: Option<&[String]>) -> (String, Vec<Value>) {
    let Some(document_ids) = document_ids else {
        return (String::new(), Vec::new());
    };
    let placeholders = (0..document_ids.len())
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(", ");
    (
        format!("AND uploaded_document_fts.document_id IN ({placeholders})"),
        document_ids.iter().cloned().map(Value::Text).collect(),
    )
}

/// Turn a broad/fuzzy query into safe FTS5 terms for candidate lookup.
fn fts_fuzzy_terms(query: &str) -> Vec<String> {
    fts_terms(query, 12)
}

#[cfg(test)]
fn fts_fuzzy_queries(query: &str) -> Vec<String> {
    fts_fuzzy_terms(query)
        .iter()
        .map(|term| fts_alias_query(term))
        .collect()
}

fn comparison_terms(
    terms: &[String],
    queries: &[String],
    broad_search: bool,
) -> Vec<(String, String)> {
    if !broad_search {
        return Vec::new();
    }

    let mut seen = HashSet::new();
    let unique = terms
        .iter()
        .cloned()
        .zip(queries.iter().cloned())
        .filter(|(term, _)| seen.insert(term.to_lowercase()))
        .collect::<Vec<_>>();
    if (2..=MAX_COMPARISON_TERMS).contains(&unique.len()) {
        unique
    } else {
        Vec::new()
    }
}

/// Convert each user phrase to one safely quoted FTS5 phrase. The larger bound
/// preserves normal quotations while keeping pathological pasted input finite.
fn fts_phrase_queries(phrases: &[String]) -> Vec<String> {
    phrases
        .iter()
        .filter_map(|phrase| {
            let terms = fts_terms(phrase, 128);
            (!terms.is_empty()).then(|| fts_alias_query(&terms.join(" ")))
        })
        .collect()
}

fn fts_terms(query: &str, limit: usize) -> Vec<String> {
    collapse_hyphen_spacing(query)
        .split_whitespace()
        .map(|part| part.trim_matches(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '-'))
        .filter(|part| !part.is_empty())
        .take(limit)
        .map(|term| term.replace('"', ""))
        .collect()
}

fn normalize_exact_text(text: &str) -> String {
    normalize_exact_display(text).to_lowercase()
}

fn normalize_exact_display(text: &str) -> String {
    let punctuation = text
        .replace(['\u{2018}', '\u{2019}'], "'")
        .replace(['\u{201c}', '\u{201d}'], "\"");
    remove_internal_word_hyphens(&collapse_hyphen_spacing(&punctuation))
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Treat each of the first four internal hyphens independently as punctuation
/// or a PDF line wrap. The cap prevents pasted input from expanding into an
/// unbounded FTS expression while covering normal multi-compound phrases.
fn fts_alias_query(text: &str) -> String {
    let aliases = internal_hyphen_aliases(text);
    if aliases.len() == 1 {
        return quote_fts_term(text);
    }
    format!(
        "({})",
        aliases
            .iter()
            .map(|alias| quote_fts_term(alias))
            .collect::<Vec<_>>()
            .join(" OR ")
    )
}

fn internal_hyphen_aliases(text: &str) -> Vec<String> {
    const MAX_VARIABLE_HYPHENS: usize = 4;

    let characters = text.chars().collect::<Vec<_>>();
    let positions = characters
        .iter()
        .enumerate()
        .filter_map(|(index, character)| {
            (*character == '-'
                && index > 0
                && index + 1 < characters.len()
                && characters[index - 1].is_alphabetic()
                && characters[index + 1].is_alphabetic())
            .then_some(index)
        })
        .take(MAX_VARIABLE_HYPHENS)
        .collect::<Vec<_>>();
    if positions.is_empty() {
        return vec![text.to_string()];
    }

    let mut aliases = Vec::with_capacity(1 << positions.len());
    for mask in 0..(1 << positions.len()) {
        let alias = characters
            .iter()
            .enumerate()
            .filter_map(|(index, character)| {
                let position = positions.iter().position(|candidate| *candidate == index);
                let removed = position.is_some_and(|position| mask & (1 << position) != 0);
                (!removed).then_some(*character)
            })
            .collect::<String>();
        if !aliases.contains(&alias) {
            aliases.push(alias);
        }
    }

    let fully_joined = remove_internal_word_hyphens(text);
    if !aliases.contains(&fully_joined) {
        aliases.push(fully_joined);
    }
    aliases
}

/// Collapse only whitespace between a letter-ending hyphen and the next
/// letter, turning copied PDF text such as `high- lights` into one query term.
fn collapse_hyphen_spacing(text: &str) -> String {
    let characters = text.chars().collect::<Vec<_>>();
    let mut output = String::with_capacity(text.len());
    let mut index = 0;

    while index < characters.len() {
        let character = characters[index];
        let follows_letter = output.chars().last().is_some_and(char::is_alphabetic);
        output.push(character);
        index += 1;
        if character != '-' || !follows_letter {
            continue;
        }

        let whitespace_start = index;
        while index < characters.len() && characters[index].is_whitespace() {
            index += 1;
        }
        if index == whitespace_start
            || index >= characters.len()
            || !characters[index].is_alphabetic()
        {
            for character in &characters[whitespace_start..index] {
                output.push(*character);
            }
        }
    }

    output
}

/// Build the canonical alias used by dehyphenated PDF search projections.
fn remove_internal_word_hyphens(text: &str) -> String {
    let characters = text.chars().collect::<Vec<_>>();
    characters
        .iter()
        .enumerate()
        .filter_map(|(index, character)| {
            let internal_hyphen = *character == '-'
                && index > 0
                && index + 1 < characters.len()
                && characters[index - 1].is_alphabetic()
                && characters[index + 1].is_alphabetic();
            (!internal_hyphen).then_some(*character)
        })
        .collect()
}

fn fts_and_query(queries: &[String]) -> String {
    queries.join(" AND ")
}

fn fts_or_query(queries: &[String]) -> String {
    queries.join(" OR ")
}

fn quote_fts_term(term: &str) -> String {
    format!("\"{}\"", term.replace('"', ""))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use rusqlite::{params, Connection};

    use super::{
        attach_exact_phrase_evidence, attach_search_term_matches, comparison_terms,
        count_normalized_matches, document_candidates, document_concordance,
        document_ids_matching_all_queries, fts_and_query, fts_fuzzy_queries, fts_fuzzy_terms,
        fts_or_query, fts_phrase_queries, normalize_exact_text, retain_exact_phrase_candidates,
        retain_exact_phrase_hits, search_cross_section_document_hits,
        search_results_for_candidates, search_section_hits,
    };

    #[test]
    fn pdf_find_counts_literal_matches_after_pdf_hyphen_normalization() {
        let query = normalize_exact_text("high-lights");
        assert_eq!(
            count_normalized_matches("Highlights and high- lights.", &query),
            2
        );
    }

    #[test]
    fn fuzzy_search_falls_back_when_terms_live_in_different_sections() {
        let db = test_db();
        insert_document(
            &db,
            "sample-cross-section",
            "/uploads/sample-cross-section.html",
            "Cross Section Sample",
            &[
                "The north archive mentions a silver compass.",
                "The south appendix records a copper lantern.",
            ],
        );

        let queries = fts_fuzzy_queries("compass lantern");
        let same_section = search_section_hits(&db, &fts_and_query(&queries), 10, &[])
            .expect("same-section search");
        assert!(same_section.is_empty());

        let excluded = HashSet::new();
        let fallback = search_cross_section_document_hits(&db, &queries, 10, &[], &excluded)
            .expect("cross-section fallback");

        assert_eq!(fallback.len(), 1);
        assert_eq!(fallback[0].document_id, "sample-cross-section");
        assert_eq!(fallback[0].match_scope, "document");
    }

    #[test]
    fn fuzzy_search_keeps_hyphenated_terms_together() {
        let db = test_db();
        insert_document(
            &db,
            "compound-hit",
            "/uploads/compound-hit.html",
            "Compound Hit",
            &["The workshop praised the well-made astrolabe."],
        );
        insert_document(
            &db,
            "scattered-words",
            "/uploads/scattered-words.html",
            "Scattered Words",
            &[
                "The first note mentions something well.",
                "The second note says the tool was made elsewhere.",
                "The third note studies astrolabe diagrams.",
            ],
        );

        let terms = fts_fuzzy_terms("well-made astrolabe");
        assert_eq!(terms, vec!["well-made", "astrolabe"]);
        let queries = fts_fuzzy_queries("well-made astrolabe");

        let same_section = search_section_hits(&db, &fts_and_query(&queries), 10, &[])
            .expect("same-section search");
        let excluded = same_section
            .iter()
            .map(|result| result.document_id.clone())
            .collect::<HashSet<_>>();
        let fallback = search_cross_section_document_hits(&db, &queries, 10, &[], &excluded)
            .expect("cross-section fallback");

        assert_eq!(same_section.len(), 1);
        assert_eq!(same_section[0].document_id, "compound-hit");
        assert!(fallback.is_empty());
    }

    #[test]
    fn cross_section_fallback_ranks_before_limiting() {
        let db = test_db();
        insert_document_with_imported_at(
            &db,
            "aaa-older-match",
            "/uploads/aaa-older-match.html",
            "Older Match",
            &[
                "The archive mentions a silver compass.",
                "The appendix records a copper lantern.",
            ],
            100,
        );
        insert_document_with_imported_at(
            &db,
            "zzz-newer-match",
            "/uploads/zzz-newer-match.html",
            "Newer Match",
            &[
                "The archive mentions a silver compass.",
                "The appendix records a copper lantern.",
            ],
            200,
        );

        let queries = fts_fuzzy_queries("compass lantern");
        let excluded = HashSet::new();
        let fallback = search_cross_section_document_hits(&db, &queries, 1, &[], &excluded)
            .expect("cross-section fallback");

        assert_eq!(fallback.len(), 1);
        assert_eq!(fallback[0].document_id, "zzz-newer-match");
    }

    #[test]
    fn fuzzy_terms_trim_edge_punctuation_but_keep_compounds() {
        assert_eq!(
            fts_fuzzy_terms("(well-made) lantern, archive!"),
            vec!["well-made", "lantern", "archive"]
        );
    }

    #[test]
    fn fuzzy_search_accepts_joined_and_explicitly_hyphenated_forms() {
        let db = test_db();
        insert_document(
            &db,
            "dehyphenated-pdf",
            "/uploads/dehyphenated-pdf.pdf",
            "Dehyphenated PDF",
            &["The page highlights a result."],
        );

        for query in ["highlights", "high-lights", "high- lights"] {
            let queries = fts_fuzzy_queries(query);
            let hits = search_section_hits(&db, &fts_and_query(&queries), 10, &[])
                .expect("hyphen-equivalent search");
            assert_eq!(hits.len(), 1, "query: {query}");
        }
    }

    #[test]
    fn exact_normalization_accepts_hyphen_equivalent_forms() {
        let expected = "the highlights remain";
        for text in [
            "The highlights remain",
            "The high-lights remain",
            "The high- lights remain",
        ] {
            assert_eq!(normalize_exact_text(text), expected);
        }
    }

    #[test]
    fn exact_search_accepts_joined_and_explicitly_hyphenated_forms() {
        let db = test_db();
        insert_document(
            &db,
            "dehyphenated-exact-pdf",
            "/uploads/dehyphenated-exact-pdf.pdf",
            "Dehyphenated Exact PDF",
            &["The page highlights the final result."],
        );

        for phrase in [
            "highlights the final",
            "high-lights the final",
            "high- lights the final",
        ] {
            let queries = fts_phrase_queries(&[phrase.to_string()]);
            let candidates = search_section_hits(&db, &fts_and_query(&queries), 10, &[])
                .expect("hyphen-equivalent phrase candidates");
            let hits = retain_exact_phrase_hits(&db, candidates, &[phrase.to_string()])
                .expect("hyphen-equivalent exact verification");
            assert_eq!(hits.len(), 1, "phrase: {phrase}");
        }
    }

    #[test]
    fn exact_search_accepts_independently_hyphenated_compounds() {
        let db = test_db();
        insert_document(
            &db,
            "mixed-hyphen-exact-pdf",
            "/uploads/mixed-hyphen-exact-pdf.pdf",
            "Mixed Hyphen Exact PDF",
            &["The highlights remain stateowned."],
        );

        for phrase in [
            "highlights remain state-owned",
            "high-lights remain state-owned",
            "highlights remain stateowned",
            "high-lights remain stateowned",
        ] {
            let queries = fts_phrase_queries(&[phrase.to_string()]);
            let candidates = search_section_hits(&db, &fts_and_query(&queries), 10, &[])
                .expect("mixed hyphen phrase candidates");
            let hits = retain_exact_phrase_hits(&db, candidates, &[phrase.to_string()])
                .expect("mixed hyphen exact verification");
            assert_eq!(hits.len(), 1, "phrase: {phrase}");
        }
    }

    #[test]
    fn mixed_exact_search_returns_native_count_excerpt_and_phrase_locator() {
        let db = test_db();
        insert_document(
            &db,
            "mixed-exact-locator",
            "/uploads/mixed-exact-locator.html",
            "Mixed Exact Locator",
            &[
                "Anne writes a letter.",
                "The Green Gables orchard.",
                "Anne later returns to GREEN GABLES.",
            ],
        );
        insert_document(
            &db,
            "phrase-only",
            "/uploads/phrase-only.html",
            "Phrase Only",
            &["The Green Gables orchard has no required name."],
        );

        let phrases = vec!["green gables".to_string()];
        let mut queries = fts_fuzzy_queries("anne");
        queries.extend(fts_phrase_queries(&phrases));
        let document_ids = document_ids_matching_all_queries(&db, &queries, &[])
            .expect("documents matching every clause");
        let query = fts_or_query(&queries);
        let candidates = document_candidates(&db, &query, &[], Some(&document_ids), "document")
            .expect("mixed candidates");
        let mut verified = retain_exact_phrase_candidates(&db, candidates, &phrases)
            .expect("literal phrase verification");
        attach_exact_phrase_evidence(&db, &mut verified, &phrases)
            .expect("complete exact evidence");
        let results =
            search_results_for_candidates(&db, &query, &verified).expect("native exact evidence");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].section_index, 1);
        assert_eq!(results[0].match_count, Some(2));
        assert!(results[0].excerpt.contains("<mark>Green Gables</mark>"));
    }

    #[test]
    fn concordance_pages_literal_occurrences_with_exact_section_positions() {
        let db = test_db();
        insert_document(
            &db,
            "concordance",
            "/uploads/concordance.html",
            "Concordance",
            &["Orchard and orchard.", "No fruit here.", "Final orchard."],
        );

        let first = document_concordance(&db, "concordance", "orchard", 1, 1)
            .expect("second literal occurrence");
        assert_eq!(first.total_matches, 3);
        assert_eq!(first.next_offset, Some(2));
        assert_eq!(first.entries.len(), 1);
        assert_eq!(first.entries[0].occurrence_index, 1);
        assert_eq!(first.entries[0].section_index, 0);
        assert_eq!(first.entries[0].section_occurrence_index, 1);
        assert!(first.entries[0].excerpt.contains("<mark>orchard</mark>"));

        let remaining = document_concordance(&db, "concordance", "orchard", 2, 50)
            .expect("remaining literal occurrences");
        assert_eq!(remaining.entries.len(), 1);
        assert_eq!(remaining.entries[0].section_index, 2);
        assert_eq!(remaining.next_offset, None);
    }

    #[test]
    fn fuzzy_search_prefers_same_section_hits() {
        let db = test_db();
        insert_document(
            &db,
            "sample-same-section",
            "/uploads/sample-same-section.html",
            "Same Section Sample",
            &["The field notes mention a compass and lantern together."],
        );

        let queries = fts_fuzzy_queries("compass lantern");
        let same_section = search_section_hits(&db, &fts_and_query(&queries), 10, &[])
            .expect("same-section search");

        assert_eq!(same_section.len(), 1);
        assert_eq!(same_section[0].match_scope, "section");
    }

    #[test]
    fn section_hits_are_grouped_before_the_document_limit() {
        let db = test_db();
        insert_document(
            &db,
            "repeated-hit",
            "/uploads/repeated-hit.html",
            "Repeated Hit",
            &["orchard", "orchard", "orchard"],
        );
        insert_document(
            &db,
            "single-hit",
            "/uploads/single-hit.html",
            "Single Hit",
            &["orchard"],
        );

        let hits = search_section_hits(&db, "\"orchard\"", 2, &[]).expect("grouped search");

        assert_eq!(hits.len(), 2);
        assert_eq!(
            hits.iter()
                .find(|hit| hit.document_id == "repeated-hit")
                .map(|hit| hit.matching_sections),
            Some(3)
        );
    }

    #[test]
    fn search_evidence_bounds_passages_and_bins_match_locations() {
        let db = test_db();
        let sections = (0..24)
            .map(|index| format!("Orchard evidence passage {index}."))
            .collect::<Vec<_>>();
        let section_refs = sections.iter().map(String::as_str).collect::<Vec<_>>();
        insert_document(
            &db,
            "evidence-map",
            "/uploads/evidence-map.html",
            "Evidence Map",
            &section_refs,
        );

        let hits = search_section_hits(&db, "\"orchard\"", 1, &[]).expect("evidence search");
        let hit = &hits[0];

        assert_eq!(hit.passages.len(), 3);
        assert_eq!(hit.match_locations.len(), 12);
        assert_eq!(
            hit.match_locations
                .iter()
                .map(|location| location.match_count)
                .sum::<usize>(),
            24
        );
        assert_eq!(hit.match_locations[0].bin_index, 0);
        assert_eq!(hit.match_locations[11].bin_index, 11);
        assert!(hit
            .match_locations
            .iter()
            .all(|location| location.text.as_deref() == Some("Orchard")));
    }

    #[test]
    fn comparison_counts_each_term_and_keeps_its_first_source_locator() {
        let db = test_db();
        insert_document(
            &db,
            "comparison-map",
            "/uploads/comparison-map.html",
            "Comparison Map",
            &["Orchard notes.", "Lantern notes.", "Orchard appendix."],
        );

        let mut hits =
            search_section_hits(&db, "\"orchard\"", 1, &[]).expect("comparison candidate");
        let terms = fts_fuzzy_terms("orchard lantern");
        let queries = fts_fuzzy_queries("orchard lantern");
        attach_search_term_matches(&db, &comparison_terms(&terms, &queries, true), &mut hits)
            .expect("term evidence");

        assert_eq!(hits[0].term_matches.len(), 2);
        assert_eq!(hits[0].term_matches[0].term, "orchard");
        assert_eq!(hits[0].term_matches[0].matching_sections, 2);
        assert_eq!(hits[0].term_matches[0].section_index, Some(0));
        assert_eq!(hits[0].term_matches[0].text.as_deref(), Some("Orchard"));
        assert_eq!(hits[0].term_matches[1].matching_sections, 1);
        assert_eq!(hits[0].term_matches[1].section_index, Some(1));
        assert_eq!(hits[0].term_matches[1].text.as_deref(), Some("Lantern"));
    }

    #[test]
    fn candidate_counts_include_documents_beyond_the_visible_limit() {
        let db = test_db();
        for index in 0..3 {
            insert_document(
                &db,
                &format!("document-{index}"),
                &format!("/uploads/document-{index}.html"),
                &format!("Document {index}"),
                &["lantern"],
            );
        }

        let candidates = document_candidates(&db, "\"lantern\"", &[], None, "section")
            .expect("document candidates");

        assert_eq!(candidates.len(), 3);
        assert_eq!(
            search_section_hits(&db, "\"lantern\"", 2, &[])
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn repeated_fts_titles_do_not_inflate_matching_section_counts() {
        let db = test_db();
        insert_document(
            &db,
            "title-only-hit",
            "/uploads/title-only-hit.html",
            "Orchard Almanac",
            &["Apples", "Pears", "Plums"],
        );

        let hits = search_section_hits(&db, "\"orchard\"", 10, &[]).expect("title search");

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].matching_sections, 0);
    }

    #[test]
    fn pdf_search_hit_retains_its_page_index() {
        let db = test_db();
        insert_document(
            &db,
            "pdf-page-hit",
            "/uploads/pdf-page-hit.pdf",
            "PDF Sample",
            &["The indexed page mentions an astrolabe."],
        );
        db.execute(
            "UPDATE uploaded_sections SET page_index = 6, heading = NULL WHERE document_id = ?1",
            params!["pdf-page-hit"],
        )
        .expect("set PDF page locator");
        db.execute(
            "UPDATE uploaded_document_fts SET heading = NULL WHERE document_id = ?1",
            params!["pdf-page-hit"],
        )
        .expect("clear PDF page heading");

        let hits = search_section_hits(&db, "\"astrolabe\"", 10, &[]).expect("search PDF page");

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].page_index, Some(6));
        assert!(hits[0].excerpt.contains("<mark>astrolabe</mark>"));
    }

    #[test]
    fn committed_pdf_fixture_phrase_is_searchable_once_on_its_page() {
        let db = test_db();
        insert_document(
            &db,
            "pdf-fixture",
            "/uploads/pdf-fixture.pdf",
            "PDF Fixture",
            &["P01 BASIC LATIN P02 INLINE FORMAT PRESERVED P06 LEFT 1 P06 RIGHT 1"],
        );
        db.execute(
            "UPDATE uploaded_sections SET page_index = 0, heading = NULL WHERE document_id = ?1",
            params!["pdf-fixture"],
        )
        .expect("set fixture page locator");
        db.execute(
            "UPDATE uploaded_documents SET format = 'pdf' WHERE id = ?1",
            params!["pdf-fixture"],
        )
        .expect("mark fixture as PDF");
        db.execute(
            "UPDATE uploaded_document_fts SET heading = NULL WHERE document_id = ?1",
            params!["pdf-fixture"],
        )
        .expect("clear fixture page heading");

        let queries = fts_phrase_queries(&["inline format preserved".to_string()]);
        let candidates = search_section_hits(&db, &fts_and_query(&queries), 10, &[])
            .expect("fixture phrase candidates");
        let hits =
            retain_exact_phrase_hits(&db, candidates, &["inline format preserved".to_string()])
                .expect("verify fixture phrase");

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].page_index, Some(0));
    }

    #[test]
    fn phrase_search_rejects_words_with_an_intervening_token() {
        let db = test_db();
        insert_document(
            &db,
            "exact-phrase",
            "/uploads/exact-phrase.pdf",
            "Exact Phrase",
            &["The silver compass points north."],
        );
        insert_document(
            &db,
            "separated-phrase",
            "/uploads/separated-phrase.pdf",
            "Separated Phrase",
            &["The silver pocket compass points north."],
        );

        let queries = fts_phrase_queries(&["silver compass".to_string()]);
        let hits =
            search_section_hits(&db, &fts_and_query(&queries), 10, &[]).expect("phrase search");

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].document_id, "exact-phrase");
    }

    #[test]
    fn phrase_verifier_rejects_porter_stem_false_positives() {
        let db = test_db();
        insert_document(
            &db,
            "stemmed-candidate",
            "/uploads/stemmed-candidate.pdf",
            "Stemmed Candidate",
            &["The running compass points north."],
        );

        let queries = fts_phrase_queries(&["run compass".to_string()]);
        let candidates = search_section_hits(&db, &fts_and_query(&queries), 10, &[])
            .expect("stemmed FTS candidates");
        let verified = retain_exact_phrase_hits(&db, candidates, &["run compass".to_string()])
            .expect("literal phrase verification");

        assert!(verified.is_empty());
    }

    #[test]
    fn phrase_verifier_ignores_punctuation_only_phrases() {
        let db = test_db();
        insert_document(
            &db,
            "exact-phrase",
            "/uploads/exact-phrase.pdf",
            "Exact Phrase",
            &["The archive contains hello world."],
        );

        let queries = fts_phrase_queries(&["hello world".to_string(), "??".to_string()]);
        let candidates =
            search_section_hits(&db, &fts_and_query(&queries), 10, &[]).expect("phrase candidates");
        let verified = retain_exact_phrase_hits(
            &db,
            candidates,
            &["hello world".to_string(), "??".to_string()],
        )
        .expect("literal phrase verification");

        assert_eq!(verified.len(), 1);
        assert_eq!(verified[0].document_id, "exact-phrase");
    }

    fn test_db() -> Connection {
        let db = Connection::open_in_memory().expect("open test db");
        db.execute_batch(
            "CREATE TABLE uploaded_documents (
               id TEXT PRIMARY KEY,
               url TEXT NOT NULL UNIQUE,
               title TEXT NOT NULL,
               format TEXT NOT NULL,
               imported_at_ms INTEGER NOT NULL,
               bytes INTEGER NOT NULL,
               sections INTEGER NOT NULL
             );
             CREATE TABLE uploaded_sections (
               id INTEGER PRIMARY KEY AUTOINCREMENT,
               document_id TEXT NOT NULL,
               ordinal INTEGER NOT NULL,
               page_index INTEGER,
               heading TEXT,
               text TEXT NOT NULL
             );
             CREATE INDEX uploaded_sections_document_order_idx
               ON uploaded_sections(document_id, ordinal);
             CREATE VIRTUAL TABLE uploaded_document_fts USING fts5(
               document_id UNINDEXED,
               section_id UNINDEXED,
               title,
               heading,
               text,
               tokenize = 'porter unicode61 remove_diacritics 1'
             );",
        )
        .expect("create schema");
        db
    }

    fn insert_document(db: &Connection, id: &str, url: &str, title: &str, sections: &[&str]) {
        insert_document_with_imported_at(db, id, url, title, sections, 100);
    }

    fn insert_document_with_imported_at(
        db: &Connection,
        id: &str,
        url: &str,
        title: &str,
        sections: &[&str],
        imported_at_ms: i64,
    ) {
        db.execute(
            "INSERT INTO uploaded_documents (id, url, title, format, imported_at_ms, bytes, sections)
             VALUES (?1, ?2, ?3, 'epub', ?4, 10, ?5)",
            params![id, url, title, imported_at_ms, sections.len() as i64],
        )
        .expect("insert document");

        for (index, text) in sections.iter().enumerate() {
            db.execute(
                "INSERT INTO uploaded_sections (document_id, ordinal, heading, text)
                 VALUES (?1, ?2, ?3, ?4)",
                params![id, index as i64, format!("Section {}", index + 1), text],
            )
            .expect("insert section");
            let section_id = db.last_insert_rowid();
            db.execute(
                "INSERT INTO uploaded_document_fts (document_id, section_id, title, heading, text)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    id,
                    section_id,
                    title,
                    format!("Section {}", index + 1),
                    text
                ],
            )
            .expect("insert fts row");
        }
    }
}
