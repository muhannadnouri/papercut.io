//! Native publication keeps the reviewed generation, reader files and index in one durable outcome.
use super::{
    fixture, invoke,
    review_command_tests::{edit, load, published, request, save, source},
};
use crate::{
    document_uploads::open_document_uploads_db,
    translation::{
        review::{EditRequest, Expected},
        storage::{finish_translation_review, persist_translated_document},
    },
};
use serde_json::{json, Value};
use std::{
    fs,
    path::Path,
    sync::{mpsc, Arc, Barrier},
    time::Duration,
};
use tauri::test::MockRuntime;

#[path = "review_interrupted_delete_tests.rs"]
mod review_interrupted_delete_tests;

fn finish(window: &tauri::WebviewWindow<MockRuntime>, draft: &Value) -> Result<Value, Value> {
    invoke(
        window,
        "translation_review_finish",
        json!({"request":draft["document"]["expected"].to_string()}),
    )
}

fn approve_all(window: &tauri::WebviewWindow<MockRuntime>, mut draft: Value) -> Value {
    for block in 0..draft["document"]["blocks"].as_array().unwrap().len() {
        draft = save(window, &edit(&draft, block, json!({"action":"approve"}))).unwrap();
    }
    draft
}

fn evidence(
    app: &tauri::AppHandle<MockRuntime>,
    id: &str,
) -> (String, String, i64, Option<i64>, Option<String>) {
    let db = open_document_uploads_db(app).unwrap();
    db.query_row("SELECT snapshot_json,document_json,revision,published_revision,published_hash FROM translation_review WHERE variant_id=?1", [id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).unwrap()
}

/// Capture durable semantic rows and owned bytes; WAL/checkpoint bytes are deliberately not evidence.
fn tree(root: &Path) -> Vec<(String, Vec<u8>)> {
    fn visit(root: &Path, at: &Path, output: &mut Vec<(String, Vec<u8>)>) {
        for entry in fs::read_dir(at).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                visit(root, &path, output);
            } else {
                output.push((
                    path.strip_prefix(root).unwrap().to_str().unwrap().into(),
                    fs::read(path).unwrap(),
                ));
            }
        }
    }
    let mut output = Vec::new();
    visit(root, root, &mut output);
    output.sort();
    output
}

/// Formatting-only promotion must not churn section/FTS identities or invalidate semantic coverage.
fn index_ids(app: &tauri::AppHandle<MockRuntime>, id: &str) -> [Vec<i64>; 3] {
    let db = open_document_uploads_db(app).unwrap();
    [
        "SELECT id FROM uploaded_sections WHERE document_id=?1 ORDER BY ordinal",
        "SELECT rowid FROM uploaded_document_fts WHERE document_id=?1 ORDER BY section_id",
        "SELECT rowid FROM uploaded_document_search_fts WHERE document_id=?1 ORDER BY section_id",
    ]
    .map(|query| {
        db.prepare(query)
            .unwrap()
            .query_map([id], |row| row.get(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    })
}

fn copy_tree(source: &Path, target: &Path) {
    fs::create_dir_all(target).unwrap();
    for (path, bytes) in tree(source) {
        let path = target.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
}

#[test]
fn native_finish_is_repeatable_and_later_draft_edits_preserve_the_last_publication() {
    let (dir, app, window) = fixture();
    let source = source(app.handle());
    let source_before = tree(&dir.0.join("document_uploads/abc123"));
    let info = persist_translated_document(app.handle(), request(&source), || Ok(())).unwrap();
    let initial = published(app.handle(), &dir.0, &info.id);
    let indexed = index_ids(app.handle(), &info.id);
    let draft = load(&window, &info.id).unwrap();
    assert!(finish(&window, &draft).is_err());
    assert_eq!(published(app.handle(), &dir.0, &info.id), initial);
    let changed = save(
        &window,
        &edit(
            &draft,
            0,
            json!({"action":"replaceMarks","marks":[{"range":{"start":2,"end":7},"kind":"bold"}]}),
        ),
    )
    .unwrap();
    let ready = approve_all(&window, changed);
    let immutable = evidence(app.handle(), &info.id).0;
    let audio = dir.0.join("audiobooks/owned");
    fs::create_dir_all(&audio).unwrap();
    for (file, bytes) in [
        (
            "manifest.json",
            json!({"documentUrl":info.document_url})
                .to_string()
                .into_bytes(),
        ),
        ("audio.wav", b"unchanged audio".to_vec()),
        ("timings.json", b"unchanged word coordinates".to_vec()),
    ] {
        fs::write(audio.join(file), bytes).unwrap();
    }
    let audio_before = tree(&audio);
    let result = finish(&window, &ready).unwrap();
    assert_eq!(result["id"], info.id);
    assert_eq!(result["documentUrl"], info.document_url);
    assert_eq!(result["status"], "ready");
    let finished = published(app.handle(), &dir.0, &info.id);
    assert_ne!(finished.0, initial.0);
    assert_eq!(finished.1, initial.1);
    assert_eq!(index_ids(app.handle(), &info.id), indexed);
    assert_eq!(finished.2, "ready");
    let html = std::str::from_utf8(&finished.0).unwrap();
    assert!(html.contains("<b>house</b>"));
    assert!(!html.contains("data-papercut-translation-review"));
    let dom = crate::translation::html::parse_html_document(html);
    for id in ["first", "second"] {
        assert_eq!(dom.select(&format!("#{id}")).unwrap().count(), 1);
    }
    let completed = evidence(app.handle(), &info.id);
    assert_eq!(completed.0, immutable);
    assert_eq!(completed.3, Some(completed.2));
    assert!(completed.4.is_some());
    assert_eq!(load(&window, &info.id).unwrap(), ready);
    assert_eq!(finish(&window, &ready).unwrap(), result);
    assert_eq!(evidence(app.handle(), &info.id), completed);
    assert_eq!(published(app.handle(), &dir.0, &info.id), finished);
    let canonical = dir
        .0
        .join(format!("document_uploads/{}/source.html", info.id));
    fs::write(&canonical, b"<p>Changed after publication</p>").unwrap();
    assert!(finish(&window, &ready).is_err());
    assert!(load(&window, &info.id).is_err());
    fs::write(&canonical, &finished.0).unwrap();
    assert_eq!(finish(&window, &ready).unwrap(), result);
    let reset = save(&window, &edit(&ready, 0, json!({"action":"reset"}))).unwrap();
    assert!(finish(&window, &ready).is_err());
    assert!(finish(&window, &reset).is_err());
    assert_eq!(published(app.handle(), &dir.0, &info.id), finished);
    let approved = save(&window, &edit(&reset, 0, json!({"action":"approve"}))).unwrap();
    assert_eq!(finish(&window, &approved).unwrap()["status"], "ready");
    assert!(
        !std::str::from_utf8(&published(app.handle(), &dir.0, &info.id).0)
            .unwrap()
            .contains("<b>house</b>")
    );
    assert_eq!(evidence(app.handle(), &info.id).0, immutable);
    assert_eq!(index_ids(app.handle(), &info.id), indexed);
    assert_eq!(tree(&audio), audio_before);
    assert_eq!(tree(&dir.0.join("document_uploads/abc123")), source_before);
}

#[test]
fn finish_rejects_unresolved_link_obligations_and_forged_ipc_without_writing() {
    let (dir, app, window) = fixture();
    let mut db = open_document_uploads_db(app.handle()).unwrap();
    crate::document_uploads::persist_derived_document(
        app.handle(),
        &mut db,
        "abc123",
        "/uploads/abc123.html",
        "Synthetic linked source",
        "html",
        "<p>Une <strong>maison</strong><a href='https://example.com'> lien</a>.</p>".into(),
        &[],
        1,
        |_| Ok(()),
    )
    .unwrap();
    let source = crate::translation::source::load_translation_source_document(
        app.handle(),
        "/uploads/abc123.html",
    )
    .unwrap();
    let info = persist_translated_document(app.handle(), request(&source), || Ok(())).unwrap();
    let draft = approve_all(&window, load(&window, &info.id).unwrap());
    let before = published(app.handle(), &dir.0, &info.id);
    let stored = evidence(app.handle(), &info.id);
    assert!(finish(&window, &draft).unwrap_err()["message"]
        .as_str()
        .unwrap()
        .contains("unresolved"));
    let mut injected = draft["document"]["expected"].clone();
    injected["html"] = "<p>forged replacement</p>".into();
    for payload in [
        "{}".to_string(),
        "x".repeat(16 * 1024 + 1),
        injected.to_string(),
    ] {
        assert!(invoke(
            &window,
            "translation_review_finish",
            json!({"request":payload})
        )
        .is_err());
    }
    let second = tauri::WebviewWindowBuilder::new(&app, "other", Default::default())
        .build()
        .unwrap();
    assert!(finish(&second, &draft).is_err());
    let legacy = json!({"document":{"expected":{"schemaVersion":1,"policy":"opus-assisted-emphasis-v1","variantId":"abc123","generation":"00000000000000000000000000000000","revision":0}}});
    assert!(finish(&window, &legacy).is_err());
    assert_eq!(published(app.handle(), &dir.0, &info.id), before);
    assert_eq!(evidence(app.handle(), &info.id), stored);
}

#[test]
fn upload_metadata_and_receipt_failures_roll_back_every_publication_write_and_retry() {
    for (table, event) in [
        ("uploaded_documents", "UPDATE"),
        ("translated_documents", "UPDATE"),
        ("translation_review", "UPDATE"),
    ] {
        let (dir, app, window) = fixture();
        let source = source(app.handle());
        let info = persist_translated_document(app.handle(), request(&source), || Ok(())).unwrap();
        let ready = approve_all(&window, load(&window, &info.id).unwrap());
        let before = published(app.handle(), &dir.0, &info.id);
        let snapshot = evidence(app.handle(), &info.id);
        let live = dir.0.join(format!("document_uploads/{}", info.id));
        let files = tree(&live);
        let db = open_document_uploads_db(app.handle()).unwrap();
        db.execute_batch(&format!("CREATE TRIGGER reject_finish BEFORE {event} ON {table} BEGIN SELECT RAISE(ABORT,'injected publication failure'); END;")).unwrap();
        assert!(finish(&window, &ready).is_err(), "{table}");
        assert_eq!(published(app.handle(), &dir.0, &info.id), before, "{table}");
        assert_eq!(evidence(app.handle(), &info.id), snapshot, "{table}");
        assert_eq!(tree(&live), files, "{table}");
        assert_eq!(load(&window, &info.id).unwrap(), ready);
        for suffix in ["staging", "replacing"] {
            assert!(!dir
                .0
                .join(format!("document_uploads/.{}.{suffix}", info.id))
                .exists());
        }
        db.execute_batch("DROP TRIGGER reject_finish").unwrap();
        assert_eq!(finish(&window, &ready).unwrap()["status"], "ready");
    }
}

#[test]
fn review_reload_recovers_precommit_missing_live_and_postcommit_backups() {
    let (dir, app, window) = fixture();
    let source = source(app.handle());
    let info = persist_translated_document(app.handle(), request(&source), || Ok(())).unwrap();
    let ready = approve_all(&window, load(&window, &info.id).unwrap());
    let live = dir.0.join(format!("document_uploads/{}", info.id));
    let backup = dir
        .0
        .join(format!("document_uploads/.{}.replacing", info.id));
    let old_files = tree(&live);
    let old = evidence(app.handle(), &info.id);
    fs::rename(&live, &backup).unwrap();
    fs::create_dir(&live).unwrap();
    fs::write(live.join("source.html"), b"<p>Uncommitted replacement</p>").unwrap();
    fs::write(live.join(".generation"), b"uncommitted:1").unwrap();
    assert_eq!(load(&window, &info.id).unwrap(), ready);
    assert_eq!(tree(&live), old_files);
    assert!(!backup.exists());
    assert_eq!(evidence(app.handle(), &info.id), old);
    fs::rename(&live, &backup).unwrap();
    assert_eq!(load(&window, &info.id).unwrap(), ready);
    assert_eq!(tree(&live), old_files);
    assert!(!backup.exists());
    let old_copy = dir.0.join("old-generation");
    copy_tree(&live, &old_copy);
    let completed = finish(&window, &ready).unwrap();
    let new_files = tree(&live);
    let receipt = evidence(app.handle(), &info.id);
    copy_tree(&old_copy, &backup);
    assert_eq!(load(&window, &info.id).unwrap(), ready);
    assert_eq!(tree(&live), new_files);
    assert!(!backup.exists());
    assert_eq!(evidence(app.handle(), &info.id), receipt);
    assert_eq!(finish(&window, &ready).unwrap(), completed);
}

#[test]
fn finish_waits_for_draft_commit_and_rejects_the_now_stale_revision() {
    let (dir, app, window) = fixture();
    let source = source(app.handle());
    let info = persist_translated_document(app.handle(), request(&source), || Ok(())).unwrap();
    let ready = approve_all(&window, load(&window, &info.id).unwrap());
    let expected: Expected = serde_json::from_value(ready["document"]["expected"].clone()).unwrap();
    let reset: EditRequest =
        serde_json::from_value(edit(&ready, 0, json!({"action":"reset"}))).unwrap();
    let before = published(app.handle(), &dir.0, &info.id);
    let (start_tx, start_rx) = mpsc::channel();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let handle = app.handle().clone();
    let worker = std::thread::spawn(move || {
        start_rx.recv().unwrap();
        entered_tx.send(()).unwrap();
        done_tx
            .send(finish_translation_review(&handle, &expected))
            .unwrap();
    });
    let (premature, saved) = crate::native_tts::with_audiobook_reference_lock(|| {
        start_tx.send(()).unwrap();
        entered_rx.recv().unwrap();
        let premature = done_rx.recv_timeout(Duration::from_millis(100));
        let mut db = crate::translation::storage::open_translation_db(app.handle())?;
        crate::translation::review_store::apply_prepared_in(&mut db, &reset, |_tx, snapshot| {
            Ok((premature, snapshot.document.expected().revision))
        })
    })
    .unwrap();
    worker.join().unwrap();
    assert!(matches!(premature, Err(mpsc::RecvTimeoutError::Timeout)));
    assert!(done_rx
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .is_err());
    assert_eq!(
        load(&window, &info.id).unwrap()["document"]["expected"]["revision"],
        saved
    );
    assert_eq!(published(app.handle(), &dir.0, &info.id), before);
}

#[test]
fn finish_waits_for_real_delete_and_cannot_recreate_rows_or_files() {
    let (dir, app, window) = fixture();
    let source = source(app.handle());
    let info = persist_translated_document(app.handle(), request(&source), || Ok(())).unwrap();
    let ready = approve_all(&window, load(&window, &info.id).unwrap());
    let expected: Expected = serde_json::from_value(ready["document"]["expected"].clone()).unwrap();
    let (start_tx, start_rx) = mpsc::channel();
    let (entered_tx, entered_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let handle = app.handle().clone();
    let id = info.id.clone();
    let worker = std::thread::spawn(move || {
        start_rx.recv().unwrap();
        entered_tx.send(()).unwrap();
        done_tx
            .send(finish_translation_review(&handle, &expected))
            .unwrap();
    });
    let mut premature = None;
    crate::document_uploads::delete_derived_document(app.handle(), &id, |tx| {
        let owned = tx
            .execute("DELETE FROM translated_documents WHERE id=?1", [&id])
            .unwrap()
            > 0;
        start_tx.send(()).unwrap();
        entered_rx.recv().unwrap();
        premature = Some(done_rx.recv_timeout(Duration::from_millis(100)));
        Ok(owned)
    })
    .unwrap();
    worker.join().unwrap();
    assert!(matches!(
        premature.unwrap(),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    assert!(done_rx
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .is_err());
    assert!(load(&window, &id).unwrap().is_null());
    for path in [
        id.clone(),
        format!(".{id}.staging"),
        format!(".{id}.replacing"),
    ] {
        assert!(!dir.0.join("document_uploads").join(path).exists());
    }
    let db = open_document_uploads_db(app.handle()).unwrap();
    for (table, column) in [
        ("uploaded_documents", "id"),
        ("translated_documents", "id"),
        ("translation_review", "variant_id"),
        ("uploaded_sections", "document_id"),
        ("uploaded_document_fts", "document_id"),
        ("uploaded_document_search_fts", "document_id"),
    ] {
        assert_eq!(
            db.query_row(
                &format!("SELECT count(*) FROM {table} WHERE {column}=?1"),
                [&id],
                |r| r.get::<_, usize>(0)
            )
            .unwrap(),
            0,
            "{table}"
        );
    }
}

#[test]
fn regeneration_and_finish_keep_independent_generations_and_reject_cross_generation_input() {
    let (dir, app, window) = fixture();
    let source = source(app.handle());
    let req = request(&source);
    let first = persist_translated_document(app.handle(), request(&source), || Ok(())).unwrap();
    let ready = approve_all(&window, load(&window, &first.id).unwrap());
    let expected: Expected = serde_json::from_value(ready["document"]["expected"].clone()).unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let b = barrier.clone();
    let handle = app.handle().clone();
    let publisher = std::thread::spawn(move || {
        b.wait();
        finish_translation_review(&handle, &expected)
    });
    let b = barrier.clone();
    let handle = app.handle().clone();
    let regenerator = std::thread::spawn(move || {
        b.wait();
        persist_translated_document(&handle, req, || Ok(()))
    });
    barrier.wait();
    publisher.join().unwrap().unwrap();
    let second = regenerator.join().unwrap().unwrap();
    assert_ne!(first.id, second.id);
    let new = load(&window, &second.id).unwrap();
    assert_ne!(
        ready["document"]["expected"]["generation"],
        new["document"]["expected"]["generation"]
    );
    assert_eq!(load(&window, &first.id).unwrap(), ready);
    assert_eq!(published(app.handle(), &dir.0, &first.id).2, "ready");
    assert_eq!(
        published(app.handle(), &dir.0, &second.id).2,
        "needs-review"
    );
    let second_before = published(app.handle(), &dir.0, &second.id);
    let second_evidence = evidence(app.handle(), &second.id);
    let mut forged = new.clone();
    forged["document"]["expected"]["generation"] =
        ready["document"]["expected"]["generation"].clone();
    assert!(finish(&window, &forged).is_err());
    assert_eq!(load(&window, &second.id).unwrap(), new);
    assert_eq!(published(app.handle(), &dir.0, &second.id), second_before);
    assert_eq!(evidence(app.handle(), &second.id), second_evidence);
}

#[test]
fn concurrent_finish_requests_return_one_stable_publication_receipt() {
    let (dir, app, window) = fixture();
    let source = source(app.handle());
    let info = persist_translated_document(app.handle(), request(&source), || Ok(())).unwrap();
    let ready = approve_all(&window, load(&window, &info.id).unwrap());
    let expected: Expected = serde_json::from_value(ready["document"]["expected"].clone()).unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let mut workers = Vec::new();
    for _ in 0..2 {
        let barrier = barrier.clone();
        let handle = app.handle().clone();
        let expected = expected.clone();
        workers.push(std::thread::spawn(move || {
            barrier.wait();
            serde_json::to_value(finish_translation_review(&handle, &expected).unwrap()).unwrap()
        }));
    }
    barrier.wait();
    let first = workers.remove(0).join().unwrap();
    let second = workers.remove(0).join().unwrap();
    assert_eq!(first, second);
    assert_eq!(finish(&window, &ready).unwrap(), first);
    assert_eq!(load(&window, &info.id).unwrap(), ready);
    let saved = evidence(app.handle(), &info.id);
    assert_eq!(saved.3, Some(saved.2));
    assert_eq!(published(app.handle(), &dir.0, &info.id).2, "ready");
    for suffix in ["staging", "replacing"] {
        assert!(!dir
            .0
            .join(format!("document_uploads/.{}.{suffix}", info.id))
            .exists());
    }
}
