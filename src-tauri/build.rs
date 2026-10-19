fn main() {
    verify_bundled_preprocessor();
    generate_model_downloads();
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let native_tts_shared = std::env::var_os("CARGO_FEATURE_NATIVE_TTS_SHARED").is_some();

    if target_os == "linux" && native_tts_shared {
        // Debian/RPM installs place Tauri resources in /usr/lib/<productName>.
        // The app binary is /usr/bin/app and resources install to /usr/lib/Papercut, so this rpath lets the dynamic loader
        // find bundled sherpa-onnx shared libraries without requiring users to
        // edit LD_LIBRARY_PATH. The same relative layout is used in AppImage.
        println!("cargo:rustc-link-arg-bin=app=-Wl,-rpath,$ORIGIN/../lib/Papercut");
    }

    if target_os == "macos" && native_tts_shared {
        // Tauri places bundled resources in Papercut.app/Contents/Resources while
        // the app binary lives in Contents/MacOS. sherpa-onnx-sys emits @loader_path
        // for dev runs (dylibs are copied next to the binary during cargo build);
        // this additional rpath lets the installed .app locate the dylibs bundled
        // as resources without requiring users to set DYLD_LIBRARY_PATH.
        println!("cargo:rustc-link-arg-bin=app=-Wl,-rpath,@loader_path/../Resources");
    }

    let windows_msvc =
        target_os == "windows" && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    let mut attributes = tauri_build::Attributes::new();
    if windows_msvc {
        attributes = attributes
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
    }
    tauri_build::try_build(attributes).expect("Failed to build Tauri resources");

    if windows_msvc {
        // Tauri's resource manifest reaches app binaries only; library tests also import Common Controls v6.
        let manifest = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
            .join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }
}

/// Pin the unchanged upstream asset before it is compiled into native builds.
fn verify_bundled_preprocessor() {
    use sha2::{Digest, Sha256};
    let path = "tts/libtashkeel-1.5.0/model.onnx";
    println!("cargo:rerun-if-changed={path}");
    if std::env::var_os("CARGO_FEATURE_NATIVE_TEXT_PREPROCESSING_CORE").is_none() {
        return;
    }
    let bytes = std::fs::read(path).expect("Missing bundled Libtashkeel 1.5.0 model");
    assert_eq!(
        bytes.len(),
        4_788_213,
        "Incorrect bundled Libtashkeel model size"
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "52c40d42ca094ff9ff07a091d0a1eafd7baa5cf2388e60a9cc89bacd8e5e955f",
        "Incorrect bundled Libtashkeel model checksum"
    );
}

/// Compile the same pinned artifact metadata used by the download monitor.
fn generate_model_downloads() {
    use std::{collections::BTreeMap, fs, path::PathBuf};

    let path = "tts/model-manifest.json";
    println!("cargo:rerun-if-changed={path}");
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(manifest["schemaVersion"], 2);
    let mut downloads = BTreeMap::new();
    for model in manifest["archives"].as_array().unwrap() {
        let name = model["modelName"].as_str().unwrap();
        assert!(name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c)));
        let url = model["url"].as_str().unwrap();
        let hash = model["sha256"].as_str().unwrap();
        let bytes = model["archiveBytes"].as_u64().unwrap();
        let fallback = model["fallbackUrl"].as_str();
        assert!(url.starts_with("https://") && fallback.is_none_or(|u| u.starts_with("https://")));
        assert!(hash.len() == 64 && hash.bytes().all(|c| c.is_ascii_hexdigit()) && bytes > 0);
        let constant = name.replace('-', "_").to_ascii_uppercase();
        let definition = format!(
            "const {constant}: ModelDownload = ModelDownload {{ url: {url:?}, sha256: {hash:?}, bytes: {bytes}, fallback_url: {fallback:?} }};\n"
        );
        assert!(
            downloads.insert(constant, definition).is_none(),
            "Duplicate archive constant: {name}"
        );
    }
    let output = PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("model_downloads.rs");
    fs::write(output, downloads.into_values().collect::<String>()).unwrap();
}
