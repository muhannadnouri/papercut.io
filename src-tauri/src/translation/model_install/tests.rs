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
fn auxiliary_and_unknown_ids_are_not_download_admission_or_engine_entries() {
    for id in ["alignment-awesome-align-int8", "unknown-model"] {
        assert!(crate::translation::model_store::manifest_by_id(id).is_none());
        assert!(crate::translation::models::find_planned_model(id).is_none());
    }
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
