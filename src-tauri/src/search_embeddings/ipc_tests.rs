//! Real command handlers/serialization on Tauri's mock runtime, not a WebView test.
use super::*;
use serde_json::{json, Value};
use tauri::{test::MockRuntime, Manager};

fn app(identifier: &str) -> tauri::App<MockRuntime> {
    let mut context = tauri::test::mock_context(tauri::test::noop_assets());
    context.config_mut().identifier = identifier.into();
    tauri::test::mock_builder()
        .manage(SearchEmbeddingState::default())
        .manage(crate::document_uploads::semantic::SemanticIndexState::default())
        .invoke_handler(tauri::generate_handler![
            crate::document_uploads::semantic::query::search_semantic_query,
            search_embedding_status,
            search_embedding_configure,
            search_embedding_cache_status,
            search_embedding_download,
            search_embedding_cancel_download,
            search_embedding_remove,
            crate::document_uploads::semantic::search_semantic_index_status,
            crate::document_uploads::semantic::search_semantic_index_build,
            crate::document_uploads::semantic::search_semantic_index_cancel,
            crate::document_uploads::semantic::search_semantic_index_remove,
            crate::document_uploads::commands::document_uploads_import_pasted_text,
            crate::document_uploads::commands::document_uploads_update_title,
            crate::document_uploads::commands::document_uploads_delete,
        ])
        .build(context)
        .unwrap()
}

fn window(app: &tauri::App<MockRuntime>) -> tauri::WebviewWindow<MockRuntime> {
    tauri::WebviewWindowBuilder::new(app, "main", Default::default())
        // MockRuntime never starts a browser; avoid Tauri creating a real profile.
        .data_directory(std::env::temp_dir())
        .build()
        .unwrap()
}

fn invoke(
    window: &tauri::WebviewWindow<MockRuntime>,
    cmd: &str,
    body: Value,
) -> Result<Value, Value> {
    tauri::test::get_ipc_response(
        window,
        tauri::webview::InvokeRequest {
            cmd: cmd.into(),
            callback: tauri::ipc::CallbackFn(0),
            error: tauri::ipc::CallbackFn(1),
            url: "tauri://localhost".parse().unwrap(),
            body: tauri::ipc::InvokeBody::Json(body),
            headers: Default::default(),
            invoke_key: tauri::test::INVOKE_KEY.into(),
        },
    )
    .map(|body| body.deserialize().unwrap())
}

#[test]
fn command_capability_and_argument_contract() {
    let app = app(&format!("io.papercut.command-test-{}", std::process::id()));
    let window = window(&app);
    let status = invoke(&window, "search_embedding_status", json!({})).unwrap();
    assert_eq!(
        status["compiled"],
        cfg!(feature = "experimental-search-embeddings-core")
    );
    for key in [
        "activeFormat",
        "loadingFormat",
        "downloadingFormat",
        "encoderKey",
    ] {
        assert!(status.get(key).unwrap().is_null(), "camelCase field {key}");
    }
    for cmd in [
        "search_embedding_configure",
        "search_embedding_download",
        "search_embedding_remove",
    ] {
        assert!(invoke(&window, cmd, json!({"format":"unknown-model"})).is_err());
    }
    assert_eq!(
        invoke(
            &window,
            "search_embedding_configure",
            json!({"format":null})
        )
        .unwrap(),
        status
    );
    assert_eq!(
        invoke(&window, "search_embedding_cancel_download", json!({})).unwrap(),
        status
    );
    #[cfg(not(feature = "experimental-search-embeddings-core"))]
    for cmd in [
        "search_embedding_cache_status",
        "search_embedding_configure",
        "search_embedding_download",
        "search_embedding_remove",
    ] {
        assert!(invoke(&window, cmd, json!({"format":"float32"}))
            .unwrap_err()
            .as_str()
            .unwrap()
            .contains("not compiled"));
    }
    assert!(!app.path().app_data_dir().unwrap().exists());
}

#[test]
fn semantic_index_commands_are_default_off_and_do_not_initialize_storage() {
    let app = app(&format!(
        "io.papercut.index-contract-test-{}",
        std::process::id()
    ));
    let window = window(&app);
    let status = invoke(&window, "search_semantic_index_status", json!({})).unwrap();
    assert_eq!(
        status["compiled"],
        cfg!(feature = "experimental-search-embeddings-core")
    );
    assert_eq!(status["running"], false);
    assert_eq!(status["indexedDocuments"], 0);
    assert_eq!(status["eligibleDocuments"], 0);
    assert_eq!(
        invoke(&window, "search_semantic_index_cancel", json!({})).unwrap(),
        Value::Null
    );
    assert!(invoke(&window, "search_semantic_index_build", json!({})).is_err());
    assert!(invoke(
        &window,
        "search_semantic_query",
        json!({"request":{"query":"travel"}})
    )
    .is_err());
    assert!(invoke(
        &window,
        "search_semantic_query",
        json!({"request":{"query":"travel","limit":51}})
    )
    .unwrap_err()
    .as_str()
    .unwrap()
    .contains("limit"));
    assert_eq!(
        invoke(
            &window,
            "search_semantic_query",
            json!({"request":{"query":"word ".repeat(129)}})
        )
        .unwrap_err(),
        "searchQueryTooLong"
    );
    assert!(!app.path().app_data_dir().unwrap().exists());
}

#[cfg(feature = "experimental-search-embeddings-core")]
#[test]
#[ignore = "requires disposable XDG_DATA_HOME, cached model assets and packaged ONNX Runtime"]
fn cached_semantic_index_commands_checkpoint_and_preserve_sources() {
    use std::{fs, path::Path};
    let root = fs::canonicalize(
        std::env::var_os("PAPERCUT_EMBEDDING_IPC_TEST_DATA").expect("set disposable test root"),
    )
    .unwrap();
    assert_eq!(
        root,
        fs::canonicalize(std::env::var_os("XDG_DATA_HOME").expect("set same disposable XDG root"))
            .unwrap()
    );
    let temp = fs::canonicalize(std::env::temp_dir()).unwrap();
    assert!(root.starts_with(&temp) && root != temp);
    let app = app(&format!("io.papercut.index-test-{}", std::process::id()));
    let window = window(&app);
    assert!(!app.path().app_data_dir().unwrap().exists());
    let format = match std::env::var("PAPERCUT_EMBEDDING_TEST_FORMAT")
        .as_deref()
        .unwrap_or("float32")
    {
        "float32" => ModelFormat::Float32,
        "dynamic-int8" => ModelFormat::DynamicInt8,
        other => panic!("unsupported test format {other}"),
    };
    let source =
        std::env::var_os("PAPERCUT_EMBEDDING_TEST_MODELS").expect("set cached models source");
    let folder = if format == ModelFormat::Float32 {
        "float32"
    } else {
        "dynamic-int8"
    };
    let dir = cache_dir(app.handle(), format).unwrap();
    for asset in format.assets() {
        let path = dir.join(asset.path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::copy(Path::new(&source).join(folder).join(asset.path), path).unwrap();
    }
    invoke(
        &window,
        "search_embedding_configure",
        json!({"format":format}),
    )
    .unwrap();
    let provider = app.state::<SearchEmbeddingState>();
    let key = provider.index_encoder_key().unwrap();
    assert!(provider
        .index_encode("wrong-model", "passage: guide")
        .is_err());
    for text in [
        "A city transport guide. ".repeat(250),
        "😀 المواصلات في المدينة بالحافلات. ".repeat(200),
        "城市公共交通指南。".repeat(200),
    ] {
        let spans = provider
            .index_passage_ranges(&key, "Travel", None, &text)
            .unwrap();
        assert!(spans.len() > 1);
        assert_eq!(spans.first().unwrap().start, 0);
        assert!(spans.last().unwrap().end <= text.len());
        for pair in spans.windows(2) {
            assert!(
                pair[0].start < pair[1].start && pair[0].end >= pair[1].start,
                "passages must overlap without skipping source"
            );
        }
        for span in spans {
            assert!(text.get(span).is_some());
        }
    }
    assert!(provider
        .index_passage_ranges(&key, &"long title ".repeat(400), None, "Body")
        .is_err());
    let body = format!(
        "{}\n\n😀 دليل النقل في المدينة. 城市公共交通指南。",
        "Residents travel by electric bus. ".repeat(200)
    );
    let document = invoke(
        &window,
        "document_uploads_import_pasted_text",
        json!({"request":{"title":"Travel guide","text":body}}),
    )
    .unwrap();
    let url = document["url"].as_str().unwrap();
    invoke(&window, "search_semantic_index_build", json!({})).unwrap();
    let status = invoke(&window, "search_semantic_index_status", json!({})).unwrap();
    assert_eq!(status["indexedDocuments"], 1);
    assert_eq!(status["eligibleDocuments"], 1);
    assert!(status["embeddedPassages"].as_u64().unwrap() > 1);
    let response = invoke(
        &window,
        "search_semantic_query",
        json!({"request":{
        "query":"How do residents get around the city?", "documentUrls":[url], "limit":1,
        "exactPhrases":["electric bus"]}}),
    )
    .unwrap();
    assert_eq!(response["availability"], "ready");
    assert_eq!(response["results"][0]["url"], url);
    assert_eq!(response["results"][0]["provenance"], "conceptual");
    let hit = &response["results"][0];
    let db = rusqlite::Connection::open_with_flags(
        app.path()
            .app_data_dir()
            .unwrap()
            .join("document_uploads/search.sqlite3"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let text: String = db
        .query_row(
            "SELECT text FROM uploaded_sections WHERE document_id=?1 AND ordinal=?2",
            rusqlite::params![
                hit["documentId"].as_str().unwrap(),
                hit["sectionIndex"].as_u64().unwrap()
            ],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        &text[hit["byteStart"].as_u64().unwrap() as usize
            ..hit["byteEnd"].as_u64().unwrap() as usize],
        hit["text"].as_str().unwrap()
    );
    assert!(hit.get("matchCount").is_none());
    assert!(invoke(
        &window,
        "search_semantic_query",
        json!({"request":{"query":"travel", "documentUrls":[]}})
    )
    .unwrap()["results"]
        .as_array()
        .unwrap()
        .is_empty());
    drop(db);
    invoke(&window, "search_semantic_index_build", json!({})).unwrap();
    assert_eq!(
        invoke(&window, "search_semantic_index_status", json!({})).unwrap()["embeddedPassages"],
        0
    );
    invoke(
        &window,
        "document_uploads_update_title",
        json!({"request":{"documentUrl":url,"title":"Updated travel guide"}}),
    )
    .unwrap();
    assert_eq!(
        invoke(&window, "search_semantic_index_status", json!({})).unwrap()["indexedDocuments"],
        0
    );
    invoke(&window, "search_semantic_index_build", json!({})).unwrap();
    assert_eq!(
        invoke(&window, "search_semantic_index_status", json!({})).unwrap()["indexedDocuments"],
        1
    );
    invoke(&window, "search_semantic_index_remove", json!({})).unwrap();
    let status = invoke(&window, "search_semantic_index_status", json!({})).unwrap();
    assert_eq!(status["indexedDocuments"], 0);
    assert_eq!(
        status["eligibleDocuments"], 1,
        "removal must retain the document"
    );
    invoke(&window, "search_semantic_index_build", json!({})).unwrap();
    invoke(
        &window,
        "document_uploads_delete",
        json!({"request":{"documentUrl":url}}),
    )
    .unwrap();
    let status = invoke(&window, "search_semantic_index_status", json!({})).unwrap();
    assert_eq!(status["indexedDocuments"], 0);
    assert_eq!(status["passages"], 0);
    invoke(
        &window,
        "search_embedding_configure",
        json!({"format":null}),
    )
    .unwrap();
    let data = app.path().app_data_dir().unwrap();
    drop(window);
    drop(app);
    fs::remove_dir_all(data).unwrap();
}

#[cfg(feature = "experimental-search-embeddings-core")]
#[test]
#[ignore = "requires explicit disposable XDG_DATA_HOME and verified cached INT8 assets"]
fn command_cache_lifecycle_in_isolated_app_data() {
    use std::{
        fs,
        path::Path,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc, Mutex,
        },
        time::{Duration, Instant},
    };
    use tauri::Listener;

    // Refuse to run against a normal app-data root, even when explicitly selected.
    let root = fs::canonicalize(
        std::env::var_os("PAPERCUT_EMBEDDING_IPC_TEST_DATA")
            .expect("set a disposable test-data root"),
    )
    .unwrap();
    assert_eq!(
        root,
        fs::canonicalize(
            std::env::var_os("XDG_DATA_HOME")
                .expect("set XDG_DATA_HOME to the same disposable root")
        )
        .unwrap()
    );
    let temp = fs::canonicalize(std::env::temp_dir()).unwrap();
    assert!(root.starts_with(&temp) && root != temp);
    let source = std::env::var_os("PAPERCUT_EMBEDDING_TEST_MODELS")
        .expect("set read-only cached models source");
    let restart_id = std::env::var("PAPERCUT_EMBEDDING_IPC_RESTART_ID").ok();
    if let Some(identifier) = &restart_id {
        assert!(
            identifier
                .strip_prefix("io.papercut.cache-test-")
                .is_some_and(
                    |suffix| !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit())
                )
        );
    }
    let identifier = restart_id
        .clone()
        .unwrap_or_else(|| format!("io.papercut.cache-test-{}", std::process::id()));
    let data = root.join(&identifier);
    if restart_id.is_some() {
        let phase = std::env::var("PAPERCUT_EMBEDDING_IPC_RESTART_PHASE").unwrap();
        assert!(matches!(phase.as_str(), "active" | "recover"));
        assert!(data.exists());
        let app = app(&identifier);
        let window = window(&app);
        let initial = invoke(&window, "search_embedding_status", json!({})).unwrap();
        for key in [
            "activeFormat",
            "loadingFormat",
            "downloadingFormat",
            "encoderKey",
        ] {
            assert!(initial[key].is_null());
        }
        assert_eq!(
            invoke(&window, "search_embedding_cache_status", json!({})).unwrap()[1]["filesPresent"],
            true
        );
        let request = json!({"format":"dynamic-int8"});
        assert!(
            invoke(&window, "search_embedding_download", request.clone()).unwrap()["activeFormat"]
                .is_null()
        );
        let active = invoke(&window, "search_embedding_configure", request.clone()).unwrap();
        assert_eq!(active["activeFormat"], "dynamic-int8");
        assert_eq!(active["encoderKey"], ModelFormat::DynamicInt8.encoder_key());
        assert!(invoke(&window, "search_embedding_remove", request.clone()).is_err());
        if phase == "active" {
            let ready = data.join("restart-ready");
            fs::write(ready.with_extension("tmp"), b"active").unwrap();
            fs::rename(ready.with_extension("tmp"), ready).unwrap();
            loop {
                std::thread::park();
            }
        }
        let disabled = invoke(
            &window,
            "search_embedding_configure",
            json!({"format":null}),
        )
        .unwrap();
        assert!(disabled["activeFormat"].is_null() && disabled["encoderKey"].is_null());
        invoke(&window, "search_embedding_remove", request).unwrap();
        assert_eq!(
            invoke(&window, "search_embedding_cache_status", json!({})).unwrap()[1]["filesPresent"],
            false
        );
        return;
    }
    assert!(!data.exists(), "test app data must be fresh");
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(data.clone());
    let app = app(&identifier);
    assert_eq!(app.path().app_data_dir().unwrap(), data);
    let window = window(&app);
    let request = json!({"format":"dynamic-int8"});
    let dir = cache_dir(app.handle(), ModelFormat::DynamicInt8).unwrap();
    assert!(dir.starts_with(data.join("models/search")));
    let initial = invoke(&window, "search_embedding_cache_status", json!({})).unwrap();
    assert_eq!(
        initial[0],
        json!({"format":"float32","filesPresent":false,"expectedBytes":487352505})
    );
    assert_eq!(
        initial[1],
        json!({"format":"dynamic-int8","filesPresent":false,"expectedBytes":135430819})
    );
    assert!(!data.exists(), "status must not create cache directories");
    assert!(invoke(&window, "search_embedding_configure", request.clone()).is_err());
    assert!(
        !data.exists(),
        "missing activation must not install anything"
    );

    // A held reservation checks IPC cancellation semantics; the existing worker
    // regression separately proves busy-slot retention until actual cleanup.
    let cancel = Arc::new(AtomicBool::new(false));
    let state = app.state::<SearchEmbeddingState>();
    state.0.lock().unwrap().downloading = Some((ModelFormat::DynamicInt8, cancel.clone()));
    for cmd in [
        "search_embedding_download",
        "search_embedding_configure",
        "search_embedding_remove",
    ] {
        assert!(invoke(&window, cmd, request.clone()).is_err());
    }
    let cancelled = invoke(&window, "search_embedding_cancel_download", json!({})).unwrap();
    assert_eq!(cancelled["downloadingFormat"], "dynamic-int8");
    assert!(cancel.load(Ordering::Relaxed));
    assert!(invoke(&window, "search_embedding_remove", request.clone()).is_err());
    cancel.store(false, Ordering::Relaxed);
    assert_eq!(
        invoke(
            &window,
            "search_embedding_configure",
            json!({"format":null})
        )
        .unwrap()["downloadingFormat"],
        "dynamic-int8"
    );
    assert!(cancel.load(Ordering::Relaxed));
    state.0.lock().unwrap().downloading = None;
    assert!(
        !data.exists(),
        "busy rejection must precede writes or fetching"
    );

    // Complete-by-size, deliberately corrupt cache: real command must refuse
    // integrity without going to the network or replacing the final directory.
    for asset in ModelFormat::DynamicInt8.assets() {
        let path = dir.join(asset.path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::File::create(path)
            .unwrap()
            .set_len(asset.bytes)
            .unwrap();
    }
    let events = Arc::new(Mutex::new(Vec::<Value>::new()));
    let captured = events.clone();
    let listener = app.listen("search-embedding-download-progress", move |event| {
        captured
            .lock()
            .unwrap()
            .push(serde_json::from_str(event.payload()).unwrap());
    });
    assert_eq!(
        invoke(&window, "search_embedding_cache_status", json!({})).unwrap()[1]["filesPresent"],
        true
    );
    assert!(
        invoke(&window, "search_embedding_download", request.clone())
            .unwrap_err()
            .as_str()
            .unwrap()
            .contains("checksum")
    );
    assert!(invoke(&window, "search_embedding_configure", request.clone()).is_err());
    assert_eq!(fs::read(dir.join("config.json")).unwrap(), vec![0; 655]);
    let staging = dir.with_extension("installing");
    fs::create_dir(&staging).unwrap();
    fs::write(staging.join("partial"), b"partial").unwrap();
    invoke(&window, "search_embedding_remove", request.clone()).unwrap();
    invoke(&window, "search_embedding_remove", request.clone()).unwrap();
    assert!(!dir.exists() && !staging.exists());

    // Real pinned cached bytes; copy, never link or mutate the source models.
    for asset in ModelFormat::DynamicInt8.assets() {
        let path = dir.join(asset.path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::copy(
            Path::new(&source).join("dynamic-int8").join(asset.path),
            path,
        )
        .unwrap();
    }
    let verified = invoke(&window, "search_embedding_download", request.clone()).unwrap();
    assert!(verified["activeFormat"].is_null());
    let mut active = Value::Null;
    for cancel_load in [true, false] {
        let response = std::thread::scope(|scope| {
            let worker_window = window.clone();
            let worker_request = request.clone();
            let worker = scope.spawn(move || {
                invoke(&worker_window, "search_embedding_configure", worker_request)
            });
            let started = Instant::now();
            let mut saw_loading = false;
            let mut status_calls = 0;
            let mut max_status = Duration::ZERO;
            let mut disable_ms = None;
            while !worker.is_finished() {
                let call = Instant::now();
                let status = invoke(&window, "search_embedding_status", json!({})).unwrap();
                let elapsed = call.elapsed();
                max_status = max_status.max(elapsed);
                status_calls += 1;
                // Generous deadlock guard, not a product responsiveness target.
                assert!(elapsed < Duration::from_secs(2));
                if status["loadingFormat"] == "dynamic-int8" {
                    assert!(status["activeFormat"].is_null() && status["encoderKey"].is_null());
                    if !saw_loading {
                        assert!(
                            invoke(&window, "search_embedding_configure", request.clone())
                                .unwrap_err()
                                .as_str()
                                .unwrap()
                                .contains("already running")
                        );
                        saw_loading = true;
                        if cancel_load {
                            let call = Instant::now();
                            let disabled = invoke(
                                &window,
                                "search_embedding_configure",
                                json!({"format":null}),
                            )
                            .unwrap();
                            let elapsed = call.elapsed();
                            assert!(elapsed < Duration::from_secs(2));
                            disable_ms = Some(elapsed.as_secs_f64() * 1000.0);
                            assert!(
                                disabled["activeFormat"].is_null()
                                    && disabled["encoderKey"].is_null()
                            );
                        }
                    }
                }
                assert!(started.elapsed() < Duration::from_secs(30));
                std::thread::sleep(Duration::from_millis(10));
            }
            let response = worker.join().unwrap();
            assert!(saw_loading, "must observe the actual loader reservation");
            eprintln!(
                "{}",
                json!({"cancelLoad":cancel_load, "statusCalls":status_calls,
                    "maxStatusMillis":max_status.as_secs_f64()*1000.0,
                    "disableMillis":disable_ms, "activationSeconds":started.elapsed().as_secs_f64()})
            );
            response
        });
        if cancel_load {
            assert_eq!(response.unwrap_err(), "Embedding activation cancelled");
            let status = invoke(&window, "search_embedding_status", json!({})).unwrap();
            assert!(status["activeFormat"].is_null() && status["loadingFormat"].is_null());
        } else {
            active = response.unwrap();
        }
    }
    assert_eq!(active["activeFormat"], "dynamic-int8");
    assert_eq!(active["encoderKey"], ModelFormat::DynamicInt8.encoder_key());
    assert!(invoke(&window, "search_embedding_remove", request.clone())
        .unwrap_err()
        .as_str()
        .unwrap()
        .contains("Disable embeddings"));
    let other = cache_dir(app.handle(), ModelFormat::Float32).unwrap();
    fs::create_dir_all(&other).unwrap();
    fs::write(other.join("partial"), b"partial").unwrap();
    assert_eq!(
        invoke(
            &window,
            "search_embedding_remove",
            json!({"format":"float32"})
        )
        .unwrap()["activeFormat"],
        "dynamic-int8"
    );
    let disabled = invoke(
        &window,
        "search_embedding_configure",
        json!({"format":null}),
    )
    .unwrap();
    assert!(disabled["activeFormat"].is_null() && disabled["encoderKey"].is_null());
    let fresh = self::app(&identifier);
    let fresh_window = self::window(&fresh);
    assert!(
        invoke(&fresh_window, "search_embedding_status", json!({})).unwrap()["activeFormat"]
            .is_null()
    );
    assert_eq!(
        invoke(&fresh_window, "search_embedding_cache_status", json!({})).unwrap()[1]
            ["filesPresent"],
        true
    );
    // A new process must not inherit opt-in state after abrupt native termination.
    {
        use std::{os::unix::process::ExitStatusExt, process::Command};
        struct Child(std::process::Child);
        impl Drop for Child {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let spawn = |phase: &str| {
            let log = fs::File::create(data.join(format!("restart-{phase}.log"))).unwrap();
            Child(Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "search_embeddings::ipc_tests::command_cache_lifecycle_in_isolated_app_data", "--ignored", "--nocapture"])
                .env("PAPERCUT_EMBEDDING_IPC_RESTART_ID", &identifier)
                .env("PAPERCUT_EMBEDDING_IPC_RESTART_PHASE", phase)
                .stdout(log.try_clone().unwrap()).stderr(log).spawn().unwrap())
        };
        let mut child = spawn("active");
        let started = Instant::now();
        let ready = data.join("restart-ready");
        while !ready.exists() {
            assert!(
                child.0.try_wait().unwrap().is_none(),
                "{}",
                fs::read_to_string(data.join("restart-active.log")).unwrap()
            );
            assert!(started.elapsed() < Duration::from_secs(30));
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(fs::read(&ready).unwrap(), b"active");
        child.0.kill().unwrap();
        assert_eq!(child.0.wait().unwrap().signal(), Some(9));
        fs::remove_file(ready).unwrap();
        assert_eq!(
            invoke(&window, "search_embedding_cache_status", json!({})).unwrap()[1]["filesPresent"],
            true
        );
        let mut recovered = spawn("recover");
        let started = Instant::now();
        let status = loop {
            if let Some(status) = recovered.0.try_wait().unwrap() {
                break status;
            }
            assert!(started.elapsed() < Duration::from_secs(30));
            std::thread::sleep(Duration::from_millis(10));
        };
        assert!(
            status.success(),
            "{}",
            fs::read_to_string(data.join("restart-recover.log")).unwrap()
        );
        assert_eq!(
            invoke(&window, "search_embedding_cache_status", json!({})).unwrap()[1]["filesPresent"],
            false
        );
        eprintln!("SIGKILL after cached native command activation: fresh-process default-off, cache reuse, activation, disable and removal passed");
    }
    invoke(&window, "search_embedding_remove", request).unwrap();
    assert!(!dir.exists());
    app.unlisten(listener);
    let events = events.lock().unwrap();
    assert_eq!(
        events
            .iter()
            .map(|event| event["phase"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["starting", "failed", "starting", "installed"]
    );
    assert!(events
        .iter()
        .all(|event| event["format"] == "dynamic-int8" && event["totalBytes"] == 135430819));
    assert_eq!(events.last().unwrap()["downloadedBytes"], 135430819);
    eprintln!("Isolated command cache reuse, integrity, cancellation contract, activation and removal passed; no live download.");
}
