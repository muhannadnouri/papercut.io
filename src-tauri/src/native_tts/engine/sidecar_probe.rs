//! Dev-only SILMA sidecar mechanics probe.
//!
//! This validates that Rust can spawn the JSONL Python worker and that the
//! worker can write a WAV into app data. Real SILMA model loading stays out of
//! this slice.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde_json::{json, Value};
use tauri::Manager;

use super::cache::wav_info;
use crate::native_tts::types::NativeSilmaSidecarProbeResponse;

/// Probe only the sidecar transport and app-data file access: health check,
/// probe WAV write, Rust WAV validation, then shutdown.
pub(crate) fn probe_silma_sidecar(
    app: tauri::AppHandle,
) -> Result<NativeSilmaSidecarProbeResponse, String> {
    let worker_path = silma_worker_path()?;
    let python_command = silma_python_command();
    let probe_dir = app
        .path()
        .app_data_dir()
        .map_err(|err| format!("Failed to resolve app data dir for SILMA probe: {err}"))?
        .join("silma-sidecar-probe");
    fs::create_dir_all(&probe_dir).map_err(|err| {
        format!(
            "Failed to create SILMA probe directory {}: {err}",
            probe_dir.display()
        )
    })?;
    let probe_wav_path = probe_dir.join("probe.wav");

    let output = run_worker_probe(&python_command, &worker_path, &probe_wav_path)?;
    let health = response_by_id(&output, "1")?;
    let probe = response_by_id(&output, "2")?;
    let info = wav_info(&probe_wav_path).ok_or_else(|| {
        format!(
            "SILMA sidecar probe wrote an invalid WAV at {}",
            probe_wav_path.display()
        )
    })?;

    Ok(NativeSilmaSidecarProbeResponse {
        worker_path: worker_path.display().to_string(),
        python_command,
        probe_wav_path: probe_wav_path.display().to_string(),
        health_version: health
            .get("version")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string(),
        sample_rate: probe
            .get("sample_rate")
            .and_then(Value::as_i64)
            .unwrap_or(24_000) as i32,
        audio_duration_sec: info.audio_duration_sec,
        wav_bytes: info.wav_bytes,
    })
}

/// Start the worker, send the minimal Stage 1 JSONL script, and require every
/// response to be a successful protocol object before returning it to callers.
fn run_worker_probe(
    python_command: &str,
    worker_path: &PathBuf,
    probe_wav_path: &PathBuf,
) -> Result<Vec<Value>, String> {
    let mut child = Command::new(python_command)
        .arg(worker_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("Failed to start SILMA worker with {python_command:?}: {err}"))?;

    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| "Failed to open SILMA worker stdin".to_string())?;
        let requests = [
            json!({"id": "1", "op": "health"}),
            json!({
                "id": "2",
                "op": "write_probe_wav",
                "output_wav": probe_wav_path.display().to_string()
            }),
            json!({"id": "3", "op": "shutdown"}),
        ];
        for request in requests {
            writeln!(stdin, "{request}")
                .map_err(|err| format!("Failed to write SILMA worker request: {err}"))?;
        }
    }

    let output = child
        .wait_with_output()
        .map_err(|err| format!("Failed to read SILMA worker output: {err}"))?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        return Err(format!(
            "SILMA worker exited with status {}: {}",
            output.status, stderr
        ));
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|err| format!("SILMA worker stdout was not UTF-8: {err}"))?;
    let responses = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str::<Value>(line)
                .map_err(|err| format!("Failed to parse SILMA worker response {line:?}: {err}"))
        })
        .collect::<Result<Vec<_>, _>>()?;

    for response in &responses {
        if response.get("ok").and_then(Value::as_bool) != Some(true) {
            return Err(format!(
                "SILMA worker probe failed: {response}; stderr: {stderr}"
            ));
        }
    }

    Ok(responses)
}

/// Find a response by request id so later probe steps can stay position-agnostic
/// if the worker ever emits extra successful protocol messages.
fn response_by_id<'a>(responses: &'a [Value], id: &str) -> Result<&'a Value, String> {
    responses
        .iter()
        .find(|response| response.get("id").and_then(Value::as_str) == Some(id))
        .ok_or_else(|| format!("SILMA worker did not return response id {id}"))
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

/// Pick the Python executable for the Stage 1 probe without adding project-wide
/// configuration. Developers can point at a venv with `PAPERCUT_SILMA_PYTHON`.
fn silma_python_command() -> String {
    std::env::var("PAPERCUT_SILMA_PYTHON").unwrap_or_else(|_| {
        if cfg!(windows) {
            "python".into()
        } else {
            "python3".into()
        }
    })
}
