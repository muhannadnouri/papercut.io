use std::collections::HashSet;

use rusqlite::{params, Connection};

use super::query::{
    comparison_terms, fts_and_query, fts_fuzzy_queries, fts_fuzzy_terms, fts_or_query,
    fts_phrase_queries, normalize_exact_text,
};
use super::{
    attach_exact_phrase_evidence, attach_search_term_matches, count_normalized_matches,
    document_candidates, document_concordance, document_ids_matching_all_queries,
    highlighted_exact_excerpt, retain_exact_phrase_candidates, retain_exact_phrase_hits,
    search_cross_section_document_hits, search_results_for_candidates, search_section_hits,
};

#[test]
fn pdf_find_counts_literal_matches_after_pdf_hyphen_normalization() {
    let query = normalize_exact_text("high—lights");
    assert_eq!(
        count_normalized_matches("Highlights, high- lights, and high–lights.", &query),
        3
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
    let same_section =
        search_section_hits(&db, &fts_and_query(&queries), 10, &[]).expect("same-section search");
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

    let same_section =
        search_section_hits(&db, &fts_and_query(&queries), 10, &[]).expect("same-section search");
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

    for query in [
        "highlights",
        "high-lights",
        "high- lights",
        "high–lights",
        "high—lights",
    ] {
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
        "The high–lights remain",
        "The high—lights remain",
    ] {
        assert_eq!(normalize_exact_text(text), expected);
    }
}

#[test]
fn exact_excerpt_avoids_wrong_marks_when_lowercasing_expands_unicode() {
    let display = "İstanbul archive";
    let normalized = display.to_lowercase();
    let match_start = normalized.find("archive").expect("normalized match");
    let excerpt = highlighted_exact_excerpt(display, &normalized, "archive", match_start);

    assert!(excerpt.contains("archive"));
    assert!(!excerpt.contains("<mark>"));
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
    attach_exact_phrase_evidence(&db, &mut verified, &phrases).expect("complete exact evidence");
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
    let same_section =
        search_section_hits(&db, &fts_and_query(&queries), 10, &[]).expect("same-section search");

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

    let mut hits = search_section_hits(&db, "\"orchard\"", 1, &[]).expect("comparison candidate");
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

    let candidates =
        document_candidates(&db, "\"lantern\"", &[], None, "section").expect("document candidates");

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
    let hits = retain_exact_phrase_hits(&db, candidates, &["inline format preserved".to_string()])
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
    let hits = search_section_hits(&db, &fts_and_query(&queries), 10, &[]).expect("phrase search");

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
