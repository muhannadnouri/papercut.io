use std::collections::{BTreeMap, HashSet};
use std::env;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use bzip2::read::BzDecoder;
use sha2::{Digest, Sha256};
use tar::Archive;

mod runtime_copy;
use runtime_copy::copy_file_atomically;

const SHERPA_ONNX_STATIC_LIBS: &[&str] = &[
    "sherpa-onnx-c-api",
    "sherpa-onnx-core",
    "kaldi-decoder-core",
    "sherpa-onnx-kaldifst-core",
    "sherpa-onnx-fstfar",
    "sherpa-onnx-fst",
    "kaldi-native-fbank-core",
    "kissfft-float",
    "utf8proc",
    "onnxruntime",
    "ssentencepiece_core",
];

type DynError = Box<dyn Error>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LinkMode {
    Static,
    Shared,
}

fn main() {
    if let Err(err) = try_main() {
        panic!("{err}");
    }
}

fn try_main() -> Result<(), DynError> {
    println!("cargo:rerun-if-env-changed=SHERPA_ONNX_LIB_DIR");
    println!("cargo:rerun-if-env-changed=SHERPA_ONNX_ARCHIVE_DIR");
    println!("cargo:rerun-if-changed=artifacts.json");
    println!("cargo:rerun-if-env-changed=DOCS_RS");

    if env::var_os("DOCS_RS").is_some() {
        // docs.rs sets DOCS_RS=1; skip downloading/linking native libraries
        // so that `cargo doc` can succeed without the real C artifacts.
        return Ok(());
    }

    let target_os = env::var("CARGO_CFG_TARGET_OS")?;
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH")?;
    let link_mode = resolve_link_mode()?;
    let lib_dir = resolve_lib_dir(link_mode, &target_os, &target_arch)?;

    println!("cargo:rustc-link-search=native={}", lib_dir.display());

    if link_mode == LinkMode::Shared && matches!(target_os.as_str(), "linux" | "macos") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());
        emit_relative_rpath(&target_os);
        copy_unix_runtime_libs(&lib_dir, &target_os)?;
    }

    if link_mode == LinkMode::Shared && target_os == "windows" {
        copy_windows_runtime_dlls(&lib_dir)?;
    }

    match link_mode {
        LinkMode::Static if target_os == "ios" => emit_ios_static_link_directives(),
        LinkMode::Static => emit_static_link_directives(&target_os),
        LinkMode::Shared => emit_shared_link_directives(),
    }

    Ok(())
}

/// Explicit static mode must survive on iOS; the app prepares and stages mobile artifacts itself.
fn resolve_link_mode() -> Result<LinkMode, DynError> {
    let static_enabled = env::var_os("CARGO_FEATURE_STATIC").is_some();
    let shared_enabled = env::var_os("CARGO_FEATURE_SHARED").is_some();

    if static_enabled && shared_enabled {
        return Err("Features `static` and `shared` cannot be enabled at the same time".into());
    }

    if shared_enabled {
        Ok(LinkMode::Shared)
    } else {
        Ok(LinkMode::Static)
    }
}

fn resolve_lib_dir(
    link_mode: LinkMode,
    target_os: &str,
    target_arch: &str,
) -> Result<PathBuf, DynError> {
    if let Some(path) = env::var_os("SHERPA_ONNX_LIB_DIR") {
        let path = PathBuf::from(path);
        if !path.is_dir() {
            return Err(format!(
                "SHERPA_ONNX_LIB_DIR does not exist or is not a directory: {}",
                path.display()
            )
            .into());
        }
        return Ok(path);
    }

    download_prebuilt_libs(link_mode, target_os, target_arch)
}

fn download_prebuilt_libs(mode: LinkMode, os: &str, arch: &str) -> Result<PathBuf, DynError> {
    prepared_archive("sherpa-onnx-prebuilt", &archive_name(mode, os, arch)?)
}

/// A verified extraction receipt rejects empty/restored skeleton caches; versioned names isolate old runtimes.
fn prepared_archive(cache: &str, name: &str) -> Result<PathBuf, DynError> {
    let pins: serde_json::Value = serde_json::from_str(include_str!("artifacts.json"))?;
    let pin = &pins[name];
    let hash = pin["sha256"]
        .as_str()
        .ok_or("Native archive has no reviewed SHA-256 pin")?;
    let root = target_dir_from_out_dir(&PathBuf::from(env::var("OUT_DIR")?))?.join(cache);
    let extracted = root.join(name.trim_end_matches(".tar.bz2"));
    let lib = extracted.join("lib");
    let receipt = extracted.join(".verified.json");
    if let Ok(bytes) = fs::read(&receipt) {
        if let Ok(files) = serde_json::from_slice::<BTreeMap<String, String>>(&bytes) {
            if !files.is_empty()
                && files.iter().all(|(file, expected)| {
                    fs::read(lib.join(file))
                        .is_ok_and(|b| format!("{:x}", Sha256::digest(b)) == *expected)
                })
            {
                return Ok(lib);
            }
        }
    }
    fs::create_dir_all(&root)?;
    let archive = root.join(name);
    if !archive.is_file() {
        if let Some(local) = env::var_os("SHERPA_ONNX_ARCHIVE_DIR")
            .map(PathBuf::from)
            .filter(|p| p.join(name).is_file())
        {
            copy_file_atomically(&local.join(name), &archive)?;
        } else {
            let url = pin["url"].as_str().ok_or("Missing native archive URL")?;
            let response = ureq::builder()
                .try_proxy_from_env(true)
                .build()
                .get(url)
                .call()?;
            write_reader_atomically(&mut response.into_reader(), &archive)?;
        }
    }
    let bytes = fs::read(&archive)?;
    if bytes.len() as u64 != pin["bytes"].as_u64().ok_or("Missing native archive size")?
        || format!("{:x}", Sha256::digest(&bytes)) != hash
    {
        fs::remove_file(&archive)?;
        return Err(format!("Native archive checksum/size mismatch: {name}").into());
    }
    if extracted.exists() {
        fs::remove_dir_all(&extracted)?;
    }
    fs::create_dir_all(&extracted)?;
    let mut tar = Archive::new(BzDecoder::new(File::open(&archive)?));
    tar.unpack(&root)?;
    let mut files = BTreeMap::new();
    for file in fs::read_dir(&lib)? {
        let file = file?;
        if file.path().is_file() {
            files.insert(
                file.file_name().to_string_lossy().into_owned(),
                format!("{:x}", Sha256::digest(fs::read(file.path())?)),
            );
        }
    }
    if files.is_empty() {
        return Err("Native archive has no usable libraries".into());
    }
    fs::write(receipt, serde_json::to_vec(&files)?)?;
    Ok(lib)
}

fn archive_name(
    link_mode: LinkMode,
    target_os: &str,
    target_arch: &str,
) -> Result<String, DynError> {
    let version = env!("CARGO_PKG_VERSION");
    let name = match (link_mode, target_os, target_arch) {
        (LinkMode::Static, "linux", "x86_64") => {
            format!("sherpa-onnx-v{version}-linux-x64-static-lib.tar.bz2")
        }
        (LinkMode::Static, "linux", "aarch64") => {
            format!("sherpa-onnx-v{version}-linux-aarch64-static-lib.tar.bz2")
        }
        (LinkMode::Static, "macos", "x86_64") => {
            format!("sherpa-onnx-v{version}-osx-x64-static-lib.tar.bz2")
        }
        (LinkMode::Static, "macos", "aarch64") => {
            format!("sherpa-onnx-v{version}-osx-arm64-static-lib.tar.bz2")
        }
        (LinkMode::Static, "windows", "x86_64") => {
            format!("sherpa-onnx-v{version}-win-x64-static-MT-Release-lib.tar.bz2")
        }
        (LinkMode::Shared, "linux", "x86_64") => {
            format!("sherpa-onnx-v{version}-linux-x64-shared-lib.tar.bz2")
        }
        (LinkMode::Shared, "linux", "aarch64") => {
            format!("sherpa-onnx-v{version}-linux-aarch64-shared-cpu-lib.tar.bz2")
        }
        (LinkMode::Shared, "macos", "x86_64") => {
            format!("sherpa-onnx-v{version}-osx-x64-shared-lib.tar.bz2")
        }
        (LinkMode::Shared, "macos", "aarch64") => {
            format!("sherpa-onnx-v{version}-osx-arm64-shared-lib.tar.bz2")
        }
        (LinkMode::Shared, "windows", "x86_64") => {
            format!("sherpa-onnx-v{version}-win-x64-shared-MT-Release-lib.tar.bz2")
        }
        (LinkMode::Static, "ios", _) => return Err(
            "iOS builds must set SHERPA_ONNX_LIB_DIR to prepared sherpa-onnx iOS static libraries"
                .into(),
        ),
        _ => {
            return Err(format!(
            "Unsupported target for sherpa-onnx prebuilt libs: os={target_os}, arch={target_arch}"
        )
            .into())
        }
    };

    Ok(name)
}

fn emit_shared_link_directives() {
    println!("cargo:rustc-link-lib=dylib=sherpa-onnx-c-api");
    println!("cargo:rustc-link-lib=dylib=onnxruntime");
}

fn emit_ios_static_link_directives() {
    // The upstream iOS package ships aggregate static archives inside
    // XCFramework slices. Link the aggregate sherpa archive once instead of
    // the desktop/Linux component archive list.
    println!("cargo:rustc-link-lib=static=sherpa-onnx");
    println!("cargo:rustc-link-lib=static=onnxruntime");
    println!("cargo:rustc-link-lib=dylib=c++");
    println!("cargo:rustc-link-lib=framework=Foundation");
}

fn emit_static_link_directives(target_os: &str) {
    for lib in SHERPA_ONNX_STATIC_LIBS {
        println!("cargo:rustc-link-lib=static={lib}");
    }

    match target_os {
        "linux" => {
            println!("cargo:rustc-link-lib=dylib=stdc++");
            println!("cargo:rustc-link-lib=dylib=m");
            println!("cargo:rustc-link-lib=dylib=pthread");
            println!("cargo:rustc-link-lib=dylib=dl");
        }
        "macos" => {
            println!("cargo:rustc-link-lib=dylib=c++");
            println!("cargo:rustc-link-lib=framework=Foundation");
        }
        _ => {}
    }
}

fn target_dir_from_out_dir(out_dir: &Path) -> Result<PathBuf, DynError> {
    if let Ok(explicit_target_dir) = env::var("CARGO_TARGET_DIR") {
        return Ok(PathBuf::from(explicit_target_dir));
    }

    if let Some(target_dir) = out_dir
        .ancestors()
        .find(|path| path.file_name() == Some(OsStr::new("target")))
    {
        return Ok(target_dir.to_path_buf());
    }

    Ok(out_dir.to_path_buf())
}

fn emit_relative_rpath(target_os: &str) {
    match target_os {
        "linux" => println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN"),
        "macos" => println!("cargo:rustc-link-arg=-Wl,-rpath,@loader_path"),
        _ => {}
    }
}

/// Cargo OUT_DIR ends in <profile>/build/<crate>/out, including custom profiles and cross targets.
fn profile_output_dirs() -> Result<[PathBuf; 2], DynError> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let profile_dir = out_dir
        .ancestors()
        .nth(3)
        .ok_or("Invalid Cargo OUT_DIR")?
        .to_path_buf();

    Ok([profile_dir.clone(), profile_dir.join("examples")])
}

fn copy_unix_runtime_libs(lib_dir: &Path, target_os: &str) -> Result<(), DynError> {
    let runtime_libs: Vec<PathBuf> = fs::read_dir(lib_dir)?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| {
            path.file_name()
                .and_then(OsStr::to_str)
                .map(|name| match target_os {
                    "linux" => name.contains(".so"),
                    "macos" => name.ends_with(".dylib"),
                    _ => false,
                })
                .unwrap_or(false)
        })
        .collect();

    if runtime_libs.is_empty() {
        return Err(format!("No shared runtime libraries found in {}", lib_dir.display()).into());
    }

    let mut copy_plan = Vec::<(PathBuf, OsString)>::new();
    let mut planned_names = HashSet::<OsString>::new();

    for lib in runtime_libs {
        if !lib.exists() {
            continue;
        }

        let lib_name = lib
            .file_name()
            .ok_or_else(|| format!("Invalid runtime library path: {}", lib.display()))?
            .to_os_string();

        let source = fs::canonicalize(&lib).unwrap_or(lib.clone());
        if planned_names.insert(lib_name.clone()) {
            copy_plan.push((source.clone(), lib_name));
        }

        if let Some(source_name) = source.file_name() {
            let source_name = source_name.to_os_string();
            if planned_names.insert(source_name.clone()) {
                copy_plan.push((source.clone(), source_name));
            }
        }
    }

    if copy_plan.is_empty() {
        return Err(format!(
            "No usable shared runtime libraries found in {}",
            lib_dir.display()
        )
        .into());
    }

    for dest_dir in profile_output_dirs()? {
        fs::create_dir_all(&dest_dir)?;
        for (source, dest_name) in &copy_plan {
            let dest = dest_dir.join(dest_name);
            // Replace the inode; a running app may still map the previous library.
            copy_file_atomically(source, &dest)?;
        }
    }

    Ok(())
}

fn temp_path_for(path: &Path) -> PathBuf {
    let mut temp_name = path
        .file_name()
        .map(OsStr::to_os_string)
        .unwrap_or_else(|| OsString::from("tmp"));
    temp_name.push(".part");
    path.with_file_name(temp_name)
}

fn write_reader_atomically(reader: &mut dyn io::Read, dst: &Path) -> Result<(), DynError> {
    let temp_path = temp_path_for(dst);
    if temp_path.exists() {
        let _ = fs::remove_file(&temp_path);
    }

    {
        let mut file = File::create(&temp_path)?;
        io::copy(reader, &mut file)?;
        file.sync_all()?;
    }

    fs::rename(&temp_path, dst)?;
    Ok(())
}

fn copy_windows_runtime_dlls(lib_dir: &Path) -> Result<(), DynError> {
    let dlls: Vec<PathBuf> = fs::read_dir(lib_dir)?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension() == Some(OsStr::new("dll")))
        .collect();

    if dlls.is_empty() {
        println!(
            "cargo:warning=No runtime DLLs found in {}",
            lib_dir.display()
        );
        return Ok(());
    }

    let [profile_dir, examples_dir] = profile_output_dirs()?;
    // Tests live in deps; app-local DLLs must take precedence over system/PATH copies.
    for dest_dir in [profile_dir.clone(), examples_dir, profile_dir.join("deps")] {
        fs::create_dir_all(&dest_dir)?;
        for dll in &dlls {
            let dest = dest_dir.join(
                dll.file_name()
                    .ok_or_else(|| format!("Invalid DLL path: {}", dll.display()))?,
            );
            fs::copy(dll, &dest)?;
        }
    }

    println!(
        "cargo:warning=Copied Windows runtime DLLs to {}, {}/examples and {}/deps",
        profile_dir.display(),
        profile_dir.display(),
        profile_dir.display()
    );

    Ok(())
}
