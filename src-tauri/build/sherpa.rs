//! Published Sherpa bindings stay unchanged; the application owns platform linking and preparation.

use std::{env, path::PathBuf};

use super::native;

/// Empty Cargo links overrides suppress dependency build.rs; app features choose native metadata here.
pub(super) fn prepare() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=build/sherpa.rs");
    println!("cargo:rerun-if-env-changed=SHERPA_ONNX_LIB_DIR");
    println!("cargo:rerun-if-env-changed=SHERPA_ONNX_ARCHIVE_DIR");
    println!("cargo:rerun-if-changed=tts/native-artifacts.json");
    if env::var_os("CARGO_FEATURE_NATIVE_TTS_CORE").is_none() {
        return Ok(());
    }
    let os = env::var("CARGO_CFG_TARGET_OS")?;
    let static_link = os == "ios" || env::var_os("CARGO_FEATURE_NATIVE_TTS_STATIC").is_some();
    if static_link && env::var_os("CARGO_FEATURE_NATIVE_TTS_SHARED").is_some() {
        return Err("Conflicting native TTS modes".into());
    }
    // A restored directory alone is not a usable input set; reject before emitting link metadata.
    let names: &[&str] = if static_link && os == "ios" {
        &["sherpa-onnx", "onnxruntime"]
    } else if static_link {
        &[
            "sherpa-onnx-c-api",
            "sherpa-onnx-core",
            "kaldi-decoder-core",
            "sherpa-onnx-kaldifst-core",
            "sherpa-onnx-fstfar",
            "sherpa-onnx-fst",
            "kaldi-native-fbank-core",
            "kissfft-float",
            "onnxruntime",
            "ssentencepiece_core",
            "utf8proc",
        ]
    } else {
        &["sherpa-onnx-c-api", "onnxruntime"]
    };
    let prefix = if os == "windows" { "" } else { "lib" };
    let suffix = if os == "windows" {
        "lib"
    } else if static_link {
        "a"
    } else if os == "macos" {
        "dylib"
    } else {
        "so"
    };
    let mut required: Vec<String> = names
        .iter()
        .map(|name| format!("{prefix}{name}.{suffix}"))
        .collect();
    if os == "windows" && !static_link {
        required.extend(names.iter().map(|name| format!("{name}.dll")));
    }
    let out = PathBuf::from(env::var_os("OUT_DIR").ok_or("Missing Cargo OUT_DIR")?);
    let dir = if let Some(path) = env::var_os("SHERPA_ONNX_LIB_DIR") {
        PathBuf::from(path)
    } else {
        if matches!(os.as_str(), "android" | "ios") {
            return Err(
                "Prepare mobile source/ABI-verified libraries using the platform wrapper first"
                    .into(),
            );
        }
        let root = native::target_dir(&out, &env::var("TARGET")?)?.join("sherpa-onnx-prebuilt");
        native::prepared_archive(
            &root,
            &archive_name(&os, &env::var("CARGO_CFG_TARGET_ARCH")?, static_link)?,
            include_str!("../tts/native-artifacts.json"),
            &required,
        )?
    };
    native::require_libraries(&dir, &required)?;
    let dir = env::current_dir()?.join(dir);
    println!("cargo:rustc-link-search=native={}", dir.display());
    let kind = if static_link { "static" } else { "dylib" };
    for lib in names {
        println!("cargo:rustc-link-lib={kind}={lib}");
    }
    if !static_link && (os == "linux" || os == "macos") {
        // ORT's Rust loader prefers an adjacent file; C API must select that same copy before cache fallbacks.
        println!(
            "cargo:rustc-link-arg=-Wl,-rpath,{}",
            if os == "linux" {
                "$ORIGIN"
            } else {
                "@loader_path"
            }
        );
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", dir.display());
    }
    if static_link {
        match os.as_str() {
            "linux" => {
                for lib in ["stdc++", "m", "pthread", "dl"] {
                    println!("cargo:rustc-link-lib=dylib={lib}");
                }
            }
            "macos" | "ios" => {
                println!("cargo:rustc-link-lib=dylib=c++");
                println!("cargo:rustc-link-lib=framework=Foundation");
            }
            _ => {}
        }
    }
    if !static_link && os != "android" {
        native::stage_runtime(&dir, &os, &out)?;
    }
    Ok(())
}

/// Release archive names differ by platform; use reviewed pins rather than deriving the version from the app.
fn archive_name(os: &str, arch: &str, static_link: bool) -> native::Result<String> {
    let arch = match (os, arch) {
        ("linux", "aarch64") => "aarch64",
        ("macos", "aarch64") => "arm64",
        ("linux" | "macos" | "windows", "x86_64") => "x64",
        _ => return Err("Unsupported Sherpa desktop target".into()),
    };
    let platform = if os == "macos" {
        "osx"
    } else if os == "windows" {
        "win"
    } else {
        os
    };
    let mode = if static_link { "static" } else { "shared" };
    let extra = if os == "windows" {
        "-MT-Release"
    } else if os == "linux" && arch == "aarch64" && !static_link {
        "-cpu"
    } else {
        ""
    };
    Ok(format!(
        "sherpa-onnx-v2.0.1-{platform}-{arch}-{mode}{extra}-lib.tar.bz2"
    ))
}
