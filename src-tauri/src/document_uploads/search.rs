//! FTS5 query building and execution for uploaded documents.
//!
//! Read-only against the schema owned by [`super::store`]. Kept separate so
//! query/ranking behavior can evolve without touching the write path.

use std::collections::{HashMap, HashSet};

use rusqlite::{params, params_from_iter, types::Value, Connection, Row};
use tauri::Runtime;

use super::storage::{upload_reference_from_url, StoredSourceKind};
use super::store::{db_err, open_db};
use super::types::{
    UploadedDocumentSearchRequest, UploadedDocumentSearchResult, UploadedPdfFindPage,
    UploadedPdfFindRequest, UploadedPdfFindResult,
};

struct RankedDocumentHit {
    score: f64,
    imported_at_ms: i64,
    result: UploadedDocumentSearchResult,
}

/// Run an FTS5 MATCH query, joining hits back to their section and document and
/// returning BM25-ranked results with `<mark>`-highlighted snippets.
pub(crate) fn search_uploads<R: Runtime>(
    app: &tauri::AppHandle<R>,
    request: UploadedDocumentSearchRequest,
) -> Result<Vec<UploadedDocumentSearchResult>, String> {
    let fuzzy_queries = fts_fuzzy_queries(&request.query);
    if fuzzy_queries.is_empty() {
        return Ok(Vec::new());
    }
    let exact_phrases = request.exact_phrases.unwrap_or_default();
    let exact_queries = fts_phrase_queries(&exact_phrases);
    let mut queries = fuzzy_queries;
    queries.extend(exact_queries.iter().cloned());
    let query = fts_and_query(&queries);

    let db = open_db(app)?;
    let limit = request.limit.unwrap_or(50).clamp(1, 100) as i64;
    let document_urls = request
        .document_urls
        .unwrap_or_default()
        .into_iter()
        .filter(|url| !url.trim().is_empty())
        .collect::<Vec<_>>();

    let mut results = search_section_hits(&db, &query, limit, &document_urls)?;
    if queries.len() > 1 && results.len() < limit as usize {
        let existing_document_ids = results
            .iter()
            .map(|result| result.document_id.clone())
            .collect::<HashSet<_>>();
        results.extend(search_cross_section_document_hits(
            &db,
            &queries,
            limit - results.len() as i64,
            &document_urls,
            &existing_document_ids,
        )?);
    }
    if !exact_queries.is_empty() {
        results = retain_exact_phrase_hits(&db, results, &exact_phrases)?;
    }
    results.truncate(limit as usize);
    Ok(results)
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

/// Stream indexed sections in reading order and stop once every phrase has
/// appeared; memory stays bounded even when a candidate PDF has many pages.
fn document_contains_exact_phrases(
    db: &Connection,
    document_id: &str,
    phrases: &[String],
) -> Result<bool, String> {
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

fn search_section_hits(
    db: &Connection,
    query: &str,
    limit: i64,
    document_urls: &[String],
) -> Result<Vec<UploadedDocumentSearchResult>, String> {
    let (scope_sql, mut values) = document_url_scope(document_urls);
    let sql = format!(
        "SELECT d.id, d.url, d.title, s.ordinal, s.page_index, s.heading, \
                snippet(uploaded_document_fts, 4, '<mark>', '</mark>', '…', 18) AS excerpt \
         FROM uploaded_document_fts \
         JOIN uploaded_sections s ON s.id = uploaded_document_fts.section_id \
         JOIN uploaded_documents d ON d.id = uploaded_document_fts.document_id \
         WHERE uploaded_document_fts MATCH ? {scope_sql} \
         ORDER BY bm25(uploaded_document_fts), d.imported_at_ms DESC \
         LIMIT ?"
    );
    values.insert(0, Value::Text(query.to_string()));
    values.push(Value::Integer(limit));

    let mut stmt = db.prepare(&sql).map_err(db_err)?;
    let rows = stmt
        .query_map(params_from_iter(values.iter()), |row| {
            row_to_search_result(row, "section")
        })
        .map_err(db_err)?;

    rows.collect::<Result<Vec<_>, _>>().map_err(db_err)
}

/// Find documents where every term or quoted phrase exists somewhere, even when
/// no single section contains all of them. This keeps broad book searches useful
/// without loading complete PDF text into the WebView.
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
    let mut hits = Vec::new();
    for document_id in candidate_ids {
        if let Some(hit) = best_document_term_hit(db, &query, &document_id)? {
            hits.push(hit);
        }
    }
    hits.sort_by(|left, right| {
        left.score
            .total_cmp(&right.score)
            .then_with(|| right.imported_at_ms.cmp(&left.imported_at_ms))
    });
    Ok(hits
        .into_iter()
        .take(limit as usize)
        .map(|hit| hit.result)
        .collect())
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

fn best_document_term_hit(
    db: &Connection,
    query: &str,
    document_id: &str,
) -> Result<Option<RankedDocumentHit>, String> {
    let mut stmt = db
        .prepare(
            "SELECT d.id, d.url, d.title, s.ordinal, s.page_index, s.heading, \
                    snippet(uploaded_document_fts, 4, '<mark>', '</mark>', '…', 18) AS excerpt, \
                    bm25(uploaded_document_fts) AS score, d.imported_at_ms \
             FROM uploaded_document_fts \
             JOIN uploaded_sections s ON s.id = uploaded_document_fts.section_id \
             JOIN uploaded_documents d ON d.id = uploaded_document_fts.document_id \
             WHERE uploaded_document_fts MATCH ?1 AND d.id = ?2 \
             ORDER BY bm25(uploaded_document_fts), s.ordinal ASC \
             LIMIT 1",
        )
        .map_err(db_err)?;
    match stmt.query_row(params![query, document_id], |row| {
        Ok(RankedDocumentHit {
            score: row.get(7)?,
            imported_at_ms: row.get(8)?,
            result: row_to_search_result(row, "document")?,
        })
    }) {
        Ok(hit) => Ok(Some(hit)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(err) => Err(db_err(err)),
    }
}

fn row_to_search_result(
    row: &Row<'_>,
    match_scope: &str,
) -> rusqlite::Result<UploadedDocumentSearchResult> {
    let document_id: String = row.get(0)?;
    let section_index: i64 = row.get(3)?;
    Ok(UploadedDocumentSearchResult {
        id: format!("upload:{match_scope}:{document_id}:{section_index}"),
        document_id,
        url: row.get(1)?,
        title: row.get(2)?,
        section_index: section_index as usize,
        page_index: row
            .get::<_, Option<i64>>(4)?
            .map(|page_index| page_index as usize),
        section_title: row.get(5)?,
        excerpt: row.get(6)?,
        match_scope: match_scope.to_string(),
    })
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

/// Turn a broad/fuzzy query into safe FTS5 terms for candidate lookup.
fn fts_fuzzy_terms(query: &str) -> Vec<String> {
    fts_terms(query, 12)
}

fn fts_fuzzy_queries(query: &str) -> Vec<String> {
    fts_fuzzy_terms(query)
        .iter()
        .map(|term| fts_alias_query(term))
        .collect()
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
    let punctuation = text
        .replace('\u{2018}', "'")
        .replace('\u{2019}', "'")
        .replace('\u{201c}', "\"")
        .replace('\u{201d}', "\"");
    remove_internal_word_hyphens(&collapse_hyphen_spacing(&punctuation))
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
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
        count_normalized_matches, fts_and_query, fts_fuzzy_queries, fts_fuzzy_terms,
        fts_phrase_queries, normalize_exact_text, retain_exact_phrase_hits,
        search_cross_section_document_hits, search_section_hits,
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
