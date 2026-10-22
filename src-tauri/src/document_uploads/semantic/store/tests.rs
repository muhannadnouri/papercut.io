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
    let index = PageTextLayer {
        page_index: 2,
        blocks: [
            "Anchors (equipment),",
            "12, 24",
            "Anchor lights (see Beacons)",
            "Berths (allocation), 15–18",
            "Cargo handling, 19, 21",
            "Dock schedules, 30",
            "Electric ferries, 35",
            "Freight routes, 41",
            "Harbor maps, 50",
            "Island service, 52",
        ]
        .iter()
        .enumerate()
        .map(|(i, text)| PageTextBlock {
            text: format!("{text}\n"),
            bounds: [40.0, 100.0 + i as f32 * 16.0, 500.0, 12.0],
            order: i as u32,
            confidence: None,
        })
        .collect(),
        ..page.clone()
    };
    let index_text = narration::reconstruct_search_text(index.clone());
    put(
        &mut db,
        "abc123",
        "Harbor Guide",
        &["港 Ferries connect the island.", &references, &index_text],
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
    // Schema 10 could persist wrapped index entries as body; both upgrades preserve authored rows.
    for (ordinal, text) in ["港 Ferries connect the island.", &references, &index_text]
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
    page_text::write_page_text_layer(&library.0.join("abc123"), &index).unwrap();
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
        "UPDATE upload_schema_metadata SET value='10' WHERE key='schema_version'",
        [],
    )
    .unwrap();
    drop(db);

    let mut db = library.db();
    assert_eq!(
        db.query_row(
            "SELECT value FROM upload_schema_metadata WHERE key='schema_version'",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        "13"
    );
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
    assert_eq!(pending, 3);
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
    assert_eq!(
        db.query_row(
            "SELECT body_ranges FROM uploaded_sections WHERE document_id='abc123' AND ordinal=2",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        "[]"
    );
    // A missing body-page sidecar retains its text; classified citations stay literal-searchable.
    assert_eq!(db.query_row("SELECT count(*) FROM uploaded_document_fts WHERE uploaded_document_fts MATCH '\"Sapphire Ferry Guide\"'", [], |row| row.get::<_,usize>(0)).unwrap(), 1);
    assert_eq!(db.query_row("SELECT count(*) FROM uploaded_document_search_fts WHERE uploaded_document_search_fts MATCH 'sapphire'", [], |row| row.get::<_,usize>(0)).unwrap(), 1);
    assert_eq!(db.query_row("SELECT count(*) FROM uploaded_document_fts WHERE uploaded_document_fts MATCH 'anchors'", [], |row| row.get::<_,usize>(0)).unwrap(), 1);
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
    let stale = retrieve(
        &mut db,
        &key,
        &request(json!({"query":"harbor travel", "documentUrls":["/uploads/abc123.html"]})),
        |_| panic!("a stale PDF must not invoke query inference"),
        || Ok(()),
    )
    .unwrap();
    let stale = serde_json::to_value(stale).unwrap();
    assert_eq!(stale["availability"], "partial");
    assert!(stale["results"].as_array().unwrap().is_empty());
    sync(&mut db, &key, "abc123");
    assert_eq!(coverage(&mut db, Some(&key)).unwrap().indexed_documents, 2);
    let hit = retrieve(&mut db, &key, &request(json!({"query":"harbor travel", "exactPhrases":["Sapphire Ferry Guide", "Anchors (equipment)"], "documentUrls":["/uploads/abc123.html"]})), |_| Ok(vec![1.0,0.0]), || Ok(())).unwrap();
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
fn unicode_pdf_upgrade_repairs_fts_and_spelling_but_requires_explicit_vector_update() {
    use crate::document_uploads::{
        pdf::{
            narration,
            page_text::{self, PageTextBlock, PageTextLayer},
        },
        search::search_uploads_with_db,
        store::{rebuild_search_content_batch, rebuild_spelling_batch},
        types::UploadedDocumentSearchRequest,
    };
    let library = Library::new();
    let mut db = library.db();
    let page = PageTextLayer {
        schema_version: 1,
        page_index: 0,
        width: 600.0,
        height: 800.0,
        blocks: vec![
            PageTextBlock {
                text: "港 A high‐\n".into(),
                bounds: [40.0, 100.0, 500.0, 12.0],
                order: 0,
                confidence: None,
            },
            PageTextBlock {
                text: "light guides the ferry.".into(),
                bounds: [40.0, 117.0, 500.0, 12.0],
                order: 1,
                confidence: None,
            },
        ],
    };
    let legacy = narration::legacy_search_text(page.clone());
    let repaired = narration::reconstruct_search_text(page.clone());
    let second = PageTextLayer {
        page_index: 1,
        blocks: vec![
            PageTextBlock {
                text: "A beau‑\n".into(),
                ..page.blocks[0].clone()
            },
            PageTextBlock {
                text: "tiful harbor.".into(),
                ..page.blocks[1].clone()
            },
        ],
        ..page.clone()
    };
    let legacy_second = narration::legacy_search_text(second.clone());
    put(
        &mut db,
        "abc123",
        "Harbor Guide",
        &[
            &legacy,
            &legacy_second,
            "Saved OCR text.",
            "A missing sidecar.",
        ],
    );
    put(
        &mut db,
        "def456",
        "EPUB Guide",
        &["A high‐\nlight EPUB note."],
    );
    db.execute(
        "UPDATE uploaded_documents SET format='pdf',source_kind='pdf' WHERE id='abc123'",
        [],
    )
    .unwrap();
    db.execute(
        "UPDATE uploaded_documents SET format='epub',source_kind='epub' WHERE id='def456'",
        [],
    )
    .unwrap();
    db.execute(
        "UPDATE uploaded_sections SET body_ranges='[]' WHERE document_id='def456'",
        [],
    )
    .unwrap();
    page_text::write_page_text_layer(&library.0.join("abc123"), &page).unwrap();
    page_text::write_page_text_layer(&library.0.join("abc123"), &second).unwrap();
    let mismatched = PageTextLayer {
        page_index: 2,
        ..page.clone()
    };
    page_text::write_page_text_layer(&library.0.join("abc123"), &mismatched).unwrap();
    let source_path = library.0.join("abc123/source.pdf");
    std::fs::write(&source_path, b"Original PDF source bytes").unwrap();
    let locators = |db: &Connection| -> Vec<(i64, String, usize, Option<u32>)> {
        db.prepare("SELECT id,document_id,ordinal,page_index FROM uploaded_sections ORDER BY id")
            .unwrap()
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    // Schema 11 persisted body roles before these Unicode wraps were repaired.
    for (ordinal, text) in [
        &legacy,
        &legacy_second,
        "Saved OCR text.",
        "A missing sidecar.",
    ]
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
    put(
        &mut db,
        "fed789",
        "Unaffected PDF",
        &["A state‐owned archive."],
    );
    db.execute(
        "UPDATE uploaded_documents SET format='pdf',source_kind='pdf' WHERE id='fed789'",
        [],
    )
    .unwrap();
    db.execute(
        "UPDATE uploaded_sections SET body_ranges=?1 WHERE document_id='fed789'",
        [serde_json::to_string(&vec![0.."A state‐owned archive.".len()]).unwrap()],
    )
    .unwrap();
    let before = locators(&db);
    let lexical = |mode: &str, urls: Value, phrase: &str| {
        let input: UploadedDocumentSearchRequest = serde_json::from_value(json!({
            "query": format!("\"{phrase}\""), "exactPhrases": [phrase], "mode": mode, "documentUrls": urls, "limit": 10
        })).unwrap();
        search_uploads_with_db(input, || Ok(library.db()), |_| {})
            .unwrap()
            .0
    };
    assert_eq!(
        lexical("all", Value::Null, "A highlight guides the ferry").total_documents,
        0
    );
    let key = identity();
    initialize(&mut db, &key).unwrap();
    sync(&mut db, &key, "abc123");
    sync(&mut db, &key, "def456");
    sync(&mut db, &key, "fed789");
    db.execute(
        "UPDATE upload_schema_metadata SET value='11' WHERE key='schema_version'",
        [],
    )
    .unwrap();
    drop(db);
    let mut db = library.db();
    assert_eq!(coverage(&mut db, Some(&key)).unwrap().indexed_documents, 2);
    assert_eq!(db.query_row("SELECT count(*) FROM uploaded_sections WHERE document_id='fed789' AND body_ranges IS NULL", [], |r|r.get::<_,usize>(0)).unwrap(), 0);
    // A mid-document write error rolls back both FTS projections and canonical text.
    db.execute_batch(
        "CREATE TEMP TRIGGER fail_pdf_repair BEFORE UPDATE ON uploaded_sections
        WHEN old.document_id='abc123' AND old.ordinal=1 AND old.text<>new.text
        BEGIN SELECT RAISE(ABORT,'repair interrupted'); END;",
    )
    .unwrap();
    assert!(rebuild_search_content_batch(&mut db, &library.0).is_err());
    assert_eq!(
        db.query_row(
            "SELECT text FROM uploaded_sections WHERE document_id='abc123' AND ordinal=0",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        legacy
    );
    assert_eq!(
        lexical("all", Value::Null, "A highlight guides the ferry").total_documents,
        0
    );
    db.execute_batch("DROP TRIGGER fail_pdf_repair;").unwrap();
    assert!(rebuild_search_content_batch(&mut db, &library.0).unwrap());
    assert!(!rebuild_search_content_batch(&mut db, &library.0).unwrap());
    assert_eq!(locators(&db), before);
    let texts: Vec<String> = db
        .prepare("SELECT text FROM uploaded_sections WHERE document_id='abc123' ORDER BY ordinal")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        texts,
        [
            repaired,
            narration::reconstruct_search_text(second.clone()),
            "Saved OCR text.".into(),
            "A missing sidecar.".into()
        ]
    );
    assert_eq!(
        std::fs::read(&source_path).unwrap(),
        b"Original PDF source bytes"
    );
    assert_eq!(
        serde_json::to_value(
            page_text::read_page_text_layer(&library.0.join("abc123"), 0).unwrap()
        )
        .unwrap(),
        serde_json::to_value(&page).unwrap()
    );
    assert_eq!(
        db.query_row(
            "SELECT body_ranges FROM uploaded_sections WHERE document_id='def456'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "[]"
    );
    for table in ["uploaded_document_fts", "uploaded_document_search_fts"] {
        assert_eq!(db.query_row(&format!("SELECT count(*) FROM {table} WHERE {table} MATCH '\"A highlight guides the ferry\"'"), [], |r|r.get::<_,usize>(0)).unwrap(), 1);
        assert_eq!(
            db.query_row(
                &format!("SELECT count(*) FROM {table} WHERE {table} MATCH 'beautiful'"),
                [],
                |r| r.get::<_, usize>(0)
            )
            .unwrap(),
            1
        );
    }
    assert_eq!(
        db.query_row(
            "SELECT value FROM upload_schema_metadata WHERE key='spelling_ready'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "0"
    );
    while rebuild_spelling_batch(&mut db).unwrap() {}
    assert_eq!(db.query_row("SELECT count(*) FROM uploaded_spelling_terms WHERE document_id='abc123' AND term='highlight'", [], |r|r.get::<_,usize>(0)).unwrap(), 1);
    assert_eq!(db.query_row("SELECT count(*) FROM uploaded_spelling_terms WHERE document_id='abc123' AND term='high'", [], |r|r.get::<_,usize>(0)).unwrap(), 0);
    for mode in ["all", "broader"] {
        assert_eq!(
            lexical(
                mode,
                json!(["/uploads/abc123.html"]),
                "A highlight guides the ferry"
            )
            .total_documents,
            1
        );
        assert_eq!(
            lexical(
                mode,
                json!(["/uploads/def456.html"]),
                "A highlight guides the ferry"
            )
            .total_documents,
            0
        );
        assert_eq!(
            lexical(mode, Value::Null, "A highlight guides an airport").total_documents,
            0
        );
    }
    let stale = retrieve(
        &mut db,
        &key,
        &request(json!({"query":"harbor travel", "documentUrls":["/uploads/abc123.html"]})),
        |_| panic!("stale vectors must not run inference"),
        || Ok(()),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(stale).unwrap()["availability"],
        "partial"
    );
    let (encoded, _) = sync(&mut db, &key, "abc123");
    assert!(encoded > 0);
    assert_eq!(coverage(&mut db, Some(&key)).unwrap().indexed_documents, 3);
    let hit = retrieve(&mut db, &key, &request(json!({"query":"harbor travel", "exactPhrases":["A highlight guides the ferry"], "documentUrls":["/uploads/abc123.html"]})), |_| Ok(vec![1.0,0.0]), || Ok(())).unwrap();
    assert_eq!(
        serde_json::to_value(hit).unwrap()["results"][0]["text"],
        texts[0]
    );
    drop(db);
    let mut db = library.db();
    assert!(!rebuild_search_content_batch(&mut db, &library.0).unwrap());
    assert!(!rebuild_spelling_batch(&mut db).unwrap());
    assert_eq!(coverage(&mut db, Some(&key)).unwrap().indexed_documents, 3);
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

#[test]
fn inline_role_upgrade_preserves_sources_invalidates_vectors_and_keeps_notes_literal_searchable() {
    use crate::document_uploads::{html::parse_html_document, store::rebuild_search_content_batch};
    let library = Library::new();
    let mut db = library.db();
    let mut parsed = parse_html_document(
        r#"<p>港 Ferry routes. <span epub:type="footnote">Sapphire citation.</span> Island stops.</p>"#,
    );
    let saved_html = parsed.view_html.clone();
    let text = parsed.sections[0].text.clone();
    let corrected = parsed.sections[0].body_ranges.clone().unwrap();
    parsed.sections[0].body_ranges = Some(vec![0..text.len()]);
    for id in ["abc123", "def456", "fed789"] {
        parsed.format = if id == "abc123" { "epub" } else { "html" }.into();
        upsert_document(
            &mut db,
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
    // Missing/mismatched saved sources must not destroy known classifications.
    db.execute(
        "UPDATE uploaded_sections SET body_ranges='[]' WHERE document_id<>'abc123'",
        [],
    )
    .unwrap();
    std::fs::create_dir_all(library.0.join("abc123")).unwrap();
    std::fs::write(library.0.join("abc123/source.html"), &saved_html).unwrap();
    std::fs::create_dir_all(library.0.join("fed789")).unwrap();
    std::fs::write(
        library.0.join("fed789/source.html"),
        "<p>Different stored source.</p>",
    )
    .unwrap();
    let key = identity();
    initialize(&mut db, &key).unwrap();
    for id in ["abc123", "def456", "fed789"] {
        sync(&mut db, &key, id);
    }
    let before: Vec<(i64, usize, String)> = db
        .prepare("SELECT id,ordinal,text FROM uploaded_sections ORDER BY id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    db.execute(
        "UPDATE upload_schema_metadata SET value='12' WHERE key='schema_version'",
        [],
    )
    .unwrap();
    drop(db);
    let mut db = library.db();
    assert!(rebuild_search_content_batch(&mut db, &library.0).unwrap());
    assert_eq!(coverage(&mut db, Some(&key)).unwrap().indexed_documents, 2);
    // A restart continues the repair without re-queuing already repaired documents.
    drop(db);
    let mut db = library.db();
    while rebuild_search_content_batch(&mut db, &library.0).unwrap() {}
    assert!(!rebuild_search_content_batch(&mut db, &library.0).unwrap());
    let after: Vec<(i64, usize, String)> = db
        .prepare("SELECT id,ordinal,text FROM uploaded_sections ORDER BY id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(before, after);
    assert_eq!(
        std::fs::read_to_string(library.0.join("abc123/source.html")).unwrap(),
        saved_html
    );
    assert_eq!(
        source(&mut db, "abc123").unwrap().unwrap().sections[0].body_ranges,
        corrected
    );
    for id in ["def456", "fed789"] {
        assert!(source(&mut db, id).unwrap().unwrap().sections[0]
            .body_ranges
            .is_empty());
    }
    sync(&mut db, &key, "abc123");
    let found = retrieve(&mut db, &key, &request(json!({"query":"island travel", "exactPhrases":["Sapphire citation"], "documentUrls":["/uploads/abc123.html"]})), |_| Ok(vec![1.0,0.0]), || Ok(())).unwrap();
    let found = serde_json::to_value(found).unwrap();
    assert!(!found["results"].as_array().unwrap().is_empty());
    assert!(!found["results"][0]["text"]
        .as_str()
        .unwrap()
        .contains("citation"));
    let literal = crate::document_uploads::search::search_uploads_with_db(
        serde_json::from_value(
            json!({"query":"", "exactPhrases":["Sapphire citation"], "limit":10}),
        )
        .unwrap(),
        || Ok(library.db()),
        |_| {},
    )
    .unwrap()
    .0;
    assert!(!serde_json::to_value(literal).unwrap()["results"]
        .as_array()
        .unwrap()
        .is_empty());
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
    assert_eq!(auxiliary["results"][0]["context"], json!([]));
}

/// Nearby evidence keeps independent anchors and does not turn omitted notes into synthetic prose.
#[test]
fn conceptual_context_is_bounded_body_text_in_source_order_with_individual_anchors() {
    let library = Library::new();
    let mut db = library.db();
    let large = "🙂港".repeat(3000);
    let mut texts = vec![
        "Far side of notes",
        "Note only",
        "Before four",
        "Before three",
        "Before two",
        "Before one",
        "港 Before. Note. 港 Intro. MATCH Qualification. Note. 港 After. Footer",
        "Note. Following body.",
        "Following two",
        "Following three",
        "Following four",
        "Too far",
    ];
    put(&mut db, "abc123", "Transit", &texts);
    put(&mut db, "other", "Other", &["Different document text"]);
    db.execute(
        "UPDATE uploaded_sections SET body_ranges='[]' WHERE document_id='abc123' AND ordinal=1",
        [],
    )
    .unwrap();
    let primary = texts[6];
    let next = texts[7];
    for (ordinal, ranges) in [
        (
            6,
            vec![
                0..primary.find(" Note.").unwrap(),
                primary.find("港 Intro.").unwrap()..primary.rfind(" Note.").unwrap(),
                primary.find("港 After.").unwrap()..primary.find(" Footer").unwrap(),
            ],
        ),
        (7, vec![next.find("Following").unwrap()..next.len()]),
    ] {
        db.execute(
            "UPDATE uploaded_sections SET body_ranges=?1 WHERE document_id='abc123' AND ordinal=?2",
            params![serde_json::to_string(&ranges).unwrap(), ordinal],
        )
        .unwrap();
    }
    let key = identity();
    initialize(&mut db, &key).unwrap();
    let index = |db: &mut Connection| {
        let snapshot = source(db, "abc123").unwrap().unwrap();
        synchronize(
            db,
            &key,
            &snapshot,
            |_, _, text| {
                Ok(vec![if let Some(start) = text.find("MATCH") {
                    start..start + 5
                } else {
                    0..text.len()
                }])
            },
            |text| {
                Ok(if text.ends_with("MATCH") {
                    vec![1.0, 0.0]
                } else {
                    vec![0.0, 1.0]
                })
            },
            || Ok(()),
            |_| {},
        )
        .unwrap();
    };
    index(&mut db);
    let changes = db.total_changes();
    let result = conceptual(
        &mut db,
        json!({"query":"travel", "documentUrls":["/uploads/abc123.html"]}),
    );
    let hit = &result["results"][0];
    assert_eq!(hit["text"], "MATCH");
    let context = hit["context"].as_array().unwrap();
    assert_eq!(context.len(), 10);
    assert_eq!(context.first().unwrap()["sectionIndex"], 3);
    assert_eq!(context.last().unwrap()["sectionIndex"], 9);
    assert!(context.iter().any(|span| span["text"] == "港 Before."));
    assert!(context.iter().any(|span| span["text"] == "港 Intro. "));
    assert!(context.iter().any(|span| span["text"] == " Qualification."));
    assert!(context.iter().any(|span| span["text"] == "港 After."));
    for span in context {
        let ordinal = span["sectionIndex"].as_u64().unwrap() as usize;
        assert_eq!(span["pageIndex"], ordinal);
        assert_eq!(span["sectionTitle"], format!("Section {ordinal}"));
        let start = span["byteStart"].as_u64().unwrap() as usize;
        let end = span["byteEnd"].as_u64().unwrap() as usize;
        assert_eq!(&texts[ordinal][start..end], span["text"].as_str().unwrap());
        assert!(!span["text"].as_str().unwrap().contains("Note"));
        assert!(!span["text"].as_str().unwrap().contains("Footer"));
    }
    assert_eq!(
        db.total_changes(),
        changes,
        "context retrieval must remain read-only"
    );

    db.execute("UPDATE uploaded_sections SET body_ranges='[]' WHERE document_id='abc123' AND ordinal IN (5,9)", []).unwrap();
    index(&mut db);
    let guarded = conceptual(
        &mut db,
        json!({"query":"travel", "documentUrls":["/uploads/abc123.html"]}),
    );
    assert_eq!(guarded["results"][0]["sectionIndex"], 6);
    assert_eq!(
        guarded["results"][0]["context"].as_array().unwrap().len(),
        6
    );
    for span in guarded["results"][0]["context"].as_array().unwrap() {
        let ordinal = span["sectionIndex"].as_u64().unwrap();
        assert!(
            (6..9).contains(&ordinal),
            "context must stop at auxiliary-only sections"
        );
    }

    // Large multilingual blocks exercise suffix/prefix clipping on UTF-8 boundaries and both budgets.
    texts[5] = &large;
    texts[7] = &large;
    put(&mut db, "abc123", "Transit", &texts);
    index(&mut db);
    let result = conceptual(
        &mut db,
        json!({"query":"travel", "documentUrls":["/uploads/abc123.html"]}),
    );
    let spans = result["results"][0]["context"].as_array().unwrap();
    assert!(spans.len() <= 10);
    assert!(
        spans
            .iter()
            .map(|span| span["text"].as_str().unwrap().chars().count())
            .sum::<usize>()
            <= 8192
    );
    for span in spans {
        let body = span["text"].as_str().unwrap();
        assert!(body.chars().count() <= 2048);
        let ordinal = span["sectionIndex"].as_u64().unwrap() as usize;
        assert_eq!(
            &texts[ordinal][span["byteStart"].as_u64().unwrap() as usize
                ..span["byteEnd"].as_u64().unwrap() as usize],
            body
        );
    }
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
        source_formatting: None,
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

/// Real invalidation triggers must distinguish unchanged English evidence from changed embedding inputs.
#[test]
fn formatting_only_upsert_retains_ready_semantic_vectors_but_changed_inputs_invalidate() {
    let baseline = "<title>Harbor Guide</title><p>Electric ferries serve the harbor.</p>";
    let styled =
        "<title>Harbor Guide</title><p>Electric <b>ferries</b> serve the <i>harbor</i>.</p>";
    for (reason, changed) in [
        ("text", "<title>Harbor Guide</title><p>Solar ferries serve the harbor.</p>"),
        ("title", "<title>New Harbor Guide</title><p>Electric ferries serve the harbor.</p>"),
        ("body ranges", "<title>Harbor Guide</title><p role='doc-footnote'>Electric ferries serve the harbor.</p>"),
    ] {
        let library = Library::new();
        let mut db = library.db();
        let save = |db: &mut Connection, html: &str, at| {
            let parsed = crate::document_uploads::prepare_html_document(html);
            upsert_document(db, "abc123", "/uploads/abc123.html", &parsed, None,
                StoredSourceKind::Html, at, html.len() as u64, PdfTextStatus::Ready).unwrap();
        };
        let state = |db: &Connection| {
            db.query_row("SELECT m.generation,d.generation,d.ready FROM upload_semantic_metadata m JOIN upload_semantic_documents d ON d.document_id='abc123' WHERE m.id=1",
                [], |row| Ok((row.get::<_, i64>(0)?,row.get::<_, i64>(1)?,row.get::<_, bool>(2)?))).unwrap()
        };
        let passages = |db: &Connection| {
            db.prepare("SELECT section_ordinal,byte_start,byte_end,content_hash,vector,vector_hash FROM upload_semantic_passages WHERE document_id='abc123' ORDER BY section_ordinal,byte_start,byte_end").unwrap()
                .query_map([], |row| Ok((row.get::<_, i64>(0)?,row.get::<_, i64>(1)?,row.get::<_, i64>(2)?,row.get::<_, String>(3)?,row.get::<_, Vec<u8>>(4)?,row.get::<_, String>(5)?))).unwrap()
                .collect::<Result<Vec<_>, _>>().unwrap()
        };
        save(&mut db, baseline, 1);
        let identity = identity();
        initialize(&mut db, &identity).unwrap();
        assert_eq!(sync(&mut db, &identity, "abc123"), (1, 0));
        let before = state(&db);
        let indexed = passages(&db);
        assert!(before.2);
        assert_eq!(indexed.len(), 1);
        save(&mut db, styled, 2);
        assert_eq!(state(&db), before, "formatting only: {reason}");
        assert_eq!(passages(&db), indexed, "vectors and their input coordinates remain exact");
        assert!(source(&mut db, "abc123").unwrap().unwrap().indexed);
        assert_eq!(coverage(&mut db, Some(&identity)).unwrap().indexed_documents, 1);
        save(&mut db, changed, 3);
        let after = state(&db);
        assert!(!after.2, "changed {reason} must invalidate ready coverage");
        assert!(after.0 > before.0 && after.1 > before.1, "changed {reason} advances generations");
        assert_eq!(coverage(&mut db, Some(&identity)).unwrap().indexed_documents, 0);
    }
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
        1,
        "identical reimports retain their ready section/index projection"
    );
    assert_eq!(
        sync(&mut db, &identity, "abc123"),
        (0, 0),
        "unchanged section row IDs require neither inference nor vector synchronization"
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
fn indexing_failures_identify_the_section_and_keep_committed_work_reusable() {
    for (stage, reason) in [
        ("ranges", "Title/heading leaves too little room"),
        ("encode", "Embedding runtime failed"),
        (
            "vector",
            "Invalid semantic vector; expected finite unit-normalized values",
        ),
    ] {
        let library = Library::new();
        let mut db = library.db();
        put(
            &mut db,
            "abc123",
            "Private title",
            &["First private passage", "Second private passage"],
        );
        let identity = identity();
        initialize(&mut db, &identity).unwrap();
        let snapshot = source(&mut db, "abc123").unwrap().unwrap();
        let encoded = Cell::new(0);
        let failure = synchronize(
            &mut db,
            &identity,
            &snapshot,
            |_, _, text| {
                if stage == "ranges" && text.starts_with("Second") {
                    Err(reason.into())
                } else {
                    Ok(vec![0..text.len()])
                }
            },
            |_| {
                encoded.set(encoded.get() + 1);
                if encoded.get() == 2 {
                    if stage == "encode" {
                        return Err(reason.into());
                    }
                    if stage == "vector" {
                        return Ok(vec![f32::NAN, 0.0]);
                    }
                }
                Ok(vec![1.0, 0.0])
            },
            || Ok(()),
            |_| {},
        )
        .unwrap_err();
        assert_eq!(
            failure,
            format!("Semantic index document abc123, section 2: {reason}")
        );
        assert!(!failure.contains("private") && !failure.contains("Private"));
        assert_eq!(
            coverage(&mut db, Some(&identity))
                .unwrap()
                .indexed_documents,
            0
        );
        assert_eq!(sync(&mut db, &identity, "abc123"), (1, 1));
        assert_eq!(
            coverage(&mut db, Some(&identity))
                .unwrap()
                .indexed_documents,
            1
        );
    }
}

#[test]
fn malformed_source_roles_identify_the_document_and_section() {
    let library = Library::new();
    let mut db = library.db();
    put(
        &mut db,
        "abc123",
        "Private title",
        &["Valid body", "Private second body"],
    );
    initialize(&mut db, &identity()).unwrap();
    db.execute("UPDATE uploaded_sections SET body_ranges='[[0,999]]' WHERE document_id='abc123' AND ordinal=1", []).unwrap();
    let failure = source(&mut db, "abc123").err().unwrap();
    assert!(
        failure.starts_with("Semantic index document abc123, section 2: "),
        "{failure}"
    );
    assert!(!failure.contains("Private"));
    db.execute(
        "UPDATE uploaded_sections SET body_ranges=NULL WHERE document_id='abc123' AND ordinal=1",
        [],
    )
    .unwrap();
    assert!(source(&mut db, "abc123").unwrap().is_some());
}

#[test]
fn document_level_failures_do_not_claim_a_section() {
    let library = Library::new();
    let mut db = library.db();
    put(&mut db, "abc123", "Private title", &[" "]);
    let identity = identity();
    initialize(&mut db, &identity).unwrap();
    let snapshot = source(&mut db, "abc123").unwrap().unwrap();
    let failure = synchronize(
        &mut db,
        &identity,
        &snapshot,
        |_, _, _| panic!("empty sections must not reach the tokenizer"),
        |_| panic!("empty sections must not reach inference"),
        || Ok(()),
        |_| {},
    )
    .unwrap_err();
    assert_eq!(
        failure,
        "Semantic index document abc123: Document has no readable semantic passages"
    );
    db.execute("DROP TABLE uploaded_sections", []).unwrap();
    let failure = source(&mut db, "abc123").err().unwrap();
    assert!(
        failure.starts_with("Semantic index document abc123: "),
        "{failure}"
    );
    assert!(!failure.contains("section 1") && !failure.contains("Private"));
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
                Err("Semantic indexing cancelled".into())
            } else {
                Ok(())
            }
        },
        |_| cancelled.set(true),
    );
    assert_eq!(result.err().unwrap(), "Semantic indexing cancelled");
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
