fn main() {
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
