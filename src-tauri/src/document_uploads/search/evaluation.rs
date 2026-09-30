//! Small, deterministic search baseline using the real store and search path.

use std::collections::HashSet;
use std::path::Path;

use serde::Deserialize;
use serde_json::json;

use super::super::parsed::{ParsedDocument, ParsedSection};
use super::super::storage::StoredSourceKind;
use super::super::store::{open_db_in, upsert_document, PdfTextStatus};
use super::super::types::UploadedDocumentSearchRequest;
use super::search_uploads_with_db;

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
    relevant: Vec<Judgment>,
}

#[derive(Deserialize)]
struct Judgment {
    id: String,
    grade: u8,
    section: Option<usize>,
    page: Option<usize>,
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
    let mut db = open_db_in(&temp).expect("isolated production schema");
    for doc in &corpus.documents {
        let format = doc.format.as_deref().unwrap_or("html");
        let kind = if format == "pdf" {
            StoredSourceKind::Pdf
        } else {
            assert_eq!(format, "html", "{}: unsupported fixture format", doc.id);
            StoredSourceKind::Html
        };
        let parsed = ParsedDocument {
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
    drop(db);

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
        assert_eq!(
            query.mode, "all",
            "{}: unsupported evaluation mode",
            query.id
        );
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
            if let Some(expected) = &query.contract {
                let result = response
                    .results
                    .first()
                    .unwrap_or_else(|| panic!("{}: missing contract result", query.id));
                let mut actual = serde_json::to_value(result).unwrap();
                actual["passageCount"] = json!(result.passages.len());
                actual["locationCount"] = json!(result.match_locations.len());
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
                "measurements": measurements
            }));
        }
        rows.push(json!({ "id": query.id, "first": attempts[0], "warm": &attempts[1..] }));
    }

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
