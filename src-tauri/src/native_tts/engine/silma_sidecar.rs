//! Minimal SILMA JSONL sidecar process wrapper.
//!
//! This is still a dev/local runner: it resolves either the repo worker script
//! plus a Python executable, or a packaged worker executable, then keeps the
//! JSONL process alive between calls. Bundle discovery and long-lived
//! supervision come later.

use std::fs;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use bzip2::read::BzDecoder;
use reqwest::header::RANGE;
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tauri::Manager;

const SILMA_RUNTIME_PACK_MANIFEST: &str = include_str!("../../../tts/silma-runtime-packs.json");
const SILMA_LOCAL_RUNTIME_MANIFEST: &str = "silma-runtime.local.json";
pub(super) const DEFAULT_SILMA_NFE_STEP: i32 = 32;

pub(super) struct SilmaSidecar {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    pub(super) python_command: String,
    pub(super) worker_path: PathBuf,
}

pub(super) struct SilmaSynthesisResult {
    pub(super) audio_duration_sec: f32,
    pub(super) synthesis_ms: u128,
}

pub(super) struct SilmaLoadResult {
    pub(super) sample_rate: i32,
    pub(super) device: String,
    pub(super) torch_threads: i32,
    pub(super) torch_interop_threads: i32,
}

/// Lightweight status used by model status without starting Python.
pub(super) struct SilmaRuntimeStatus {
    pub(super) installed: bool,
    pub(super) runtime_dir: Option<PathBuf>,
    pub(super) message: String,
    pub(super) install_supported: bool,
    pub(super) archive_bytes: u64,
}

impl SilmaSidecar {
    /// Start the SILMA worker, preferring explicit dev overrides and then a bundled resource.
    pub(super) fn start(app: &tauri::AppHandle) -> Result<Self, String> {
        let launch = silma_launch_command(app)?;
        let mut command = silma_worker_command(&launch.program);
        let mut child = command
            .args(launch.args.iter().map(|arg| arg.as_os_str()))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|err| {
                format!(
                    "Failed to start SILMA worker with {:?}: {err}",
                    launch.program
                )
            })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Failed to open SILMA worker stdin".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Failed to open SILMA worker stdout".to_string())?;
        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            python_command: launch.python_command,
            worker_path: launch.worker_path,
        })
    }

    /// Send one JSONL request and read the matching single-line JSON response.
    pub(super) fn request(&mut self, request: Value) -> Result<Value, String> {
        let request_id = request
            .get("id")
            .and_then(Value::as_str)
            .map(str::to_string);
        writeln!(self.stdin, "{request}")
            .map_err(|err| format!("Failed to write SILMA worker request: {err}"))?;
        self.stdin
            .flush()
            .map_err(|err| format!("Failed to flush SILMA worker request: {err}"))?;

        let mut line = String::new();
        let bytes = self
            .stdout
            .read_line(&mut line)
            .map_err(|err| format!("Failed to read SILMA worker response: {err}"))?;
        if bytes == 0 {
            return Err("SILMA worker exited before returning a response".into());
        }
        let response = serde_json::from_str::<Value>(&line)
            .map_err(|err| format!("Failed to parse SILMA worker response {line:?}: {err}"))?;
        if let Some(request_id) = request_id {
            if response.get("id").and_then(Value::as_str) != Some(request_id.as_str()) {
                return Err(format!(
                    "SILMA worker returned response for the wrong request: {response}"
                ));
            }
        }
        if response.get("ok").and_then(Value::as_bool) != Some(true) {
            return Err(format!("SILMA worker request failed: {response}"));
        }
        Ok(response)
    }

    /// Load SILMA once in the worker and return the sample rate it reports.
    pub(super) fn load_model(
        &mut self,
        model_dir: &Path,
        torch_threads: i32,
    ) -> Result<SilmaLoadResult, String> {
        let response = self.request(serde_json::json!({
            "id": "load_model",
            "op": "load_model",
            "model_dir": model_dir.display().to_string(),
            "torch_threads": torch_threads,
        }))?;
        Ok(SilmaLoadResult {
            sample_rate: response
                .get("sample_rate")
                .and_then(Value::as_i64)
                .unwrap_or(24_000) as i32,
            device: response
                .get("device")
                .and_then(Value::as_str)
                .unwrap_or("unreported")
                .to_string(),
            torch_threads: response
                .get("torch_threads")
                .and_then(Value::as_i64)
                .unwrap_or(torch_threads as i64) as i32,
            torch_interop_threads: response
                .get("torch_interop_threads")
                .and_then(Value::as_i64)
                .unwrap_or(0) as i32,
        })
    }

    /// Ask the loaded worker to synthesize one WAV at the exact path provided.
    pub(super) fn synthesize_to_wav(
        &mut self,
        text: &str,
        output_wav: &Path,
        speed: f32,
        nfe_step: i32,
    ) -> Result<SilmaSynthesisResult, String> {
        let speed = if speed.is_finite() && speed > 0.0 {
            speed
        } else {
            1.0
        };
        let nfe_step = normalize_silma_nfe_step(nfe_step);
        let response = self.request(serde_json::json!({
            "id": "synthesize",
            "op": "synthesize",
            "text": text,
            "output_wav": output_wav.display().to_string(),
            "speed": speed,
            "nfe_step": nfe_step,
        }))?;
        Ok(SilmaSynthesisResult {
            audio_duration_sec: response
                .get("audio_duration_sec")
                .and_then(Value::as_f64)
                .unwrap_or(0.0) as f32,
            synthesis_ms: response
                .get("synthesis_ms")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u128,
        })
    }

    /// Ask the worker to exit and wait for it, surfacing abnormal status.
    pub(super) fn shutdown(mut self) -> Result<(), String> {
        let _ = self.request(serde_json::json!({"id": "shutdown", "op": "shutdown"}))?;
        let status = self
            .child
            .wait()
            .map_err(|err| format!("Failed to wait for SILMA worker shutdown: {err}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("SILMA worker exited with status {status}"))
        }
    }
}

impl Drop for SilmaSidecar {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = writeln!(
                self.stdin,
                "{}",
                serde_json::json!({"id": "drop", "op": "shutdown"})
            );
            let _ = self.stdin.flush();
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

struct SilmaLaunchCommand {
    program: PathBuf,
    args: Vec<PathBuf>,
    python_command: String,
    worker_path: PathBuf,
}

struct InstalledSilmaRuntime {
    worker_path: PathBuf,
    label: &'static str,
    message: &'static str,
}

/// Prefer explicit env overrides, then user-installed local runtimes, runtime packs, and finally repo dev scripts.
fn silma_launch_command(app: &tauri::AppHandle) -> Result<SilmaLaunchCommand, String> {
    if !silma_supported_on_current_platform() {
        return Err("SILMA runtime is currently supported on Linux x64 only".into());
    }

    if let Ok(path) = std::env::var("PAPERCUT_SILMA_WORKER_BIN") {
        let worker_path = require_file("PAPERCUT_SILMA_WORKER_BIN", PathBuf::from(path))?;
        return Ok(SilmaLaunchCommand {
            program: worker_path.clone(),
            args: Vec::new(),
            python_command: "<packaged>".into(),
            worker_path,
        });
    }

    if std::env::var_os("PAPERCUT_SILMA_WORKER").is_some()
        || std::env::var_os("PAPERCUT_SILMA_PYTHON").is_some()
    {
        let worker_path = silma_worker_path()?;
        let python_command = silma_python_command();
        return Ok(SilmaLaunchCommand {
            program: PathBuf::from(&python_command),
            args: vec![worker_path.clone()],
            python_command,
            worker_path,
        });
    }

    if let Some(runtime) = installed_runtime_worker_path(app)? {
        return Ok(SilmaLaunchCommand {
            program: runtime.worker_path.clone(),
            args: Vec::new(),
            python_command: runtime.label.into(),
            worker_path: runtime.worker_path,
        });
    }

    let worker_path = silma_worker_path()?;
    let python_command = silma_python_command();
    Ok(SilmaLaunchCommand {
        program: PathBuf::from(&python_command),
        args: vec![worker_path.clone()],
        python_command,
        worker_path,
    })
}

/// Report whether SILMA can start without asking users for a Python install.
pub(super) fn silma_runtime_status(app: &tauri::AppHandle) -> SilmaRuntimeStatus {
    if !silma_supported_on_current_platform() {
        return SilmaRuntimeStatus {
            installed: false,
            runtime_dir: None,
            install_supported: false,
            archive_bytes: 0,
            message: "SILMA runtime is currently supported on Linux x64 only".into(),
        };
    }

    if let Ok(path) = std::env::var("PAPERCUT_SILMA_WORKER_BIN") {
        let path = PathBuf::from(path);
        return SilmaRuntimeStatus {
            installed: path.is_file(),
            runtime_dir: path.parent().map(Path::to_path_buf),
            install_supported: false,
            archive_bytes: 0,
            message: if path.is_file() {
                "SILMA runtime available from PAPERCUT_SILMA_WORKER_BIN".into()
            } else {
                format!(
                    "PAPERCUT_SILMA_WORKER_BIN does not point to a file: {}",
                    path.display()
                )
            },
        };
    }

    if std::env::var_os("PAPERCUT_SILMA_WORKER").is_some()
        || std::env::var_os("PAPERCUT_SILMA_PYTHON").is_some()
    {
        return match silma_worker_path() {
            Ok(worker_path) => SilmaRuntimeStatus {
                installed: true,
                runtime_dir: worker_path.parent().map(Path::to_path_buf),
                install_supported: false,
                archive_bytes: 0,
                message: "SILMA runtime available from development Python settings".into(),
            },
            Err(err) => SilmaRuntimeStatus {
                installed: false,
                runtime_dir: None,
                install_supported: silma_runtime_pack_install_source().is_some(),
                archive_bytes: silma_runtime_pack_archive_bytes(),
                message: err,
            },
        };
    }

    match installed_runtime_worker_path(app) {
        Ok(Some(runtime)) => SilmaRuntimeStatus {
            installed: true,
            runtime_dir: runtime.worker_path.parent().map(Path::to_path_buf),
            install_supported: false,
            archive_bytes: 0,
            message: runtime.message.into(),
        },
        Ok(None) if silma_runtime_pack_install_source().is_some() => SilmaRuntimeStatus {
            installed: false,
            runtime_dir: runtime_pack_dir(app).ok(),
            install_supported: true,
            archive_bytes: silma_runtime_pack_archive_bytes(),
            message: silma_runtime_pack_install_source()
                .map(|source| source.status_message())
                .unwrap_or_else(|| "SILMA runtime pack is ready to install".into()),
        },
        Ok(None) => repo_worker_runtime_status(app),
        Err(err) => SilmaRuntimeStatus {
            installed: false,
            runtime_dir: None,
            install_supported: silma_runtime_pack_install_source().is_some(),
            archive_bytes: silma_runtime_pack_archive_bytes(),
            message: err,
        },
    }
}

fn repo_worker_runtime_status(app: &tauri::AppHandle) -> SilmaRuntimeStatus {
    match silma_worker_path() {
        Ok(worker_path) => SilmaRuntimeStatus {
            installed: true,
            runtime_dir: worker_path.parent().map(Path::to_path_buf),
            install_supported: false,
            archive_bytes: 0,
            message: "SILMA development worker available from the repository".into(),
        },
        Err(_) => SilmaRuntimeStatus {
            installed: false,
            runtime_dir: runtime_pack_dir(app).ok(),
            install_supported: silma_runtime_pack_install_source().is_some(),
            archive_bytes: silma_runtime_pack_archive_bytes(),
            message: "SILMA runtime pack is not installed".into(),
        },
    }
}

/// Promote a prepared source-preserving Python runtime into the app-data runtime-pack slot.
pub(super) fn install_silma_runtime_pack(
    app: &tauri::AppHandle,
    mut on_progress: impl FnMut(&str, &str, u64, u64),
) -> Result<PathBuf, String> {
    let source = silma_runtime_pack_install_source().ok_or_else(|| {
        "SILMA runtime pack source is not available. Run `npm run prepare:silma-sidecar -- --self-test`, or add release metadata to src-tauri/tts/silma-runtime-packs.json.".to_string()
    })?;
    let progress_total = source.archive_bytes().max(1);
    let final_dir = runtime_pack_dir(app)?;
    let work_dir = runtime_pack_work_dir(app)?;
    let staging_dir = work_dir.join("current.installing");
    let archive_path = work_dir.join("silma-runtime-pack.tar.bz2");
    fs::create_dir_all(&work_dir).map_err(|err| {
        format!(
            "Failed to create SILMA runtime work directory {}: {err}",
            work_dir.display()
        )
    })?;
    let _ = fs::remove_dir_all(&staging_dir);
    fs::create_dir_all(&staging_dir).map_err(|err| {
        format!(
            "Failed to create SILMA runtime staging directory {}: {err}",
            staging_dir.display()
        )
    })?;
    let staging_guard = RuntimeWorkDirGuard::new(staging_dir.clone());
    match source {
        SilmaRuntimePackSource::Directory(path) => {
            on_progress(
                "extracting",
                "Preparing SILMA runtime pack",
                0,
                progress_total,
            );
            copy_dir_contents(&path, &staging_dir)?;
        }
        SilmaRuntimePackSource::Archive {
            url,
            parts,
            sha256,
            bytes,
        } => {
            if parts.is_empty() {
                let url = url.ok_or_else(|| "SILMA runtime pack URL is missing".to_string())?;
                download_runtime_pack_file(
                    &url,
                    bytes,
                    &archive_path,
                    0,
                    bytes,
                    &mut |downloaded, total| {
                        on_progress(
                            "downloading",
                            "Downloading SILMA runtime pack",
                            downloaded,
                            total,
                        );
                    },
                )?;
            } else {
                download_runtime_pack_parts(
                    &parts,
                    bytes,
                    &archive_path,
                    &work_dir,
                    &mut |downloaded, total| {
                        on_progress(
                            "downloading",
                            "Downloading SILMA runtime pack",
                            downloaded,
                            total,
                        );
                    },
                )?;
            }
            on_progress("verifying", "Verifying SILMA runtime pack", bytes, bytes);
            if let Err(err) = verify_runtime_pack_archive(&archive_path, &sha256) {
                let _ = fs::remove_file(&archive_path);
                remove_runtime_pack_part_cache(&work_dir);
                return Err(err);
            }
            on_progress("extracting", "Extracting SILMA runtime pack", bytes, bytes);
            extract_runtime_pack_archive(&archive_path, &staging_dir)?;
        }
    }
    let worker_path = runtime_pack_worker_path_in(&staging_dir)?;
    on_progress(
        "testing",
        "Testing SILMA runtime pack",
        progress_total,
        progress_total,
    );
    verify_silma_runtime_worker(&worker_path)?;
    on_progress(
        "installing",
        "Installing SILMA runtime pack",
        progress_total,
        progress_total,
    );
    if let Some(parent) = final_dir.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "Failed to create SILMA runtime directory {}: {err}",
                parent.display()
            )
        })?;
    }
    let _ = fs::remove_dir_all(&final_dir);
    fs::rename(&staging_dir, &final_dir).map_err(|err| {
        format!(
            "Failed to install SILMA runtime pack {}: {err}",
            final_dir.display()
        )
    })?;
    staging_guard.disarm();
    let _ = fs::remove_dir_all(&work_dir);
    Ok(final_dir)
}

struct RuntimeWorkDirGuard {
    path: PathBuf,
    armed: bool,
}

impl RuntimeWorkDirGuard {
    /// Arm cleanup for the large runtime staging tree until promotion succeeds.
    fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }

    /// Consume the guard after a successful install so Drop skips cleanup.
    fn disarm(mut self) {
        self.armed = false;
    }
}

impl Drop for RuntimeWorkDirGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

enum SilmaRuntimePackSource {
    Directory(PathBuf),
    Archive {
        url: Option<String>,
        parts: Vec<SilmaRuntimePackPart>,
        sha256: String,
        bytes: u64,
    },
}

struct SilmaRuntimePackPart {
    url: String,
    bytes: u64,
}

impl SilmaRuntimePackSource {
    fn status_message(&self) -> String {
        match self {
            Self::Directory(_) => "Prepared SILMA runtime pack is ready to install".into(),
            Self::Archive { .. } => "SILMA runtime pack is ready to download".into(),
        }
    }

    fn archive_bytes(&self) -> u64 {
        match self {
            Self::Directory(_) => 0,
            Self::Archive { bytes, .. } => *bytes,
        }
    }
}

/// Resolve the dev worker script. Env overrides keep local experiments cheap.
fn silma_worker_path() -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("PAPERCUT_SILMA_WORKER") {
        return require_file("PAPERCUT_SILMA_WORKER", PathBuf::from(path));
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = manifest_dir
        .parent()
        .ok_or_else(|| "Failed to resolve repository root from Cargo manifest dir".to_string())?
        .join("sidecars/silma/silma_worker.py");
    if path.is_file() {
        return Ok(path);
    }
    Err(format!("SILMA worker not found at {}", path.display()))
}

/// Prefer the checked runtime manifest, then the local build output used by dev.
fn silma_runtime_pack_install_source() -> Option<SilmaRuntimePackSource> {
    if let Some(source) = silma_runtime_pack_manifest_source() {
        return Some(source);
    }
    silma_runtime_pack_default_source_dir().map(SilmaRuntimePackSource::Directory)
}

fn silma_runtime_pack_archive_bytes() -> u64 {
    silma_runtime_pack_manifest_source()
        .map(|source| source.archive_bytes())
        .unwrap_or(0)
}

#[derive(Deserialize)]
struct SilmaRuntimePackManifest {
    runtimes: Vec<SilmaRuntimePackManifestEntry>,
}

#[derive(Deserialize)]
struct SilmaRuntimePackManifestEntry {
    #[serde(rename = "runtimeId")]
    runtime_id: String,
    #[serde(default)]
    url: String,
    sha256: String,
    #[serde(rename = "archiveBytes")]
    archive_bytes: u64,
    #[serde(default)]
    parts: Vec<SilmaRuntimePackManifestPart>,
}

#[derive(Deserialize)]
struct SilmaRuntimePackManifestPart {
    url: String,
    bytes: u64,
}

fn silma_runtime_pack_manifest_source() -> Option<SilmaRuntimePackSource> {
    let manifest: SilmaRuntimePackManifest =
        serde_json::from_str(SILMA_RUNTIME_PACK_MANIFEST).ok()?;
    manifest
        .runtimes
        .into_iter()
        .find(|entry| {
            entry.runtime_id == runtime_pack_id()
                && !entry.sha256.is_empty()
                && entry.archive_bytes > 0
                && (!entry.url.is_empty() || valid_runtime_pack_parts(entry))
        })
        .map(|entry| SilmaRuntimePackSource::Archive {
            url: (!entry.url.is_empty()).then_some(entry.url),
            parts: entry
                .parts
                .into_iter()
                .map(|part| SilmaRuntimePackPart {
                    url: part.url,
                    bytes: part.bytes,
                })
                .collect(),
            sha256: entry.sha256,
            bytes: entry.archive_bytes,
        })
}

/// Accept split release metadata only when its parts exactly rebuild the archive.
fn valid_runtime_pack_parts(entry: &SilmaRuntimePackManifestEntry) -> bool {
    !entry.parts.is_empty()
        && entry
            .parts
            .iter()
            .all(|part| !part.url.is_empty() && part.bytes > 0)
        && entry.parts.iter().map(|part| part.bytes).sum::<u64>() == entry.archive_bytes
}

fn silma_runtime_pack_default_source_dir() -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir.parent()?;
    let source = repo_root
        .join("sidecars")
        .join("silma")
        .join("runtime")
        .join("x86_64-unknown-linux-gnu")
        .join("onedir");
    if runtime_pack_worker_path_in(&source).is_ok() {
        return Some(source);
    }
    None
}

/// Download a pinned runtime-pack file, resuming partial cache files when the server allows it.
fn download_runtime_pack_file(
    url: &str,
    expected_bytes: u64,
    destination: &Path,
    completed_before_file: u64,
    reported_total: u64,
    on_download: &mut impl FnMut(u64, u64),
) -> Result<(), String> {
    let resume_from = runtime_archive_resume_offset(destination, expected_bytes)?;
    let expected_total = if expected_bytes > 0 {
        expected_bytes
    } else {
        resume_from
    };
    if expected_bytes > 0 && resume_from == expected_bytes {
        on_download(completed_before_file + resume_from, reported_total);
        return Ok(());
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60 * 60))
        .user_agent("Papercut SILMA runtime installer")
        .build()
        .map_err(|err| format!("Failed to create SILMA runtime downloader: {err}"))?;
    let mut request = client.get(url);
    if resume_from > 0 {
        request = request.header(RANGE, format!("bytes={resume_from}-"));
    }
    let mut response = request
        .send()
        .map_err(|err| format!("Failed to download SILMA runtime pack from {url}: {err}"))?
        .error_for_status()
        .map_err(|err| format!("Failed to download SILMA runtime pack from {url}: {err}"))?;
    let appending = resume_from > 0 && response.status() == StatusCode::PARTIAL_CONTENT;
    let mut downloaded = if appending { resume_from } else { 0 };
    let total = response
        .content_length()
        .map(|length| length.saturating_add(downloaded))
        .filter(|value| *value > 0)
        .unwrap_or(expected_total);
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(appending)
        .truncate(!appending)
        .open(destination)
        .map_err(|err| {
            format!(
                "Failed to create SILMA runtime archive {}: {err}",
                destination.display()
            )
        })?;
    let mut writer = BufWriter::new(file);
    let mut last_percent = download_percent(downloaded, total);
    let mut buffer = [0u8; 256 * 1024];
    let progress_total = reported_total.max(completed_before_file + total);
    on_download(completed_before_file + downloaded, progress_total);
    loop {
        let read = response
            .read(&mut buffer)
            .map_err(|err| format!("Failed while downloading SILMA runtime pack: {err}"))?;
        if read == 0 {
            break;
        }
        writer.write_all(&buffer[..read]).map_err(|err| {
            format!(
                "Failed to write SILMA runtime archive {}: {err}",
                destination.display()
            )
        })?;
        downloaded += read as u64;
        let percent = download_percent(downloaded, total);
        if percent >= last_percent.saturating_add(2) || percent == 100 {
            last_percent = percent;
            on_download(completed_before_file + downloaded, progress_total);
        }
    }
    writer.flush().map_err(|err| {
        format!(
            "Failed to finish SILMA runtime archive {}: {err}",
            destination.display()
        )
    })?;
    Ok(())
}

/// Download every GitHub Release part into cache before rebuilding the archive.
fn download_runtime_pack_parts(
    parts: &[SilmaRuntimePackPart],
    expected_bytes: u64,
    archive_path: &Path,
    work_dir: &Path,
    on_download: &mut impl FnMut(u64, u64),
) -> Result<(), String> {
    let mut completed = 0;
    let mut part_paths = Vec::with_capacity(parts.len());
    for (index, part) in parts.iter().enumerate() {
        let part_path = work_dir.join(format!("silma-runtime-pack.part{:03}", index + 1));
        download_runtime_pack_file(
            &part.url,
            part.bytes,
            &part_path,
            completed,
            expected_bytes,
            on_download,
        )?;
        completed += part.bytes;
        part_paths.push(part_path);
    }
    assemble_runtime_pack_parts(&part_paths, archive_path)?;
    on_download(expected_bytes, expected_bytes);
    Ok(())
}

/// Recreate the original archive from verified-size release asset parts.
fn assemble_runtime_pack_parts(part_paths: &[PathBuf], archive_path: &Path) -> Result<(), String> {
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(archive_path)
        .map_err(|err| {
            format!(
                "Failed to create SILMA runtime archive {}: {err}",
                archive_path.display()
            )
        })?;
    let mut writer = BufWriter::new(file);
    for part_path in part_paths {
        let mut part = fs::File::open(part_path).map_err(|err| {
            format!(
                "Failed to open SILMA runtime archive part {}: {err}",
                part_path.display()
            )
        })?;
        std::io::copy(&mut part, &mut writer).map_err(|err| {
            format!(
                "Failed to assemble SILMA runtime archive from {}: {err}",
                part_path.display()
            )
        })?;
    }
    writer.flush().map_err(|err| {
        format!(
            "Failed to finish SILMA runtime archive {}: {err}",
            archive_path.display()
        )
    })
}

/// Clear cached parts after a checksum failure so retry starts from trusted bytes.
fn remove_runtime_pack_part_cache(work_dir: &Path) {
    if let Ok(entries) = fs::read_dir(work_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("silma-runtime-pack.part"))
            {
                let _ = fs::remove_file(path);
            }
        }
    }
}

/// Return the cached archive byte count that is safe to resume from.
fn runtime_archive_resume_offset(archive_path: &Path, expected_bytes: u64) -> Result<u64, String> {
    let Ok(metadata) = fs::metadata(archive_path) else {
        return Ok(0);
    };
    let size = metadata.len();
    if expected_bytes > 0 && size > expected_bytes {
        fs::remove_file(archive_path).map_err(|err| {
            format!(
                "Failed to remove oversized SILMA runtime archive {}: {err}",
                archive_path.display()
            )
        })?;
        return Ok(0);
    }
    Ok(size)
}

/// Verify the downloaded runtime-pack archive before extraction.
fn verify_runtime_pack_archive(archive_path: &Path, expected_sha256: &str) -> Result<(), String> {
    let file = fs::File::open(archive_path).map_err(|err| {
        format!(
            "Failed to open SILMA runtime archive {}: {err}",
            archive_path.display()
        )
    })?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 256 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|err| format!("Failed to hash SILMA runtime archive: {err}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let actual = format!("{:x}", hasher.finalize());
    if actual == expected_sha256 {
        Ok(())
    } else {
        Err(format!(
            "SILMA runtime checksum mismatch. Expected {expected_sha256}, got {actual}"
        ))
    }
}

/// Extract the verified `.tar.bz2` runtime archive into staging.
fn extract_runtime_pack_archive(archive_path: &Path, destination: &Path) -> Result<(), String> {
    let file = fs::File::open(archive_path).map_err(|err| {
        format!(
            "Failed to open SILMA runtime archive {}: {err}",
            archive_path.display()
        )
    })?;
    let decoder = BzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    archive
        .unpack(destination)
        .map_err(|err| format!("Failed to extract SILMA runtime pack: {err}"))
}

fn runtime_pack_worker_path(app: &tauri::AppHandle) -> Result<Option<PathBuf>, String> {
    let Some(relative_path) = runtime_pack_worker_relative_path() else {
        return Ok(None);
    };
    let worker_path = runtime_pack_dir(app)?.join(relative_path);
    if worker_path.is_file() {
        return Ok(Some(worker_path));
    }
    Ok(None)
}

/// Prefer a user-local runtime manifest over the downloaded CPU pack.
fn installed_runtime_worker_path(
    app: &tauri::AppHandle,
) -> Result<Option<InstalledSilmaRuntime>, String> {
    if let Some(worker_path) = local_runtime_worker_path(app)? {
        return Ok(Some(InstalledSilmaRuntime {
            worker_path,
            label: "<local-runtime>",
            message: "SILMA local runtime installed",
        }));
    }
    if let Some(worker_path) = runtime_pack_worker_path(app)? {
        return Ok(Some(InstalledSilmaRuntime {
            worker_path,
            label: "<runtime-pack>",
            message: "SILMA runtime pack installed",
        }));
    }
    Ok(None)
}

fn local_runtime_worker_path(app: &tauri::AppHandle) -> Result<Option<PathBuf>, String> {
    let manifest_path = local_runtime_manifest_path(app)?;
    if !manifest_path.is_file() {
        return Ok(None);
    }
    local_runtime_manifest_worker_path(&manifest_path)
}

/// Resolve a user-installed CUDA/runtime wrapper from app data without trusting cwd.
fn local_runtime_manifest_worker_path(manifest_path: &Path) -> Result<Option<PathBuf>, String> {
    let content = fs::read_to_string(manifest_path).map_err(|err| {
        format!(
            "Failed to read SILMA local runtime manifest {}: {err}",
            manifest_path.display()
        )
    })?;
    let manifest: SilmaLocalRuntimeManifest = serde_json::from_str(&content).map_err(|err| {
        format!(
            "Failed to parse SILMA local runtime manifest {}: {err}",
            manifest_path.display()
        )
    })?;
    let worker_path = PathBuf::from(manifest.worker_path);
    let worker_path = if worker_path.is_absolute() {
        worker_path
    } else {
        manifest_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(worker_path)
    };
    Ok(worker_path.is_file().then_some(worker_path))
}

#[derive(Deserialize)]
struct SilmaLocalRuntimeManifest {
    #[serde(rename = "workerPath", alias = "worker")]
    worker_path: String,
}

fn local_runtime_manifest_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|err| format!("Failed to resolve app data dir for SILMA local runtime: {err}"))?;
    Ok(app_data
        .join("runtimes")
        .join("silma")
        .join(SILMA_LOCAL_RUNTIME_MANIFEST))
}

/// Resolve the executable inside a runtime-pack root and reject wrong layouts.
fn runtime_pack_worker_path_in(root: &Path) -> Result<PathBuf, String> {
    let relative_path = runtime_pack_worker_relative_path()
        .ok_or_else(|| "SILMA runtime packs are not supported on this platform yet".to_string())?;
    let worker_path = root.join(relative_path);
    if worker_path.is_file() {
        return Ok(worker_path);
    }
    Err(format!(
        "SILMA runtime pack is missing worker executable at {}",
        worker_path.display()
    ))
}

fn runtime_pack_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|err| format!("Failed to resolve app data dir for SILMA runtime pack: {err}"))?;
    Ok(app_data
        .join("runtimes")
        .join("silma")
        .join(runtime_pack_id())
        .join("current"))
}

/// Cache-backed staging area used before atomically promoting the runtime pack.
fn runtime_pack_work_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let cache_dir = app
        .path()
        .app_cache_dir()
        .or_else(|_| app.path().app_data_dir())
        .map_err(|err| format!("Failed to resolve cache dir for SILMA runtime install: {err}"))?;
    Ok(cache_dir
        .join("runtime-installer")
        .join("silma")
        .join(runtime_pack_id()))
}

fn runtime_pack_id() -> &'static str {
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "linux-x64-cpu"
    } else {
        "unsupported"
    }
}

fn runtime_pack_worker_relative_path() -> Option<PathBuf> {
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return Some(PathBuf::from(
            "silma-worker-x86_64-unknown-linux-gnu/silma-worker-x86_64-unknown-linux-gnu",
        ));
    }
    None
}

fn silma_supported_on_current_platform() -> bool {
    cfg!(all(target_os = "linux", target_arch = "x86_64"))
}

/// Recursively copy a prepared runtime directory without pulling in another dependency.
fn copy_dir_contents(source: &Path, destination: &Path) -> Result<(), String> {
    for entry in fs::read_dir(source).map_err(|err| {
        format!(
            "Failed to read SILMA runtime source directory {}: {err}",
            source.display()
        )
    })? {
        let entry = entry.map_err(|err| format!("Failed to read SILMA runtime entry: {err}"))?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let metadata = fs::metadata(&source_path)
            .map_err(|err| format!("Failed to inspect SILMA runtime entry: {err}"))?;
        if metadata.is_dir() {
            fs::create_dir_all(&destination_path).map_err(|err| {
                format!(
                    "Failed to create SILMA runtime directory {}: {err}",
                    destination_path.display()
                )
            })?;
            copy_dir_contents(&source_path, &destination_path)?;
        } else if metadata.is_file() {
            fs::copy(&source_path, &destination_path).map_err(|err| {
                format!(
                    "Failed to copy SILMA runtime file {}: {err}",
                    source_path.display()
                )
            })?;
        }
    }
    Ok(())
}

/// Run the packaged worker's model-free self-test before trusting the runtime.
fn verify_silma_runtime_worker(worker_path: &Path) -> Result<(), String> {
    let output = silma_worker_command(worker_path)
        .arg("--self-test")
        .output()
        .map_err(|err| {
            format!(
                "Failed to run SILMA runtime self-test {}: {err}",
                worker_path.display()
            )
        })?;
    if output.status.success() {
        Ok(())
    } else {
        let detail = worker_check_failure_detail(&output);
        Err(format!(
            "SILMA runtime self-test failed with status {}{}",
            output.status, detail
        ))
    }
}

/// AppImage launches can export loader paths that break `/bin/sh` before the
/// worker launcher resets its own Python paths, so strip only that inherited bit.
fn silma_worker_command(program: &Path) -> Command {
    let mut command = Command::new(program);
    command.env_remove("LD_LIBRARY_PATH");
    command
}

fn worker_check_failure_detail(output: &std::process::Output) -> String {
    let bytes = if output.stderr.is_empty() {
        &output.stdout
    } else {
        &output.stderr
    };
    let text = String::from_utf8_lossy(bytes).trim().to_string();
    if text.is_empty() {
        String::new()
    } else {
        format!(": {text}")
    }
}

fn download_percent(downloaded: u64, total: u64) -> u8 {
    if total == 0 {
        return 0;
    }
    ((downloaded.saturating_mul(100) / total).min(100)) as u8
}

fn require_file(var_name: &str, path: PathBuf) -> Result<PathBuf, String> {
    if path.is_file() {
        return Ok(path);
    }
    Err(format!(
        "{var_name} does not point to a file: {}",
        path.display()
    ))
}

/// Pick the Python executable without adding project-wide configuration yet.
fn silma_python_command() -> String {
    std::env::var("PAPERCUT_SILMA_PYTHON").unwrap_or_else(|_| {
        if cfg!(windows) {
            "python".into()
        } else {
            "python3".into()
        }
    })
}

pub(super) fn normalize_silma_nfe_step(step: i32) -> i32 {
    match step {
        4 | 8 | 12 | 16 | 32 | 64 => step,
        _ => DEFAULT_SILMA_NFE_STEP,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn silma_nfe_step_matches_ui_quality_options() {
        for step in [4, 8, 12, 16, 32, 64] {
            assert_eq!(normalize_silma_nfe_step(step), step);
        }
        assert_eq!(normalize_silma_nfe_step(3), DEFAULT_SILMA_NFE_STEP);
    }

    #[test]
    fn local_runtime_manifest_resolves_relative_worker() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("papercut-silma-local-runtime-{nonce}"));
        let worker = dir.join("run-silma-worker");
        let manifest = dir.join(SILMA_LOCAL_RUNTIME_MANIFEST);
        fs::create_dir_all(&dir).expect("create temp runtime dir");
        fs::write(&worker, "").expect("create worker");
        fs::write(
            &manifest,
            r#"{"runtimeId":"linux-x64-cuda-local","workerPath":"run-silma-worker"}"#,
        )
        .expect("create manifest");

        assert_eq!(
            local_runtime_manifest_worker_path(&manifest).expect("resolve manifest"),
            Some(worker)
        );

        let _ = fs::remove_dir_all(dir);
    }
}
