//! Small, deterministic search baseline using the real store and search path.

use std::collections::HashSet;
use std::path::Path;

use rusqlite::{Connection, OpenFlags};
use serde::Deserialize;
use serde_json::json;

use super::super::parsed::{ParsedDocument, ParsedSection};
use super::super::storage::StoredSourceKind;
use super::super::store::{open_db_in, upsert_document, PdfTextStatus};
use super::super::types::UploadedDocumentSearchMode;
use super::super::types::UploadedDocumentSearchRequest;
use super::search_uploads_with_db;

#[derive(Deserialize)]
struct LibraryQuery {
    id: String,
    request: UploadedDocumentSearchRequest,
}

fn open_read_only(path: &Path) -> Result<Connection, String> {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| error.to_string())
}

fn snapshot_library(source: &Path, destination: &Path) {
    open_read_only(source)
        .expect("open library read-only")
        .execute(
            "VACUUM INTO ?1",
            [destination.to_str().expect("snapshot path")],
        )
        .expect("copy consistent library snapshot without migrations");
}

fn library_query_results(path: &Path, queries: Vec<LibraryQuery>) -> Vec<serde_json::Value> {
    let mut seen = HashSet::new();
    queries
        .into_iter()
        .map(|case| {
            assert!(
                !case.id.is_empty() && seen.insert(case.id.clone()),
                "unique query IDs required"
            );
            let mode = case.request.mode;
            let limit = case.request.limit;
            let scope = case.request.document_urls.clone();
            let (response, measurements) =
                search_uploads_with_db(case.request, || open_read_only(path), |_| {})
                    .expect("production search on read-only snapshot");
            let retry = response.suggested_query.as_ref().map(|suggestion| {
                let (response, measurements) = search_uploads_with_db(
                    UploadedDocumentSearchRequest {
                        query: suggestion.clone(),
                        mode,
                        limit,
                        document_urls: scope,
                        exact_phrases: None,
                    },
                    || open_read_only(path),
                    |_| {},
                )
                .expect("production spelling retry with the original scope");
                json!({"ids": response.results.iter().map(|hit| &hit.document_id).collect::<Vec<_>>(),
                    "response": response, "measurements": measurements})
            });
            json!({"id": case.id,
            "ids": response.results.iter().map(|hit| &hit.document_id).collect::<Vec<_>>(),
            "response": response, "measurements": measurements, "retry": retry})
        })
        .collect()
}

fn embedding_documents(db: &Connection, queries: &[LibraryQuery]) -> Vec<serde_json::Value> {
    let scoped = queries
        .iter()
        .all(|case| case.request.document_urls.is_some());
    let urls = queries
        .iter()
        .flat_map(|case| case.request.document_urls.iter().flatten())
        .collect::<HashSet<_>>();
    let mut stmt = db
        .prepare("SELECT id, title, url FROM uploaded_documents ORDER BY id")
        .unwrap();
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .unwrap();
    let mut exported = Vec::new();
    for row in rows {
        let (id, title, url) = row.unwrap();
        if scoped && !urls.contains(&url) {
            continue;
        }
        let mut sections = db.prepare("SELECT ordinal, page_index, heading, text FROM uploaded_sections WHERE document_id = ?1 ORDER BY ordinal").unwrap();
        let parts = sections
            .query_map([&id], |row| {
                Ok((
                    row.get::<_, usize>(0)?,
                    row.get::<_, Option<usize>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .unwrap();
        let parts = parts
            .enumerate()
            .map(|(index, row)| {
                let (ordinal, page, heading, text) = row.unwrap();
                assert_eq!(
                    ordinal, index,
                    "source section indices must match exported anchors"
                );
                json!({"page_index": page, "heading": heading, "text": text})
            })
            .collect::<Vec<_>>();
        exported.push(json!({"id": id, "title": title, "url": url, "sections": parts}));
    }
    exported
}

#[test]
#[ignore = "requires private library, parsed requests, snapshot and output paths"]
fn search_v2_library_evaluation() {
    let source = std::env::var("PAPERCUT_SEARCH_EVAL_DB").expect("library path");
    let snapshot = std::env::var("PAPERCUT_SEARCH_EVAL_SNAPSHOT").expect("snapshot path");
    let requests = std::env::var("PAPERCUT_SEARCH_EVAL_REQUESTS").expect("parsed requests path");
    let output = std::env::var("PAPERCUT_SEARCH_EVAL_OUTPUT").expect("private output path");
    let queries: Vec<LibraryQuery> =
        serde_json::from_slice(&std::fs::read(requests).unwrap()).unwrap();
    assert!(!queries.is_empty(), "query list must not be empty");
    snapshot_library(Path::new(&source), Path::new(&snapshot));
    let db = open_read_only(Path::new(&snapshot)).unwrap();
    let documents: usize = db
        .query_row("SELECT COUNT(*) FROM uploaded_documents", [], |row| {
            row.get(0)
        })
        .unwrap();
    let sections: usize = db
        .query_row("SELECT COUNT(*) FROM uploaded_sections", [], |row| {
            row.get(0)
        })
        .unwrap();
    if let Ok(path) = std::env::var("PAPERCUT_SEARCH_EVAL_CORPUS") {
        let exported = embedding_documents(&db, &queries);
        serde_json::to_writer(
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .unwrap(),
            &exported,
        )
        .unwrap();
    }
    drop(db);
    let report = json!({"documents": documents, "sections": sections,
        "build": if cfg!(debug_assertions) { "debug" } else { "release" },
        "method": "Production parser and uploaded-search pipeline on one read-only SQLite snapshot; no schema migration. Direct results and explicit spelling retries are separate. No relevance grades inferred.",
        "queries": library_query_results(Path::new(&snapshot), queries)});
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .unwrap();
    serde_json::to_writer_pretty(file, &report).unwrap();
}

#[test]
fn library_evaluation_preserves_source_quotes_scope_and_spelling_retry_read_only() {
    let root = std::env::temp_dir().join(format!(
        "papercut-library-check-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut db = open_db_in(&root).unwrap();
    for (id, text) in [
        (
            "e100",
            "The guide calls it “silver compass” for navigation.",
        ),
        ("e101", "The guide calls it silver compass for navigation."),
    ] {
        let parsed = ParsedDocument {
            source_formatting: None,
            title: "Navigation guide".into(),
            format: "html".into(),
            view_html: String::new(),
            sections: vec![ParsedSection {
                heading: Some("Instruments".into()),
                text: text.into(),
                page_index: None,
                body_ranges: None,
            }],
            cover: None,
            assets: Vec::new(),
        };
        upsert_document(
            &mut db,
            id,
            &url(id, "html"),
            &parsed,
            None,
            StoredSourceKind::Html,
            1,
            0,
            PdfTextStatus::Ready,
        )
        .unwrap();
    }
    drop(db);
    let source = root.join("search.sqlite3");
    let snapshot = root.join("snapshot.sqlite3");
    let original = std::fs::read(&source).unwrap();
    snapshot_library(&source, &snapshot);
    let before_search = std::fs::read(&snapshot).unwrap();
    let exported = embedding_documents(&open_read_only(&snapshot).unwrap(), &serde_json::from_value::<Vec<LibraryQuery>>(json!([
        {"id": "scoped-export", "request": {"query": "navigation", "documentUrls": [url("e100", "html")]}}
    ])).unwrap());
    assert_eq!(exported.len(), 1);
    assert_eq!(exported[0]["id"], "e100");
    assert_eq!(exported[0]["url"], url("e100", "html"));
    assert_eq!(exported[0]["sections"][0]["heading"], "Instruments");
    assert_eq!(
        exported[0]["sections"][0]["text"],
        "The guide calls it “silver compass” for navigation."
    );
    assert!(exported[0]["sections"][0]["page_index"].is_null());

    let requests = serde_json::from_value(json!([
        {"id": "quote", "request": {"query": "", "exactPhrases": ["the guide calls it \"silver compass\" for navigation"], "mode": "all"}},
        {"id": "typo", "request": {"query": "compas navigation", "mode": "all", "documentUrls": [url("e100", "html")]}}
    ])).unwrap();
    let rows = library_query_results(&snapshot, requests);
    assert_eq!(rows[0]["ids"], json!(["e100"]));
    assert_eq!(rows[0]["response"]["results"][0]["sectionIndex"], 0);
    assert!(rows[0]["response"]["results"][0]["excerpt"]
        .as_str()
        .unwrap()
        .contains("<mark>"));
    assert_eq!(rows[1]["ids"], json!([]));
    assert_eq!(rows[1]["response"]["suggestedQuery"], "compass navigation");
    assert_eq!(rows[1]["retry"]["ids"], json!(["e100"]));
    assert!(open_read_only(&snapshot)
        .unwrap()
        .execute("DELETE FROM uploaded_documents", [])
        .is_err());
    assert_eq!(std::fs::read(&source).unwrap(), original);
    assert_eq!(std::fs::read(&snapshot).unwrap(), before_search);
    std::fs::remove_dir_all(root).unwrap();
}

#[derive(Deserialize)]
struct Corpus {
    version: u32,
    documents: Vec<Document>,
    queries: Vec<Query>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Document {
    id: String,
    title: String,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    imported_at_ms: Option<u128>,
    sections: Vec<Section>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Section {
    heading: Option<String>,
    text: String,
    #[serde(alias = "page_index")]
    page_index: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Query {
    id: String,
    split: String,
    mode: String,
    native_query: String,
    #[serde(default)]
    phrases: Vec<String>,
    #[serde(default)]
    scope: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
    #[serde(default)]
    ordered: Vec<String>,
    #[serde(default)]
    contract: Option<serde_json::Value>,
    #[serde(default)]
    suggestion: Option<String>,
    relevant: Vec<Judgment>,
}

#[derive(Deserialize)]
struct Judgment {
    id: String,
    grade: u8,
    section: Option<usize>,
    page: Option<usize>,
}

fn index_documents(root: &Path, documents: &[Document]) {
    assert!(
        !documents.is_empty(),
        "nonempty evaluation library required"
    );
    let mut seen = HashSet::new();
    for doc in documents {
        assert!(
            !doc.id.trim().is_empty() && seen.insert(&doc.id),
            "invalid or duplicate document identity"
        );
        assert!(
            matches!(doc.format.as_deref().unwrap_or("html"), "html" | "pdf"),
            "unsupported fixture format"
        );
    }
    let mut db = open_db_in(root).expect("isolated production schema");
    for doc in documents {
        let format = doc.format.as_deref().unwrap_or("html");
        let kind = if format == "pdf" {
            StoredSourceKind::Pdf
        } else {
            assert_eq!(format, "html", "{}: unsupported fixture format", doc.id);
            StoredSourceKind::Html
        };
        let parsed = ParsedDocument {
            source_formatting: None,
            title: doc.title.clone(),
            format: format.into(),
            view_html: String::new(),
            sections: doc
                .sections
                .iter()
                .map(|section| ParsedSection {
                    heading: section.heading.clone(),
                    text: section.text.clone(),
                    page_index: section.page_index,
                    body_ranges: None,
                })
                .collect(),
            cover: None,
            assets: Vec::new(),
        };
        upsert_document(
            &mut db,
            &doc.id,
            &url(&doc.id, format),
            &parsed,
            None,
            kind,
            doc.imported_at_ms.unwrap_or(100),
            0,
            PdfTextStatus::Ready,
        )
        .expect("index fixture through production store");
    }
}

#[test]
#[ignore = "requires external source JSON and a reserved empty output directory"]
fn search_v2_prepare_library() {
    let input = std::env::var("PAPERCUT_SEARCH_EVAL_INPUT").expect("external source JSON");
    let root =
        std::env::var("PAPERCUT_SEARCH_EVAL_PREPARED_LIBRARY").expect("reserved output directory");
    let root = Path::new(&root);
    assert!(
        std::fs::read_dir(root)
            .expect("reserved directory")
            .next()
            .is_none(),
        "prepared library directory must be empty"
    );
    let source: serde_json::Value = serde_json::from_slice(&std::fs::read(input).unwrap()).unwrap();
    let documents: Vec<Document> =
        serde_json::from_value(source["documents"].clone()).expect("source documents");
    index_documents(root, &documents);
}

#[test]
fn prepared_library_keeps_pdf_source_locators() {
    let root = std::env::temp_dir().join(format!(
        "papercut-prepared-check-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let documents: Vec<Document> = serde_json::from_value(json!([
        {"id":"c001","title":"Navigation","format":"pdf","sections":[
            {"text":"The compass points north. 😀","page_index":3}
        ]}
    ]))
    .unwrap();
    index_documents(&root, &documents);
    let requests = serde_json::from_value(json!([
        {"id":"source","request":{"query":"compass","documentUrls":[url("c001","pdf")]}}
    ]))
    .unwrap();
    let rows = library_query_results(&root.join("search.sqlite3"), requests);
    assert_eq!(rows[0]["ids"], json!(["c001"]));
    assert_eq!(rows[0]["response"]["results"][0]["pageIndex"], 3);
    assert_eq!(rows[0]["response"]["results"][0]["sectionIndex"], 0);
    assert!(rows[0]["response"]["results"][0]["excerpt"]
        .as_str()
        .unwrap()
        .contains("<mark>compass</mark>"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn search_v2_evaluation() {
    let corpus: Corpus = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../scripts/fixtures/search-v2/corpus.json"
    )))
    .expect("valid search corpus");
    let temp = std::env::temp_dir().join(format!(
        "papercut-search-v2-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    index_documents(&temp, &corpus.documents);

    let unique_ids = corpus
        .documents
        .iter()
        .map(|document| document.id.as_str())
        .collect::<HashSet<_>>();
    assert_eq!(
        unique_ids.len(),
        corpus.documents.len(),
        "duplicate document IDs"
    );
    let mut seen_queries = HashSet::new();
    let mut rows = Vec::new();
    let selected_split = std::env::var("PAPERCUT_SEARCH_EVAL_SPLIT").ok();
    for query in &corpus.queries {
        if selected_split
            .as_deref()
            .is_some_and(|split| split != query.split)
        {
            continue;
        }
        assert!(
            seen_queries.insert(&query.id),
            "duplicate query ID: {}",
            query.id
        );
        for judgment in &query.relevant {
            assert!(
                unique_ids.contains(judgment.id.as_str()),
                "{}: unknown relevant ID",
                query.id
            );
            assert!(
                (1..=2).contains(&judgment.grade),
                "{}: invalid relevance grade",
                query.id
            );
        }
        let mut attempts: Vec<serde_json::Value> = Vec::new();
        for run in 0..6 {
            let request = UploadedDocumentSearchRequest {
                query: query.native_query.clone(),
                mode: match query.mode.as_str() {
                    "all" => UploadedDocumentSearchMode::All,
                    "broader" => UploadedDocumentSearchMode::Broader,
                    _ => panic!("{}: unsupported evaluation mode", query.id),
                },
                limit: Some(50),
                document_urls: (!query.scope.is_empty()).then(|| {
                    query
                        .scope
                        .iter()
                        .map(|id| {
                            let doc = corpus
                                .documents
                                .iter()
                                .find(|doc| &doc.id == id)
                                .unwrap_or_else(|| panic!("{}: unknown scope ID {id}", query.id));
                            url(id, doc.format.as_deref().unwrap_or("html"))
                        })
                        .collect()
                }),
                exact_phrases: Some(query.phrases.clone()),
            };
            let (response, measurements) =
                search_uploads_with_db(request, || open_db_in(&temp), |_| {})
                    .unwrap_or_else(|error| panic!("{}: {error}", query.id));
            let ids = response
                .results
                .iter()
                .map(|result| result.document_id.as_str())
                .collect::<Vec<_>>();
            assert_eq!(
                response.suggested_query.as_deref(),
                query.suggestion.as_deref(),
                "{}: suggestion contract",
                query.id
            );
            assert_eq!(
                ids.len(),
                ids.iter().copied().collect::<HashSet<_>>().len(),
                "{}: duplicate results",
                query.id
            );
            assert!(ids.len() <= 50, "{}: result limit exceeded", query.id);
            for id in &ids {
                assert!(
                    !query.exclude.iter().any(|excluded| excluded == id),
                    "{}: forbidden hit {id}",
                    query.id
                );
                assert!(
                    query.scope.is_empty() || query.scope.iter().any(|allowed| allowed == id),
                    "{}: scope leak {id}",
                    query.id
                );
            }
            if !query.ordered.is_empty() {
                assert_eq!(
                    ids,
                    query.ordered.iter().map(String::as_str).collect::<Vec<_>>(),
                    "{}: unstable ranking",
                    query.id
                );
            }
            if query.id == "en-title-rank" {
                assert_eq!(ids.first(), Some(&"a017"), "title ranking regressed");
            }
            if query.id == "en-title" {
                assert_eq!(ids.first(), Some(&"a001"), "body ranking regressed");
            }
            if matches!(
                query.id.as_str(),
                "ar-mark-gap" | "ar-tatweel-gap" | "ar-alef-gap"
            ) {
                assert_eq!(
                    ids.first(),
                    Some(&query.relevant[0].id.as_str()),
                    "{}: Arabic normalization miss",
                    query.id
                );
                let excerpt = &response.results[0].excerpt;
                assert!(
                    excerpt.contains("<mark>"),
                    "{}: missing source highlight",
                    query.id
                );
                let source = &corpus
                    .documents
                    .iter()
                    .find(|doc| doc.id == query.relevant[0].id)
                    .unwrap()
                    .sections[0]
                    .text;
                let marked = excerpt
                    .split("<mark>")
                    .nth(1)
                    .unwrap()
                    .split("</mark>")
                    .next()
                    .unwrap();
                assert!(
                    source.contains(marked),
                    "{}: highlight is not authored text",
                    query.id
                );
            }
            if let Some(expected) = &query.contract {
                let result = response
                    .results
                    .first()
                    .unwrap_or_else(|| panic!("{}: missing contract result", query.id));
                let mut actual = serde_json::to_value(result).unwrap();
                actual["passageCount"] = json!(result.passages.len());
                actual["locationCount"] = json!(result.match_locations.len());
                actual["missingTerms"] = json!(result
                    .term_matches
                    .iter()
                    .filter(|term| term.matched == Some(false))
                    .map(|term| term.term.as_str())
                    .collect::<Vec<_>>());
                for (key, value) in expected.as_object().expect("contract object") {
                    assert_eq!(&actual[key], value, "{}: {key}", query.id);
                }
            }
            for judgment in &query.relevant {
                if let Some(result) = response
                    .results
                    .iter()
                    .find(|result| result.document_id == judgment.id)
                {
                    if let Some(section) = judgment.section {
                        assert_eq!(
                            result.section_index, section,
                            "{}: section locator",
                            query.id
                        );
                    }
                    if let Some(page) = judgment.page {
                        assert_eq!(
                            result.page_index,
                            Some(page),
                            "{}: PDF page locator",
                            query.id
                        );
                    }
                }
            }
            let result_ids = ids.iter().map(|id| (*id).to_owned()).collect::<Vec<_>>();
            if run > 0 {
                assert_eq!(
                    json!(result_ids),
                    attempts[0]["ids"],
                    "{}: changed between runs",
                    query.id
                );
            }
            attempts.push(json!({
                "ids": result_ids,
                "passages": response.results.iter().flat_map(|result| result.passages.iter().map(|passage| json!({
                    "document": result.document_id,
                    "section": passage.section_index,
                    "page": passage.page_index
                }))).collect::<Vec<_>>(),
                "total_documents": response.total_documents,
                "total_matching_sections": response.total_matching_sections,
                "suggested_query": response.suggested_query,
                "measurements": measurements
            }));
        }
        rows.push(json!({ "id": query.id, "first": attempts[0], "warm": &attempts[1..] }));
    }

    // The forgiving index may nominate a marked Arabic section, but an
    // unmarked quotation must still be rejected by source verification.
    let (quoted_variant, _) = search_uploads_with_db(
        UploadedDocumentSearchRequest {
            query: String::new(),
            mode: UploadedDocumentSearchMode::All,
            limit: Some(10),
            document_urls: Some(vec![url("a00c", "html")]),
            exact_phrases: Some(vec!["إنتاج الطاقة".into()]),
        },
        || open_db_in(&temp),
        |_| {},
    )
    .expect("verify Arabic quotation against authored text");
    assert!(quoted_variant.results.is_empty());

    let (accepted_retry, _) = search_uploads_with_db(
        UploadedDocumentSearchRequest {
            query: "environment".into(),
            mode: UploadedDocumentSearchMode::All,
            limit: Some(10),
            document_urls: Some(vec![url("a009", "html")]),
            exact_phrases: None,
        },
        || open_db_in(&temp),
        |_| {},
    )
    .expect("explicit typo retry");
    assert_eq!(accepted_retry.results[0].document_id, "a009");
    assert_eq!(accepted_retry.suggested_query, None);

    if let Ok(path) = std::env::var("PAPERCUT_SEARCH_EVAL_OUTPUT") {
        let bytes = std::fs::metadata(temp.join("search.sqlite3"))
            .unwrap()
            .len();
        let output = json!({
            "corpus_version": corpus.version,
            "documents": corpus.documents.len(),
            "sections": corpus.documents.iter().map(|doc| doc.sections.len()).sum::<usize>(),
            "sqlite_bytes": bytes,
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "build": if cfg!(debug_assertions) { "debug" } else { "release" },
            "peak_rss_kib": peak_rss_kib(),
            "query_results": rows
        });
        std::fs::write(&path, serde_json::to_vec_pretty(&output).unwrap())
            .unwrap_or_else(|error| panic!("cannot write {path}: {error}"));
    }
    std::fs::remove_dir_all(&temp).expect("remove isolated corpus database");
}

fn url(id: &str, format: &str) -> String {
    format!("/uploads/{id}.{format}")
}

fn peak_rss_kib() -> Option<u64> {
    let text = std::fs::read_to_string(Path::new("/proc/self/status")).ok()?;
    text.lines().find_map(|line| {
        line.strip_prefix("VmHWM:")
            .and_then(|value| value.split_whitespace().next())
            .and_then(|value| value.parse().ok())
    })
}

#[test]
fn normalized_pdf_search_keeps_page_locator_after_ocr_replacement() {
    let root = std::env::temp_dir().join(format!(
        "papercut-arabic-pdf-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut db = open_db_in(&root).unwrap();
    let mut parsed = ParsedDocument {
        source_formatting: None,
        title: "بحث عربي".into(),
        format: "pdf".into(),
        view_html: String::new(),
        sections: vec![ParsedSection {
            heading: None,
            text: "🙂 إِنْتَاجُ الطـاقةِ".into(),
            page_index: Some(7),
            body_ranges: None,
        }],
        cover: None,
        assets: Vec::new(),
    };
    upsert_document(
        &mut db,
        "abc123",
        "/uploads/abc123.pdf",
        &parsed,
        None,
        StoredSourceKind::Pdf,
        1,
        1,
        PdfTextStatus::Ready,
    )
    .unwrap();
    db.execute(
        "UPDATE upload_schema_metadata SET value = '0' WHERE key = 'search_form_ready'",
        [],
    )
    .unwrap();
    drop(db);
    let request = |query: &str| UploadedDocumentSearchRequest {
        query: query.into(),
        mode: UploadedDocumentSearchMode::All,
        limit: Some(10),
        document_urls: None,
        exact_phrases: None,
    };
    assert!(
        search_uploads_with_db(request("إنتاج الطاقة"), || open_db_in(&root), |_| {})
            .unwrap()
            .0
            .results
            .is_empty()
    );
    let db = open_db_in(&root).unwrap();
    db.execute(
        "UPDATE upload_schema_metadata SET value = '1' WHERE key = 'search_form_ready'",
        [],
    )
    .unwrap();
    drop(db);
    let (response, _) =
        search_uploads_with_db(request("إنتاج الطاقة"), || open_db_in(&root), |_| {}).unwrap();
    assert_eq!(response.results.len(), 1);
    assert_eq!(response.results[0].page_index, Some(7));
    assert_eq!(response.results[0].section_index, 0);
    assert!(response.results[0].excerpt.contains("<mark>إِنْتَاجُ</mark>"));

    let mut db = open_db_in(&root).unwrap();
    parsed.sections[0].text = "البيئة النظيفة".into();
    upsert_document(
        &mut db,
        "abc123",
        "/uploads/abc123.pdf",
        &parsed,
        None,
        StoredSourceKind::Pdf,
        1,
        1,
        PdfTextStatus::Ready,
    )
    .unwrap();
    drop(db);
    assert!(
        search_uploads_with_db(request("إنتاج"), || open_db_in(&root), |_| {})
            .unwrap()
            .0
            .results
            .is_empty()
    );
    assert_eq!(
        search_uploads_with_db(request("البيئة"), || open_db_in(&root), |_| {})
            .unwrap()
            .0
            .results[0]
            .page_index,
        Some(7)
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn typo_retry_protects_quotes_scripts_identifiers_and_literal_results() {
    let root = std::env::temp_dir().join(format!(
        "papercut-typo-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut db = open_db_in(&root).unwrap();
    let parsed = ParsedDocument {
        source_formatting: None,
        title: "Colonialism".into(),
        format: "html".into(),
        view_html: String::new(),
        sections: vec![ParsedSection {
            heading: None,
            text: "Colonialism shaped the coast.".into(),
            page_index: None,
            body_ranges: None,
        }],
        cover: None,
        assets: Vec::new(),
    };
    upsert_document(
        &mut db,
        "abc123",
        "/uploads/abc123.html",
        &parsed,
        None,
        StoredSourceKind::Html,
        1,
        1,
        PdfTextStatus::Ready,
    )
    .unwrap();
    drop(db);
    let run = |query: &str, phrases: Vec<String>| {
        search_uploads_with_db(
            UploadedDocumentSearchRequest {
                query: query.into(),
                mode: UploadedDocumentSearchMode::All,
                limit: Some(10),
                document_urls: None,
                exact_phrases: Some(phrases),
            },
            || open_db_in(&root),
            |_| {},
        )
        .unwrap()
        .0
    };
    let miss = run("collonialism", vec![]);
    assert!(miss.results.is_empty());
    assert_eq!(miss.suggested_query.as_deref(), Some("colonialism"));
    let accepted = run("colonialism", vec![]);
    assert_eq!(accepted.results[0].document_id, "abc123");
    assert!(accepted.results[0]
        .excerpt
        .contains("<mark>Colonialism</mark>"));
    for query in [
        "Collonialism",
        "collonialism-7",
        "collonialism العربية",
        "cat",
    ] {
        assert!(
            run(query, vec![]).suggested_query.is_none(),
            "{query}: protected query"
        );
    }
    assert!(run("", vec!["collonialism".into()])
        .suggested_query
        .is_none());
    let mut db = open_db_in(&root).unwrap();
    let tx = db.transaction().unwrap();
    {
        let mut insert = tx.prepare(
            "INSERT INTO uploaded_spelling_terms (document_id, term, source) VALUES ('abc123', ?1, 1)"
        ).unwrap();
        for n in 0..1024 {
            let suffix = [n / 676, n / 26 % 26, n % 26]
                .map(|digit| (b'a' + digit as u8) as char)
                .iter()
                .collect::<String>();
            insert.execute([format!("coaaaaaaa{suffix}")]).unwrap();
        }
    }
    tx.commit().unwrap();
    drop(db);
    assert!(run("cozzzzzzzzzz", vec![]).suggested_query.is_none());
    std::fs::remove_dir_all(root).unwrap();
}
