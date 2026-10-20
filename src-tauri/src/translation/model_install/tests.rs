use super::download_percent;

#[test]
fn progress_percent_clamps_and_handles_zero_total() {
    assert_eq!(download_percent(0, 0), 0);
    assert_eq!(download_percent(50, 100), 50);
    assert_eq!(download_percent(250, 100), 100);
}

#[test]
fn explicit_mozilla_urls_do_not_change_hugging_face_pins() {
    use crate::translation::model_store::manifest_by_id;
    let opus = manifest_by_id("opus-mt-fr-en-ctranslate2").unwrap();
    assert_eq!(super::model_file_url(opus, &opus.files[0]),
        "https://huggingface.co/michaelfeil/ct2fast-opus-mt-fr-en/resolve/cb3b2d680bf35591a508d8479e2c99c44e281ef3/config.json");
    let worker = manifest_by_id("bergamot-fr-en-worker").unwrap();
    assert_eq!(
        super::model_file_url(worker, &worker.files[0]),
        worker.files[0].download_url.unwrap()
    );
}

#[test]
fn auxiliary_install_admission_is_feature_gated_and_never_an_engine_entry() {
    let id = "alignment-awesome-align-int8";
    assert_eq!(
        crate::translation::model_store::manifest_by_id(id).is_some(),
        cfg!(feature = "native-translation-alignment")
    );
    assert!(crate::translation::models::find_planned_model(id).is_none());
    assert!(crate::translation::model_store::manifest_by_id("unknown-model").is_none());
    assert!(
        !crate::translation::capabilities::translation_capabilities()
            .models
            .iter()
            .any(|m| m.id == id)
    );
}

#[cfg(feature = "native-translation")]
#[test]
fn actual_downloader_transaction_and_leases_round_trip_without_touching_app_data() {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::{Duration, Instant};
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url =
        Box::leak(format!("http://{}/asset", listener.local_addr().unwrap()).into_boxed_str());
    // Serve only the two required pack files, through the production HTTP downloader.
    let server = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        for _ in 0..2 {
            let mut socket = loop {
                if let Ok((socket, _)) = listener.accept() {
                    break socket;
                }
                assert!(
                    Instant::now() < deadline,
                    "installer did not request required files"
                );
                std::thread::sleep(Duration::from_millis(5));
            };
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = [0; 2048];
            socket.read(&mut request).unwrap();
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\nConnection: close\r\n\r\nabc")
                .unwrap();
        }
    });
    let files = Box::leak(
        vec!["model.bin", "LICENSE.txt"]
            .into_iter()
            .map(|path| TranslationModelFile {
                path,
                download_url: Some(&*url),
                bytes: 3,
                sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );
    let manifest = TranslationModelManifest {
        model_id: "pack",
        directory_name: "pack",
        source_label: "fixture",
        source_url: "",
        revision: "fixture",
        files,
        installable: true,
    };
    let mut nonce = [0; 16];
    getrandom::fill(&mut nonce).unwrap();
    let root = std::env::temp_dir().join(format!("papercut-installer-http-{nonce:x?}"));
    fs::create_dir(&root).unwrap();
    let _cleanup = WorkDirGuard::new(root.clone());
    let paths = transaction::Paths::new(root.join("pack")).unwrap();
    let app = tauri::test::mock_builder()
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .unwrap();
    let state = TranslationState::default();
    let lease = state.claim_model_install(manifest.directory_name).unwrap();
    assert!(state.claim_model_removal(manifest.directory_name).is_err());
    assert_eq!(
        transaction::install(
            &paths,
            manifest,
            |f, p, n| download_and_verify_file(app.handle(), manifest, f, p, n),
            |p, n| crate::library_transfer::ensure_available_space(p, n).map_err(|e| e.to_string()),
            |a, b| fs::rename(a, b)
        )
        .unwrap(),
        6
    );
    server.join().unwrap();
    drop(lease);
    assert!(manifest.has_required_files(&paths.final_dir));
    manifest.verify_files(&paths.final_dir).unwrap();
    let read = state.claim_model_read(manifest.directory_name).unwrap();
    assert!(state.claim_model_removal(manifest.directory_name).is_err());
    drop(read);
    let remove = state.claim_model_removal(manifest.directory_name).unwrap();
    assert_eq!(paths.remove().unwrap(), (true, 6));
    drop(remove);
    assert!(!manifest.has_required_files(&paths.final_dir));
    assert!(state.claim_model_install(manifest.directory_name).is_ok());
}

#[cfg(feature = "native-translation")]
#[test]
fn actual_downloader_verifies_bytes_and_rejects_oversize_before_writing() {
    use crate::translation::model_store::{manifest_by_id, TranslationModelFile};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::{Duration, Instant};
    for (body, declared) in [
        (b"abc".as_slice(), 3),
        (b"abd", 3),
        (b"ab", 2),
        (b"abcd", 4),
        (b"a", 3),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url: &'static str =
            Box::leak(format!("http://{}/asset", listener.local_addr().unwrap()).into_boxed_str());
        let body = body.to_vec();
        let good = body == b"abc";
        let oversize = body.len() > 3;
        let server = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut socket = loop {
                if let Ok((socket, _)) = listener.accept() {
                    break socket;
                }
                assert!(Instant::now() < deadline, "test downloader did not connect");
                std::thread::sleep(Duration::from_millis(5));
            };
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = [0; 2048];
            socket.read(&mut request).unwrap();
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                declared
            )
            .unwrap();
            socket.write_all(&body).unwrap();
        });
        let manifest = manifest_by_id("bergamot-fr-en-worker").unwrap();
        let pin = TranslationModelFile {
            path: "fixture.bin",
            download_url: Some(url),
            bytes: 3,
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        };
        let mut nonce = [0u8; 16];
        getrandom::fill(&mut nonce).unwrap();
        let dir = std::env::temp_dir().join(format!("papercut-download-{nonce:x?}"));
        let guard = super::WorkDirGuard::new(dir.clone());
        let target = dir.join(pin.path);
        let app = tauri::test::mock_builder()
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap();
        let mut total = 0;
        let result =
            super::download_and_verify_file(app.handle(), manifest, &pin, &target, &mut total);
        server.join().unwrap();
        assert_eq!(result.is_ok(), good);
        if good {
            assert_eq!(std::fs::read(&target).unwrap(), b"abc");
        }
        if oversize {
            assert_eq!(std::fs::metadata(&target).unwrap().len(), 0);
            assert_eq!(total, 0);
        }
        drop(guard);
        assert!(!dir.exists());
    }
}

#[cfg(feature = "native-translation-alignment")]
#[test]
#[ignore = "requires external pinned alignment files; copies locally, never downloads weights"]
fn pinned_auxiliary_commands_verify_reuse_notices_and_remove_under_the_runtime_lease_key() {
    use super::*;
    use crate::translation::{
        capabilities::translation_model_status, commands, types::TranslationModelStatusRequest,
    };
    use tauri::Manager;
    let source = std::env::var_os("PAPERCUT_ALIGNMENT_ASSETS_DIR")
        .expect("Set PAPERCUT_ALIGNMENT_ASSETS_DIR");
    let manifest = manifest_by_id("alignment-awesome-align-int8").unwrap();
    let mut nonce = [0; 16];
    getrandom::fill(&mut nonce).unwrap();
    let root = std::env::temp_dir().join(format!("papercut-aux-commands-{nonce:x?}"));
    fs::create_dir(&root).unwrap();
    let _cleanup = WorkDirGuard::new(root.clone());
    let mut context = tauri::test::mock_context(tauri::test::noop_assets());
    // Mock-only absolute identifier redirects both resolver roots without global environment changes.
    context.config_mut().identifier = root.to_string_lossy().into_owned();
    let app = tauri::test::mock_builder()
        .manage(TranslationState::default())
        .build(context)
        .unwrap();
    assert_eq!(app.path().app_data_dir().unwrap(), root);
    assert_eq!(app.path().app_cache_dir().unwrap(), root);
    let state = app.state::<TranslationState>();
    let status = || {
        translation_model_status(
            app.handle(),
            &state,
            TranslationModelStatusRequest {
                model_id: manifest.model_id.into(),
            },
        )
    };
    assert!(!status().installed);
    assert!(status().message.contains("Auxiliary alignment"));
    let dir = installed_translation_model_dir(app.handle(), manifest).unwrap();
    fs::create_dir_all(&dir).unwrap();
    for pin in manifest.files {
        if let Some(bytes) = manifest.bundled_file(pin.path) {
            fs::write(dir.join(pin.path), bytes).unwrap();
        } else {
            fs::copy(Path::new(&source).join(pin.path), dir.join(pin.path)).unwrap();
        }
    }
    assert!(status().installed);
    assert!(status().message.contains("separate checks"));
    assert_eq!(status().archive_bytes, 153_652_432);
    let installed = tauri::async_runtime::block_on(commands::translation_install_model(
        app.handle().clone(),
        app.state(),
        manifest.model_id.into(),
    ))
    .unwrap();
    assert_eq!(installed.bytes, manifest.total_bytes()); // Verified reuse must not reach network delivery.
    for name in ["LICENSE-APACHE-2.0.txt", "ATTRIBUTION.md"] {
        let path = dir.join(name);
        let original = fs::read(&path).unwrap();
        let mut altered = original.clone();
        altered[0] ^= 1;
        fs::write(&path, &altered).unwrap();
        assert!(!status().installed);
        fs::write(&path, &original).unwrap();
        fs::remove_file(&path).unwrap();
        assert!(!status().installed);
        fs::write(&path, &original).unwrap();
    }
    let lease = state.claim_model_read(manifest.directory_name).unwrap();
    assert!(
        tauri::async_runtime::block_on(commands::translation_install_model(
            app.handle().clone(),
            app.state(),
            manifest.model_id.into()
        ))
        .is_err()
    );
    assert!(
        tauri::async_runtime::block_on(commands::translation_remove_model(
            app.handle().clone(),
            app.state(),
            manifest.model_id.into()
        ))
        .is_err()
    );
    assert!(status().installed);
    drop(lease);
    let sentinel = root.join("preserved-document.html");
    fs::write(&sentinel, b"unchanged").unwrap();
    let removed = tauri::async_runtime::block_on(commands::translation_remove_model(
        app.handle().clone(),
        app.state(),
        manifest.model_id.into(),
    ))
    .unwrap();
    assert!(removed.removed);
    assert_eq!(removed.bytes_freed, manifest.total_bytes());
    assert!(!status().installed);
    assert_eq!(fs::read(sentinel).unwrap(), b"unchanged");
}
