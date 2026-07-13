//! Dev-only SILMA sidecar mechanics probe.
//!
//! This validates that Rust can spawn the JSONL Python worker and that the
//! worker can write a WAV into app data. Real SILMA model loading stays out of
//! this slice.

use std::fs;

use serde_json::{json, Value};
use tauri::Manager;

use super::cache::wav_info;
use super::silma_sidecar::SilmaSidecar;
use crate::native_tts::types::NativeSilmaSidecarProbeResponse;

/// Probe only the sidecar transport and app-data file access: health check,
/// probe WAV write, Rust WAV validation, then shutdown.
pub(crate) fn probe_silma_sidecar(
    app: tauri::AppHandle,
) -> Result<NativeSilmaSidecarProbeResponse, String> {
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

    let mut sidecar = SilmaSidecar::start(&app)?;
    let worker_path = sidecar.worker_path.display().to_string();
    let python_command = sidecar.python_command.clone();
    let health = sidecar.request(json!({"id": "1", "op": "health"}))?;
    let probe = sidecar.request(json!({
        "id": "2",
        "op": "write_probe_wav",
        "output_wav": probe_wav_path.display().to_string()
    }))?;
    sidecar.shutdown()?;
    let info = wav_info(&probe_wav_path).ok_or_else(|| {
        format!(
            "SILMA sidecar probe wrote an invalid WAV at {}",
            probe_wav_path.display()
        )
    })?;

    Ok(NativeSilmaSidecarProbeResponse {
        worker_path,
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
