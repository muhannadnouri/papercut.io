//! Synthesis provenance for explicit restart of incompatible unfinished saves.
//!
//! The book writer lease covers validation, confirmed chunk deletion and generation.
//! Keep old provenance until deletion finishes: an interrupted reset must require
//! confirmation again, never adopt leftover old chunks as current-engine progress.

use std::fs;
use std::path::Path;

use sha2::{Digest, Sha256};

use super::cache::scan_audiobook;
use super::manifest::{read_manifest, NativeAudiobookManifest};
use super::models::{model_definition, TtsModelBackend, SILMA_MODEL_ID};
use super::silma_sidecar::DEFAULT_SILMA_NFE_STEP;
use crate::native_tts::types::NativeAudiobookSaveRequest;

// Change this when synthesis semantics change; cache IDs and completed audio stay stable.
const SHERPA_SYNTHESIS_REVISION: &str = "sherpa/1.13.4;frontend/embedded;normalization/1";

/// Version Sherpa's synthesis contract separately from cache IDs; SILMA keeps its existing resume rules.
pub(super) fn synthesis_revision(model_id: &str) -> Option<String> {
    model_definition(model_id).ok().and_then(|model| {
        (model.backend == TtsModelBackend::SherpaOnnx).then(|| SHERPA_SYNTHESIS_REVISION.into())
    })
}

/// Complete legacy WAV coverage is exempt from engine restart, even if its timing index needs repair.
fn complete_audio(dir: &Path, manifest: &NativeAudiobookManifest) -> bool {
    !manifest.chunks.is_empty()
        && scan_audiobook(dir, &manifest.chunks, false).cached_chunks == manifest.chunks.len()
}

/// NFE is meaningful only for SILMA; older manifests may omit it or store it for unrelated models.
fn settings_match(
    manifest: &NativeAudiobookManifest,
    request: &NativeAudiobookSaveRequest,
) -> bool {
    (manifest.audiobook_id.is_empty() || manifest.audiobook_id == request.audiobook_id)
        && manifest.model_id == request.model_id
        && manifest.voice == request.voice
        && manifest.speed == request.speed
        && manifest.text_preprocessor == request.text_preprocessor
        && (request.model_id != SILMA_MODEL_ID
            || manifest.silma_nfe_step.unwrap_or(DEFAULT_SILMA_NFE_STEP)
                == request.silma_nfe_step.unwrap_or(DEFAULT_SILMA_NFE_STEP))
}

/// Missing/different provenance gates unfinished Sherpa saves only; source-edit policy remains separate.
pub(super) fn required_restart_token(dir: &Path) -> Result<Option<String>, String> {
    let needs_restart = match read_manifest(dir) {
        Ok(manifest) => synthesis_revision(&manifest.model_id).is_some_and(|revision| {
            manifest.synthesis_revision.as_ref() != Some(&revision)
                && !complete_audio(dir, &manifest)
        }),
        Err(_) => {
            dir.exists()
                && fs::read_dir(dir)
                    .map_err(|err| err.to_string())?
                    .next()
                    .is_some()
        }
    };
    if needs_restart {
        Ok(Some(generation_token(dir)?))
    } else {
        Ok(None)
    }
}

/// Bind consent to manifest bytes, or entry metadata when missing; this is not a WAV-integrity checksum.
/// Cooperating writers must hold the book lease; payload integrity is validated separately.
/// For example, changing a manifest while the confirmation dialog is open invalidates that dialog's token.
fn generation_token(dir: &Path) -> Result<String, String> {
    let mut hash = Sha256::new();
    match fs::read(dir.join("manifest.json")) {
        Ok(bytes) => hash.update(bytes),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            let mut paths: Vec<_> = fs::read_dir(dir)
                .map_err(|err| err.to_string())?
                .map(|entry| entry.map(|entry| entry.path()))
                .collect::<Result<_, _>>()
                .map_err(|err| err.to_string())?;
            paths.sort();
            for path in paths {
                hash.update(path.file_name().unwrap().as_encoded_bytes());
                let metadata = fs::metadata(path).map_err(|err| err.to_string())?;
                hash.update(metadata.len().to_le_bytes());
                hash.update(format!(
                    "{:?}",
                    metadata.modified().map_err(|err| err.to_string())?
                ));
            }
        }
        Err(err) => return Err(format!("Failed to inspect retained audiobook: {err}")),
    }
    Ok(format!("{:x}", hash.finalize()))
}

/// Under the book writer lease, reject stale consent before intentionally discarding this job's old progress.
/// Delete chunks before stamping current provenance: a failed/interrupted reset cannot mix surviving old WAVs with new ones.
pub(super) fn prepare_save(dir: &Path, request: &NativeAudiobookSaveRequest) -> Result<(), String> {
    if let Ok(manifest) = read_manifest(dir) {
        if !settings_match(&manifest, request) {
            return Err("Audiobook settings do not match its retained generation".into());
        }
    }
    if let Some(token) = required_restart_token(dir)? {
        if request.restart_generation_token.as_deref() != Some(token.as_str()) {
            return Err(
                "Audiobook restart confirmation required; check the saved generation again".into(),
            );
        }
        let chunks = dir.join("chunks");
        if chunks.exists() {
            fs::remove_dir_all(&chunks)
                .map_err(|err| format!("Failed to clear unfinished audiobook progress: {err}"))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::super::manifest::{
        manifest_has_complete_index, read_or_rebuild_manifest_index, write_manifest,
        write_pending_manifest,
    };
    use super::super::models::DEFAULT_MODEL_ID;
    use super::super::paths::{chunk_path, stable_hex_hash};
    use super::*;
    use crate::native_tts::types::NativeTtsInputChunk;

    struct Fixture {
        root: PathBuf,
        dir: PathBuf,
        request: NativeAudiobookSaveRequest,
    }

    impl Fixture {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!("papercut-generation-{nonce}"));
            let request = NativeAudiobookSaveRequest {
                job_id: "job".into(),
                audiobook_id: "generation-fixture".into(),
                document_url: "/book.html".into(),
                title: "Book".into(),
                chunks: ["Hello world.", "Second sentence."]
                    .iter()
                    .enumerate()
                    .map(|(index, text)| NativeTtsInputChunk {
                        id: format!("c{index}"),
                        text: (*text).into(),
                        text_hash: None,
                        source_span: None,
                    })
                    .collect(),
                model_id: DEFAULT_MODEL_ID.into(),
                text_preprocessor: "none".into(),
                voice: "af_heart".into(),
                speed: 1.0,
                thread_count: Some(1),
                silma_nfe_step: None,
                restart_generation_token: None,
            };
            let dir = root.join(stable_hex_hash(&request.audiobook_id));
            fs::create_dir_all(dir.join("chunks")).unwrap();
            let fixture = Self { root, dir, request };
            write_pending_manifest(&fixture.dir, &fixture.request, &fixture.request.chunks)
                .unwrap();
            fixture
        }

        /// Minimal valid PCM for persistence assertions, not speech-quality evidence.
        fn wav(&self, dir: &Path, index: usize, sample: u8) {
            fs::create_dir_all(dir.join("chunks")).unwrap();
            let mut wav = Vec::new();
            wav.extend_from_slice(b"RIFF");
            wav.extend_from_slice(&40u32.to_le_bytes());
            wav.extend_from_slice(b"WAVEfmt ");
            wav.extend_from_slice(&16u32.to_le_bytes());
            wav.extend_from_slice(&[1, 0, 1, 0, 0x40, 0x1f, 0, 0, 0x80, 0x3e, 0, 0, 2, 0, 16, 0]);
            wav.extend_from_slice(b"data");
            wav.extend_from_slice(&4u32.to_le_bytes());
            wav.extend_from_slice(&[sample; 4]);
            fs::write(chunk_path(dir, index, &self.request.chunks[index]), wav).unwrap();
        }

        /// Remove only provenance to exercise prior schema-2 defaults without changing source/audio identity.
        fn legacy(&self) {
            let path = self.dir.join("manifest.json");
            let mut json: serde_json::Value =
                serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            json.as_object_mut().unwrap().remove("synthesisRevision");
            fs::write(path, serde_json::to_vec(&json).unwrap()).unwrap();
        }

        fn authorize(&mut self) {
            self.request.restart_generation_token =
                Some(required_restart_token(&self.dir).unwrap().unwrap());
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    /// Compare retained bytes, including derived playback; file counts would miss destructive overwrites.
    fn snapshot(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        fn visit(root: &Path, dir: &Path, files: &mut Vec<(PathBuf, Vec<u8>)>) {
            for entry in fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    visit(root, &path, files);
                } else {
                    files.push((
                        path.strip_prefix(root).unwrap().to_path_buf(),
                        fs::read(path).unwrap(),
                    ));
                }
            }
        }
        let mut files = Vec::new();
        visit(dir, dir, &mut files);
        files.sort();
        files
    }

    #[test]
    fn restart_requires_current_consent_and_cancel_keeps_every_byte() {
        let mut fixture = Fixture::new();
        fixture.wav(&fixture.dir, 0, 1);
        fixture.legacy();
        let original = snapshot(&fixture.dir);
        assert!(prepare_save(&fixture.dir, &fixture.request)
            .unwrap_err()
            .contains("restart confirmation required"));
        assert_eq!(snapshot(&fixture.dir), original);
        fixture.authorize();
        let path = fixture.dir.join("manifest.json");
        let mut bytes = fs::read(&path).unwrap();
        bytes.push(b'\n');
        fs::write(&path, bytes).unwrap();
        let changed = snapshot(&fixture.dir);
        assert!(prepare_save(&fixture.dir, &fixture.request).is_err());
        assert_eq!(snapshot(&fixture.dir), changed);
        fixture.authorize();
        prepare_save(&fixture.dir, &fixture.request).unwrap();
        assert!(!fixture.dir.join("chunks").exists());
        assert!(read_manifest(&fixture.dir)
            .unwrap()
            .synthesis_revision
            .is_none());
        assert_eq!(
            fs::read(path).unwrap(),
            changed
                .iter()
                .find(|(p, _)| p == Path::new("manifest.json"))
                .unwrap()
                .1
        );
    }

    #[test]
    fn confirmed_restart_drops_old_chunks_and_new_interrupted_progress_resumes_normally() {
        let mut fixture = Fixture::new();
        fixture.wav(&fixture.dir, 0, 1);
        fixture.legacy();
        fixture.authorize();
        prepare_save(&fixture.dir, &fixture.request).unwrap();
        fs::create_dir_all(fixture.dir.join("chunks")).unwrap();
        write_pending_manifest(&fixture.dir, &fixture.request, &fixture.request.chunks).unwrap();
        assert_eq!(
            scan_audiobook(&fixture.dir, &fixture.request.chunks, false).cached_chunks,
            0
        );
        fixture.wav(&fixture.dir, 1, 42);
        fixture.request.restart_generation_token = None;
        let current = snapshot(&fixture.dir);
        assert!(required_restart_token(&fixture.dir).unwrap().is_none());
        prepare_save(&fixture.dir, &fixture.request).unwrap();
        assert_eq!(snapshot(&fixture.dir), current);
        assert_eq!(
            scan_audiobook(&fixture.dir, &fixture.request.chunks, false).cached_chunks,
            1
        );
    }

    #[test]
    fn interrupted_or_failed_reset_keeps_old_provenance_until_chunk_removal_finishes() {
        let mut fixture = Fixture::new();
        fixture.wav(&fixture.dir, 0, 1);
        fixture.legacy();
        fs::remove_file(chunk_path(&fixture.dir, 0, &fixture.request.chunks[0])).unwrap();
        assert!(prepare_save(&fixture.dir, &fixture.request).is_err());
        fixture.authorize();
        fs::remove_dir_all(fixture.dir.join("chunks")).unwrap();
        fs::write(
            fixture.dir.join("chunks"),
            b"invalid directory blocks reset",
        )
        .unwrap();
        let original = snapshot(&fixture.dir);
        assert!(prepare_save(&fixture.dir, &fixture.request)
            .unwrap_err()
            .contains("Failed to clear"));
        assert_eq!(snapshot(&fixture.dir), original);
        assert!(read_manifest(&fixture.dir)
            .unwrap()
            .synthesis_revision
            .is_none());
    }

    #[test]
    fn completed_legacy_audio_ignores_stale_restart_consent_and_preserves_provenance_on_repair() {
        let mut fixture = Fixture::new();
        fixture.wav(&fixture.dir, 0, 1);
        fixture.legacy();
        fixture.authorize();
        // A job may finish after the dialog opens; even that earlier confirmation must not erase completed audio.
        fixture.wav(&fixture.dir, 1, 1);
        let original = snapshot(&fixture.dir);
        assert!(required_restart_token(&fixture.dir).unwrap().is_none());
        prepare_save(&fixture.dir, &fixture.request).unwrap();
        assert_eq!(snapshot(&fixture.dir), original);
        let manifest = read_or_rebuild_manifest_index(&fixture.dir).unwrap();
        assert!(manifest.synthesis_revision.is_none());
        assert!(manifest_has_complete_index(&manifest));
        prepare_save(&fixture.dir, &fixture.request).unwrap();
        for (path, bytes) in original
            .into_iter()
            .filter(|(path, _)| path.extension().is_some_and(|ext| ext == "wav"))
        {
            assert_eq!(fs::read(fixture.dir.join(path)).unwrap(), bytes);
        }
    }

    #[test]
    fn current_revision_and_silma_partials_resume_without_restart() {
        for model_id in [DEFAULT_MODEL_ID, SILMA_MODEL_ID] {
            let mut fixture = Fixture::new();
            fixture.request.model_id = model_id.into();
            write_pending_manifest(&fixture.dir, &fixture.request, &fixture.request.chunks)
                .unwrap();
            fixture.wav(&fixture.dir, 0, 1);
            if model_id == SILMA_MODEL_ID {
                fixture.request.silma_nfe_step = Some(DEFAULT_SILMA_NFE_STEP);
            }
            let original = snapshot(&fixture.dir);
            assert!(required_restart_token(&fixture.dir).unwrap().is_none());
            prepare_save(&fixture.dir, &fixture.request).unwrap();
            assert_eq!(snapshot(&fixture.dir), original);
        }
    }

    #[test]
    fn different_revision_and_damaged_caches_require_consent_before_reset() {
        for variant in ["different", "missing", "malformed"] {
            let mut fixture = Fixture::new();
            fixture.wav(&fixture.dir, 0, 1);
            let path = fixture.dir.join("manifest.json");
            match variant {
                "different" => {
                    let mut json: serde_json::Value =
                        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                    json["synthesisRevision"] = "sherpa/older-frontend".into();
                    fs::write(&path, serde_json::to_vec(&json).unwrap()).unwrap();
                }
                "missing" => fs::remove_file(&path).unwrap(),
                _ => fs::write(&path, b"invalid JSON").unwrap(),
            }
            let original = snapshot(&fixture.dir);
            assert!(prepare_save(&fixture.dir, &fixture.request).is_err());
            assert_eq!(snapshot(&fixture.dir), original);
            fixture.authorize();
            prepare_save(&fixture.dir, &fixture.request).unwrap();
            assert!(!fixture.dir.join("chunks").exists());
        }
    }

    #[test]
    fn completed_source_edits_are_outside_engine_restart_and_wrong_settings_cannot_clear_a_job() {
        let mut fixture = Fixture::new();
        for index in 0..fixture.request.chunks.len() {
            fixture.wav(&fixture.dir, index, 1);
        }
        write_manifest(
            &fixture.dir,
            &fixture.request,
            &fixture.request.chunks,
            1,
            None,
        )
        .unwrap();
        let original = snapshot(&fixture.dir);
        fixture.request.chunks[0].text = "Edited text.".into();
        assert!(required_restart_token(&fixture.dir).unwrap().is_none());
        prepare_save(&fixture.dir, &fixture.request).unwrap();
        assert_eq!(snapshot(&fixture.dir), original);
        fixture.request.voice = "other-voice".into();
        assert!(prepare_save(&fixture.dir, &fixture.request).is_err());
        assert_eq!(snapshot(&fixture.dir), original);
    }
}
