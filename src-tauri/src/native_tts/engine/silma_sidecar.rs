//! Minimal SILMA JSONL sidecar process wrapper.
//!
//! This is still a dev/local runner: it resolves either the repo worker script
//! plus a Python executable, or a packaged worker executable, then keeps the
//! JSONL process alive between calls. Bundle discovery and long-lived
//! supervision come later.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::Value;
use tauri::Manager;

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

pub(super) struct SilmaRuntimeStatus {
    pub(super) installed: bool,
    pub(super) runtime_dir: Option<PathBuf>,
    pub(super) message: String,
}

impl SilmaSidecar {
    /// Start the SILMA worker, preferring explicit dev overrides and then a bundled resource.
    pub(super) fn start(app: &tauri::AppHandle) -> Result<Self, String> {
        let launch = silma_launch_command(app)?;
        let mut child = Command::new(&launch.program)
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
                .unwrap_or("unknown")
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

/// Prefer explicit env overrides, then optional runtime packs, bundled resources, then the repo script.
fn silma_launch_command(app: &tauri::AppHandle) -> Result<SilmaLaunchCommand, String> {
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

    if let Some(worker_path) = runtime_pack_worker_path(app)? {
        return Ok(SilmaLaunchCommand {
            program: worker_path.clone(),
            args: Vec::new(),
            python_command: "<runtime-pack>".into(),
            worker_path,
        });
    }

    if let Some(worker_path) = bundled_worker_path(app)? {
        return Ok(SilmaLaunchCommand {
            program: worker_path.clone(),
            args: Vec::new(),
            python_command: "<bundled>".into(),
            worker_path,
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
    if let Ok(path) = std::env::var("PAPERCUT_SILMA_WORKER_BIN") {
        let path = PathBuf::from(path);
        return SilmaRuntimeStatus {
            installed: path.is_file(),
            runtime_dir: path.parent().map(Path::to_path_buf),
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
                message: "SILMA runtime available from development Python settings".into(),
            },
            Err(err) => SilmaRuntimeStatus {
                installed: false,
                runtime_dir: None,
                message: err,
            },
        };
    }

    match runtime_pack_worker_path(app) {
        Ok(Some(worker_path)) => SilmaRuntimeStatus {
            installed: true,
            runtime_dir: worker_path.parent().map(Path::to_path_buf),
            message: "SILMA runtime pack installed".into(),
        },
        Ok(None) => match bundled_worker_path(app) {
            Ok(Some(worker_path)) => SilmaRuntimeStatus {
                installed: true,
                runtime_dir: worker_path.parent().map(Path::to_path_buf),
                message: "Bundled SILMA runtime available".into(),
            },
            Ok(None) => repo_worker_runtime_status(app),
            Err(err) => SilmaRuntimeStatus {
                installed: false,
                runtime_dir: runtime_pack_dir(app).ok(),
                message: err,
            },
        },
        Err(err) => SilmaRuntimeStatus {
            installed: false,
            runtime_dir: None,
            message: err,
        },
    }
}

fn repo_worker_runtime_status(app: &tauri::AppHandle) -> SilmaRuntimeStatus {
    match silma_worker_path() {
        Ok(worker_path) => SilmaRuntimeStatus {
            installed: true,
            runtime_dir: worker_path.parent().map(Path::to_path_buf),
            message: "SILMA development worker available from the repository".into(),
        },
        Err(_) => SilmaRuntimeStatus {
            installed: false,
            runtime_dir: runtime_pack_dir(app).ok(),
            message: "SILMA runtime pack is not installed".into(),
        },
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

fn runtime_pack_id() -> &'static str {
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "linux-x64-cpu"
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "windows-x64-cpu"
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "macos-aarch64-cpu"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "macos-x64-cpu"
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

fn bundled_worker_path(app: &tauri::AppHandle) -> Result<Option<PathBuf>, String> {
    let Some(relative_path) = bundled_worker_relative_path() else {
        return Ok(None);
    };
    let resource_dir = app
        .path()
        .resource_dir()
        .map_err(|err| format!("Failed to resolve app resource dir for SILMA sidecar: {err}"))?;
    let worker_path = resource_dir.join(relative_path);
    if worker_path.is_file() {
        return Ok(Some(worker_path));
    }
    Ok(None)
}

fn bundled_worker_relative_path() -> Option<PathBuf> {
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return Some(PathBuf::from(
            "silma-sidecar/silma-worker-x86_64-unknown-linux-gnu/silma-worker-x86_64-unknown-linux-gnu",
        ));
    }
    None
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
        4 | 8 | 12 | 16 => step,
        _ => 16,
    }
}
