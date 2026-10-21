//! Reject case aliases before file access while retaining exact legacy identities.
use super::{fixture, invoke, seed, seed_pdf, snapshot, SOURCE, VARIANT};
use crate::document_uploads::open_document_uploads_db;
use serde_json::{json, Value};
use std::fs;
use tauri::test::MockRuntime;

fn attempt(
    window: &tauri::WebviewWindow<MockRuntime>,
    route: &str,
    id: &str,
    kind: &str,
) -> Result<Value, Value> {
    match route {
        "translation" => invoke(
            window,
            "translation_delete_document",
            json!({"request":{"id":id}}),
        ),
        "single" => invoke(
            window,
            "document_uploads_delete",
            json!({"request":{"documentUrl":format!("/uploads/{id}.{kind}?reader=1#section")}}),
        ),
        _ => invoke(
            window,
            "document_uploads_delete_batch",
            json!({"request":{"documentUrls":[format!("/uploads/{id}.{kind}?reader=1#section")]}}),
        ),
    }
}

/// Batch commands retain per-item errors rather than rejecting the entire batch.
fn rejected(result: Result<Value, Value>) -> bool {
    result.is_err()
        || result.as_ref().unwrap()["failures"]
            .as_array()
            .is_some_and(|v| v.len() == 1)
}

#[test]
fn case_aliases_are_rejected_for_html_pdf_with_or_without_audio_and_without_mutating_rows_or_files()
{
    for kind in ["html", "pdf"] {
        for route in ["single", "batch"] {
            let (dir, app, window) = fixture();
            seed(app.handle(), SOURCE, None, "html");
            let id = if kind == "html" {
                seed(app.handle(), VARIANT, Some(SOURCE), "html");
                VARIANT.to_string()
            } else {
                seed_pdf(app.handle())
            };
            let mixed: String = id
                .chars()
                .enumerate()
                .map(|(n, c)| {
                    if n % 2 == 0 {
                        c.to_ascii_uppercase()
                    } else {
                        c
                    }
                })
                .collect();
            let aliases = [id.to_ascii_uppercase(), mixed];
            for alias in &aliases {
                let before = snapshot(&dir.0, app.handle());
                assert!(
                    rejected(attempt(&window, route, alias, kind)),
                    "accepted {alias}"
                );
                assert_eq!(snapshot(&dir.0, app.handle()), before);
            }
            let audio = dir.0.join("audiobooks/example");
            fs::create_dir_all(&audio).unwrap();
            fs::write(
                audio.join("manifest.json"),
                json!({"documentUrl":format!("/uploads/{}.{kind}?audio=1#part", aliases[0])})
                    .to_string(),
            )
            .unwrap();
            fs::write(audio.join("audio.wav"), b"audio sentinel").unwrap();
            fs::write(audio.join("timings.json"), b"timing sentinel").unwrap();
            assert!(crate::native_tts::document_has_audiobook_reference(
                app.handle(),
                &format!("/uploads/{id}.{kind}")
            )
            .unwrap());
            let before = snapshot(&dir.0, app.handle());
            for requested in [id.as_str(), &aliases[0], &aliases[1]] {
                assert!(rejected(attempt(&window, route, requested, kind)));
                assert_eq!(snapshot(&dir.0, app.handle()), before);
            }
            fs::remove_file(audio.join("manifest.json")).unwrap();
            attempt(&window, route, &id, kind).unwrap();
            assert!(!dir.0.join("document_uploads").join(&id).exists());
            assert_eq!(
                fs::read(audio.join("audio.wav")).unwrap(),
                b"audio sentinel"
            );
            assert_eq!(
                fs::read(audio.join("timings.json")).unwrap(),
                b"timing sentinel"
            );
        }
    }
}

#[test]
fn exact_legacy_mixed_case_ids_remain_removable_and_case_folded_audio_references_protect_them() {
    for route in ["translation", "single", "batch"] {
        let (dir, app, window) = fixture();
        seed(app.handle(), SOURCE, None, "epub");
        seed(app.handle(), "DeF456", Some(SOURCE), "html");
        let before = snapshot(&dir.0, app.handle());
        for id in ["def456", "DEF456"] {
            assert!(
                rejected(attempt(&window, route, id, "html")),
                "{route}: {id}"
            );
            assert_eq!(snapshot(&dir.0, app.handle()), before);
        }
        let audio = dir.0.join("audiobooks/example");
        fs::create_dir_all(&audio).unwrap();
        fs::write(
            audio.join("manifest.json"),
            json!({"documentUrl":"/uploads/def456.html"}).to_string(),
        )
        .unwrap();
        let before = snapshot(&dir.0, app.handle());
        assert!(rejected(attempt(&window, route, "DeF456", "html")));
        assert_eq!(snapshot(&dir.0, app.handle()), before);
        fs::remove_file(audio.join("manifest.json")).unwrap();
        attempt(&window, route, "DeF456", "html").unwrap();
        assert!(!dir.0.join("document_uploads/DeF456").exists());
        assert!(dir.0.join("document_uploads/abc123/source.html").is_file());
        assert!(invoke(&window, "translation_list_documents", json!({}))
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty());
    }
}

#[test]
fn missing_metadata_retries_require_exact_live_or_staged_directory_case() {
    for staged in [false, true] {
        for route in ["single", "batch"] {
            let (dir, app, window) = fixture();
            seed(app.handle(), SOURCE, None, "html");
            let name = if staged { ".FeEd.deleting" } else { "FeEd" };
            let storage = dir.0.join("document_uploads").join(name);
            fs::create_dir(&storage).unwrap();
            fs::write(storage.join("source.html"), b"legacy reader sentinel").unwrap();
            let before = snapshot(&dir.0, app.handle());
            for alias in ["feed", "FEED"] {
                assert!(
                    rejected(attempt(&window, route, alias, "html")),
                    "{route}: {name}: {alias}"
                );
                assert_eq!(snapshot(&dir.0, app.handle()), before);
            }
            let audio = dir.0.join("audiobooks/example");
            fs::create_dir_all(&audio).unwrap();
            fs::write(
                audio.join("manifest.json"),
                json!({"documentUrl":"/uploads/feed.html?audio=1"}).to_string(),
            )
            .unwrap();
            let before = snapshot(&dir.0, app.handle());
            assert!(rejected(attempt(&window, route, "FeEd", "html")));
            assert_eq!(snapshot(&dir.0, app.handle()), before);
            fs::remove_file(audio.join("manifest.json")).unwrap();
            attempt(&window, route, "FeEd", "html").unwrap();
            assert!(!storage.exists());
            assert!(!dir.0.join("document_uploads/FeEd").exists());
            assert!(dir.0.join("document_uploads/abc123/source.html").is_file());
        }
    }
}

#[test]
fn conflicting_case_only_metadata_fails_closed_even_for_an_exact_row() {
    for route in ["translation", "single", "batch"] {
        let (dir, app, window) = fixture();
        seed(app.handle(), SOURCE, None, "html");
        seed(app.handle(), VARIANT, Some(SOURCE), "html");
        // No second directory: this fixture is safe on case-insensitive hosts too.
        open_document_uploads_db(app.handle()).unwrap().execute("INSERT INTO uploaded_documents
            (id,url,title,format,source_kind,imported_at_ms,bytes,sections)
            VALUES ('DEF456','/uploads/DEF456.html','Conflicting legacy identity','html','html',1,0,0)", []).unwrap();
        let before = snapshot(&dir.0, app.handle());
        for id in [VARIANT, "DEF456"] {
            assert!(
                rejected(attempt(&window, route, id, "html")),
                "{route}: {id}"
            );
            assert_eq!(snapshot(&dir.0, app.handle()), before);
        }
    }
}
