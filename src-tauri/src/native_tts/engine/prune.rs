//! Disk cleanup for saved-audiobook chunk directories.
//!
//! The save loop writes content-addressed chunk filenames. When source text
//! changes, valid old WAVs no longer match the current chunk set, so pruning is
//! intentionally separate from cache scanning: invalid expected files are handled
//! by `scan_audiobook`, while this module removes valid-but-stale leftovers.

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::time::SystemTime;

use super::paths::chunk_path;
use crate::native_tts::types::NativeTtsInputChunk;

/// Remove chunk WAVs that no longer belong to the current chunk set.
///
/// Chunk filenames embed a content hash, so editing the source document and
/// re-saving into the same audiobook id writes new filenames while the stale
/// ones linger forever. In-flight `.tmp` staging files are skipped on purpose:
/// a concurrent save may still need them for its atomic rename.
pub(super) fn prune_orphan_chunk_files(dir: &Path, chunks: &[NativeTtsInputChunk]) {
    let expected: HashSet<std::ffi::OsString> = chunks
        .iter()
        .enumerate()
        .filter_map(|(index, chunk)| {
            chunk_path(dir, index, chunk)
                .file_name()
                .map(|name| name.to_os_string())
        })
        .collect();

    let Ok(entries) = fs::read_dir(dir.join("chunks")) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry
            .file_type()
            .map(|kind| kind.is_file())
            .unwrap_or(false)
        {
            continue;
        }
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "tmp") {
            continue;
        }
        if !expected.contains(&entry.file_name()) {
            let _ = fs::remove_file(path);
        }
    }
}

/// Reclaim abandoned `.tmp` chunk staging files from crashed earlier writes.
///
/// Only files last modified before `cutoff` (this job's start) are swept: a
/// concurrent save's in-flight temp is necessarily written after this job began,
/// so its commit rename is never disturbed. Temps with unreadable mtime are left
/// alone rather than risking a live write.
pub(super) fn prune_stale_temp_files(dir: &Path, cutoff: SystemTime) {
    let Ok(entries) = fs::read_dir(dir.join("chunks")) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry
            .file_type()
            .map(|kind| kind.is_file())
            .unwrap_or(false)
        {
            continue;
        }
        let path = entry.path();
        if !path.extension().is_some_and(|ext| ext == "tmp") {
            continue;
        }
        let modified = entry.metadata().ok().and_then(|meta| meta.modified().ok());
        if modified.is_some_and(|time| time < cutoff) {
            let _ = fs::remove_file(path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::UNIX_EPOCH;

    fn chunks() -> Vec<NativeTtsInputChunk> {
        vec![
            NativeTtsInputChunk {
                id: "a".into(),
                text: "First".into(),
                text_hash: Some("hash-a".into()),
                source_span: None,
            },
            NativeTtsInputChunk {
                id: "b".into(),
                text: "Second".into(),
                text_hash: Some("hash-b".into()),
                source_span: None,
            },
        ]
    }

    #[test]
    fn prune_removes_orphan_chunk_files_only() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("papercut-prune-orphans-{nonce}"));
        let chunks = chunks();
        fs::create_dir_all(dir.join("chunks")).expect("create chunks dir");

        // Write the expected WAV for each current chunk plus a leftover from an
        // earlier save of different source text (a different content hash).
        for (index, chunk) in chunks.iter().enumerate() {
            fs::write(chunk_path(&dir, index, chunk), b"wav").expect("write expected chunk");
        }
        let orphan = dir.join("chunks").join("00001-a-deadbeefdeadbeef.wav");
        fs::write(&orphan, b"stale").expect("write orphan chunk");
        // A concurrent save of the same audiobook id stages its chunk here mid-write.
        let in_flight = dir.join("chunks").join("00001-a-hash-a.123456789.tmp");
        fs::write(&in_flight, b"writing").expect("write in-flight temp");

        prune_orphan_chunk_files(&dir, &chunks);

        assert!(!orphan.exists(), "orphan chunk should be removed");
        assert!(
            in_flight.exists(),
            "in-flight temp must be left for the concurrent save's commit rename"
        );
        for (index, chunk) in chunks.iter().enumerate() {
            assert!(
                chunk_path(&dir, index, chunk).is_file(),
                "current chunk should be kept"
            );
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn prune_stale_temp_files_removes_only_pre_job_temps() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("papercut-prune-temps-{nonce}"));
        fs::create_dir_all(dir.join("chunks")).expect("create chunks dir");

        // An abandoned temp from a crashed earlier write, then the job-start mark,
        // then a concurrent save's in-flight temp staged after the job began.
        let stale = dir.join("chunks").join("00001-a-hash-a.111.tmp");
        fs::write(&stale, b"abandoned").expect("write stale temp");
        std::thread::sleep(std::time::Duration::from_millis(20));
        let cutoff = SystemTime::now();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let in_flight = dir.join("chunks").join("00002-b-hash-b.222.tmp");
        fs::write(&in_flight, b"writing").expect("write in-flight temp");
        // A committed WAV must be untouched by the temp sweep.
        let committed = dir.join("chunks").join("00001-a-hash-a.wav");
        fs::write(&committed, b"wav").expect("write committed chunk");

        prune_stale_temp_files(&dir, cutoff);

        assert!(!stale.exists(), "abandoned pre-job temp should be removed");
        assert!(
            in_flight.exists(),
            "a concurrent save's temp written after the job started must be kept"
        );
        assert!(committed.exists(), "committed WAVs must not be touched");
        let _ = fs::remove_dir_all(dir);
    }
}
