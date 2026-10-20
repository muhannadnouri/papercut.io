use super::*;
use crate::translation::{
    model_store, models, state::TranslationState, types::TranslationModelStatusRequest,
};
use std::fs;

struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        let mut nonce = [0u8; 16];
        getrandom::fill(&mut nonce).unwrap();
        let path = std::env::temp_dir().join(format!("papercut-bergamot-assets-{nonce:x?}"));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
const TINY: TranslationModelFile = TranslationModelFile {
    path: "model.bin",
    download_url: None,
    bytes: 3,
    sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
};

#[test]
fn catalog_entries_reuse_the_exact_retained_asset_pins() {
    let evidence: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../docs/offline-translation-evaluation.json"
    ))
    .unwrap();
    for language in ["fr", "es"] {
        let manifest = manifest_for_language(language).unwrap();
        assert!(model_store::manifest_by_id(manifest.model_id).is_some());
        let model = models::find_planned_model(manifest.model_id).unwrap();
        assert_eq!(model.engine, "bergamot");
        assert_eq!(model.source_languages, &[language]);
        assert_eq!(model.target_languages, &["en"]);
        assert_eq!(
            model_store::manifest_for(model).total_bytes(),
            manifest.total_bytes()
        );
        assert!(models::planned_models()
            .iter()
            .any(|m| m.id == manifest.model_id));
        assert!(manifest.total_bytes() < 64 * 1024 * 1024);
        for asset in [Asset::Wasm, Asset::Model, Asset::Lex, Asset::Vocab] {
            let pin = manifest
                .files
                .iter()
                .find(|p| p.path == asset.path())
                .unwrap();
            let key = match asset {
                Asset::Wasm => "wasm",
                Asset::Model => "model",
                Asset::Lex => "lex",
                Asset::Vocab => "vocab",
            };
            let record = if key == "wasm" {
                &evidence["bergamotWasmRecord"]
            } else {
                evidence["bergamotModelRecords"][language]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|r| r["fileType"] == key)
                    .unwrap()
            };
            assert_eq!(pin.bytes, record["attachment"]["size"].as_u64().unwrap());
            assert_eq!(pin.sha256, record["attachment"]["hash"].as_str().unwrap());
            assert_eq!(
                pin.download_url.unwrap(),
                format!(
                    "{}{}",
                    evidence["mozillaAttachmentBaseUrl"].as_str().unwrap(),
                    record["attachment"]["location"].as_str().unwrap()
                )
            );
        }
    }
    assert!(manifest_for_language("../fr").is_err());
    assert!(manifest_for_language("ar").is_err());
    assert!(model_store::manifest_by_id("../../private").is_none());
    for value in ["../secret", "model.bin", "constructor", "weights"] {
        assert!(serde_json::from_value::<Asset>(serde_json::json!(value)).is_err());
    }
}

#[test]
fn only_exact_size_hash_verified_regular_files_are_delivered() {
    let dir = Fixture::new();
    let path = dir.0.join(TINY.path);
    assert!(read_verified(&dir.0, &TINY).is_err());
    fs::write(&path, b"abc").unwrap();
    assert_eq!(read_verified(&dir.0, &TINY).unwrap(), b"abc");
    let fixture = TranslationModelManifest {
        files: &[TINY],
        ..manifest_for_language("fr").unwrap()
    };
    assert_eq!(read_asset(&dir.0, fixture, Asset::Model).unwrap(), b"abc");
    assert!(read_asset(&dir.0, fixture, Asset::Wasm).is_err());
    assert!(verify_install(&dir.0, fixture).is_ok());
    for bytes in [b"abd".as_slice(), b"abcd", b"ab"] {
        fs::write(&path, bytes).unwrap();
        assert!(read_verified(&dir.0, &TINY).is_err());
        assert!(verify_install(&dir.0, fixture).is_err());
    }
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(read_verified(&dir.0, &TINY).is_err());
    let huge = TranslationModelFile {
        bytes: 32 * 1024 * 1024 + 1,
        ..TINY
    };
    assert!(read_verified(&dir.0, &huge)
        .unwrap_err()
        .contains("size limit"));
}

#[cfg(unix)]
#[test]
fn symlinked_files_and_model_roots_cannot_deliver_outside_data() {
    use std::os::unix::fs::symlink;
    let dir = Fixture::new();
    let outside = Fixture::new();
    fs::write(outside.0.join(TINY.path), b"abc").unwrap();
    symlink(outside.0.join(TINY.path), dir.0.join(TINY.path)).unwrap();
    assert!(read_verified(&dir.0, &TINY).is_err());
    let link = dir.0.join("root-link");
    symlink(&outside.0, &link).unwrap();
    assert!(read_verified(&link, &TINY).is_err());
}

#[test]
fn raw_asset_command_rejects_other_windows_and_releases_its_claim_on_failure() {
    use tauri::{
        ipc::{InvokeResponseBody, IpcResponse},
        Manager,
    };
    assert!(
        matches!(tauri::ipc::Response::new(vec![1u8, 2, 3]).body().unwrap(), InvokeResponseBody::Raw(bytes) if bytes == [1, 2, 3])
    );
    let mut context = tauri::test::mock_context(tauri::test::noop_assets());
    let name = format!("io.papercut.bergamot-assets-{}", std::process::id());
    context.config_mut().identifier = name;
    let app = tauri::test::mock_builder()
        .manage(TranslationState::default())
        .build(context)
        .unwrap();
    for label in ["other", "main"] {
        let window = tauri::WebviewWindowBuilder::new(&app, label, Default::default())
            .data_directory(std::env::temp_dir())
            .build()
            .unwrap();
        let result = tauri::async_runtime::block_on(
            crate::translation::commands::translation_bergamot_asset(
                app.handle().clone(),
                app.state(),
                window,
                "ar".into(),
                Asset::Model,
            ),
        );
        let error = match result {
            Ok(_) => panic!("unexpected asset delivery"),
            Err(error) => error,
        };
        assert!(error.message.contains(if label == "other" {
            "main WebView"
        } else {
            "does not support"
        }));
    }
    let state = app.state::<TranslationState>();
    let manifest = manifest_for_language("fr").unwrap();
    let status =
        tauri::async_runtime::block_on(crate::translation::commands::translation_model_status(
            app.handle().clone(),
            app.state(),
            TranslationModelStatusRequest {
                model_id: manifest.model_id.into(),
            },
        ))
        .unwrap();
    assert!(!status.installed);
    assert!(status
        .message
        .contains(if cfg!(feature = "native-translation") {
            "installable"
        } else {
            "not compiled"
        }));
    let window = app.get_webview_window("main").unwrap();
    let result =
        tauri::async_runtime::block_on(crate::translation::commands::translation_bergamot_asset(
            app.handle().clone(),
            app.state(),
            window,
            "fr".into(),
            Asset::Model,
        ));
    assert!(result.is_err());
    let read = state
        .claim_job("bergamot-asset-read", manifest.directory_name)
        .unwrap();
    let lease = state.claim_model_read(manifest.directory_name).unwrap();
    assert!(state.claim_model_install(manifest.directory_name).is_err());
    assert!(state.claim_model_removal(manifest.directory_name).is_err());
    drop(read);
    drop(lease);
    let mutation = state.claim_model_install(manifest.directory_name).unwrap();
    assert!(state
        .claim_job("bergamot-asset-read", manifest.directory_name)
        .is_err());
    assert!(state.claim_model_read(manifest.directory_name).is_err());
    drop(mutation);
    assert!(state.claim_model_removal(manifest.directory_name).is_ok());
}
