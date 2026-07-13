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

impl SilmaSidecar {
    /// Start the local worker used by development probes and early routing.
    pub(super) fn start_dev() -> Result<Self, String> {
        let launch = silma_launch_command()?;
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
    pub(super) fn load_model(&mut self, model_dir: &Path) -> Result<i32, String> {
        let response = self.request(serde_json::json!({
            "id": "load_model",
            "op": "load_model",
            "model_dir": model_dir.display().to_string(),
        }))?;
        Ok(response
            .get("sample_rate")
            .and_then(Value::as_i64)
            .unwrap_or(24_000) as i32)
    }

    /// Ask the loaded worker to synthesize one WAV at the exact path provided.
    pub(super) fn synthesize_to_wav(
        &mut self,
        text: &str,
        output_wav: &Path,
        speed: f32,
    ) -> Result<SilmaSynthesisResult, String> {
        let speed = if speed.is_finite() && speed > 0.0 {
            speed
        } else {
            1.0
        };
        let response = self.request(serde_json::json!({
            "id": "synthesize",
            "op": "synthesize",
            "text": text,
            "output_wav": output_wav.display().to_string(),
            "speed": speed,
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

/// Prefer a packaged JSONL executable when supplied; otherwise use the editable
/// Python worker script. This keeps packaging tests out of the normal dev path.
fn silma_launch_command() -> Result<SilmaLaunchCommand, String> {
    if let Ok(path) = std::env::var("PAPERCUT_SILMA_WORKER_BIN") {
        let worker_path = require_file("PAPERCUT_SILMA_WORKER_BIN", PathBuf::from(path))?;
        return Ok(SilmaLaunchCommand {
            program: worker_path.clone(),
            args: Vec::new(),
            python_command: "<packaged>".into(),
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

/// Resolve the dev worker script. Production packaging will replace this with a
/// bundled sidecar/resource path, but the env override keeps local experiments cheap.
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
