//! SQLite search orchestration, ranking, and source-linked evidence for uploads.
//!
//! Read-only against the schema owned by [`super::store`]. User-query parsing
//! and FTS5 expression construction live in [`query`].

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use rusqlite::{params, params_from_iter, types::Value, Connection, Row};
use tauri::Runtime;

mod query;

use super::search_form;
use super::spelling;
use super::storage::{upload_reference_from_url, StoredSourceKind};
use super::store::{db_err, open_db};
use super::types::{
    UploadedDocumentConcordanceEntry, UploadedDocumentConcordanceRequest,
    UploadedDocumentConcordanceResponse, UploadedDocumentSearchLocation,
    UploadedDocumentSearchMode, UploadedDocumentSearchPassage, UploadedDocumentSearchRequest,
    UploadedDocumentSearchResponse, UploadedDocumentSearchResult, UploadedDocumentSearchStage,
    UploadedDocumentSearchTermMatch, UploadedPdfFindPage, UploadedPdfFindRequest,
    UploadedPdfFindResult,
};
use query::{
    comparison_terms, fts_alias_query, fts_and_query, fts_fuzzy_terms, fts_or_query,
    fts_phrase_queries, fts_terms, normalize_exact_display, normalize_exact_text,
};

const MAX_EVIDENCE_PASSAGES: usize = 3;
const OCCURRENCE_MAP_BINS: usize = 12;
const DEFAULT_CONCORDANCE_LIMIT: usize = 50;
const MAX_CONCORDANCE_LIMIT: usize = 100;

// FTS columns are document_id, section_id (both UNINDEXED), title, heading,
// text. Keep candidate and evidence passage ranking on the same score policy.
const BM25_SCORE_SQL: &str = "bm25(uploaded_document_fts, 0, 0, 5, 3, 1)";

#[derive(Clone, Copy, PartialEq, Eq)]
enum SearchIndex {
    Original,
    SearchForm,
}

impl SearchIndex {
    fn table(self) -> &'static str {
        match self {
            Self::Original => "uploaded_document_fts",
            Self::SearchForm => "uploaded_document_search_fts",
        }
    }
    fn score(self) -> &'static str {
        match self {
            Self::Original => BM25_SCORE_SQL,
            Self::SearchForm => "bm25(uploaded_document_search_fts, 0, 0, 5, 3, 1)",
        }
    }
}

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

/// Internal diagnostics; never add private queries or source text to logs/IPC.
#[derive(Default, serde::Serialize)]
struct SearchMeasurements {
    candidate_documents: usize,
    db_ms: f64,
    candidate_ms: f64,
    verification_ms: f64,
    exact_evidence_ms: f64,
    result_evidence_ms: f64,
    term_matches_ms: f64,
    result_ms: f64,
    total_ms: f64,
}

/// Run an FTS5 MATCH query, joining hits back to their section and document and
/// returning BM25-ranked results with `<mark>`-highlighted snippets.
pub(crate) fn search_uploads<R: Runtime>(
    app: &tauri::AppHandle<R>,
    request: UploadedDocumentSearchRequest,
    progress: impl FnMut(UploadedDocumentSearchStage),
) -> Result<UploadedDocumentSearchResponse, String> {
    search_uploads_with_db(request, || open_db(app), progress).map(|(response, _)| response)
}

/// Keep evaluation on the complete production path, including lazy database
/// opening, snapshot, phrase verification, result limits, and evidence.
fn search_uploads_with_db(
    request: UploadedDocumentSearchRequest,
    open: impl FnOnce() -> Result<Connection, String>,
    mut progress: impl FnMut(UploadedDocumentSearchStage),
) -> Result<(UploadedDocumentSearchResponse, SearchMeasurements), String> {
    let search_started = Instant::now();
    let fuzzy_terms = fts_fuzzy_terms(&request.query)
        .into_iter()
        .filter(|term| !search_form::normalize(term).is_empty())
        .collect::<Vec<_>>();
    let exact_phrases = request.exact_phrases.unwrap_or_default();
    let original_exact_queries = fts_phrase_queries(&exact_phrases);
    if fuzzy_terms.is_empty() && original_exact_queries.is_empty() {
        return Ok((empty_search_response(), SearchMeasurements::default()));
    }
    let db_started = Instant::now();
    let mut db = open()?;
    let db_ms = db_started.elapsed().as_secs_f64() * 1000.0;
    let wants_search_form = fuzzy_terms
        .iter()
        .chain(exact_phrases.iter())
        .any(|term| search_form::has_arabic(term));
    let index = if wants_search_form && search_form_ready(&db)? {
        SearchIndex::SearchForm
    } else {
        SearchIndex::Original
    };
    let fuzzy_queries = fuzzy_terms
        .iter()
        .map(|term| {
            fts_alias_query(&if index == SearchIndex::SearchForm {
                search_form::normalize(term)
            } else {
                term.clone()
            })
        })
        .collect::<Vec<_>>();
    let exact_queries = if index == SearchIndex::SearchForm {
        fts_phrase_queries(
            &exact_phrases
                .iter()
                .map(|phrase| search_form::normalize(phrase))
                .collect::<Vec<_>>(),
        )
    } else {
        original_exact_queries
    };
    let requested_broader = request.mode == UploadedDocumentSearchMode::Broader;
    let comparison_terms = comparison_terms(
        &fuzzy_terms,
        &fuzzy_queries,
        exact_phrases.is_empty() || requested_broader,
        requested_broader,
    );
    // One- and two-word queries retain the all-required policy. A single
    // common word is too weak a broader-search candidate.
    let broader = requested_broader && comparison_terms.len() >= 3;
    let mut queries = fuzzy_queries;
    queries.extend(exact_queries.iter().cloned());
    let query = fts_and_query(&queries);
    let or_query = fts_or_query(&queries);
    let section_query = if broader { &or_query } else { &query };

    // Keep candidate ranking, exact verification, and evidence on one SQLite
    // snapshot if an import, OCR update, or deletion commits concurrently.
    let tx = db.transaction().map_err(db_err)?;
    let limit = request.limit.unwrap_or(50).clamp(1, 100);
    let document_urls = request
        .document_urls
        .unwrap_or_default()
        .into_iter()
        .filter(|url| !url.trim().is_empty())
        .collect::<Vec<_>>();

    progress(UploadedDocumentSearchStage::FindingCandidates);
    let candidate_started = Instant::now();
    let (mut section_candidates, mut document_candidates, term_presence) = if broader {
        let presence = document_term_presence(&tx, &comparison_terms, &document_urls, index)?;
        let minimum = ((comparison_terms.len() + 1) / 2).max(2);
        let phrase_ids = (!exact_queries.is_empty())
            .then(|| document_ids_matching_all_queries(&tx, &exact_queries, &document_urls, index))
            .transpose()?
            .map(|ids| ids.into_iter().collect::<HashSet<_>>());
        let mut ids = presence
            .iter()
            .filter(|(id, terms)| {
                terms.iter().filter(|matched| **matched).count() >= minimum
                    && phrase_ids
                        .as_ref()
                        .is_none_or(|allowed| allowed.contains(*id))
            })
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>();
        ids.sort();
        let mut candidates =
            document_candidates(&tx, &or_query, &document_urls, Some(&ids), "section", index)?;
        // Stable sorting preserves BM25 and the existing import/ID tie breaks
        // within each document-level coverage tier.
        candidates.sort_by_key(|candidate| {
            std::cmp::Reverse(
                presence[&candidate.document_id]
                    .iter()
                    .filter(|matched| **matched)
                    .count(),
            )
        });
        (candidates, Vec::new(), Some(presence))
    } else {
        let section = document_candidates(&tx, &query, &document_urls, None, "section", index)?;
        let section_document_ids = section
            .iter()
            .map(|candidate| candidate.document_id.clone())
            .collect::<HashSet<_>>();
        let document = if queries.len() > 1 {
            let mut document_ids =
                document_ids_matching_all_queries(&tx, &queries, &document_urls, index)?;
            document_ids.retain(|id| !section_document_ids.contains(id));
            document_candidates(
                &tx,
                &or_query,
                &document_urls,
                Some(&document_ids),
                "document",
                index,
            )?
        } else {
            Vec::new()
        };
        (section, document, None)
    };
    let candidate_documents = section_candidates.len() + document_candidates.len();
    let candidate_ms = candidate_started.elapsed().as_secs_f64() * 1000.0;

    let verification_started = Instant::now();
    if !exact_queries.is_empty() {
        progress(UploadedDocumentSearchStage::VerifyingPhrases);
        section_candidates =
            retain_exact_phrase_candidates(&tx, section_candidates, &exact_phrases)?;
        document_candidates =
            retain_exact_phrase_candidates(&tx, document_candidates, &exact_phrases)?;
    }
    let verification_ms = verification_started.elapsed().as_secs_f64() * 1000.0;

    let total_documents = section_candidates.len() + document_candidates.len();
    let total_matching_sections = section_candidates
        .iter()
        .chain(&document_candidates)
        .map(|candidate| candidate.matching_sections)
        .sum();
    section_candidates.truncate(limit);
    let remaining = limit.saturating_sub(section_candidates.len());
    document_candidates.truncate(remaining);
    progress(UploadedDocumentSearchStage::BuildingResults);
    let result_started = Instant::now();
    let exact_evidence_started = Instant::now();
    if !exact_queries.is_empty() {
        attach_exact_phrase_evidence(&tx, &mut section_candidates, &exact_phrases)?;
        attach_exact_phrase_evidence(&tx, &mut document_candidates, &exact_phrases)?;
    }
    let exact_evidence_ms = exact_evidence_started.elapsed().as_secs_f64() * 1000.0;
    let result_evidence_started = Instant::now();
    let mut results = search_results_for_candidates(
        &tx,
        section_query,
        &section_candidates,
        index,
        &fuzzy_terms,
    )?;
    results.extend(search_results_for_candidates(
        &tx,
        &or_query,
        &document_candidates,
        index,
        &fuzzy_terms,
    )?);
    let result_evidence_ms = result_evidence_started.elapsed().as_secs_f64() * 1000.0;
    let term_matches_started = Instant::now();
    attach_search_term_matches(&tx, &comparison_terms, &mut results, index)?;
    if let Some(presence) = term_presence {
        for result in &mut results {
            if let Some(matched) = presence.get(&result.document_id) {
                for (term, matched) in result.term_matches.iter_mut().zip(matched) {
                    term.matched = Some(*matched);
                }
            }
        }
    }
    let term_matches_ms = term_matches_started.elapsed().as_secs_f64() * 1000.0;
    let suggested_query = if total_documents == 0
        && exact_phrases.is_empty()
        && request.mode == UploadedDocumentSearchMode::All
        && spelling_ready(&tx)?
    {
        suggest_spelling(&tx, &fuzzy_terms, &document_urls)?
    } else {
        None
    };
    let result_ms = result_started.elapsed().as_secs_f64() * 1000.0;
    tx.commit().map_err(db_err)?;
    let total_ms = search_started.elapsed().as_secs_f64() * 1000.0;
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
            total_ms,
        );
    }

    Ok((
        UploadedDocumentSearchResponse {
            results,
            total_documents,
            total_matching_sections,
            suggested_query,
        },
        SearchMeasurements {
            candidate_documents,
            db_ms,
            candidate_ms,
            verification_ms,
            exact_evidence_ms,
            result_evidence_ms,
            term_matches_ms,
            result_ms,
            total_ms,
        },
    ))
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
        suggested_query: None,
    }
}

fn search_form_ready(db: &Connection) -> Result<bool, String> {
    db.query_row(
        "SELECT value = '1' FROM upload_schema_metadata WHERE key = 'search_form_ready'",
        [],
        |row| row.get(0),
    )
    .map_err(db_err)
}

fn spelling_ready(db: &Connection) -> Result<bool, String> {
    db.query_row(
        "SELECT value = '1' FROM upload_schema_metadata WHERE key = 'spelling_ready'",
        [],
        |row| row.get(0),
    )
    .map_err(db_err)
}

/// Suggest one explicit retry only after a miss. Indexed authored words avoid
/// Porter stems; the prefix/length index bounds work on large libraries.
fn suggest_spelling(
    db: &Connection,
    terms: &[String],
    document_urls: &[String],
) -> Result<Option<String>, String> {
    if terms.is_empty()
        || terms.len() > 4
        || terms
            .iter()
            .any(|term| !term.bytes().all(|ch| ch.is_ascii_lowercase()))
    {
        return Ok(None);
    }
    let (scope_sql, scope_values) = document_url_scope(document_urls);
    for (index, term) in terms.iter().enumerate() {
        if !spelling::eligible_query_word(term) {
            continue;
        }
        let exact_sql = format!(
            "SELECT EXISTS(SELECT 1 FROM uploaded_spelling_terms t \
             JOIN uploaded_documents d ON d.id = t.document_id \
             WHERE t.term = ? {scope_sql})"
        );
        let mut exact_values = vec![Value::Text(term.clone())];
        exact_values.extend(scope_values.iter().cloned());
        let known: bool = db
            .query_row(&exact_sql, params_from_iter(exact_values.iter()), |row| {
                row.get(0)
            })
            .map_err(db_err)?;
        if known {
            continue;
        }

        let sql = format!(
            "SELECT t.term FROM uploaded_spelling_terms t \
             JOIN uploaded_documents d ON d.id = t.document_id \
             WHERE substr(t.term, 1, 2) = ? \
               AND length(t.term) BETWEEN ? AND ? {scope_sql} \
             GROUP BY t.term ORDER BY COUNT(DISTINCT t.document_id) DESC, t.term LIMIT 256"
        );
        let mut values = vec![
            Value::Text(term[..2].to_string()),
            Value::Integer((term.len() - 1) as i64),
            Value::Integer((term.len() + 1) as i64),
        ];
        values.extend(scope_values.iter().cloned());
        let mut stmt = db.prepare(&sql).map_err(db_err)?;
        let candidates = stmt
            .query_map(params_from_iter(values.iter()), |row| {
                row.get::<_, String>(0)
            })
            .map_err(db_err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(db_err)?;
        // ponytail: verify only the eight strongest one-edit candidates; use
        // a deletion index if measured recall requires more than this cap.
        for candidate in candidates
            .into_iter()
            .filter(|word| spelling::one_edit_apart(term, word))
            .take(8)
        {
            let mut retry = terms.to_vec();
            retry[index] = candidate;
            let queries = retry
                .iter()
                .map(|word| fts_alias_query(word))
                .collect::<Vec<_>>();
            if !document_ids_matching_all_queries(
                db,
                &queries,
                document_urls,
                SearchIndex::Original,
            )?
            .is_empty()
            {
                return Ok(Some(retry.join(" ")));
            }
        }
    }
    Ok(None)
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

/// Verify the complete ranked candidate set before the visible-result limit is
/// applied, so stemmed FTS false positives cannot crowd out later literal hits.
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

/// Highlight only when the display and case-folded copies have matching
/// character counts; expanding Unicode folds would otherwise mark wrong text.
fn highlighted_exact_excerpt(
    display: &str,
    normalized: &str,
    phrase: &str,
    match_byte_index: usize,
) -> String {
    const CONTEXT: usize = 120;

    let match_start = normalized[..match_byte_index].chars().count();
    let characters = display.chars().collect::<Vec<_>>();
    let offsets_align = characters.len() == normalized.chars().count();
    let highlight_start = match_start.min(characters.len());
    let highlight_end = (highlight_start + phrase.chars().count()).min(characters.len());
    let start = highlight_start.saturating_sub(CONTEXT);
    let end = (highlight_end + CONTEXT).min(characters.len());
    let mut excerpt = String::new();
    if start > 0 {
        excerpt.push_str("… ");
    }
    if offsets_align {
        excerpt.extend(characters[start..highlight_start].iter().copied());
        excerpt.push_str("<mark>");
        excerpt.extend(characters[highlight_start..highlight_end].iter().copied());
        excerpt.push_str("</mark>");
        excerpt.extend(characters[highlight_end..end].iter().copied());
    } else {
        excerpt.extend(characters[start..end].iter().copied());
    }
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
    let candidates = document_candidates(
        db,
        query,
        document_urls,
        None,
        "section",
        SearchIndex::Original,
    )?
    .into_iter()
    .take(limit.max(0) as usize)
    .collect::<Vec<_>>();
    search_results_for_candidates(db, query, &candidates, SearchIndex::Original, &[])
}

/// Collapse section hits to one ranked candidate per document, retaining the
/// best BM25 section as the opening target and a separate complete section count.
fn document_candidates(
    db: &Connection,
    query: &str,
    document_urls: &[String],
    document_ids: Option<&[String]>,
    match_scope: &'static str,
    index: SearchIndex,
) -> Result<Vec<RankedDocumentCandidate>, String> {
    if document_ids.is_some_and(|ids| ids.is_empty()) {
        return Ok(Vec::new());
    }

    let (url_scope_sql, mut values) = document_url_scope(document_urls);
    let (id_scope_sql, id_values) = document_id_scope(document_ids, index);
    let table = index.table();
    let score = index.score();
    let sql = format!(
        "SELECT {table}.document_id, \
                CAST({table}.section_id AS INTEGER), s.ordinal, \
                {score}, d.imported_at_ms, d.sections \
         FROM {table} \
         JOIN uploaded_sections s ON s.id = {table}.section_id \
         JOIN uploaded_documents d ON d.id = {table}.document_id \
         WHERE {table} MATCH ? {url_scope_sql} {id_scope_sql}"
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

    for (document_id, count) in
        matching_section_counts(db, query, document_urls, document_ids, index)?
    {
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

/// Count text/heading matches only. Title-only FTS hits still produce a
/// document candidate but deliberately report zero matching body sections.
fn matching_section_counts(
    db: &Connection,
    query: &str,
    document_urls: &[String],
    document_ids: Option<&[String]>,
    index: SearchIndex,
) -> Result<HashMap<String, usize>, String> {
    let (url_scope_sql, mut values) = document_url_scope(document_urls);
    let (id_scope_sql, id_values) = document_id_scope(document_ids, index);
    let table = index.table();
    let sql = format!(
        "SELECT {table}.document_id, COUNT(*) \
         FROM {table} \
         JOIN uploaded_documents d ON d.id = {table}.document_id \
         WHERE {table} MATCH ? {url_scope_sql} {id_scope_sql} \
         GROUP BY {table}.document_id"
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

    let mut candidate_ids =
        document_ids_matching_all_queries(db, queries, document_urls, SearchIndex::Original)?;
    candidate_ids.retain(|id| !excluded_document_ids.contains(id));

    let query = fts_or_query(queries);
    let candidates = document_candidates(
        db,
        &query,
        document_urls,
        Some(&candidate_ids),
        "document",
        SearchIndex::Original,
    )?
    .into_iter()
    .take(limit as usize)
    .collect::<Vec<_>>();
    search_results_for_candidates(db, &query, &candidates, SearchIndex::Original, &[])
}

/// Intersect per-clause document sets so required terms may live in different
/// sections without weakening the all-clauses-required query contract.
fn document_ids_matching_all_queries(
    db: &Connection,
    queries: &[String],
    document_urls: &[String],
    index: SearchIndex,
) -> Result<Vec<String>, String> {
    let mut intersection: Option<HashSet<String>> = None;

    for query in queries {
        let (scope_sql, mut values) = document_url_scope(document_urls);
        let table = index.table();
        let sql = format!(
            "SELECT DISTINCT {table}.document_id \
             FROM {table} \
             JOIN uploaded_documents d ON d.id = {table}.document_id \
             WHERE {table} MATCH ? {scope_sql}"
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

/// Record distinct query-term presence at document scope; section rows can
/// satisfy different words without pretending a single passage contains all.
fn document_term_presence(
    db: &Connection,
    terms: &[(String, String)],
    document_urls: &[String],
    search_index: SearchIndex,
) -> Result<HashMap<String, Vec<bool>>, String> {
    let mut presence = HashMap::<String, Vec<bool>>::new();
    for (index, (_, query)) in terms.iter().enumerate() {
        for id in document_ids_matching_all_queries(
            db,
            std::slice::from_ref(query),
            document_urls,
            search_index,
        )? {
            presence
                .entry(id)
                .or_insert_with(|| vec![false; terms.len()])[index] = true;
        }
    }
    Ok(presence)
}

/// Rehydrate the already-ranked opening section for each visible document.
/// Exact evidence replaces the broad snippet/locator; broad results receive
/// their bounded passage and occurrence-map evidence here.
fn search_results_for_candidates(
    db: &Connection,
    query: &str,
    candidates: &[RankedDocumentCandidate],
    index: SearchIndex,
    terms: &[String],
) -> Result<Vec<UploadedDocumentSearchResult>, String> {
    if candidates.is_empty() {
        return Ok(Vec::new());
    }

    let placeholders = (0..candidates.len())
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(", ");
    let table = index.table();
    let source_column = if index == SearchIndex::SearchForm {
        "s.text"
    } else {
        "NULL"
    };
    let sql = format!(
        "SELECT d.id, d.url, d.title, s.ordinal, s.page_index, s.heading, \
                snippet({table}, 4, '<mark>', '</mark>', '…', 18) AS excerpt, \
                CAST({table}.section_id AS INTEGER), {source_column} \
         FROM {table} \
         JOIN uploaded_sections s ON s.id = {table}.section_id \
         JOIN uploaded_documents d ON d.id = {table}.document_id \
         WHERE {table} MATCH ? \
           AND {table}.section_id IN ({placeholders})"
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
            row_to_search_result(row, candidate, index, terms)
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
        attach_search_evidence(db, query, candidates, &mut results, index, terms)?;
    }
    Ok(results)
}

fn row_to_search_result(
    row: &Row<'_>,
    candidate: &RankedDocumentCandidate,
    index: SearchIndex,
    terms: &[String],
) -> rusqlite::Result<UploadedDocumentSearchResult> {
    let document_id: String = row.get(0)?;
    let broad_section_index = row.get::<_, i64>(3)? as usize;
    let broad_page_index = row
        .get::<_, Option<i64>>(4)?
        .map(|page_index| page_index as usize);
    let broad_section_title = row.get::<_, Option<String>>(5)?;
    let broad_excerpt = if index == SearchIndex::SearchForm {
        source_excerpt(&row.get::<_, String>(8)?, terms, &row.get::<_, String>(6)?)
    } else {
        row.get::<_, String>(6)?
    };
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
    index: SearchIndex,
    terms: &[String],
) -> Result<(), String> {
    let placeholders = (0..candidates.len())
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(", ");
    let table = index.table();
    let score = index.score();
    let source_column = if index == SearchIndex::SearchForm {
        "s.text"
    } else {
        "NULL"
    };
    let sql = format!(
        "SELECT {table}.document_id, s.ordinal, s.page_index, s.heading, \
                snippet({table}, 4, '<mark>', '</mark>', '…', 18), \
                {score}, {source_column} \
         FROM {table} \
         JOIN uploaded_sections s ON s.id = {table}.section_id \
         WHERE {table} MATCH ? \
           AND {table}.document_id IN ({placeholders}) \
         ORDER BY {table}.document_id, s.ordinal"
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
                row.get::<_, Option<String>>(6)?,
            ))
        })
        .map_err(db_err)?;
    for row in rows {
        let (document_id, section_index, page_index, section_title, excerpt, score, source) =
            row.map_err(db_err)?;
        let excerpt = if let Some(source) = source {
            source_excerpt(&source, terms, &excerpt)
        } else {
            excerpt
        };
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

// ponytail: scan a matched source section to recover authored characters;
// persist offsets only if the large-library evidence latency gate fails.
fn source_excerpt(source: &str, terms: &[String], indexed_snippet: &str) -> String {
    let chars = source.chars().collect::<Vec<_>>();
    let span = terms
        .iter()
        .find_map(|term| search_form::source_span(source, &search_form::normalize(term)))
        .or_else(|| {
            first_marked_text(indexed_snippet)
                .and_then(|term| search_form::source_span(source, &term))
        });
    let Some((match_start, match_end)) = span else {
        return chars.iter().take(240).collect();
    };
    let start = match_start.saturating_sub(120);
    let end = (match_end + 120).min(chars.len());
    let mut excerpt = String::new();
    if start > 0 {
        excerpt.push_str("… ");
    }
    excerpt.extend(chars[start..match_start].iter());
    excerpt.push_str("<mark>");
    excerpt.extend(chars[match_start..match_end].iter());
    excerpt.push_str("</mark>");
    excerpt.extend(chars[match_end..end].iter());
    if end < chars.len() {
        excerpt.push_str(" …");
    }
    excerpt
}

/// Add comparison counts only for small broad queries and visible results.
/// SQLite aggregates each term to one row per document, keeping IPC and Rust
/// memory bounded even when a common term occurs on many pages.
fn attach_search_term_matches(
    db: &Connection,
    terms: &[(String, String)],
    results: &mut [UploadedDocumentSearchResult],
    index: SearchIndex,
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
                matched: None,
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
    let table = index.table();
    let (source_columns, source_join) = if index == SearchIndex::SearchForm {
        (
            "s.text, s.heading",
            "JOIN uploaded_sections s ON s.id = matches.section_id",
        )
    } else {
        ("'', NULL", "")
    };
    let sql = format!(
        "WITH matches AS (\
           SELECT {table}.document_id, \
                  CAST({table}.section_id AS INTEGER) AS section_id, \
                  s.ordinal, s.page_index, \
                  COUNT(*) OVER (PARTITION BY {table}.document_id) AS match_count, \
                  ROW_NUMBER() OVER (\
                    PARTITION BY {table}.document_id ORDER BY s.ordinal\
                  ) AS match_number \
           FROM {table} \
           JOIN uploaded_sections s ON s.id = {table}.section_id \
           WHERE {table} MATCH ? \
             AND {table}.document_id IN ({placeholders})\
         ) \
         SELECT matches.document_id, matches.ordinal, matches.page_index, matches.match_count, \
                snippet({table}, -1, '<mark>', '</mark>', '…', 18), {source_columns} \
         FROM matches \
         JOIN {table} \
           ON CAST({table}.section_id AS INTEGER) = matches.section_id \
         {source_join} \
         WHERE matches.match_number = 1 AND {table} MATCH ?"
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
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                ))
            })
            .map_err(db_err)?;
        for row in rows {
            let (
                document_id,
                section_index,
                page_index,
                matching_sections,
                excerpt,
                source,
                heading,
            ) = row.map_err(db_err)?;
            let Some(result_index) = result_indexes.get(&document_id) else {
                continue;
            };
            let term_match = &mut results[*result_index].term_matches[term_index];
            term_match.matching_sections = matching_sections;
            term_match.section_index = Some(section_index);
            term_match.page_index = page_index;
            term_match.text = if index == SearchIndex::SearchForm {
                let term = &terms[term_index].0;
                let normalized = search_form::normalize(term);
                search_form::source_span(&source, &normalized)
                    .map(|(start, end)| source.chars().skip(start).take(end - start).collect())
                    .or_else(|| {
                        heading.as_deref().and_then(|heading| {
                            search_form::source_span(heading, &normalized).map(|(start, end)| {
                                heading.chars().skip(start).take(end - start).collect()
                            })
                        })
                    })
                    .or_else(|| {
                        first_marked_text(&excerpt).and_then(|text| {
                            search_form::source_span(&source, &text).map(|(start, end)| {
                                source.chars().skip(start).take(end - start).collect()
                            })
                        })
                    })
            } else {
                first_marked_text(&excerpt)
            };
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

fn document_id_scope(document_ids: Option<&[String]>, index: SearchIndex) -> (String, Vec<Value>) {
    let Some(document_ids) = document_ids else {
        return (String::new(), Vec::new());
    };
    let placeholders = (0..document_ids.len())
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(", ");
    (
        format!("AND {}.document_id IN ({placeholders})", index.table()),
        document_ids.iter().cloned().map(Value::Text).collect(),
    )
}

#[cfg(test)]
#[path = "search/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "search/evaluation.rs"]
mod evaluation;
