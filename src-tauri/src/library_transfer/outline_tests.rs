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
) -> (ZipArchive<Cursor<Vec<u8>>>, TransferDocument) {
    let mut payloads = HashMap::new();
    let document = prepare_document(sender, book, &mut payloads).unwrap();
    assert!(document.assets.is_empty());
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
    let (mut archive, mut document) = export_book(sender.handle(), book);
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
    for (json, error) in [
        ("oops", "expected value"),
        (
            r#"[{"label":"Section","target":"ch0-absent","children":[]}]"#,
            "destination",
        ),
    ] {
        fs::write(&sender_outline, json).unwrap();
        let (mut damaged, bad_document) = export_book(sender.handle(), book);
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
