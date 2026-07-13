//! Minimal SILMA JSONL sidecar process wrapper.
//!
//! This is still a dev/local runner: it resolves the repo worker script and a
//! Python executable, sends one JSON request, reads one JSON response, and keeps
//! the process alive between calls. Packaging and long-lived supervision come
//! later.

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

impl SilmaSidecar {
    /// Start the local Python worker used by development probes and early routing.
    pub(super) fn start_dev() -> Result<Self, String> {
        let worker_path = silma_worker_path()?;
        let python_command = silma_python_command();
        let mut child = Command::new(&python_command)
            .arg(&worker_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|err| {
                format!("Failed to start SILMA worker with {python_command:?}: {err}")
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
            python_command,
            worker_path,
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

/// Resolve the dev worker script. Production packaging will replace this with a
/// bundled sidecar/resource path, but the env override keeps local experiments cheap.
fn silma_worker_path() -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("PAPERCUT_SILMA_WORKER") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!(
            "PAPERCUT_SILMA_WORKER does not point to a file: {}",
            path.display()
        ));
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
