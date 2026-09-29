fn main() {
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

    tauri_build::build()
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
