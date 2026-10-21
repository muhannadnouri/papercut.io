//! Generic tiny packs exercise real filesystem replacement; no model or private document fixtures.
use super::*;

const FILES: &[TranslationModelFile] = &[
    TranslationModelFile {
        path: "model.bin",
        download_url: None,
        bytes: 3,
        sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
    },
    TranslationModelFile {
        path: "LICENSE.txt",
        download_url: None,
        bytes: 3,
        sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
    },
];
const PACK: TranslationModelManifest = TranslationModelManifest {
    model_id: "pack",
    directory_name: "pack",
    source_label: "fixture",
    source_url: "",
    revision: "fixture",
    files: FILES,
    installable: true,
};

fn fixture() -> (WorkDirGuard, Paths) {
    let mut nonce = [0; 16];
    getrandom::fill(&mut nonce).unwrap();
    let root = std::env::temp_dir().join(format!("papercut-install-{nonce:x?}"));
    fs::create_dir(&root).unwrap();
    let paths = Paths::new(root.join("pack")).unwrap();
    (WorkDirGuard::new(root), paths)
}

fn pack(dir: &Path, contents: &[u8]) {
    fs::create_dir_all(dir).unwrap();
    for pin in FILES {
        fs::write(dir.join(pin.path), contents).unwrap();
    }
}

fn fetch(pin: &TranslationModelFile, target: &Path, total: &mut u64) -> Result<(), String> {
    assert!(FILES.iter().any(|p| p.path == pin.path));
    fs::write(target, b"abc").map_err(|e| e.to_string())?;
    *total += 3;
    Ok(())
}

fn install_ok(paths: &Paths) -> u64 {
    install(
        paths,
        PACK,
        fetch,
        |parent, bytes| {
            assert_eq!(parent, paths.final_dir.parent().unwrap());
            assert_eq!(bytes, 6);
            crate::library_transfer::ensure_available_space(parent, bytes)
                .map_err(|e| e.to_string())
        },
        |a, b| fs::rename(a, b),
    )
    .unwrap()
}

#[test]
fn install_repair_status_and_remove_preserve_neighbors_and_check_every_file() {
    let (_root, paths) = fixture();
    let neighbor = paths.final_dir.with_file_name("other-model");
    pack(&neighbor, b"old");
    let legacy = paths.final_dir.with_file_name("legacy-cache");
    pack(&legacy, b"old");
    assert_eq!(clear_legacy(&legacy).unwrap(), (true, 6));
    assert_eq!(clear_legacy(&legacy).unwrap(), (false, 0));
    assert_eq!(install_ok(&paths), 6);
    assert!(PACK.has_required_files(&paths.final_dir));
    assert!(PACK.verify_files(&paths.final_dir).is_ok());
    // Same-sized corruption must not take install's early-success path.
    for name in ["model.bin", "LICENSE.txt"] {
        fs::write(paths.final_dir.join(name), b"abd").unwrap();
        assert!(PACK.verify_files(&paths.final_dir).is_err());
        assert_eq!(install_ok(&paths), 6);
        assert!(PACK.verify_files(&paths.final_dir).is_ok());
        fs::remove_file(paths.final_dir.join(name)).unwrap();
        assert!(!PACK.has_required_files(&paths.final_dir));
        assert_eq!(install_ok(&paths), 6);
    }
    pack(&paths.staging, b"abc");
    pack(&paths.previous, b"old");
    assert_eq!(paths.storage().unwrap(), (true, 18));
    assert_eq!(paths.remove().unwrap(), (true, 18));
    assert_eq!(paths.storage().unwrap(), (false, 0));
    assert_eq!(paths.remove().unwrap(), (false, 0));
    assert_eq!(fs::read(neighbor.join("model.bin")).unwrap(), b"old");
}

#[test]
fn verified_pack_skips_network_and_space_checks_without_skipping_recovery() {
    let (_root, paths) = fixture();
    pack(&paths.final_dir, b"abc");
    pack(&paths.staging, b"bad");
    pack(&paths.previous, b"old");
    assert_eq!(
        install(
            &paths,
            PACK,
            |_, _, _| panic!("verified pack redownloaded"),
            |_, _| panic!("verified pack needs no additional space"),
            |_, _| panic!("verified pack renamed")
        )
        .unwrap(),
        6
    );
    assert!(!paths.staging.exists());
    assert!(!paths.previous.exists());
}

#[test]
fn rejected_space_download_and_verification_leave_old_pack_untouched() {
    let (_root, paths) = fixture();
    pack(&paths.final_dir, b"old");
    let result = install(
        &paths,
        PACK,
        |_, _, _| panic!("must preflight before downloads"),
        |_, _| Err("Insufficient space".into()),
        |a, b| fs::rename(a, b),
    );
    assert_eq!(result, Err("Insufficient space".into()));
    assert!(!paths.staging.exists());
    for mode in 0..3 {
        assert!(install(
            &paths,
            PACK,
            |pin, target, total| {
                fetch(pin, target, total)?;
                match mode {
                    0 => return Err("connection interrupted".into()),
                    1 => fs::write(target, b"abd").unwrap(),
                    _ => fs::write(target, b"ab").unwrap(),
                }
                Ok(())
            },
            |_, _| Ok(()),
            |_, _| panic!("unverified pack cannot promote")
        )
        .is_err());
        assert!(!paths.staging.exists());
        assert!(!paths.previous.exists());
        assert_eq!(fs::read(paths.final_dir.join("model.bin")).unwrap(), b"old");
    }
}

#[test]
fn rename_failures_rollback_or_keep_the_backup_for_retry() {
    for failures in [&[1][..], &[2][..], &[2, 3][..]] {
        let (_root, paths) = fixture();
        pack(&paths.final_dir, b"old");
        let mut calls = 0;
        assert!(install(
            &paths,
            PACK,
            fetch,
            |_, _| Ok(()),
            |a, b| {
                calls += 1;
                if failures.contains(&calls) {
                    return Err(io::Error::other("injected rename failure"));
                }
                fs::rename(a, b)
            }
        )
        .is_err());
        let retained = if paths.final_dir.exists() {
            &paths.final_dir
        } else {
            &paths.previous
        };
        assert_eq!(fs::read(retained.join("model.bin")).unwrap(), b"old");
        assert!(!paths.staging.exists());
        assert_eq!(install_ok(&paths), 6);
        assert!(!paths.previous.exists());
    }
    let (_root, paths) = fixture();
    assert!(install(
        &paths,
        PACK,
        fetch,
        |_, _| Ok(()),
        |_, _| Err(io::Error::other("first install rename failed"))
    )
    .is_err());
    assert!(!paths.final_dir.exists());
    assert!(!paths.previous.exists());
    assert!(!paths.staging.exists());
    assert_eq!(install_ok(&paths), 6);
}

#[test]
fn process_interruption_boundaries_recover_before_early_success() {
    // Before old rename; after old rename; after new rename; corrupt new final; old revision differs.
    for (final_bytes, previous_bytes) in [
        (Some(b"abc"), None),
        (None, Some(b"abc")),
        (Some(b"abc"), Some(b"old")),
        (Some(b"bad"), Some(b"abc")),
        (None, Some(b"old")),
    ] {
        let (_root, paths) = fixture();
        if let Some(b) = final_bytes {
            pack(&paths.final_dir, b);
        }
        if let Some(b) = previous_bytes {
            pack(&paths.previous, b);
        }
        pack(&paths.staging, b"bad");
        assert_eq!(install_ok(&paths), 6);
        assert!(PACK.verify_files(&paths.final_dir).is_ok());
        assert!(!paths.previous.exists());
        assert!(!paths.staging.exists());
    }
    // An older backup need not match the new manifest: failed repair must still retain it.
    for new in [None, Some(b"bad")] {
        let (_root, paths) = fixture();
        pack(&paths.previous, b"old");
        if let Some(bytes) = new {
            pack(&paths.final_dir, bytes);
        }
        assert!(install(
            &paths,
            PACK,
            |_, _, _| Err("retry is offline".into()),
            |_, _| Ok(()),
            |a, b| fs::rename(a, b)
        )
        .is_err());
        assert_eq!(fs::read(paths.final_dir.join("model.bin")).unwrap(), b"old");
        assert!(!paths.staging.exists());
        assert!(!paths.previous.exists());
    }
}

#[test]
fn invalid_inventory_rejects_before_io_and_size_overflow_cannot_wrap() {
    let (_root, paths) = fixture();
    let huge = [
        TranslationModelFile {
            bytes: u64::MAX - 1,
            ..FILES[0]
        },
        FILES[1],
    ];
    let duplicate = [FILES[0], FILES[0]];
    let escape = [TranslationModelFile {
        path: "../outside",
        ..FILES[0]
    }];
    let zero = [TranslationModelFile {
        bytes: 0,
        ..FILES[0]
    }];
    let bad_hash = [TranslationModelFile {
        sha256: "invalid",
        ..FILES[0]
    }];
    for files in [
        &huge[..],
        &duplicate[..],
        &escape[..],
        &zero[..],
        &bad_hash[..],
        &[],
    ] {
        // The real catalog is static; leaked tiny test pins keep the same manifest boundary.
        let files = Box::leak(files.to_vec().into_boxed_slice());
        assert!(install(
            &paths,
            TranslationModelManifest { files, ..PACK },
            |_, _, _| panic!("invalid inventory downloaded"),
            |_, _| panic!("invalid inventory preflight"),
            |a, b| fs::rename(a, b)
        )
        .is_err());
    }
    for directory_name in ["../pack", "pack/", "C:pack"] {
        assert!(install(
            &paths,
            TranslationModelManifest {
                directory_name,
                ..PACK
            },
            fetch,
            |_, _| panic!("invalid destination preflight"),
            |a, b| fs::rename(a, b)
        )
        .is_err());
    }
    let other = Paths::new(paths.final_dir.with_file_name("other")).unwrap();
    assert!(install(
        &other,
        PACK,
        fetch,
        |_, _| panic!("mismatched destination preflight"),
        |a, b| fs::rename(a, b)
    )
    .is_err());
    assert!(!paths.final_dir.exists());
}

#[cfg(unix)]
#[test]
fn symlinked_files_and_owned_roots_never_follow_or_delete_outside_data() {
    use std::os::unix::fs::symlink;
    let (_root, paths) = fixture();
    let outside = paths.final_dir.with_file_name("outside");
    pack(&outside, b"abc");
    for root in [&paths.final_dir, &paths.staging, &paths.previous] {
        symlink(&outside, root).unwrap();
        assert!(install(&paths, PACK, fetch, |_, _| Ok(()), |a, b| fs::rename(a, b)).is_err());
        assert!(paths.remove().is_err());
        assert!(paths.storage().is_err());
        assert!(storage(root).is_err());
        assert!(clear_legacy(root).is_err());
        fs::remove_file(root).unwrap();
        assert_eq!(fs::read(outside.join("model.bin")).unwrap(), b"abc");
    }
    pack(&paths.final_dir, b"abc");
    fs::remove_file(paths.final_dir.join("LICENSE.txt")).unwrap();
    symlink(
        outside.join("LICENSE.txt"),
        paths.final_dir.join("LICENSE.txt"),
    )
    .unwrap();
    assert!(!PACK.has_required_files(&paths.final_dir));
    assert!(PACK.verify_files(&paths.final_dir).is_err());
    assert!(directory_size(&paths.final_dir).is_err());
    assert_eq!(install_ok(&paths), 6);
    assert_eq!(fs::read(outside.join("LICENSE.txt")).unwrap(), b"abc");
}

#[test]
fn auxiliary_notices_are_bundled_verified_repaired_and_accounted_without_network_delivery() {
    use crate::translation::model_store::manifest_by_id;
    let approved = manifest_by_id("alignment-awesome-align-int8").unwrap();
    // Keep the real identity/legal pins; substitute only the large model with a tiny synthetic graph.
    let files = Box::leak(
        vec![
            TranslationModelFile {
                path: "model.onnx",
                ..FILES[0]
            },
            approved.files[7],
            approved.files[8],
        ]
        .into_boxed_slice(),
    );
    let manifest = TranslationModelManifest { files, ..approved };
    let (root, _) = fixture();
    let paths = Paths::new(root.0.join(manifest.directory_name)).unwrap();
    let run = || {
        let mut fetched = 0;
        let result = install(
            &paths,
            manifest,
            |pin, p, total| {
                assert_eq!(
                    pin.path, "model.onnx",
                    "notices must not reach network delivery"
                );
                fetched += 1;
                fs::write(p, b"abc").unwrap();
                *total += 3;
                Ok(())
            },
            |_, n| {
                assert_eq!(n, 13_596);
                Ok(())
            },
            |a, b| fs::rename(a, b),
        )
        .unwrap();
        assert_eq!(result, manifest.total_bytes());
        fetched
    };
    assert_eq!(run(), 1);
    assert!(manifest.is_installed(&paths.final_dir));
    assert_eq!(run(), 0);
    fs::write(paths.final_dir.join("model.onnx"), b"abd").unwrap();
    assert!(!manifest.is_installed(&paths.final_dir));
    assert_eq!(run(), 1);
    for name in ["LICENSE-APACHE-2.0.txt", "ATTRIBUTION.md"] {
        let path = paths.final_dir.join(name);
        assert_eq!(
            fs::read(&path).unwrap(),
            manifest.bundled_file(name).unwrap()
        );
        let mut changed = fs::read(&path).unwrap();
        changed[0] ^= 1;
        fs::write(&path, changed).unwrap();
        assert!(
            !manifest.is_installed(&paths.final_dir),
            "same-sized notice tampering must fail status"
        );
        assert_eq!(run(), 1);
        assert!(manifest.is_installed(&paths.final_dir));
        fs::remove_file(&path).unwrap();
        assert!(!manifest.is_installed(&paths.final_dir));
        assert_eq!(run(), 1);
    }
    // Compiled bytes must match inventory pins before even preflight or graph downloads.
    for pin in [
        TranslationModelFile {
            bytes: 2234,
            ..approved.files[8]
        },
        TranslationModelFile {
            sha256: FILES[0].sha256,
            ..approved.files[8]
        },
    ] {
        let bad = Box::leak(vec![pin].into_boxed_slice());
        assert!(install(
            &paths,
            TranslationModelManifest {
                files: bad,
                ..approved
            },
            |_, _, _| panic!("bad notice downloaded"),
            |_, _| panic!("bad notice preflight"),
            |a, b| fs::rename(a, b)
        )
        .is_err());
    }
    assert!(manifest.is_installed(&paths.final_dir));
    assert_eq!(paths.remove().unwrap(), (true, manifest.total_bytes()));
    assert!(!manifest.is_installed(&paths.final_dir));
}
