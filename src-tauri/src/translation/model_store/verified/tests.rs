use super::*;

struct Directory(std::path::PathBuf);
impl Directory {
    fn new() -> Self {
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce).unwrap();
        let path = std::env::temp_dir().join(format!("papercut-verified-assets-{nonce:x?}"));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
const PIN: TranslationModelFile = TranslationModelFile {
    path: "model.bin",
    download_url: None,
    bytes: 3,
    sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
};

#[test]
fn snapshots_and_streaming_verification_require_exact_hash_size_and_regular_files() {
    let dir = Directory::new();
    let path = dir.0.join(PIN.path);
    assert_eq!(read(&dir.0, &PIN, 3), Err(AssetError::Missing));
    fs::write(&path, b"abc").unwrap();
    let snapshot = read(&dir.0, &PIN, 3).unwrap();
    assert_eq!(snapshot, b"abc");
    assert_eq!(verify(&dir.0, &PIN, 3), Ok(()));
    for content in [b"abd".as_slice(), b"ab", b"abcd"] {
        fs::write(&path, content).unwrap();
        assert!(read(&dir.0, &PIN, 3).is_err());
        assert!(verify(&dir.0, &PIN, 3).is_err());
    }
    // A successful tokenizer snapshot is immutable even when the path is later replaced.
    assert_eq!(snapshot, b"abc");
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert_eq!(read(&dir.0, &PIN, 3), Err(AssetError::Path));
}

#[test]
fn manifest_names_and_size_caps_reject_escapes_and_overflow_before_reading() {
    let dir = Directory::new();
    for name in [
        "../model.bin",
        "sub/model.bin",
        "sub\\model.bin",
        "/model.bin",
        ".",
        "",
    ] {
        let pin = TranslationModelFile { path: name, ..PIN };
        assert_eq!(read(&dir.0, &pin, 3), Err(AssetError::Path));
    }
    assert_eq!(read(&dir.0, &PIN, 2), Err(AssetError::Size));
    for bytes in [0, u64::MAX] {
        let pin = TranslationModelFile { bytes, ..PIN };
        assert_eq!(verify(&dir.0, &pin, u64::MAX), Err(AssetError::Size));
    }
}

#[cfg(unix)]
#[test]
fn root_and_leaf_symlinks_cannot_read_outside_files() {
    use std::os::unix::fs::symlink;
    let dir = Directory::new();
    let outside = Directory::new();
    fs::write(outside.0.join(PIN.path), b"abc").unwrap();
    symlink(outside.0.join(PIN.path), dir.0.join(PIN.path)).unwrap();
    assert_eq!(read(&dir.0, &PIN, 3), Err(AssetError::Path));
    let root_link = dir.0.join("link");
    symlink(&outside.0, &root_link).unwrap();
    assert_eq!(verify(&root_link, &PIN, 3), Err(AssetError::Path));
}
