//! Shared, pinned native-artifact preparation; no upstream source is patched.

use std::{
    collections::BTreeMap,
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use bzip2::read::BzDecoder;
use sha2::{Digest, Sha256};

use super::runtime_copy::copy_file_atomically;

pub(super) type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// OUT_DIR reflects Cargo's resolved target directory, including config/CLI overrides and cross builds.
pub(super) fn target_dir(out: &Path, target: &str) -> Result<PathBuf> {
    let parent = out.ancestors().nth(4).ok_or("Invalid Cargo OUT_DIR")?;
    Ok(if parent.file_name().is_some_and(|name| name == target) {
        parent.parent().ok_or("Missing Cargo target root")?
    } else {
        parent
    }
    .to_path_buf())
}

/// Watching inputs, rather than staged outputs, makes repaired native caches relink without rebuild loops.
/// A repair writes inputs during a build, so Cargo can need one subsequent settling build.
pub(super) fn require_libraries(dir: &Path, names: &[String]) -> Result<()> {
    for name in names {
        let path = dir.join(name);
        println!("cargo:rerun-if-changed={}", path.display());
        if !fs::metadata(&path).is_ok_and(|m| m.is_file() && m.len() > 0) {
            return Err(
                format!("Missing/empty prepared native library: {}", path.display()).into(),
            );
        }
    }
    Ok(())
}

/// An archive-bound receipt rejects stale pins, partial extractions and changed/extra library files.
fn verified(dir: &Path, receipt: &Path, hash: &str) -> bool {
    let Ok(bytes) = fs::read(receipt) else {
        return false;
    };
    let Ok(saved) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        return false;
    };
    saved["archiveSha256"] == hash
        && file_hashes(dir).is_ok_and(|files| {
            !files.is_empty()
                && serde_json::to_value(files).is_ok_and(|value| saved["files"] == value)
        })
}

/// Record every loadable/archive byte, including dereferenced library aliases and Windows import libraries.
fn file_hashes(dir: &Path) -> Result<BTreeMap<String, String>> {
    let mut files = BTreeMap::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.path().is_file() {
            let bytes = fs::read(entry.path())?;
            if bytes.is_empty() {
                return Err("Empty native library".into());
            }
            files.insert(
                entry
                    .file_name()
                    .to_str()
                    .ok_or("Invalid native filename")?
                    .to_owned(),
                format!("{:x}", Sha256::digest(bytes)),
            );
        }
    }
    Ok(files)
}

/// Full dotted archive names preserve the existing bundle-cache layout and prevent version/mode collisions.
/// with_extension would treat part of the dotted version as an extension; trim only the archive suffix.
pub(super) fn prepared_archive(
    root: &Path,
    name: &str,
    pins: &str,
    required: &[String],
) -> Result<PathBuf> {
    let pins: serde_json::Value = serde_json::from_str(pins)?;
    let pin = &pins[name];
    let hash = pin["sha256"]
        .as_str()
        .ok_or("Native archive has no reviewed SHA-256 pin")?;
    let size = pin["bytes"].as_u64().ok_or("Missing native archive size")?;
    let url = pin["url"]
        .as_str()
        .filter(|url| url.starts_with("https://"))
        .ok_or("Missing HTTPS native archive URL")?;
    let archive = root.join(name);
    let extracted = root.join(name.trim_end_matches(".tar.bz2"));
    let lib = extracted.join("lib");
    let receipt = extracted.join(".verified.json");
    if !verified(&lib, &receipt, hash) {
        fs::create_dir_all(root)?;
        if !archive.is_file() {
            let temporary = archive.with_file_name(format!("{name}.{}.part", std::process::id()));
            let fetch = (|| -> Result<()> {
                if let Some(local) = env::var_os("SHERPA_ONNX_ARCHIVE_DIR")
                    .map(PathBuf::from)
                    .filter(|p| p.join(name).is_file())
                {
                    fs::copy(local.join(name), &temporary)?;
                } else {
                    let response = ureq::builder()
                        .try_proxy_from_env(true)
                        .build()
                        .get(url)
                        .call()?;
                    std::io::copy(
                        &mut response.into_reader(),
                        &mut fs::File::create(&temporary)?,
                    )?;
                }
                fs::rename(&temporary, &archive)?;
                Ok(())
            })();
            if fetch.is_err() {
                let _ = fs::remove_file(&temporary);
            }
            fetch?;
        }
        let bytes = fs::read(&archive)?;
        if bytes.len() as u64 != size || format!("{:x}", Sha256::digest(&bytes)) != hash {
            fs::remove_file(&archive)?;
            return Err(format!(
                "Native archive checksum/size mismatch: {name}; retry to fetch again"
            )
            .into());
        }
        if extracted.exists() {
            fs::remove_dir_all(&extracted)?;
        }
        // Piper archives start at lib/; Sherpa includes its versioned directory. tar rejects escaping paths.
        let unpack = if name.starts_with("piper-phonemize-") {
            &extracted
        } else {
            root
        };
        fs::create_dir_all(unpack)?;
        tar::Archive::new(BzDecoder::new(bytes.as_slice())).unpack(unpack)?;
        if extracted.join("bin").is_dir() {
            for entry in fs::read_dir(extracted.join("bin"))? {
                let entry = entry?;
                if entry.path().extension().is_some_and(|e| e == "dll") {
                    copy_file_atomically(&entry.path(), &lib.join(entry.file_name()))?;
                }
            }
        }
        require_libraries(&lib, required)?;
        let files = file_hashes(&lib)?;
        fs::write(
            &receipt,
            serde_json::to_vec(&serde_json::json!({"archiveSha256": hash, "files": files}))?,
        )?;
    }
    println!("cargo:rerun-if-changed={}", receipt.display());
    // Watch all receipt inputs: a changed optional runtime must also invalidate a warm build.
    for name in file_hashes(&lib)?.keys() {
        println!("cargo:rerun-if-changed={}", lib.join(name).display());
    }
    require_libraries(&lib, required)?;
    Ok(lib)
}

/// Unix replacement keeps mapped inodes alive; only Windows tests need DLLs adjacent in deps.
pub(super) fn stage_runtime(dir: &Path, os: &str, out: &Path) -> Result<()> {
    let profile = out.ancestors().nth(3).ok_or("Invalid Cargo OUT_DIR")?;
    let mut destinations = vec![profile.to_path_buf(), profile.join("examples")];
    if os == "windows" {
        destinations.push(profile.join("deps"));
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let text = name.to_string_lossy();
        // Papercut consumes Piper's C API only; its unused C++ wrapper was never bundled.
        if text.contains("piper_phonemize_cxx_api") {
            continue;
        }
        let runtime = match os {
            "linux" => text.contains(".so"),
            "macos" => text.ends_with(".dylib"),
            "windows" => text.ends_with(".dll"),
            _ => false,
        };
        if !runtime {
            continue;
        }
        println!("cargo:rerun-if-changed={}", entry.path().display());
        let source = entry.path().canonicalize()?;
        let mut names = vec![name];
        // Preserve SONAME siblings even when a library alias resolves outside lib/.
        if let Some(actual) = source
            .file_name()
            .filter(|actual| *actual != names[0].as_os_str())
        {
            names.push(actual.to_owned());
        }
        for destination in &destinations {
            fs::create_dir_all(destination)?;
            for name in &names {
                copy_file_atomically(&source, &destination.join(name))?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new(name: &str) -> Self {
            let root = env::temp_dir().join(format!(
                "papercut-{name}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(&root).unwrap();
            Self(root)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// Local archive bytes keep repair/pin regressions deterministic and offline for both upstream layouts.
    fn archive(root: &Path, name: &str) -> String {
        let encoder = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::fast());
        let mut archive = tar::Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        header.set_size(7);
        header.set_mode(0o644);
        header.set_cksum();
        let prefix = if name.starts_with("piper-") {
            String::new()
        } else {
            format!("{}/", name.trim_end_matches(".tar.bz2"))
        };
        archive
            .append_data(
                &mut header,
                format!("{prefix}lib/libtest.so"),
                &b"library"[..],
            )
            .unwrap();
        let bytes = archive.into_inner().unwrap().finish().unwrap();
        fs::File::create(root.join(name))
            .unwrap()
            .write_all(&bytes)
            .unwrap();
        serde_json::json!({name: {"sha256": format!("{:x}", Sha256::digest(&bytes)), "bytes": bytes.len(), "url": "https://invalid.example/test"}}).to_string()
    }

    #[test]
    fn cache_repairs_changed_empty_missing_receipts_and_stale_pins_for_both_layouts() {
        let fixture = Fixture::new("native-cache");
        for name in [
            "piper-phonemize-v1.5.9-test.tar.bz2",
            "sherpa-onnx-v2.0.1-test.tar.bz2",
        ] {
            let pins = archive(&fixture.0, name);
            let required = ["libtest.so".to_string()];
            let lib = prepared_archive(&fixture.0, name, &pins, &required).unwrap();
            let receipt = lib.parent().unwrap().join(".verified.json");
            let hash: serde_json::Value = serde_json::from_str(&pins).unwrap();
            let hash = hash[name]["sha256"].as_str().unwrap();
            assert!(verified(&lib, &receipt, hash));
            for contents in [b"changed".as_slice(), b""] {
                fs::write(lib.join("libtest.so"), contents).unwrap();
                prepared_archive(&fixture.0, name, &pins, &required).unwrap();
                assert_eq!(fs::read(lib.join("libtest.so")).unwrap(), b"library");
            }
            fs::remove_file(&receipt).unwrap();
            prepared_archive(&fixture.0, name, &pins, &required).unwrap();
            let mut saved: serde_json::Value =
                serde_json::from_slice(&fs::read(&receipt).unwrap()).unwrap();
            saved["archiveSha256"] = "stale".into();
            fs::write(&receipt, saved.to_string()).unwrap();
            prepared_archive(&fixture.0, name, &pins, &required).unwrap();
            assert!(verified(&lib, &receipt, hash));
            fs::write(lib.join("extra.so"), b"unexpected").unwrap();
            prepared_archive(&fixture.0, name, &pins, &required).unwrap();
            assert!(!lib.join("extra.so").exists());
            // A matching receipt cannot make a missing required link library acceptable.
            assert!(prepared_archive(&fixture.0, name, &pins, &["missing.so".into()]).is_err());
            fs::remove_file(&receipt).unwrap();
            fs::write(fixture.0.join(name), b"corrupt download").unwrap();
            assert!(prepared_archive(&fixture.0, name, &pins, &required).is_err());
            assert!(!fixture.0.join(name).exists());
        }
    }

    #[test]
    fn output_root_handles_custom_profiles_cross_targets_and_spaces() {
        let root = Path::new("custom target with spaces");
        assert_eq!(
            target_dir(&root.join("release/build/app/out"), "triple").unwrap(),
            root
        );
        assert_eq!(
            target_dir(&root.join("triple/debug/build/app/out"), "triple").unwrap(),
            root
        );
        assert!(target_dir(Path::new("out"), "triple").is_err());
    }

    #[test]
    fn unix_staging_avoids_duplicate_ort_in_deps_and_windows_stages_adjacent_dlls() {
        let fixture = Fixture::new("native-staging");
        let source = fixture.0.join("source");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("libonnxruntime.so"), b"runtime").unwrap();
        fs::write(
            source.join("libpiper_phonemize_cxx_api.so"),
            b"unused wrapper",
        )
        .unwrap();
        #[cfg(unix)]
        {
            let versioned = fixture.0.join("libprovider.so.1");
            fs::write(&versioned, b"versioned runtime").unwrap();
            std::os::unix::fs::symlink(versioned, source.join("libprovider.so")).unwrap();
        }
        let out = fixture.0.join("custom/debug/build/app/out");
        stage_runtime(&source, "linux", &out).unwrap();
        let profile = fixture.0.join("custom/debug");
        assert_eq!(
            fs::read(profile.join("examples/libonnxruntime.so")).unwrap(),
            b"runtime"
        );
        assert!(!profile.join("deps/libonnxruntime.so").exists());
        assert!(!profile.join("libpiper_phonemize_cxx_api.so").exists());
        #[cfg(unix)]
        assert_eq!(
            fs::read(profile.join("libprovider.so.1")).unwrap(),
            b"versioned runtime"
        );
        fs::write(source.join("onnxruntime.dll"), b"windows runtime").unwrap();
        stage_runtime(&source, "windows", &out).unwrap();
        assert_eq!(
            fs::read(profile.join("deps/onnxruntime.dll")).unwrap(),
            b"windows runtime"
        );
    }
}
