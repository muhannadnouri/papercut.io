use super::super::query::{retrieve, QueryRequest};
use super::*;
use crate::document_uploads::{
    parsed::{ParsedDocument, ParsedSection},
    storage::StoredSourceKind,
    store::{
        delete_document_rows, open_db_in, update_document_title, upsert_document, PdfTextStatus,
    },
};
use serde_json::{json, Value};
use std::{cell::Cell, path::PathBuf};

fn request(value: Value) -> QueryRequest {
    serde_json::from_value(value).unwrap()
}

/// Use real token boundaries/vectors with the production SQLite/lexical paths, without app data or a WebView.
#[cfg(feature = "experimental-search-embeddings-core")]
pub(crate) fn verify_native_index_queries(
    provider: &crate::search_embeddings::SearchEmbeddingState,
    disable: impl FnOnce() -> Result<(), String>,
) {
    use super::super::SemanticIndexState;
    use crate::document_uploads::{
        search::search_uploads_with_db, types::UploadedDocumentSearchRequest,
    };

    let library = Library::new();
    let mut db = library.db();
    put(
        &mut db,
        "harbor",
        "Harbor routes",
        &[
            "Electric buses connect the harbor to Central station. 🙂",
            "الحافلات الكهربائية تربط الميناء بالمحطة.",
        ],
    );
    put(
        &mut db,
        "airport",
        "Airport express",
        &["Electric buses serve the airport terminal."],
    );
    put(
        &mut db,
        "museum",
        "Electric buses",
        &["An unrelated museum exhibition."],
    );
    let lexical = || {
        let input: UploadedDocumentSearchRequest = serde_json::from_value(json!({
            "query":"electric buses", "mode":"all", "limit":10
        }))
        .unwrap();
        serde_json::to_value(
            search_uploads_with_db(input, || Ok(library.db()), |_| {})
                .unwrap()
                .0,
        )
        .unwrap()
    };
    let baseline = lexical();
    assert!(!baseline["results"].as_array().unwrap().is_empty());
    let (encoder, dimensions) = provider.index_encoder_identity().unwrap();
    let key = Identity::new(&encoder, dimensions).unwrap();
    let query = |db: &mut Connection, input: Value| {
        serde_json::to_value(
            retrieve(
                db,
                &key,
                &request(input),
                |text| provider.index_encode(&encoder, text),
                || Ok(()),
            )
            .unwrap(),
        )
        .unwrap()
    };
    assert_eq!(
        query(&mut db, json!({"query":"city travel"}))["availability"],
        "missing-index"
    );
    assert_eq!(lexical(), baseline);
    initialize(&mut db, &key).unwrap();

    // Cancel at a committed checkpoint, avoiding scheduler timing or interrupting an ORT call.
    let state = SemanticIndexState::default();
    let control = state.begin().unwrap();
    let snapshot = source(&mut db, "harbor").unwrap().unwrap();
    let cancelled = synchronize(
        &mut db,
        &key,
        &snapshot,
        |title, heading, text| provider.index_passage_ranges(&encoder, title, heading, text),
        |text| provider.index_encode(&encoder, text),
        || control.check(),
        |reused| {
            control.progress(reused);
            state.cancel().unwrap();
        },
    );
    assert_eq!(
        cancelled.as_ref().unwrap_err(),
        "Semantic indexing cancelled"
    );
    control.finish(&cancelled);
    assert!(state.snapshot().unwrap().running);
    assert!(
        state.begin().is_err(),
        "cancelled writer retains its slot until cleanup"
    );
    drop(control);
    assert!(!state.snapshot().unwrap().running);
    assert_eq!(coverage(&mut db, Some(&key)).unwrap().indexed_documents, 0);
    let partial = query(&mut db, json!({"query":"city travel"}));
    assert_eq!(partial["availability"], "partial");
    assert!(partial["results"].as_array().unwrap().is_empty());
    assert_eq!(lexical(), baseline);

    let reused = Cell::new(0);
    let embedded = Cell::new(0);
    for id in document_ids(&db).unwrap() {
        let control = state.begin().unwrap();
        let snapshot = source(&mut db, &id).unwrap().unwrap();
        let result = synchronize(
            &mut db,
            &key,
            &snapshot,
            |title, heading, text| provider.index_passage_ranges(&encoder, title, heading, text),
            |text| provider.index_encode(&encoder, text),
            || control.check(),
            |was_reused| {
                control.progress(was_reused);
                if was_reused {
                    reused.set(reused.get() + 1)
                } else {
                    embedded.set(embedded.get() + 1)
                }
            },
        );
        control.finish(&result);
        result.unwrap();
    }
    assert_eq!(
        reused.get(),
        1,
        "resume must reuse the committed checkpoint"
    );
    assert_eq!(embedded.get(), 3);
    assert_eq!(coverage(&mut db, Some(&key)).unwrap().indexed_documents, 3);
    let snapshot = source(&mut db, "harbor").unwrap().unwrap();
    synchronize(
        &mut db,
        &key,
        &snapshot,
        |_, _, _| panic!("unchanged source must skip tokenizing"),
        |_| panic!("unchanged source must skip inference"),
        || Ok(()),
        |_| {},
    )
    .unwrap();

    let changes = db.total_changes();
    let scoped = query(
        &mut db,
        json!({"query":"How do travelers get to the station?",
        "documentUrls":["/uploads/harbor.html"], "exactPhrases":["electric buses"], "limit":1}),
    );
    assert_eq!(scoped["availability"], "ready");
    assert_eq!(scoped["results"].as_array().unwrap().len(), 1);
    let hit = &scoped["results"][0];
    assert_eq!(hit["documentId"], "harbor");
    assert_eq!(hit["provenance"], "conceptual");
    assert!(hit.get("matchCount").is_none());
    let text: String = db
        .query_row(
            "SELECT text FROM uploaded_sections WHERE document_id=?1 AND ordinal=?2",
            params![
                hit["documentId"].as_str().unwrap(),
                hit["sectionIndex"].as_u64().unwrap()
            ],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        text.get(
            hit["byteStart"].as_u64().unwrap() as usize..hit["byteEnd"].as_u64().unwrap() as usize
        )
        .unwrap(),
        hit["text"]
    );
    let excluded = query(
        &mut db,
        json!({"query":"city travel", "documentUrls":["/uploads/airport.html"], "limit":1}),
    );
    assert_eq!(excluded["results"][0]["documentId"], "airport");
    let empty = query(&mut db, json!({"query":"city travel", "documentUrls":[]}));
    assert_eq!(empty["scannedPassages"], 0);
    assert!(empty["results"].as_array().unwrap().is_empty());
    for input in [
        json!({"query":"city travel", "exactPhrases":["missing authored clause"]}),
        json!({"query":"city travel", "exactPhrases":["electric buses"], "documentUrls":["/uploads/museum.html"]}),
    ] {
        assert!(
            query(&mut db, input)["results"]
                .as_array()
                .unwrap()
                .is_empty(),
            "quotes require authored body evidence"
        );
    }
    let arabic = query(
        &mut db,
        json!({"query":"النقل في المدينة", "exactPhrases":["الحافلات الكهربائية"], "documentUrls":["/uploads/harbor.html"]}),
    );
    assert_eq!(arabic["results"][0]["documentId"], "harbor");
    assert_eq!(
        db.total_changes(),
        changes,
        "conceptual queries must not write the index"
    );

    let mut checks = 0;
    let cancelled_query = retrieve(
        &mut db,
        &key,
        &request(json!({"query":"city travel"})),
        |text| provider.index_encode(&encoder, text),
        || {
            checks += 1;
            if checks > 2 {
                Err("query cancelled".into())
            } else {
                Ok(())
            }
        },
    );
    assert_eq!(cancelled_query.err().unwrap(), "query cancelled");
    assert_eq!(lexical(), baseline);
    let incompatible = Identity::new("another-encoder", dimensions).unwrap();
    assert_eq!(
        serde_json::to_value(
            retrieve(
                &mut db,
                &incompatible,
                &request(json!({"query":"city travel"})),
                |_| panic!("incompatible index must not infer"),
                || Ok(())
            )
            .unwrap()
        )
        .unwrap()["availability"],
        "incompatible-index"
    );
    assert_eq!(lexical(), baseline);

    // Real tokenizer windows must stay inside derived roles and still point into authored UTF-8.
    let content_library = Library::new();
    let mut content_db = content_library.db();
    let note = "Citation: electric buses.\n";
    let body = "港 Residents travel between the harbor and station by bus. 🙂";
    put(
        &mut content_db,
        "roles",
        "Transit guide",
        &[&format!("{note}{body}")],
    );
    content_db
        .execute(
            "UPDATE uploaded_sections SET body_ranges=?1 WHERE document_id='roles'",
            [serde_json::to_string(&vec![note.len()..note.len() + body.len()]).unwrap()],
        )
        .unwrap();
    initialize(&mut content_db, &key).unwrap();
    let snapshot = source(&mut content_db, "roles").unwrap().unwrap();
    synchronize(
        &mut content_db,
        &key,
        &snapshot,
        |title, heading, text| provider.index_passage_ranges(&encoder, title, heading, text),
        |text| provider.index_encode(&encoder, text),
        || Ok(()),
        |_| {},
    )
    .unwrap();
    let hit = query(
        &mut content_db,
        json!({"query":"electric buses", "exactPhrases":["electric buses"]}),
    );
    assert_eq!(hit["results"][0]["text"], body);
    assert_eq!(hit["results"][0]["byteStart"], note.len());
    assert_eq!(hit["results"][0]["byteEnd"], note.len() + body.len());

    // Disable from the query's native boundary: completed inference must not publish after opt-out.
    let guard = state.begin_query().unwrap();
    let version = provider.index_generation();
    let mut disable = Some(disable);
    let disabled = retrieve(
        &mut db,
        &key,
        &request(json!({"query":"city travel"})),
        |text| {
            let vector = provider.index_encode(&encoder, text)?;
            disable.take().unwrap()()?;
            Ok(vector)
        },
        || {
            if provider.index_generation() != version {
                Err("Semantic model disabled; retry query".into())
            } else {
                Ok(())
            }
        },
    );
    assert!(disabled.err().unwrap().contains("disabled"));
    drop(guard);
    assert!(
        state.begin_query().is_ok(),
        "failed query must release its slot"
    );
    assert!(provider
        .index_encode(&encoder, "query: city travel")
        .is_err());
    assert_eq!(lexical(), baseline);
    remove(&mut db).unwrap();
    assert_eq!(
        lexical(),
        baseline,
        "derived-index removal must preserve exact search"
    );
}

fn conceptual(db: &mut Connection, input: Value) -> Value {
    serde_json::to_value(
        retrieve(
            db,
            &identity(),
            &request(input),
            |_| Ok(vec![1.0, 0.0]),
            || Ok(()),
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn changed_window_policy_requires_explicit_rebuild_without_touching_source() {
    let library = Library::new();
    let mut db = library.db();
    put(&mut db, "abc123", "Guide", &["Ferries connect the harbor."]);
    let key = identity();
    initialize(&mut db, &key).unwrap();
    sync(&mut db, &key, "abc123");
    db.execute(
        "UPDATE upload_semantic_metadata SET identity=?1",
        [key.key.replace("chunks-v2", "chunks-v1")],
    )
    .unwrap();
    let response = retrieve(
        &mut db,
        &key,
        &request(json!({"query":"harbor travel"})),
        |_| panic!("old window policy must not infer"),
        || Ok(()),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(response).unwrap()["availability"],
        "incompatible-index"
    );
    initialize(&mut db, &key).unwrap();
    assert_eq!(coverage(&mut db, Some(&key)).unwrap().indexed_documents, 0);
    let snapshot = source(&mut db, "abc123").unwrap().unwrap();
    assert_eq!(snapshot.sections[0].text, "Ferries connect the harbor.");
    sync(&mut db, &key, "abc123");
    assert_eq!(coverage(&mut db, Some(&key)).unwrap().indexed_documents, 1);
}

#[test]
fn pdf_role_upgrade_preserves_sources_and_other_roles_until_explicit_semantic_update() {
    use crate::document_uploads::pdf::{
        narration,
        page_text::{self, PageTextBlock, PageTextLayer},
    };
    let library = Library::new();
    let mut db = library.db();
    let page = PageTextLayer {
        schema_version: 1,
        page_index: 1,
        width: 600.0,
        height: 800.0,
        blocks: (1..=3)
            .map(|i| PageTextBlock {
                text: format!("[{i}] A. Reed: “Sapphire Ferry Guide,” Printed Journal, 2020.\n"),
                bounds: [40.0, 100.0 + i as f32 * 16.0, 500.0, 12.0],
                order: i,
                confidence: None,
            })
            .collect(),
    };
    let references = narration::reconstruct_search_text(page.clone());
    put(
        &mut db,
        "abc123",
        "Harbor Guide",
        &["港 Ferries connect the island.", &references],
    );
    put(
        &mut db,
        "def456",
        "Port Notes",
        &["An auxiliary EPUB note."],
    );
    db.execute(
        "UPDATE uploaded_documents SET format='pdf',source_kind='pdf' WHERE id='abc123'",
        [],
    )
    .unwrap();
    db.execute(
        "UPDATE uploaded_sections SET page_index=ordinal WHERE document_id='abc123'",
        [],
    )
    .unwrap();
    // The previous classifier kept these printed references inside persisted body ranges.
    for (ordinal, text) in ["港 Ferries connect the island.", &references]
        .iter()
        .enumerate()
    {
        db.execute(
            "UPDATE uploaded_sections SET body_ranges=?1 WHERE document_id='abc123' AND ordinal=?2",
            params![
                serde_json::to_string(&vec![0..text.len()]).unwrap(),
                ordinal
            ],
        )
        .unwrap();
    }
    db.execute(
        "UPDATE uploaded_sections SET body_ranges='[]' WHERE document_id='def456'",
        [],
    )
    .unwrap();
    db.execute(
        "UPDATE uploaded_documents SET format='epub',source_kind='epub' WHERE id='def456'",
        [],
    )
    .unwrap();
    page_text::write_page_text_layer(&library.0.join("abc123"), &page).unwrap();
    let before: Vec<(i64, usize, String, Option<usize>)> = db
        .prepare("SELECT id,ordinal,text,page_index FROM uploaded_sections ORDER BY id")
        .unwrap()
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let key = identity();
    initialize(&mut db, &key).unwrap();
    sync(&mut db, &key, "abc123");
    sync(&mut db, &key, "def456");
    db.execute(
        "UPDATE upload_schema_metadata SET value='9' WHERE key='schema_version'",
        [],
    )
    .unwrap();
    drop(db);

    let mut db = library.db();
    assert_eq!(coverage(&mut db, Some(&key)).unwrap().indexed_documents, 1);
    assert_eq!(
        db.query_row(
            "SELECT body_ranges FROM uploaded_sections WHERE document_id='def456'",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        "[]"
    );
    let pending: usize = db.query_row("SELECT count(*) FROM uploaded_sections WHERE document_id='abc123' AND body_ranges IS NULL", [], |row| row.get(0)).unwrap();
    assert_eq!(pending, 2);
    assert!(
        crate::document_uploads::store::rebuild_search_content_batch(&mut db, &library.0).unwrap()
    );
    assert!(
        !crate::document_uploads::store::rebuild_search_content_batch(&mut db, &library.0).unwrap()
    );
    assert_eq!(
        db.query_row(
            "SELECT body_ranges FROM uploaded_sections WHERE document_id='abc123' AND ordinal=1",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        "[]"
    );
    // A missing body-page sidecar retains its text; classified citations stay literal-searchable.
    assert_eq!(db.query_row("SELECT count(*) FROM uploaded_document_fts WHERE uploaded_document_fts MATCH '\"Sapphire Ferry Guide\"'", [], |row| row.get::<_,usize>(0)).unwrap(), 1);
    assert_eq!(db.query_row("SELECT count(*) FROM uploaded_document_search_fts WHERE uploaded_document_search_fts MATCH 'sapphire'", [], |row| row.get::<_,usize>(0)).unwrap(), 1);
    let after: Vec<(i64, usize, String, Option<usize>)> = db
        .prepare("SELECT id,ordinal,text,page_index FROM uploaded_sections ORDER BY id")
        .unwrap()
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(before, after);
    assert_eq!(coverage(&mut db, Some(&key)).unwrap().indexed_documents, 1);
    sync(&mut db, &key, "abc123");
    assert_eq!(coverage(&mut db, Some(&key)).unwrap().indexed_documents, 2);
    let hit = retrieve(&mut db, &key, &request(json!({"query":"harbor travel", "exactPhrases":["Sapphire Ferry Guide"], "documentUrls":["/uploads/abc123.html"]})), |_| Ok(vec![1.0,0.0]), || Ok(())).unwrap();
    assert_eq!(
        serde_json::to_value(hit).unwrap()["results"][0]["text"],
        "港 Ferries connect the island."
    );
    drop(db);
    let mut db = library.db();
    assert_eq!(coverage(&mut db, Some(&key)).unwrap().indexed_documents, 2);
    assert!(
        !crate::document_uploads::store::rebuild_search_content_batch(&mut db, &library.0).unwrap()
    );
}

#[test]
fn conceptual_scope_quotes_constrain_ranking_before_limit_without_chunk_votes() {
    let library = Library::new();
    let mut db = library.db();
    let key = identity();
    for index in 0..70 {
        let id = format!("neighbor{index:03}");
        put(
            &mut db,
            &id,
            "City lights",
            &["A daytime transit timetable."],
        );
    }
    put(
        &mut db,
        "substring",
        "City lights",
        &["Subcity lightspeed. Ticket offices remain open."],
    );
    put(
        &mut db,
        "target",
        "Night transport",
        &[
            "City\n lights guide. Night-time service.",
            "Ticket offices remain open.",
        ],
    );
    initialize(&mut db, &key).unwrap();
    for id in document_ids(&db).unwrap() {
        sync(&mut db, &key, &id);
    }
    let bytes = [0.0f32, 1.0]
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect::<Vec<_>>();
    db.execute(
        "UPDATE upload_semantic_passages SET vector=?1,vector_hash=?2 WHERE document_id='target'",
        params![bytes, format!("{:x}", Sha256::digest(&bytes))],
    )
    .unwrap();
    let hit = conceptual(
        &mut db,
        json!({"query":"evening travel", "exactPhrases":["city lights","offices remain"],"limit":1}),
    );
    assert_eq!(hit["results"][0]["documentId"], "target");
    assert_eq!(hit["results"].as_array().unwrap().len(), 1);
    assert_eq!(
        hit["scannedPassages"], 2,
        "title-only quote neighbors never enter scoring"
    );
    assert_eq!(
        hit["results"][0]["score"], 0.0,
        "phrase constraints precede higher-scoring neighbors"
    );
    assert_eq!(
        hit["results"][0]["sectionIndex"], 0,
        "equal passage scores use source order"
    );
    assert_eq!(hit["results"][0]["provenance"], "conceptual");
    assert!(hit["results"][0].get("matchCount").is_none());
    let hyphen = conceptual(
        &mut db,
        json!({"query":"evening travel","exactPhrases":["night-time service"],"limit":1}),
    );
    assert_eq!(hyphen["results"][0]["documentId"], "target");
    let scope = conceptual(
        &mut db,
        json!({"query":"evening travel","documentUrls":["/uploads/target.html"],"limit":1}),
    );
    assert_eq!(scope["results"][0]["documentId"], "target");
    assert_eq!(scope["eligibleDocuments"], 1);
    let empty = conceptual(&mut db, json!({"query":"travel","documentUrls":[]}));
    assert_eq!(empty["eligibleDocuments"], 0);
    assert_eq!(empty["scannedPassages"], 0);
    assert!(empty["results"].as_array().unwrap().is_empty());
    let all = conceptual(&mut db, json!({"query":"travel","limit":50}));
    assert_eq!(all["results"].as_array().unwrap().len(), 50);
    assert_eq!(
        all["results"][0]["documentId"], "neighbor000",
        "document ties are deterministic"
    );
}

#[test]
fn conceptual_missing_incompatible_and_partial_indexes_do_not_invent_coverage_or_encode() {
    let library = Library::new();
    let mut db = library.db();
    put(&mut db, "ready", "Guide", &["🙂 المواصلات 城市交通"]);
    let input = request(json!({"query":"travel"}));
    let missing = retrieve(
        &mut db,
        &identity(),
        &input,
        |_| panic!("missing index must not infer"),
        || Ok(()),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(missing).unwrap()["availability"],
        "missing-index"
    );
    assert!(!exists(&db).unwrap());
    initialize(&mut db, &identity()).unwrap();
    sync(&mut db, &identity(), "ready");
    put(&mut db, "pending", "New guide", &["Bicycle connections"]);
    let partial = conceptual(&mut db, json!({"query":"travel"}));
    assert_eq!(partial["availability"], "partial");
    assert_eq!(partial["eligibleDocuments"], 2);
    assert_eq!(partial["indexedDocuments"], 1);
    assert_eq!(partial["results"][0]["text"], "🙂 المواصلات 城市交通");
    assert_eq!(partial["results"][0]["byteStart"], 0);
    assert_eq!(
        partial["results"][0]["byteEnd"],
        "🙂 المواصلات 城市交通".len()
    );
    assert_eq!(partial["results"][0]["pageIndex"], 0);
    let other = Identity::new("different-encoder", 2).unwrap();
    let response = retrieve(
        &mut db,
        &other,
        &input,
        |_| panic!("wrong identity must not infer"),
        || Ok(()),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(response).unwrap()["availability"],
        "incompatible-index"
    );
    db.execute("UPDATE upload_semantic_metadata SET schema_version=99", [])
        .unwrap();
    let response = retrieve(
        &mut db,
        &identity(),
        &input,
        |_| panic!("unknown schema must not infer"),
        || Ok(()),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(response).unwrap()["availability"],
        "incompatible-index"
    );
}

#[test]
fn conceptual_query_rejects_corrupt_vectors_spans_hashes_and_bad_requests() {
    let library = Library::new();
    let mut db = library.db();
    put(&mut db, "ready", "Guide", &["🙂 Transit"]);
    initialize(&mut db, &identity()).unwrap();
    sync(&mut db, &identity(), "ready");
    let input = request(json!({"query":"travel"}));
    assert!(retrieve(
        &mut db,
        &identity(),
        &input,
        |_| Ok(vec![f32::NAN, 0.0]),
        || Ok(())
    )
    .is_err());
    for value in [
        json!({"query":""}),
        json!({"query":"word ".repeat(129)}),
        json!({"query":"x".repeat(4097)}),
        json!({"query":"travel","limit":51}),
        json!({"query":"travel","documentUrls":[""]}),
    ] {
        assert!(retrieve(
            &mut db,
            &identity(),
            &request(value),
            |_| panic!("invalid request must not infer"),
            || Ok(())
        )
        .is_err());
    }
    for sql in [
        "UPDATE upload_semantic_passages SET vector_hash='bad'",
        "UPDATE upload_semantic_passages SET byte_start=1",
        "UPDATE upload_semantic_passages SET content_hash='bad'",
    ] {
        db.execute(sql, []).unwrap();
        assert!(retrieve(
            &mut db,
            &identity(),
            &input,
            |_| Ok(vec![1.0, 0.0]),
            || Ok(())
        )
        .is_err());
        remove(&mut db).unwrap();
        initialize(&mut db, &identity()).unwrap();
        sync(&mut db, &identity(), "ready");
    }
}

#[test]
fn conceptual_snapshot_rejects_edits_or_removal_during_scoring_and_obeys_cancellation() {
    let library = Library::new();
    let mut db = library.db();
    put(&mut db, "abc123", "Guide", &["Transit"]);
    initialize(&mut db, &identity()).unwrap();
    sync(&mut db, &identity(), "abc123");
    let input = request(json!({"query":"travel"}));
    let mut writer = library.db();
    let result = retrieve(
        &mut db,
        &identity(),
        &input,
        |_| {
            update_document_title(&mut writer, "/uploads/abc123.html", "Changed").unwrap();
            Ok(vec![1.0, 0.0])
        },
        || Ok(()),
    );
    assert!(result.err().unwrap().contains("library changed"));
    sync(&mut db, &identity(), "abc123");
    let result = retrieve(
        &mut db,
        &identity(),
        &input,
        |_| {
            remove(&mut writer).unwrap();
            Ok(vec![1.0, 0.0])
        },
        || Ok(()),
    );
    assert!(result.err().unwrap().contains("library changed"));
    initialize(&mut db, &identity()).unwrap();
    sync(&mut db, &identity(), "abc123");
    let mut checks = 0;
    assert!(retrieve(
        &mut db,
        &identity(),
        &input,
        |_| Ok(vec![1.0, 0.0]),
        || {
            checks += 1;
            if checks > 2 {
                Err("cancelled".into())
            } else {
                Ok(())
            }
        }
    )
    .is_err());
    let result = retrieve(
        &mut db,
        &identity(),
        &input,
        |_| {
            delete_document_rows(&mut writer, "abc123").unwrap();
            Ok(vec![1.0, 0.0])
        },
        || Ok(()),
    );
    assert!(result.err().unwrap().contains("library changed"));
}

struct Library(PathBuf);

impl Library {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!(
                "papercut-semantic-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )))
    }
    fn db(&self) -> Connection {
        open_db_in(&self.0).unwrap()
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn identity() -> Identity {
    Identity::new("synthetic-encoder/tokenizer-v1", 2).unwrap()
}

#[test]
fn main_content_wins_over_a_higher_scoring_note_with_source_and_quotes_preserved() {
    let library = Library::new();
    let mut db = library.db();
    put(
        &mut db,
        "abc123",
        "Harbor Guide",
        &[
            "Sapphire ferry citation.",
            "港 Electric ferries serve the island.",
        ],
    );
    db.execute(
        "UPDATE uploaded_sections SET body_ranges='[]' WHERE ordinal=0",
        [],
    )
    .unwrap();
    let key = identity();
    initialize(&mut db, &key).unwrap();
    let snapshot = source(&mut db, "abc123").unwrap().unwrap();
    synchronize(
        &mut db,
        &key,
        &snapshot,
        |_, _, text| Ok(vec![0..text.len()]),
        |text| {
            Ok(if text.contains("citation") {
                vec![1.0, 0.0]
            } else {
                vec![0.8, 0.6]
            })
        },
        || Ok(()),
        |_| {},
    )
    .unwrap();
    let result = serde_json::to_value(
        retrieve(
            &mut db,
            &key,
            &request(json!({"query":"island travel","exactPhrases":["Sapphire ferry"]})),
            |_| Ok(vec![1.0, 0.0]),
            || Ok(()),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(result["results"][0]["sectionIndex"], 1);
    assert_eq!(
        result["results"][0]["text"],
        "港 Electric ferries serve the island."
    );
    assert_eq!(
        db.query_row(
            "SELECT text FROM uploaded_sections WHERE ordinal=0",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        "Sapphire ferry citation."
    );
    db.execute(
        "UPDATE uploaded_sections SET body_ranges='[]' WHERE ordinal=1",
        [],
    )
    .unwrap();
    assert_eq!(coverage(&mut db, Some(&key)).unwrap().indexed_documents, 0);
    sync(&mut db, &key, "abc123");
    let auxiliary = serde_json::to_value(
        retrieve(
            &mut db,
            &key,
            &request(json!({"query":"citation"})),
            |_| Ok(vec![1.0, 0.0]),
            || Ok(()),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(auxiliary["results"].as_array().unwrap().len(), 1);
}

#[test]
fn role_boundaries_keep_original_utf8_offsets() {
    let library = Library::new();
    let mut db = library.db();
    let text = "Note.\n港 Ferry routes.\nFooter";
    put(&mut db, "abc123", "Harbor", &[text]);
    let start = text.find('港').unwrap();
    let end = text.find("\nFooter").unwrap();
    db.execute(
        "UPDATE uploaded_sections SET body_ranges=?1",
        [serde_json::to_string(&vec![start..end]).unwrap()],
    )
    .unwrap();
    let key = identity();
    initialize(&mut db, &key).unwrap();
    let source = source(&mut db, "abc123").unwrap().unwrap();
    let mut seen = Vec::new();
    synchronize(
        &mut db,
        &key,
        &source,
        |_, _, text| Ok(vec![0..text.len()]),
        |input| {
            seen.push(input.to_string());
            Ok(vec![1.0, 0.0])
        },
        || Ok(()),
        |_| {},
    )
    .unwrap();
    assert_eq!(seen.len(), 3);
    assert!(seen[1].ends_with("港 Ferry routes."));
    let result = serde_json::to_value(
        retrieve(
            &mut db,
            &key,
            &request(json!({"query":"ferry"})),
            |_| Ok(vec![1.0, 0.0]),
            || Ok(()),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(result["results"][0]["byteStart"], start);
    assert_eq!(result["results"][0]["byteEnd"], end);
}

#[test]
fn stored_index_is_independent_of_ready_coverage_and_empty_retained_tables() {
    let library = Library::new();
    let mut db = library.db();
    assert!(!has_stored_index(&db).unwrap());
    assert!(!exists(&db).unwrap());
    initialize(&mut db, &identity()).unwrap();
    assert!(has_stored_index(&db).unwrap());
    assert_eq!(coverage(&mut db, None).unwrap().passages, 0);
    remove(&mut db).unwrap();
    assert!(exists(&db).unwrap()); // Invalidation triggers remain for a later rebuild.
    assert!(!has_stored_index(&db).unwrap());
    initialize(&mut db, &identity()).unwrap();
    assert!(has_stored_index(&db).unwrap());
}

fn put(db: &mut Connection, id: &str, title: &str, texts: &[&str]) {
    let parsed = ParsedDocument {
        title: title.into(),
        format: "html".into(),
        view_html: String::new(),
        sections: texts
            .iter()
            .enumerate()
            .map(|(index, text)| ParsedSection {
                heading: Some(format!("Section {index}")),
                text: (*text).into(),
                page_index: Some(index as u32),
                body_ranges: None,
            })
            .collect(),
        cover: None,
        assets: Vec::new(),
    };
    upsert_document(
        db,
        id,
        &format!("/uploads/{id}.html"),
        &parsed,
        None,
        StoredSourceKind::Html,
        1,
        1,
        PdfTextStatus::Ready,
    )
    .unwrap();
}

fn sync(db: &mut Connection, identity: &Identity, id: &str) -> (usize, usize) {
    let source = source(db, id).unwrap().unwrap();
    let mut embedded = 0;
    let mut reused = 0;
    synchronize(
        db,
        identity,
        &source,
        |_, _, text| Ok(vec![0..text.len()]),
        |_| Ok(vec![1.0, 0.0]),
        || Ok(()),
        |old| if old { reused += 1 } else { embedded += 1 },
    )
    .unwrap();
    (embedded, reused)
}

#[test]
fn optional_storage_reuses_passages_across_reimport_and_cascades_delete() {
    let library = Library::new();
    let mut db = library.db();
    put(
        &mut db,
        "abc123",
        "Transit guide",
        &["Electric buses", "😀 اتجاه المدينة", "城市交通"],
    );
    let identity = identity();
    assert!(
        !exists(&db).unwrap(),
        "normal import must not create semantic storage"
    );
    assert_eq!(
        coverage(&mut db, Some(&identity))
            .unwrap()
            .eligible_documents,
        1
    );
    assert!(
        !exists(&db).unwrap(),
        "coverage must not create semantic storage"
    );
    initialize(&mut db, &identity).unwrap();
    assert_eq!(sync(&mut db, &identity, "abc123"), (3, 0));
    assert_eq!(
        sync(&mut db, &identity, "abc123"),
        (0, 0),
        "fully indexed documents are skipped"
    );

    // Reopening through the normal store path retains triggers without any provider.
    drop(db);
    let mut db = library.db();
    put(
        &mut db,
        "abc123",
        "Transit guide",
        &["Electric buses", "😀 اتجاه المدينة", "城市交通"],
    );
    assert_eq!(
        coverage(&mut db, Some(&identity))
            .unwrap()
            .indexed_documents,
        0
    );
    assert_eq!(
        sync(&mut db, &identity, "abc123"),
        (0, 3),
        "replacement section row IDs must not force inference"
    );
    db.execute(
        "UPDATE uploaded_sections SET ordinal=ordinal+10 WHERE document_id='abc123'",
        [],
    )
    .unwrap();
    assert_eq!(
        sync(&mut db, &identity, "abc123"),
        (0, 3),
        "identical embedded content reuses vectors after locator shifts"
    );

    put(
        &mut db,
        "abc123",
        "Transit guide",
        &["Electric buses", "Updated street map"],
    );
    assert_eq!(sync(&mut db, &identity, "abc123"), (1, 1));
    assert_eq!(
        coverage(&mut db, Some(&identity)).unwrap().passages,
        2,
        "removed sections are pruned"
    );
    update_document_title(&mut db, "/uploads/abc123.html", "Travel guide").unwrap();
    assert_eq!(
        coverage(&mut db, Some(&identity))
            .unwrap()
            .indexed_documents,
        0
    );
    assert_eq!(
        sync(&mut db, &identity, "abc123"),
        (2, 0),
        "embedded metadata changes require fresh vectors"
    );

    delete_document_rows(&mut db, "abc123").unwrap();
    let coverage = coverage(&mut db, Some(&identity)).unwrap();
    assert_eq!(
        (
            coverage.indexed_documents,
            coverage.eligible_documents,
            coverage.passages
        ),
        (0, 0, 0)
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM upload_semantic_passages", [], |row| {
            row.get::<_, usize>(0)
        })
        .unwrap(),
        0
    );
}

#[test]
fn cancellation_hides_partial_documents_and_reuses_checkpoints_after_restart() {
    let library = Library::new();
    let mut db = library.db();
    put(
        &mut db,
        "abc123",
        "Guide",
        &["One passage", "Second passage"],
    );
    let identity = identity();
    initialize(&mut db, &identity).unwrap();
    let snapshot = source(&mut db, "abc123").unwrap().unwrap();
    let cancelled = Cell::new(false);
    let result = synchronize(
        &mut db,
        &identity,
        &snapshot,
        |_, _, text| Ok(vec![0..text.len()]),
        |_| Ok(vec![1.0, 0.0]),
        || {
            if cancelled.get() {
                Err("cancelled".into())
            } else {
                Ok(())
            }
        },
        |_| cancelled.set(true),
    );
    assert_eq!(result.err().unwrap(), "cancelled");
    assert_eq!(
        coverage(&mut db, Some(&identity))
            .unwrap()
            .indexed_documents,
        0
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM upload_semantic_passages", [], |row| {
            row.get::<_, usize>(0)
        })
        .unwrap(),
        1
    );
    drop(db);
    let mut db = library.db();
    assert_eq!(sync(&mut db, &identity, "abc123"), (1, 1));
    assert_eq!(
        coverage(&mut db, Some(&identity))
            .unwrap()
            .indexed_documents,
        1
    );
}

#[test]
fn concurrent_edits_deletion_and_identity_roundtrips_reject_stale_writers() {
    let library = Library::new();
    let mut db = library.db();
    let mut writer = library.db();
    let identity = identity();
    put(&mut db, "abc123", "Guide", &["Original directions"]);
    initialize(&mut db, &identity).unwrap();
    let snapshot = source(&mut db, "abc123").unwrap().unwrap();
    let result = synchronize(
        &mut db,
        &identity,
        &snapshot,
        |_, _, text| Ok(vec![0..text.len()]),
        |_| {
            put(&mut writer, "abc123", "Guide", &["Changed directions"]);
            Ok(vec![1.0, 0.0])
        },
        || Ok(()),
        |_| {},
    );
    assert!(result.err().unwrap().contains("source or model changed"));
    assert_eq!(coverage(&mut db, Some(&identity)).unwrap().passages, 0);

    let snapshot = source(&mut db, "abc123").unwrap().unwrap();
    delete_document_rows(&mut writer, "abc123").unwrap();
    put(&mut writer, "abc123", "Guide", &["Changed directions"]);
    assert!(
        check_current(&db, &snapshot, &identity).is_err(),
        "delete/recreate must not resurrect an old job"
    );

    let snapshot = source(&mut db, "abc123").unwrap().unwrap();
    remove(&mut writer).unwrap();
    initialize(&mut writer, &identity).unwrap();
    assert!(
        check_current(&db, &snapshot, &identity).is_err(),
        "remove/rebuild with the same key still invalidates a writer"
    );
    let snapshot = source(&mut db, "abc123").unwrap().unwrap();
    initialize(&mut writer, &Identity::new("other-model", 2).unwrap()).unwrap();
    initialize(&mut writer, &identity).unwrap();
    assert!(
        check_current(&db, &snapshot, &identity).is_err(),
        "switching away and back must not accept old work"
    );
}

#[test]
fn source_spans_and_vectors_are_validated_before_publishing() {
    let library = Library::new();
    let mut db = library.db();
    put(&mut db, "abc123", "Guide", &["😀 اتجاه"]);
    let identity = identity();
    initialize(&mut db, &identity).unwrap();
    let snapshot = source(&mut db, "abc123").unwrap().unwrap();
    assert!(synchronize(
        &mut db,
        &identity,
        &snapshot,
        |_, _, _| Ok(vec![1..4]),
        |_| panic!("UTF-8 boundary must fail before inference"),
        || Ok(()),
        |_| {},
    )
    .is_err());
    assert!(synchronize(
        &mut db,
        &identity,
        &snapshot,
        |_, _, _| Ok(vec![]),
        |_| panic!("empty spans must not publish coverage"),
        || Ok(()),
        |_| {},
    )
    .is_err());
    for vector in [
        vec![1.0],
        vec![f32::NAN, 0.0],
        vec![f32::INFINITY, 0.0],
        vec![0.0, 0.0],
        vec![2.0, 0.0],
    ] {
        assert!(synchronize(
            &mut db,
            &identity,
            &snapshot,
            |_, _, text| Ok(vec![0..text.len()]),
            |_| Ok(vector.clone()),
            || Ok(()),
            |_| {},
        )
        .is_err());
    }
    assert_eq!(coverage(&mut db, Some(&identity)).unwrap().passages, 0);
    assert_eq!(sync(&mut db, &identity, "abc123"), (1, 0));
    let snapshot = source(&mut db, "abc123").unwrap().unwrap();
    db.execute(
        "UPDATE uploaded_sections SET page_index=7 WHERE document_id='abc123'",
        [],
    )
    .unwrap();
    assert!(check_current(&db, &snapshot, &identity).is_err());
    assert_eq!(
        sync(&mut db, &identity, "abc123"),
        (0, 1),
        "locator changes reuse identical embedded text"
    );
    db.execute(
        "UPDATE uploaded_sections SET text='😀 اتجاه',page_index=8 WHERE document_id='abc123'",
        [],
    )
    .unwrap();
    let changed_vector = [0.0f32, 1.0]
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect::<Vec<_>>();
    db.execute(
        "UPDATE upload_semantic_passages SET vector=?1",
        [changed_vector],
    )
    .unwrap();
    let snapshot = source(&mut db, "abc123").unwrap().unwrap();
    assert!(
        synchronize(
            &mut db,
            &identity,
            &snapshot,
            |_, _, text| Ok(vec![0..text.len()]),
            |_| panic!("corrupt cache must fail explicitly"),
            || Ok(()),
            |_| {},
        )
        .is_err(),
        "even a changed unit-normalized vector fails its checksum"
    );
    assert_eq!(
        coverage(&mut db, Some(&identity))
            .unwrap()
            .indexed_documents,
        0
    );
}

#[test]
fn incompatible_identity_rebuilds_only_derived_data_and_unknown_schema_fails_closed() {
    let library = Library::new();
    let mut db = library.db();
    put(&mut db, "abc123", "Guide", &["Source body"]);
    let identity = identity();
    initialize(&mut db, &identity).unwrap();
    sync(&mut db, &identity, "abc123");
    let other = Identity::new("synthetic-encoder/tokenizer-v2", 2).unwrap();
    assert_eq!(
        coverage(&mut db, Some(&other)).unwrap().indexed_documents,
        0
    );
    initialize(&mut db, &other).unwrap();
    assert_eq!(sync(&mut db, &other, "abc123"), (1, 0));
    assert_eq!(
        db.query_row("SELECT text FROM uploaded_sections", [], |row| row
            .get::<_, String>(0))
            .unwrap(),
        "Source body"
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM uploaded_document_fts", [], |row| {
            row.get::<_, usize>(0)
        })
        .unwrap(),
        1
    );
    db.execute("UPDATE upload_semantic_metadata SET schema_version=99", [])
        .unwrap();
    assert!(initialize(&mut db, &identity).is_err());
    assert!(remove(&mut db).is_err());
    assert_eq!(
        coverage(&mut db, Some(&other)).unwrap().indexed_documents,
        0
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM upload_semantic_passages", [], |row| {
            row.get::<_, usize>(0)
        })
        .unwrap(),
        1,
        "unknown schema must not be destructively reset"
    );
}

#[test]
#[cfg(feature = "experimental-search-embeddings-core")]
fn changing_model_capacity_requires_explicit_rebuild_and_never_reuses_vectors() {
    use crate::search_embeddings::ModelSelection;
    let library = Library::new();
    let mut db = library.db();
    put(
        &mut db,
        "guide",
        "Transport Guide",
        &["The city operates electric buses."],
    );
    for format in [
        ModelSelection::Float32,
        ModelSelection::BaseFloat32,
        ModelSelection::LargeFloat32,
        ModelSelection::Float32,
    ] {
        let identity = Identity::new(
            &serde_json::to_string(&format).unwrap(),
            format.dimensions(),
        )
        .unwrap();
        assert_eq!(
            coverage(&mut db, Some(&identity))
                .unwrap()
                .indexed_documents,
            0
        );
        let response = retrieve(
            &mut db,
            &identity,
            &request(json!({"query":"city transport"})),
            |_| panic!("incompatible or absent vectors must not infer"),
            || Ok(()),
        )
        .unwrap();
        assert!(serde_json::to_value(response).unwrap()["results"]
            .as_array()
            .unwrap()
            .is_empty());
        initialize(&mut db, &identity).unwrap();
        let source = source(&mut db, "guide").unwrap().unwrap();
        let mut vector = vec![0.0; identity.dimensions];
        vector[0] = 1.0;
        let mut reused = 0;
        synchronize(
            &mut db,
            &identity,
            &source,
            |_, _, body| Ok(vec![0..body.len()]),
            |_| Ok(vector.clone()),
            || Ok(()),
            |old| {
                if old {
                    reused += 1
                }
            },
        )
        .unwrap();
        assert_eq!(reused, 0);
        assert_eq!(
            coverage(&mut db, Some(&identity))
                .unwrap()
                .indexed_documents,
            1
        );
        let response = retrieve(
            &mut db,
            &identity,
            &request(json!({"query":"city transport"})),
            |_| Ok(vector.clone()),
            || Ok(()),
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(response).unwrap()["results"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            db.query_row("SELECT text FROM uploaded_sections", [], |row| row
                .get::<_, String>(0))
                .unwrap(),
            "The city operates electric buses."
        );
    }
}

/// Saturated synthetic vectors isolate production scan cost, not model inference or relevance.
#[test]
#[ignore = "development-host scale probe: creates 100,000 synthetic 384-dimensional passages"]
fn conceptual_query_scale() {
    use std::time::{Duration, Instant};
    let library = Library::new();
    let mut db = library.db();
    let documents = 200;
    let sections = 500;
    let title = "Transit guide";
    let text = "Electric buses connect the station and harbor.";
    for index in 0..documents {
        put(
            &mut db,
            &format!("doc{index:03}"),
            title,
            &vec![text; sections],
        );
    }
    let key = Identity::new("synthetic-scale/tokenizer-v1", 384).unwrap();
    initialize(&mut db, &key).unwrap();
    let mut vector = vec![0.0f32; 384];
    vector[0] = 1.0;
    let bytes = vector
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect::<Vec<_>>();
    let checksum = format!("{:x}", Sha256::digest(&bytes));
    let hashes = (0..sections)
        .map(|section| {
            format!(
                "{:x}",
                Sha256::digest(
                    passage_input(title, Some(&format!("Section {section}")), text).as_bytes()
                )
            )
        })
        .collect::<Vec<_>>();
    let tx = db.transaction().unwrap();
    {
        let mut insert = tx
            .prepare(
                "INSERT INTO upload_semantic_passages
            (document_id,section_ordinal,byte_start,byte_end,content_hash,vector,vector_hash)
            VALUES (?1,?2,0,?3,?4,?5,?6)",
            )
            .unwrap();
        for document in 0..documents {
            for (section, hash) in hashes.iter().enumerate() {
                insert
                    .execute(params![
                        format!("doc{document:03}"),
                        section,
                        text.len(),
                        hash,
                        bytes,
                        checksum
                    ])
                    .unwrap();
            }
        }
    }
    tx.execute("UPDATE upload_semantic_documents SET ready=1", [])
        .unwrap();
    tx.commit().unwrap();
    db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)").unwrap();
    drop(db);
    let path = library.0.join("search.sqlite3");
    let sqlite_bytes = std::fs::metadata(&path).unwrap().len();
    let mut db =
        Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let mut measurements = Vec::new();
    for (name, input, expected_scans, expected_results) in [
        (
            "first",
            json!({"query":"city travel"}),
            documents * sections,
            50,
        ),
        (
            "warm",
            json!({"query":"city travel"}),
            documents * sections,
            50,
        ),
        (
            "scoped",
            json!({"query":"city travel","documentUrls":["/uploads/doc199.html"]}),
            sections,
            1,
        ),
        (
            "common-phrase",
            json!({"query":"city travel","exactPhrases":["electric buses"]}),
            documents * sections,
            50,
        ),
        (
            "missing-phrase",
            json!({"query":"city travel","exactPhrases":["missing service"]}),
            0,
            0,
        ),
    ] {
        let start = Instant::now();
        let encoded = Cell::new(0);
        let response = retrieve(
            &mut db,
            &key,
            &request(input),
            |_| {
                encoded.set(encoded.get() + 1);
                Ok(vector.clone())
            },
            || {
                if start.elapsed() > Duration::from_secs(10) {
                    Err("Scale probe exceeded the native query budget".into())
                } else {
                    Ok(())
                }
            },
        )
        .unwrap();
        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        let response = serde_json::to_value(response).unwrap();
        assert_eq!(response["availability"], "ready");
        assert_eq!(response["scannedPassages"], expected_scans);
        assert_eq!(
            response["results"].as_array().unwrap().len(),
            expected_results
        );
        assert_eq!(encoded.get(), usize::from(expected_scans > 0));
        if expected_results == 50 {
            assert_eq!(response["results"][0]["documentId"], "doc000");
        }
        measurements.push(
            json!({"case":name,"ms":elapsed_ms,"scanned":expected_scans,"shown":expected_results}),
        );
    }
    assert!(db
        .execute("UPDATE upload_semantic_documents SET ready=0", [])
        .is_err());
    // Includes fixture construction and all scans; this is not query-only or model memory.
    let process_peak_rss_kib =
        std::fs::read_to_string("/proc/self/status")
            .ok()
            .and_then(|status| {
                status.lines().find_map(|line| {
                    line.strip_prefix("VmHWM:")
                        .and_then(|value| value.split_whitespace().next()?.parse::<u64>().ok())
                })
            });
    println!(
        "conceptual_scale {}",
        json!({"documents":documents,"passages":documents * sections,
        "dimensions":384,"sqliteBytes":sqlite_bytes,"optimized":!cfg!(debug_assertions),
        "processPeakRssKiB":process_peak_rss_kib,
        "method":"Synthetic unit vectors; production read-only retrieve; no inference/quality grading; first connection has OS-warm fixture pages",
        "measurements":measurements})
    );
}
