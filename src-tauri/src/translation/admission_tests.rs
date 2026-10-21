//! Real IPC admission checks: a stale/forged UI cannot bypass the product gate.
use super::*;
use serde_json::{json, Value};
use tauri::{test::MockRuntime, Manager};

struct Fixture(std::path::PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Keep all app-data/cache paths isolated without starting a browser or loading a model.
fn fixture() -> (
    Fixture,
    tauri::App<MockRuntime>,
    tauri::WebviewWindow<MockRuntime>,
) {
    let mut nonce = [0; 16];
    getrandom::fill(&mut nonce).unwrap();
    let dir = Fixture(std::env::temp_dir().join(format!("translation-admission-{nonce:x?}")));
    std::fs::create_dir(&dir.0).unwrap();
    let mut context = tauri::test::mock_context(tauri::test::noop_assets());
    context.config_mut().identifier = dir.0.to_str().unwrap().into();
    let app = tauri::test::mock_builder()
        .manage(TranslationState::default())
        .invoke_handler(tauri::generate_handler![
            translation_capabilities,
            translation_start,
            translation_bergamot_document,
            translation_bergamot_batch,
            translation_bergamot_asset,
            translation_install_model,
            translation_model_status,
            translation_remove_model,
        ])
        .build(context)
        .unwrap();
    assert_eq!(app.path().app_data_dir().unwrap(), dir.0);
    let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
        .data_directory(dir.0.clone())
        .build()
        .unwrap();
    (dir, app, window)
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
fn public_commands_gate_unqualified_providers_before_files_downloads_or_workers() {
    let (dir, app, window) = fixture();
    let caps = invoke(&window, "translation_capabilities", json!({})).unwrap();
    for id in [
        "hy-mt2-1.8b-q8",
        "bergamot-fr-en-worker",
        "bergamot-es-en-worker",
    ] {
        let source = if id.contains("-es-") { "es" } else { "fr" };
        let model = caps["models"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["id"] == id)
            .unwrap();
        assert_eq!(model["available"], false);
        assert_eq!(model["unavailableReason"], "acceptance-pending");
        let request = json!({"documentUrl":"/uploads/missing.html", "modelId":id,
            "sourceLanguage":source, "targetLanguage":"en", "qualityMode":"balanced"});
        let mut calls = vec![
            ("translation_start", json!({"request":request})),
            ("translation_install_model", json!({"modelId":id})),
        ];
        if id.starts_with("bergamot-") {
            calls.extend([
                ("translation_bergamot_document", json!({"request":request,"onRequest":"__CHANNEL__:9"})),
                ("translation_bergamot_batch", json!({"request":{"jobId":"blocked","sourceLanguage":source,
                    "targetLanguage":"en","segments":[{"id":"s","text":"Bonjour."}]},"onRequest":"__CHANNEL__:9"})),
                ("translation_bergamot_asset", json!({"sourceLanguage":source,"asset":"model"})),
            ]);
        }
        for (command, body) in calls {
            let error = invoke(&window, command, body).unwrap_err();
            assert_eq!(
                error["code"], "provider-acceptance-pending",
                "{command}: {error}"
            );
        }
        let state = app.state::<TranslationState>();
        let _claim = state
            .claim_model_removal(id)
            .expect("rejected jobs must not retain ownership");
    }
    assert!(
        !dir.0.join("translation").exists(),
        "no downloads or checkpoints"
    );
    assert!(!dir.0.join("uploads").exists(), "no document writes");

    // Admitted OPUS requests must still reach ordinary model checks, not the acceptance rejection.
    for (id, source) in [
        ("opus-mt-fr-en-ctranslate2", "fr"),
        ("opus-mt-es-en-ctranslate2", "es"),
    ] {
        let error = invoke(
            &window,
            "translation_start",
            json!({"request":{
                "documentUrl":"/uploads/missing.html", "modelId":id,
                "sourceLanguage":source, "targetLanguage":"en", "qualityMode":"balanced"
            }}),
        )
        .unwrap_err();
        assert_eq!(error["code"], "model-not-installed", "{error}");
    }
}

#[test]
fn gated_model_status_and_removal_preserve_existing_document_cache_and_audio_files() {
    let (dir, app, window) = fixture();
    for path in [
        "uploads/saved.html",
        "translation/jobs/checkpoint.json",
        "audiobooks/saved.audio",
    ] {
        let path = dir.0.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"keep").unwrap();
    }
    for id in [
        "hy-mt2-1.8b-q8",
        "bergamot-fr-en-worker",
        "bergamot-es-en-worker",
    ] {
        let manifest = crate::translation::model_store::manifest_by_id(id).unwrap();
        let model_dir = crate::translation::model_store::installed_translation_model_dir(
            app.handle(),
            manifest,
        )
        .unwrap();
        // Each root alone must remain discoverable after an interrupted install or failed verification.
        let legacy =
            crate::translation::model_store::translation_model_work_dir(app.handle(), manifest)
                .unwrap();
        for root in [
            model_dir.clone(),
            model_dir.with_file_name(format!(".{id}.installing")),
            model_dir.with_file_name(format!(".{id}.previous")),
            legacy,
        ] {
            std::fs::create_dir_all(&root).unwrap();
            std::fs::write(root.join("model.bin"), b"old installation").unwrap();
            let status = invoke(
                &window,
                "translation_model_status",
                json!({"request":{"modelId":id}}),
            )
            .unwrap();
            assert_eq!(status["modelId"], id);
            assert_eq!(
                status["installed"], false,
                "partial files are not model readiness"
            );
            assert_eq!(status["hasStoredFiles"], true);
            assert_eq!(status["installedBytes"], 16);
            assert!(status["message"].as_str().unwrap().contains("acceptance"));
            assert_eq!(
                std::fs::read(root.join("model.bin")).unwrap(),
                b"old installation",
                "status must not recover/delete files"
            );
            let error =
                invoke(&window, "translation_install_model", json!({"modelId":id})).unwrap_err();
            assert_eq!(error["code"], "provider-acceptance-pending");
            let removed =
                invoke(&window, "translation_remove_model", json!({"modelId":id})).unwrap();
            assert_eq!(removed["removed"], true);
            assert_eq!(removed["bytesFreed"], 16);
            assert!(!root.exists());
            let after = invoke(
                &window,
                "translation_model_status",
                json!({"request":{"modelId":id}}),
            )
            .unwrap();
            assert_eq!(after["hasStoredFiles"], false);
            assert_eq!(after["installedBytes"], 0);
        }
    }
    for path in [
        "uploads/saved.html",
        "translation/jobs/checkpoint.json",
        "audiobooks/saved.audio",
    ] {
        assert_eq!(std::fs::read(dir.0.join(path)).unwrap(), b"keep");
    }
}

#[test]
fn same_size_corrupt_worker_pack_is_removable_without_becoming_installed() {
    let (_dir, app, window) = fixture();
    let id = "bergamot-fr-en-worker";
    let manifest = crate::translation::model_store::manifest_by_id(id).unwrap();
    let model_dir =
        crate::translation::model_store::installed_translation_model_dir(app.handle(), manifest)
            .unwrap();
    std::fs::create_dir_all(&model_dir).unwrap();
    // Sparse synthetic files match the inventory sizes but never a trusted model hash.
    for pin in manifest.files {
        std::fs::File::create(model_dir.join(pin.path))
            .unwrap()
            .set_len(pin.bytes)
            .unwrap();
    }
    assert!(manifest.has_required_files(&model_dir));
    let status = invoke(
        &window,
        "translation_model_status",
        json!({"request":{"modelId":id}}),
    )
    .unwrap();
    assert_eq!(status["installed"], false);
    assert_eq!(status["hasStoredFiles"], true);
    assert_eq!(status["installedBytes"], manifest.total_bytes());
    let removed = invoke(&window, "translation_remove_model", json!({"modelId":id})).unwrap();
    assert_eq!(removed["bytesFreed"], manifest.total_bytes());
    assert!(!model_dir.exists());
}
