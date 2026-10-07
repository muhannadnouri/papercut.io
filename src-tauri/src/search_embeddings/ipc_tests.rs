//! Real command handlers/serialization on Tauri's mock runtime, not a WebView test.
use super::*;
use serde_json::{json, Value};
use tauri::{test::MockRuntime, Manager};

fn app(identifier: &str) -> tauri::App<MockRuntime> {
    let mut context = tauri::test::mock_context(tauri::test::noop_assets());
    context.config_mut().identifier = identifier.into();
    tauri::test::mock_builder()
        .manage(SearchEmbeddingState::default())
        .invoke_handler(tauri::generate_handler![
            search_embedding_status,
            search_embedding_configure,
            search_embedding_cache_status,
            search_embedding_download,
            search_embedding_cancel_download,
            search_embedding_remove,
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
    let identifier = format!("io.papercut.cache-test-{}", std::process::id());
    let data = root.join(&identifier);
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
    let active = invoke(&window, "search_embedding_configure", request.clone()).unwrap();
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
