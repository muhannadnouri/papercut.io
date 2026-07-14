//! Shared WAV sink for generated audiobook chunks.
//!
//! Backends generate audio differently, but saved audiobook chunks all need the
//! same boring file discipline: write a complete temp WAV, validate it, then
//! atomically commit it into the chunk cache.

use std::fs;
use std::path::Path;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use sherpa_onnx::write as write_wav_file;

use super::cache::wav_info;
use super::file_commit::commit_staged_file;

/// Timing/size result of writing one synthesized chunk to a file.
pub(super) struct FileSynthesisResult {
    pub(super) generate_ms: u128,
    pub(super) synthesis_ms: u128,
    pub(super) write_ms: u128,
    pub(super) validate_ms: u128,
    pub(super) audio_duration_sec: f32,
    pub(super) wav_bytes: usize,
}

/// Length of the silent WAV written for a chunk with no speakable text.
pub(super) const SILENT_PLACEHOLDER_SEC: f64 = 0.25;

/// Write, validate, and atomically commit a generated chunk WAV.
pub(super) fn commit_synthesis_wav<F>(
    generate_started: Instant,
    synthesis_ms: u128,
    output_path: &Path,
    write_temp_wav: F,
) -> Result<FileSynthesisResult, String>
where
    F: FnOnce(&Path) -> Result<f32, String>,
{
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "Failed to create native audiobook chunk dir {}: {err}",
                parent.display()
            )
        })?;
    }
    let temp_path = output_path.with_extension(format!(
        "{}.tmp",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("System clock error: {err}"))?
            .as_nanos()
    ));

    let write_started = Instant::now();
    let fallback_duration_sec = write_temp_wav(&temp_path).inspect_err(|_| {
        let _ = fs::remove_file(&temp_path);
    })?;
    let write_ms = write_started.elapsed().as_millis();

    let validate_started = Instant::now();
    let Some(info) = wav_info(&temp_path) else {
        let _ = fs::remove_file(&temp_path);
        return Err(format!("Generated invalid WAV {}", temp_path.display()));
    };
    commit_staged_file(&temp_path, output_path, "generated WAV")?;
    let validate_ms = validate_started.elapsed().as_millis();

    Ok(FileSynthesisResult {
        generate_ms: generate_started.elapsed().as_millis(),
        synthesis_ms,
        write_ms,
        validate_ms,
        audio_duration_sec: info.audio_duration_sec.max(fallback_duration_sec),
        wav_bytes: info.wav_bytes,
    })
}

/// Write [`SILENT_PLACEHOLDER_SEC`] of silence to `path` and return its duration.
pub(super) fn write_silent_placeholder(path: &Path, sample_rate: i32) -> Result<f32, String> {
    if sample_rate <= 0 {
        return Err(format!(
            "Engine reported a non-positive sample rate ({sample_rate}); cannot write a silent placeholder for {}",
            path.display()
        ));
    }
    let frame_count = (sample_rate as f64 * SILENT_PLACEHOLDER_SEC).round() as usize;
    let silence = vec![0f32; frame_count];
    if !write_wav_file(&path.display().to_string(), &silence, sample_rate) {
        return Err(format!(
            "Failed to write silent placeholder WAV {}",
            path.display()
        ));
    }
    Ok(frame_count as f32 / sample_rate as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silent_placeholder_passes_wav_validation() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("papercut-silent-{nonce}.wav"));
        let duration = write_silent_placeholder(&path, 24_000).expect("write silent placeholder");
        assert!(duration > 0.0 && duration.is_finite());

        let info = wav_info(&path).expect("silent placeholder must parse as WAV");
        assert!(info.audio_duration_sec > 0.0);
        let _ = fs::remove_file(&path);
    }
}
