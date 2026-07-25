//! FTS5 query building and execution for uploaded documents.
//!
//! Read-only against the schema owned by [`super::store`]. Kept separate so
//! query/ranking behavior can evolve without touching the write path.

use std::collections::HashSet;

use rusqlite::{params, params_from_iter, types::Value, Connection, Row};
use tauri::Runtime;

use super::store::{db_err, open_db};
use super::types::{UploadedDocumentSearchRequest, UploadedDocumentSearchResult};

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
    let terms = fts_fuzzy_terms(&request.query);
    if terms.is_empty() {
        return Ok(Vec::new());
    }
    let query = fts_and_query(&terms);

    let db = open_db(app)?;
    let limit = request.limit.unwrap_or(50).clamp(1, 100) as i64;
    let document_urls = request
        .document_urls
        .unwrap_or_default()
        .into_iter()
        .filter(|url| !url.trim().is_empty())
        .collect::<Vec<_>>();

    let mut results = search_section_hits(&db, &query, limit, &document_urls)?;
    if terms.len() > 1 && results.len() < limit as usize {
        let existing_document_ids = results
            .iter()
            .map(|result| result.document_id.clone())
            .collect::<HashSet<_>>();
        results.extend(search_cross_section_document_hits(
            &db,
            &terms,
            limit - results.len() as i64,
            &document_urls,
            &existing_document_ids,
        )?);
    }
    results.truncate(limit as usize);
    Ok(results)
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
                snippet(uploaded_document_fts, 3, '<mark>', '</mark>', '…', 18) AS excerpt \
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

/// Find documents where every fuzzy term exists somewhere, even when no single
/// section contains all terms. This preserves strong same-section ranking first
/// while making broad EPUB searches behave like "these words are in this book".
fn search_cross_section_document_hits(
    db: &Connection,
    terms: &[String],
    limit: i64,
    document_urls: &[String],
    excluded_document_ids: &HashSet<String>,
) -> Result<Vec<UploadedDocumentSearchResult>, String> {
    if limit <= 0 {
        return Ok(Vec::new());
    }

    let mut candidate_ids = document_ids_matching_all_terms(db, terms, document_urls)?;
    candidate_ids.retain(|id| !excluded_document_ids.contains(id));

    let query = fts_or_query(terms);
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

fn document_ids_matching_all_terms(
    db: &Connection,
    terms: &[String],
    document_urls: &[String],
) -> Result<Vec<String>, String> {
    let mut intersection: Option<HashSet<String>> = None;

    for term in terms {
        let (scope_sql, mut values) = document_url_scope(document_urls);
        let sql = format!(
            "SELECT DISTINCT uploaded_document_fts.document_id \
             FROM uploaded_document_fts \
             JOIN uploaded_documents d ON d.id = uploaded_document_fts.document_id \
             WHERE uploaded_document_fts MATCH ? {scope_sql}"
        );
        values.insert(0, Value::Text(quote_fts_term(term)));
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
                    snippet(uploaded_document_fts, 3, '<mark>', '</mark>', '…', 18) AS excerpt, \
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

/// Turn a broad/fuzzy query into safe FTS5 terms. Exact phrase semantics live
/// in the frontend phrase verifier; this helper only builds candidate MATCH
/// terms for uploaded-document FTS lookup.
fn fts_fuzzy_terms(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(|part| part.trim_matches(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '-'))
        .filter(|part| !part.is_empty())
        .take(12)
        .map(|term| term.replace('"', ""))
        .collect()
}

fn fts_and_query(terms: &[String]) -> String {
    terms
        .iter()
        .map(|term| quote_fts_term(term))
        .collect::<Vec<_>>()
        .join(" AND ")
}

fn fts_or_query(terms: &[String]) -> String {
    terms
        .iter()
        .map(|term| quote_fts_term(term))
        .collect::<Vec<_>>()
        .join(" OR ")
}

fn quote_fts_term(term: &str) -> String {
    format!("\"{}\"", term.replace('"', ""))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use rusqlite::{params, Connection};

    use super::{
        fts_and_query, fts_fuzzy_terms, search_cross_section_document_hits, search_section_hits,
    };

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

        let terms = fts_fuzzy_terms("compass lantern");
        let same_section =
            search_section_hits(&db, &fts_and_query(&terms), 10, &[]).expect("same-section search");
        assert!(same_section.is_empty());

        let excluded = HashSet::new();
        let fallback = search_cross_section_document_hits(&db, &terms, 10, &[], &excluded)
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

        let same_section =
            search_section_hits(&db, &fts_and_query(&terms), 10, &[]).expect("same-section search");
        let excluded = same_section
            .iter()
            .map(|result| result.document_id.clone())
            .collect::<HashSet<_>>();
        let fallback = search_cross_section_document_hits(&db, &terms, 10, &[], &excluded)
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

        let terms = fts_fuzzy_terms("compass lantern");
        let excluded = HashSet::new();
        let fallback = search_cross_section_document_hits(&db, &terms, 1, &[], &excluded)
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
    fn fuzzy_search_prefers_same_section_hits() {
        let db = test_db();
        insert_document(
            &db,
            "sample-same-section",
            "/uploads/sample-same-section.html",
            "Same Section Sample",
            &["The field notes mention a compass and lantern together."],
        );

        let terms = fts_fuzzy_terms("compass lantern");
        let same_section =
            search_section_hits(&db, &fts_and_query(&terms), 10, &[]).expect("same-section search");

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
            "UPDATE uploaded_sections SET page_index = 6 WHERE document_id = ?1",
            params!["pdf-page-hit"],
        )
        .expect("set PDF page locator");

        let hits = search_section_hits(&db, "\"astrolabe\"", 10, &[]).expect("search PDF page");

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].page_index, Some(6));
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
