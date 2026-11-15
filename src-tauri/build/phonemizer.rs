//! Piper uses the unchanged public C API; preparation is shared with the app-owned Sherpa adapter.

use std::env;
use std::path::PathBuf;

use super::native;

/// Mobile recipes supply source-built libraries; desktop uses checksum-pinned official artifacts.
pub(super) fn prepare() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=build/phonemizer.rs");
    println!("cargo:rerun-if-env-changed=PIPER_PHONEMIZE_LIB_DIR");
    println!("cargo:rerun-if-env-changed=SHERPA_ONNX_LIB_DIR");
    println!("cargo:rerun-if-changed=tts/phonemizer-artifacts.json");
    if env::var_os("CARGO_FEATURE_NATIVE_TTS_CORE").is_none() {
        return Ok(());
    }
    let os = env::var("CARGO_CFG_TARGET_OS")?;
    let mode = if os == "ios" || env::var_os("CARGO_FEATURE_NATIVE_TTS_STATIC").is_some() {
        "static"
    } else {
        "shared"
    };
    let lib = match env::var_os("PIPER_PHONEMIZE_LIB_DIR") {
        Some(path) => PathBuf::from(path),
        None if matches!(os.as_str(), "android" | "ios") => PathBuf::from(
            env::var_os("SHERPA_ONNX_LIB_DIR")
                .ok_or("Prepare mobile Piper/Sherpa libraries first")?,
        ),
        None => download(&os, &env::var("CARGO_CFG_TARGET_ARCH")?, mode)?,
    };
    native::require_libraries(&lib, &library_files(&os, mode))?;
    let lib = env::current_dir()?.join(lib);
    println!("cargo:rustc-link-search=native={}", lib.display());
    if os == "ios" {
        println!("cargo:rustc-link-lib=static=piper_phonemize");
    } else if mode == "static" {
        for name in ["piper_phonemize_core", "espeak-ng", "ucd"] {
            println!("cargo:rustc-link-lib=static={name}");
        }
    } else {
        println!("cargo:rustc-link-lib=dylib=piper_phonemize_core");
        if os != "android" {
            if os != "windows" {
                println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib.display());
            }
            native::stage_runtime(
                &lib,
                &os,
                &PathBuf::from(env::var_os("OUT_DIR").ok_or("Missing Cargo OUT_DIR")?),
            )?;
        }
    }
    Ok(())
}

/// Share verification/repair with Sherpa while preserving Piper's separate, versioned cache.
fn download(os: &str, arch: &str, mode: &str) -> native::Result<PathBuf> {
    let arch = match arch {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        _ => return Err("Unsupported Piper target".into()),
    };
    if !matches!(os, "linux" | "macos" | "windows") {
        return Err("Unsupported Piper platform".into());
    }
    let name = format!("piper-phonemize-v1.5.9-{os}-{arch}-{mode}-lib.tar.bz2");
    let out = PathBuf::from(env::var_os("OUT_DIR").ok_or("Missing Cargo OUT_DIR")?);
    let root = native::target_dir(&out, &env::var("TARGET")?)?.join("piper-phonemize-prebuilt");
    native::prepared_archive(
        &root,
        &name,
        include_str!("../tts/phonemizer-artifacts.json"),
        &library_files(os, mode),
    )
}

/// Windows uses unprefixed import libraries; iOS supplies one thin aggregate instead of desktop components.
fn library_files(os: &str, mode: &str) -> Vec<String> {
    let names: &[&str] = if os == "ios" {
        &["piper_phonemize"]
    } else if mode == "static" {
        &["piper_phonemize_core", "espeak-ng", "ucd"]
    } else {
        &["piper_phonemize_core"]
    };
    let prefix = if os == "windows" { "" } else { "lib" };
    let suffix = if os == "windows" {
        "lib"
    } else if mode == "static" {
        "a"
    } else if os == "macos" {
        "dylib"
    } else {
        "so"
    };
    let mut files: Vec<String> = names
        .iter()
        .map(|name| format!("{prefix}{name}.{suffix}"))
        .collect();
    if os == "windows" && mode == "shared" {
        files.push("piper_phonemize_core.dll".into());
    }
    files
}
