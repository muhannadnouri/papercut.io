use super::*;
use sha2::{Digest, Sha256};
use std::fs;

#[test]
fn candidate_pins_are_bounded_unique_and_not_translation_or_installer_entries() {
    assert_eq!(REPOSITORY, "cstr/awesome-align-onnx-int8");
    assert_eq!(REVISION.len(), 40);
    assert_eq!(FILES.len(), 7);
    let paths: std::collections::BTreeSet<_> = FILES.iter().map(|p| p.path).collect();
    assert_eq!(paths.len(), FILES.len());
    assert!(FILES
        .iter()
        .all(|p| p.bytes > 0 && p.sha256.len() == 64 && p.download_url.is_none()));
    assert!(FILES.iter().map(|p| p.bytes).sum::<u64>() < PACK_LIMIT);
    assert!(TOKENIZER_ID.contains(
        FILES
            .iter()
            .find(|p| p.path == "tokenizer.json")
            .unwrap()
            .sha256
    ));
    for id in [
        REPOSITORY,
        "cstr/awesome-align-onnx",
        "awesome-align-onnx-int8",
    ] {
        assert!(crate::translation::models::find_planned_model(id).is_none());
        assert!(crate::translation::model_store::manifest_by_id(id).is_none());
    }
}

#[test]
fn every_asset_is_required_even_when_the_tokenizer_itself_is_valid() {
    let mut nonce = [0; 16];
    getrandom::fill(&mut nonce).unwrap();
    let dir = std::env::temp_dir().join(format!("papercut-alignment-pack-{nonce:x?}"));
    fs::create_dir(&dir).unwrap();
    let bytes = b"tokenizer";
    let pins = [
        TranslationModelFile {
            path: "tokenizer.json",
            download_url: None,
            bytes: bytes.len() as u64,
            sha256: "5f97e3774c51edd1d63706c2ec3826c564a067794770cdab0f8c4797971cacf9",
        },
        TranslationModelFile {
            path: "config.json",
            download_url: None,
            bytes: 3,
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        },
    ];
    assert_eq!(format!("{:x}", Sha256::digest(bytes)), pins[0].sha256);
    fs::write(dir.join("tokenizer.json"), bytes).unwrap();
    assert!(checked_snapshot(&dir, &pins).is_err());
    fs::write(dir.join("config.json"), b"abc").unwrap();
    assert_eq!(checked_snapshot(&dir, &pins).unwrap(), bytes);
    fs::write(dir.join("config.json"), b"abd").unwrap();
    assert!(checked_snapshot(&dir, &pins).is_err());
    let huge = [
        TranslationModelFile {
            bytes: u64::MAX,
            ..pins[0]
        },
        pins[1],
    ];
    assert!(checked_snapshot(&dir, &huge).is_err());
    assert_eq!(
        checked_snapshot(&dir, &[]).err(),
        Some(AdmissionError::Profile)
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn graph_snapshot_is_verified_and_immutable_and_cancelled_packs_do_not_escape() {
    let mut nonce = [0; 16];
    getrandom::fill(&mut nonce).unwrap();
    let dir = std::env::temp_dir().join(format!("papercut-alignment-snapshot-{nonce:x?}"));
    fs::create_dir(&dir).unwrap();
    let pins = [
        TranslationModelFile {
            path: "model.onnx",
            download_url: None,
            bytes: 3,
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        },
        TranslationModelFile {
            path: "tokenizer.json",
            download_url: None,
            bytes: 3,
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        },
    ];
    for pin in pins {
        fs::write(dir.join(pin.path), b"abc").unwrap();
    }
    let (tokenizer, graph) = snapshots(&dir, &pins, true, &|| false).unwrap();
    assert_eq!(tokenizer, b"abc");
    assert_eq!(graph, b"abc");
    fs::write(dir.join("model.onnx"), b"abd").unwrap();
    assert!(snapshots(&dir, &pins, true, &|| false).is_err());
    assert_eq!(graph, b"abc");
    assert_eq!(
        snapshots(&dir, &pins, true, &|| true).err(),
        Some(AdmissionError::Assets(verified::AssetError::Cancelled))
    );
    fs::remove_dir_all(dir).unwrap();
}
