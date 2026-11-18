//! Round-trip the document payload through the ZIP path used by file and nearby transfer.
use super::*;
use crate::document_uploads::{commands, DocumentUploadState};
use kuchikiki::{parse_html, traits::TendrilSink};
use std::io::Cursor;

/// Separate app-data roots ensure the recipient cannot resolve an outline from the sender's files.
fn library(root: &Path) -> tauri::App<tauri::test::MockRuntime> {
    let mut context = tauri::test::mock_context(tauri::test::noop_assets());
    context.config_mut().identifier = root.to_str().unwrap().to_owned();
    tauri::test::mock_builder()
        .manage(DocumentUploadState::default())
        .plugin(tauri_plugin_fs::init())
        .build(context)
        .unwrap()
}

/// An image-free fixture makes the outline the only reason to select v5.
/// Reopen the written ZIP so restore sees exactly what a recipient would receive.
fn export_book(
    sender: &tauri::AppHandle<tauri::test::MockRuntime>,
    book: &UploadedDocument,
    damaged_outline: Option<&str>,
) -> (ZipArchive<Cursor<Vec<u8>>>, TransferDocument) {
    let mut payloads = HashMap::new();
    let mut document = prepare_document(sender, book, &mut payloads).unwrap();
    assert!(document.assets.is_empty());
    // Model damaged incoming packages independently of the sender's preflight.
    if let Some(json) = damaged_outline {
        let (sha256, bytes) = sha256_reader(&mut json.as_bytes()).unwrap();
        document.outline = Some(TransferOutline { sha256, bytes });
    }
    let documents = vec![document];
    let manifest = TransferManifest {
        kind: PACKAGE_KIND.into(),
        schema_version: export_version(&documents, true),
        created_at_ms: 1,
        documents,
        organization: TransferOrganization::default(),
        audiobooks: vec![],
    };
    assert_eq!(manifest.schema_version, 5);
    let mut zip = Cursor::new(Vec::new());
    write_package(&mut zip, &manifest, |path| {
        if path == document_outline_path(&book.id) {
            if let Some(json) = damaged_outline {
                return Ok(Box::new(Cursor::new(json.as_bytes().to_vec())));
            }
        }
        Ok(Box::new(File::open(&payloads[path]).unwrap()))
    })
    .unwrap();
    zip.rewind().unwrap();
    let mut archive = open_archive(zip).unwrap();
    let decoded = read_manifest(&mut archive).unwrap();
    (archive, decoded.documents.into_iter().next().unwrap())
}

/// Both links target the same chapter. Checking the reopened headings catches a restore
/// that preserves the labels but silently sends every entry to the chapter opening.
#[test]
fn image_free_epub_restores_publisher_targets_into_a_fresh_library() {
    let root = std::env::temp_dir().join(format!(
        "papercut-transfer-outline-{}-{}",
        std::process::id(),
        now_ms().unwrap()
    ));
    let sender = library(&root.join("sender"));
    let recipient = library(&root.join("recipient"));
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../scripts/fixtures/epub/publisher-outline-3.epub");
    let imported = tauri::async_runtime::block_on(commands::document_uploads_import_paths(
        sender.handle().clone(),
        sender.state(),
        serde_json::from_value(serde_json::json!({"paths":[fixture]})).unwrap(),
    ))
    .unwrap();
    let [book] = imported.imported.as_slice() else {
        panic!("fixture import failed: {:?}", imported.failures)
    };
    let (mut archive, mut document) = export_book(sender.handle(), book, None);
    let existing = |app| {
        list_uploads(app)
            .unwrap()
            .into_iter()
            .map(|d| d.id)
            .collect::<HashSet<_>>()
    };
    let ids = existing(recipient.handle());
    assert!(ids.is_empty());
    let sidecar_dir = upload_dir(recipient.handle(), &book.id).unwrap();
    let digest = std::mem::replace(
        &mut document.outline.as_mut().unwrap().sha256,
        "0".repeat(64),
    );
    assert!(
        restore_document(recipient.handle(), &mut archive, &document, &ids)
            .unwrap_err()
            .contains("checksum")
    );
    document.outline.as_mut().unwrap().sha256 = digest;
    let sender_outline = upload_dir(sender.handle(), &book.id)
        .unwrap()
        .join(crate::document_uploads::OUTLINE_FILE_NAME);
    let original = fs::read(&sender_outline).unwrap();
    for (json, error) in [
        ("oops", "expected value"),
        (
            r#"[{"label":"Section","target":"ch0-absent","children":[]}]"#,
            "destination",
        ),
    ] {
        fs::write(&sender_outline, json).unwrap();
        let export_error =
            prepare_document(sender.handle(), book, &mut HashMap::new()).unwrap_err();
        assert!(
            export_error.contains(&book.title) && export_error.contains(error),
            "{export_error}"
        );
        fs::write(&sender_outline, &original).unwrap();
        let (mut damaged, bad_document) = export_book(sender.handle(), book, Some(json));
        assert!(
            restore_document(recipient.handle(), &mut damaged, &bad_document, &ids)
                .unwrap_err()
                .contains(error)
        );
        assert!(existing(recipient.handle()).is_empty());
        assert!(!sidecar_dir.exists());
    }
    assert!(restore_document(recipient.handle(), &mut archive, &document, &ids).unwrap());
    let reopened = tauri::async_runtime::block_on(commands::document_uploads_get_source(
        recipient.handle().clone(),
        serde_json::from_value(serde_json::json!({"documentUrl":book.url})).unwrap(),
        Some(true),
    ))
    .unwrap();
    // Chromium opens this same reader response. Keep the browser fixture tied to what restore delivers.
    let browser_fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../scripts/fixtures/epub/publisher-outline-3.reader.json"
    ))
    .unwrap();
    assert_eq!(serde_json::to_value(&reopened).unwrap(), browser_fixture);

    // A match on body text proves that a label miss is meaningful on the rebuilt recipient index.
    for (query, expected) in [("valley", 1), ("trail", 0), ("routes", 0)] {
        let response = tauri::async_runtime::block_on(commands::document_uploads_search(
            recipient.handle().clone(),
            serde_json::from_value(serde_json::json!({"query":query})).unwrap(),
            tauri::ipc::Channel::new(|_| Ok(())),
        ))
        .unwrap();
        assert_eq!(
            response.results.len(),
            expected,
            "restored search for {query}"
        );
    }
    let [group] = reopened.outline.as_slice() else {
        panic!("expected restored publisher group")
    };
    assert_eq!(group.label, "Walking routes");
    assert_eq!(group.target, None);
    assert_eq!(group.children.len(), 2);
    let dom = parse_html().one(reopened.html).document_node;
    for (entry, (label, id, heading)) in group.children.iter().zip([
        ("High trail", "ch0-ridge", "Morning on the ridge"),
        ("Low trail", "ch0-valley", "Evening in the valley"),
    ]) {
        assert_eq!(entry.label, label);
        assert_eq!(entry.target.as_deref(), Some(id));
        assert_eq!(
            dom.select_first(&format!("h2#{id}"))
                .unwrap()
                .as_node()
                .text_contents(),
            heading
        );
    }
    let sidecar = upload_dir(recipient.handle(), &book.id)
        .unwrap()
        .join(crate::document_uploads::OUTLINE_FILE_NAME);
    fs::remove_file(&sidecar).unwrap();
    assert!(!restore_document(
        recipient.handle(),
        &mut archive,
        &document,
        &existing(recipient.handle())
    )
    .unwrap());
    assert!(!sidecar.exists());
    fs::remove_dir_all(root).unwrap();
}

/// Check the same destinations before and after the receiver sanitizes the generated chapter again.
#[test]
fn publisher_body_unicode_and_named_anchors_survive_transfer() {
    let root = std::env::temp_dir().join(format!(
        "papercut-transfer-anchors-{}-{}",
        std::process::id(),
        now_ms().unwrap()
    ));
    let sender = library(&root.join("sender"));
    let recipient = library(&root.join("recipient"));
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../scripts/fixtures/epub/publisher-anchors.epub");
    let imported = tauri::async_runtime::block_on(commands::document_uploads_import_paths(
        sender.handle().clone(),
        sender.state(),
        serde_json::from_value(serde_json::json!({"paths":[fixture]})).unwrap(),
    ))
    .unwrap();
    let book = &imported.imported[0];
    let (mut archive, document) = export_book(sender.handle(), book, None);
    assert!(
        restore_document(recipient.handle(), &mut archive, &document, &HashSet::new()).unwrap()
    );
    for app in [&sender, &recipient] {
        let source = tauri::async_runtime::block_on(commands::document_uploads_get_source(
            app.handle().clone(),
            serde_json::from_value(serde_json::json!({"documentUrl":book.url})).unwrap(),
            Some(true),
        ))
        .unwrap();
        let [opening, first, last, legacy] = source.outline.as_slice() else {
            panic!("missing anchor destination")
        };
        assert_eq!(opening.target.as_deref(), Some("chapter-0"));
        let dom = parse_html().one(source.html).document_node;
        assert!(dom.select_first("section#chapter-0").is_ok());
        for (entry, heading) in [(first, "Prologue"), (last, "Epilogue")] {
            let id = entry.target.as_deref().unwrap();
            assert_eq!(
                dom.select_first(&format!("h2[id='{id}']"))
                    .unwrap()
                    .as_node()
                    .text_contents(),
                heading
            );
        }
        let last_id = last.target.as_deref().unwrap();
        assert!(dom.select_first(&format!("a[href='#{last_id}']")).is_ok());
        assert!(dom.select_first("a[href='#chapter-0']").is_ok());
        assert_eq!(legacy.target.as_deref(), Some("ch0-legacy"));
        assert!(dom.select_first("a[name='ch0-legacy']").is_ok());
    }
    fs::remove_dir_all(root).unwrap();
}
