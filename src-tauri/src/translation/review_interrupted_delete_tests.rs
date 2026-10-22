//! Native deletion must settle every publication generation before dropping its ownership rows.
use super::*;

/// Route all four public deletion entry points through the same one-item native fixture.
fn remove(
    app: &tauri::AppHandle<MockRuntime>,
    window: &tauri::WebviewWindow<MockRuntime>,
    route: &str,
    id: &str,
    folder: &mut Option<String>,
) -> Result<Value, Value> {
    let url = format!("/uploads/{id}.html");
    match route {
        "translation" => invoke(
            window,
            "translation_delete_document",
            json!({"request":{"id":id}}),
        ),
        "single" => invoke(
            window,
            "document_uploads_delete",
            json!({"request":{"documentUrl":url}}),
        ),
        "batch" => invoke(
            window,
            "document_uploads_delete_batch",
            json!({"request":{"documentUrls":[url]}}),
        ),
        "folder" => {
            let folder_id = folder.get_or_insert_with(|| {
                let created = crate::document_uploads::create_folder(
                    app,
                    crate::document_uploads::UploadedLibraryCreateFolderRequest {
                        parent_id: None,
                        name: "Publication cleanup".into(),
                    },
                )
                .unwrap();
                crate::document_uploads::move_documents(
                    app,
                    crate::document_uploads::UploadedLibraryMoveDocumentsRequest {
                        document_ids: vec![id.into()],
                        folder_id: Some(created.id.clone()),
                    },
                )
                .unwrap();
                created.id
            });
            invoke(
                window,
                "document_uploads_delete_folder",
                json!({"request":{"folderId":folder_id}}),
            )
        }
        _ => unreachable!(),
    }
}

fn rejected(result: &Result<Value, Value>) -> bool {
    result.is_err()
        || result.as_ref().unwrap()["failures"]
            .as_array()
            .is_some_and(|v| !v.is_empty())
}

/// Mirror real interruption boundaries without relying on scheduling or filesystem permissions.
fn interrupt(root: &Path, id: &str, phase: &str) {
    let live = root.join(id);
    let staging = root.join(format!(".{id}.staging"));
    let backup = root.join(format!(".{id}.replacing"));
    match phase {
        "staging" => copy_tree(&live, &staging),
        "missing-live" => {
            copy_tree(&live, &staging);
            fs::rename(&live, &backup).unwrap();
        }
        "uncommitted" => {
            fs::rename(&live, &backup).unwrap();
            copy_tree(&backup, &live);
            fs::write(live.join(".generation"), b"uncommitted:1").unwrap();
            fs::write(live.join("source.html"), b"<p>Uncommitted replacement.</p>").unwrap();
        }
        "committed" => copy_tree(&live, &backup),
        _ => unreachable!(),
    }
}

fn owned_bytes(root: &Path, id: &str) -> u64 {
    [
        id.to_string(),
        format!(".{id}.staging"),
        format!(".{id}.replacing"),
        format!(".{id}.deleting"),
    ]
    .iter()
    .map(|name| root.join(name))
    .filter(|path| path.exists())
    .flat_map(|path| tree(&path))
    .map(|(_, bytes)| bytes.len() as u64)
    .sum()
}

fn assert_retired(root: &Path, id: &str) {
    for name in [
        id.to_string(),
        format!(".{id}.staging"),
        format!(".{id}.replacing"),
        format!(".{id}.deleting"),
    ] {
        assert!(!root.join(&name).exists(), "left {name}");
    }
}

fn assert_rows_retired(app: &tauri::AppHandle<MockRuntime>, id: &str) {
    let db = open_document_uploads_db(app).unwrap();
    for (table, column) in [
        ("uploaded_documents", "id"),
        ("translated_documents", "id"),
        ("translation_review", "variant_id"),
        ("uploaded_sections", "document_id"),
        ("uploaded_document_fts", "document_id"),
        ("uploaded_document_search_fts", "document_id"),
    ] {
        let count: usize = db
            .query_row(
                &format!("SELECT count(*) FROM {table} WHERE {column}=?1"),
                [id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0, "{table}");
    }
}

#[test]
fn every_native_delete_retires_interrupted_publication_generations_without_reviving_them() {
    for route in ["translation", "single", "batch", "folder"] {
        for phase in ["staging", "missing-live", "uncommitted", "committed"] {
            let (dir, app, window) = fixture();
            let source = source(app.handle());
            let info =
                persist_translated_document(app.handle(), request(&source), || Ok(())).unwrap();
            let sibling =
                persist_translated_document(app.handle(), request(&source), || Ok(())).unwrap();
            let ready = approve_all(&window, load(&window, &info.id).unwrap());
            let root = dir.0.join("document_uploads");
            let live = root.join(&info.id);
            if phase == "committed" {
                finish(&window, &ready).unwrap();
            }
            // Include binary assets in every generation so cleanup cannot pass by deleting HTML alone.
            fs::create_dir_all(live.join("assets")).unwrap();
            fs::write(live.join("assets/owned.bin"), b"owned publication asset").unwrap();
            let source_files = tree(&root.join("abc123"));
            let sibling_files = tree(&root.join(&sibling.id));
            let sibling_evidence = evidence(app.handle(), &sibling.id);
            interrupt(&root, &info.id, phase);
            let expected_bytes = owned_bytes(&root, &info.id);
            let mut folder = None;
            let result = remove(app.handle(), &window, route, &info.id, &mut folder).unwrap();
            assert!(!rejected(&Ok(result.clone())), "{route} {phase}: {result}");
            assert_eq!(
                result["bytesFreed"].as_u64().unwrap(),
                expected_bytes,
                "{route} {phase}"
            );
            assert_retired(&root, &info.id);
            assert_rows_retired(app.handle(), &info.id);
            assert!(load(&window, &info.id).unwrap().is_null());
            assert!(finish(&window, &ready).is_err());
            assert_eq!(tree(&root.join("abc123")), source_files);
            assert_eq!(tree(&root.join(&sibling.id)), sibling_files);
            assert_eq!(evidence(app.handle(), &sibling.id), sibling_evidence);
            // A metadata-free retry must stay empty rather than recover an orphaned backup.
            let repeat = invoke(
                &window,
                "document_uploads_delete",
                json!({"request":{"documentUrl":info.document_url}}),
            )
            .unwrap();
            assert_eq!(repeat["bytesFreed"], 0);
            assert_eq!(
                invoke(
                    &window,
                    "translation_delete_document",
                    json!({"request":{"id":info.id}})
                )
                .unwrap()["deleted"],
                false
            );
            assert_retired(&root, &info.id);
        }
    }
}

#[test]
fn failed_native_delete_restores_committed_files_draft_and_ownership_then_retries() {
    for route in ["translation", "single", "batch", "folder"] {
        for phase in ["staging", "missing-live", "uncommitted", "committed"] {
            let (dir, app, window) = fixture();
            let source = source(app.handle());
            let info =
                persist_translated_document(app.handle(), request(&source), || Ok(())).unwrap();
            let ready = approve_all(&window, load(&window, &info.id).unwrap());
            if phase == "committed" {
                finish(&window, &ready).unwrap();
            }
            let root = dir.0.join("document_uploads");
            let published_before = published(app.handle(), &dir.0, &info.id);
            let files_before = tree(&root.join(&info.id));
            let saved_before = evidence(app.handle(), &info.id);
            let source_before = tree(&root.join("abc123"));
            interrupt(&root, &info.id, phase);
            let db = open_document_uploads_db(app.handle()).unwrap();
            db.execute_batch(&format!("CREATE TRIGGER reject_delete BEFORE DELETE ON uploaded_documents WHEN OLD.id='{}' BEGIN SELECT RAISE(ABORT,'injected publication deletion failure'); END;", info.id)).unwrap();
            let mut folder = None;
            let result = remove(app.handle(), &window, route, &info.id, &mut folder);
            assert!(rejected(&result), "{route} {phase}: {result:?}");
            assert_eq!(load(&window, &info.id).unwrap(), ready, "{route} {phase}");
            assert_eq!(published(app.handle(), &dir.0, &info.id), published_before);
            assert_eq!(tree(&root.join(&info.id)), files_before);
            assert_eq!(evidence(app.handle(), &info.id), saved_before);
            assert_eq!(tree(&root.join("abc123")), source_before);
            assert!(!root.join(format!(".{}.deleting", info.id)).exists());
            db.execute_batch("DROP TRIGGER reject_delete").unwrap();
            let result = remove(app.handle(), &window, route, &info.id, &mut folder);
            assert!(!rejected(&result), "{route} {phase}: {result:?}");
            assert_retired(&root, &info.id);
            assert_rows_retired(app.handle(), &info.id);
        }
    }
}

#[test]
fn publication_cleanup_failure_retains_rows_and_retryable_draft_without_permission_assumptions() {
    for route in ["translation", "single", "batch", "folder"] {
        let (dir, app, window) = fixture();
        let source = source(app.handle());
        let info = persist_translated_document(app.handle(), request(&source), || Ok(())).unwrap();
        let ready = approve_all(&window, load(&window, &info.id).unwrap());
        let root = dir.0.join("document_uploads");
        let before = published(app.handle(), &dir.0, &info.id);
        let stored = evidence(app.handle(), &info.id);
        // remove_dir_all must reject this shape on every platform, including privileged test runners.
        let invalid = root.join(format!(".{}.staging", info.id));
        fs::write(&invalid, b"not a directory").unwrap();
        let mut folder = None;
        let result = remove(app.handle(), &window, route, &info.id, &mut folder);
        assert!(rejected(&result), "{route}: {result:?}");
        assert_eq!(evidence(app.handle(), &info.id), stored);
        assert_eq!(published(app.handle(), &dir.0, &info.id), before);
        assert_eq!(load(&window, &info.id).unwrap(), ready);
        assert_eq!(fs::read(&invalid).unwrap(), b"not a directory");
        fs::remove_file(invalid).unwrap();
        let result = remove(app.handle(), &window, route, &info.id, &mut folder);
        assert!(!rejected(&result), "{route}: {result:?}");
        assert_retired(&root, &info.id);
        assert_rows_retired(app.handle(), &info.id);
    }
}

#[test]
fn unowned_or_case_alias_publication_storage_is_not_removed() {
    for route in ["translation", "single"] {
        let (dir, app, window) = fixture();
        source(app.handle());
        let root = dir.0.join("document_uploads");
        for name in [".def456.staging", ".def456.replacing"] {
            fs::create_dir(root.join(name)).unwrap();
            fs::write(root.join(name).join("sentinel"), b"unowned").unwrap();
        }
        let mut folder = None;
        let _ = remove(app.handle(), &window, route, "def456", &mut folder).unwrap();
        for name in [".def456.staging", ".def456.replacing"] {
            assert_eq!(
                fs::read(root.join(name).join("sentinel")).unwrap(),
                b"unowned"
            );
        }
        let source = source(app.handle());
        let info = persist_translated_document(app.handle(), request(&source), || Ok(())).unwrap();
        let before = published(app.handle(), &dir.0, &info.id);
        let saved = evidence(app.handle(), &info.id);
        let alias = root.join(format!(".{}.staging", info.id.to_ascii_uppercase()));
        fs::create_dir(&alias).unwrap();
        fs::write(alias.join("sentinel"), b"case mismatch").unwrap();
        let result = remove(app.handle(), &window, route, &info.id, &mut folder);
        assert!(rejected(&result), "{route}: {result:?}");
        assert_eq!(fs::read(alias.join("sentinel")).unwrap(), b"case mismatch");
        assert_eq!(published(app.handle(), &dir.0, &info.id), before);
        assert_eq!(evidence(app.handle(), &info.id), saved);
    }
}
