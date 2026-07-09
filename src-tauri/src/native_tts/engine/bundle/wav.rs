//! WAV stitching shared by bundle export and native playback preparation.
//!
//! Saved audiobooks are cached as many chunk WAVs. This module owns the one
//! reusable operation both export paths need: stream those chunks into one valid
//! RIFF/WAVE file without loading a multi-hour audiobook into memory.

use std::fs;
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use super::super::cache::{wav_metadata, WavMetadata};
use super::super::file_commit::commit_staged_file;
use super::super::paths::chunk_path;
use crate::native_tts::types::{NativeAudiobookPlaybackChunk, NativeTtsInputChunk};

/// Totals describing the single stitched WAV, threaded back up to callers.
pub(crate) struct WavExportSummary {
    pub(crate) chunks: usize,
    pub(crate) audio_duration_sec: f32,
    pub(crate) wav_bytes: usize,
    #[cfg_attr(target_os = "android", allow(dead_code))]
    pub(crate) chunk_timings: Vec<NativeAudiobookPlaybackChunk>,
}

/// Concatenate every saved chunk WAV into one valid WAV file at `output_path`.
///
/// WAV files are `RIFF` containers: a header, a `fmt ` chunk describing the
/// audio format, and a `data` chunk holding raw samples. To merge N files we
/// reuse the first file's `fmt ` block, then write one big `data` chunk that is
/// every input's samples back-to-back. We require all inputs to share the same
/// format and guard the 4 GB RIFF size limit. A completed same-directory staged
/// file replaces the destination only after every payload has been copied.
pub(crate) fn stitch_audiobook_wav(
    dir: &Path,
    chunks: &[NativeTtsInputChunk],
    output_path: &Path,
) -> Result<WavExportSummary, String> {
    if chunks.is_empty() {
        return Err("Cannot stitch an audiobook with no chunks".into());
    }

    let mut metas: Vec<(PathBuf, WavMetadata)> = Vec::with_capacity(chunks.len());
    let mut total_data_bytes = 0u64;
    let mut total_audio_duration_sec = 0f64;
    let mut chunk_timings = Vec::with_capacity(chunks.len());

    for (index, chunk) in chunks.iter().enumerate() {
        let path = chunk_path(dir, index, chunk);
        let metadata = wav_metadata(&path).ok_or_else(|| {
            format!(
                "Missing or invalid saved audiobook chunk {}/{}: {}",
                index + 1,
                chunks.len(),
                path.display()
            )
        })?;

        if let Some((_, first)) = metas.first() {
            if metadata.fmt_payload != first.fmt_payload {
                return Err(format!(
                    "Saved audiobook chunk {} has a different WAV format",
                    index + 1
                ));
            }
        }

        let duration_sec = metadata.precise_audio_duration_sec;
        chunk_timings.push(NativeAudiobookPlaybackChunk {
            index,
            chunk_id: chunk.id.clone(),
            start_sec: total_audio_duration_sec,
            duration_sec,
        });
        total_data_bytes += metadata.data_bytes as u64;
        total_audio_duration_sec += duration_sec;
        metas.push((path, metadata));
    }

    if total_data_bytes > u32::MAX as u64 {
        return Err("Exported WAV would exceed the 4 GB RIFF/WAV limit".into());
    }

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "Failed to create export directory {}: {err}",
                parent.display()
            )
        })?;
    }

    let temp_path = output_path.with_extension(format!(
        "wav.{}.tmp",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("System clock error: {err}"))?
            .as_nanos()
    ));
    let file = fs::File::create(&temp_path).map_err(|err| {
        format!(
            "Failed to create audiobook export {}: {err}",
            temp_path.display()
        )
    })?;
    let mut writer = BufWriter::new(file);

    let fmt_payload = &metas[0].1.fmt_payload;
    let fmt_padding = fmt_payload.len() % 2;
    let data_padding = total_data_bytes as usize % 2;
    let riff_size = 4u64
        + 8
        + fmt_payload.len() as u64
        + fmt_padding as u64
        + 8
        + total_data_bytes
        + data_padding as u64;
    if riff_size > u32::MAX as u64 {
        let _ = fs::remove_file(&temp_path);
        return Err("Exported WAV would exceed the 4 GB RIFF/WAV limit".into());
    }

    writer.write_all(b"RIFF").map_err(write_wav_err)?;
    writer
        .write_all(&(riff_size as u32).to_le_bytes())
        .map_err(write_wav_err)?;
    writer.write_all(b"WAVE").map_err(write_wav_err)?;
    writer.write_all(b"fmt ").map_err(write_wav_err)?;
    writer
        .write_all(&(fmt_payload.len() as u32).to_le_bytes())
        .map_err(write_wav_err)?;
    writer.write_all(fmt_payload).map_err(write_wav_err)?;
    if fmt_padding > 0 {
        writer.write_all(&[0]).map_err(write_wav_err)?;
    }

    writer.write_all(b"data").map_err(write_wav_err)?;
    writer
        .write_all(&(total_data_bytes as u32).to_le_bytes())
        .map_err(write_wav_err)?;

    for (path, metadata) in &metas {
        let mut input = fs::File::open(path)
            .map_err(|err| format!("Failed to open audiobook chunk {}: {err}", path.display()))?;
        input
            .seek(SeekFrom::Start(metadata.data_offset as u64))
            .map_err(|err| format!("Failed to seek audiobook chunk {}: {err}", path.display()))?;
        let copied = std::io::copy(&mut input.take(metadata.data_bytes as u64), &mut writer)
            .map_err(write_wav_err)?;
        if copied != metadata.data_bytes as u64 {
            let _ = fs::remove_file(&temp_path);
            return Err(format!(
                "Audiobook chunk {} ended before its WAV data payload",
                path.display()
            ));
        }
    }
    if data_padding > 0 {
        writer.write_all(&[0]).map_err(write_wav_err)?;
    }
    writer.flush().map_err(write_wav_err)?;
    drop(writer);

    commit_staged_file(&temp_path, output_path, "audiobook WAV")?;

    let wav_bytes = fs::metadata(output_path)
        .map_err(|err| format!("Failed to inspect audiobook export: {err}"))?
        .len() as usize;

    Ok(WavExportSummary {
        chunks: chunks.len(),
        audio_duration_sec: total_audio_duration_sec as f32,
        wav_bytes,
        chunk_timings,
    })
}

fn write_wav_err(err: std::io::Error) -> String {
    format!("Failed to write audiobook export: {err}")
}
