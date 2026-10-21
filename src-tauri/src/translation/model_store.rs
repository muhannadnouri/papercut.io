//! Translation model manifest and cache path helpers.
//!
//! This layer mirrors the TTS model-store boundary. Installation remains
//! separate from inference so the UI never claims translation is ready before
//! a native engine can load the verified model.

use std::fs;
use std::path::{Path, PathBuf};

use tauri::{Manager, Runtime};

use super::models::TranslationModelDefinition;

pub(super) mod legacy_alignment;
pub(super) mod verified;

#[derive(Clone, Copy, Debug)]
pub(crate) struct TranslationModelManifest {
    pub(crate) model_id: &'static str,
    pub(crate) directory_name: &'static str,
    pub(crate) source_label: &'static str,
    pub(crate) source_url: &'static str,
    /// Read only by the feature-gated downloader when building resolve URLs.
    #[cfg_attr(not(feature = "native-translation"), allow(dead_code))]
    pub(crate) revision: &'static str,
    pub(crate) files: &'static [TranslationModelFile],
    pub(crate) installable: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct TranslationModelFile {
    pub(crate) path: &'static str,
    /// Explicit pinned attachment URL; native models retain revision-based Hugging Face URLs.
    pub(crate) download_url: Option<&'static str>,
    pub(crate) bytes: u64,
    /// Checked by the downloader and again when worker assets cross binary IPC.
    #[cfg_attr(not(feature = "native-translation"), allow(dead_code))]
    pub(crate) sha256: &'static str,
}

impl TranslationModelManifest {
    /// Validate only models that have an installer-backed file manifest.
    ///
    /// Candidate-only rows intentionally return false even if a developer has a
    /// similarly named folder on disk. That prevents accidental local files from
    /// making the UI claim a model is installed before the manifest has a real
    /// URL, checksum, and required-file list.
    pub(crate) fn has_required_files(self, dir: &Path) -> bool {
        self.validate().is_ok()
            && fs::symlink_metadata(dir).is_ok_and(|m| m.is_dir())
            && self.files.iter().all(|file| {
                fs::symlink_metadata(dir.join(file.path))
                    .is_ok_and(|m| m.is_file() && m.len() == file.bytes)
            })
    }

    /// Validate trusted inventory before constructing installer paths or summing download sizes.
    pub(crate) fn validate(self) -> Result<(), String> {
        let mut paths = std::collections::BTreeSet::new();
        if !self.installable || !verified::valid_name(self.directory_name) || self.files.is_empty()
        {
            return Err("Invalid translation model inventory".into());
        }
        self.files.iter().try_fold(0u64, |total, pin| {
            if !verified::valid_name(pin.path)
                || !paths.insert(pin.path)
                || pin.bytes == 0
                || pin.bytes == u64::MAX
                || pin.sha256.len() != 64
                || !pin.sha256.bytes().all(|b| b.is_ascii_hexdigit())
            {
                return Err("Invalid translation model file inventory".to_string());
            }
            if let Some(bytes) = self.bundled_file(pin.path) {
                use sha2::{Digest, Sha256};
                if bytes.len() as u64 != pin.bytes
                    || format!("{:x}", Sha256::digest(bytes)) != pin.sha256
                {
                    return Err("Bundled model notice failed verification".into());
                }
            }
            total
                .checked_add(pin.bytes)
                .ok_or_else(|| "Translation model size overflow".into())
        })?;
        Ok(())
    }

    /// Role comes from trusted compiled lookup, never a caller's path, URL or claimed engine name.
    pub(crate) fn is_auxiliary(self) -> bool {
        legacy_alignment::manifest_by_id(self.model_id).is_some()
    }

    /// Resolve bundled legal files only for the approved auxiliary key; native downloads stay unchanged.
    pub(crate) fn bundled_file(self, path: &str) -> Option<&'static [u8]> {
        if self.is_auxiliary() {
            return legacy_alignment::notice_bytes(path);
        }
        let _ = path;
        None
    }

    /// Auxiliary status includes all hashes and notices; ordinary native status remains inexpensive.
    pub(crate) fn is_installed(self, dir: &Path) -> bool {
        self.has_required_files(dir)
            && (!self.is_auxiliary() || self.verify_files(dir).is_ok())
            && (super::bergamot::assets::manifest_by_id(self.model_id).is_none()
                || super::bergamot::assets::verify_install(dir, self).is_ok())
    }

    /// Stream hashes at install/repair admission; routine native status avoids hashing gigabytes.
    pub(crate) fn verify_files(self, dir: &Path) -> Result<(), String> {
        self.validate()?;
        for pin in self.files {
            verified::verify(dir, pin, pin.bytes)
                .map_err(|_| format!("Translation model file {} failed verification", pin.path))?;
        }
        Ok(())
    }

    pub(crate) fn total_bytes(self) -> u64 {
        self.files.iter().map(|file| file.bytes).sum()
    }
}

/// Resolve catalog models or the retired resource key, without adding it to engine choices.
pub(crate) fn manifest_by_id(id: &str) -> Option<TranslationModelManifest> {
    if let Some(manifest) = legacy_alignment::manifest_by_id(id) {
        return Some(manifest);
    }
    super::models::find_planned_model(id).map(manifest_for)
}

/// Convert catalog metadata into the install manifest shape.
///
/// Runnable models reuse pinned Hugging Face revisions or Mozilla attachment
/// manifests. Candidate-only rows intentionally stay empty so they cannot be
/// mistaken for supported downloads.
pub(crate) fn manifest_for(model: TranslationModelDefinition) -> TranslationModelManifest {
    if let Some(manifest) = super::bergamot::assets::manifest_by_id(model.id) {
        return manifest;
    }
    if model.id == "opus-mt-es-en-ctranslate2" {
        return TranslationModelManifest {
            model_id: model.id,
            directory_name: model.id,
            source_label: "michaelfeil/ct2fast-opus-mt-es-en",
            source_url: "https://huggingface.co/michaelfeil/ct2fast-opus-mt-es-en/tree/437f5ffc6c8544943c685ea405650e0d17cf6098",
            revision: "437f5ffc6c8544943c685ea405650e0d17cf6098",
            files: OPUS_MT_ES_EN_CT2_FILES,
            installable: true,
        };
    }

    if model.id == "opus-mt-fr-en-ctranslate2" {
        return TranslationModelManifest {
            model_id: model.id,
            directory_name: model.id,
            source_label: "michaelfeil/ct2fast-opus-mt-fr-en",
            source_url: "https://huggingface.co/michaelfeil/ct2fast-opus-mt-fr-en/tree/cb3b2d680bf35591a508d8479e2c99c44e281ef3",
            revision: "cb3b2d680bf35591a508d8479e2c99c44e281ef3",
            files: OPUS_MT_FR_EN_CT2_FILES,
            installable: true,
        };
    }

    if model.id == "hy-mt2-1.8b-q8" {
        return TranslationModelManifest {
            model_id: model.id,
            directory_name: model.id,
            source_label: "tencent/Hy-MT2-1.8B-GGUF",
            source_url: "https://huggingface.co/tencent/Hy-MT2-1.8B-GGUF/tree/1cd5208700acedef4ef93019b6cfc148b8522d45",
            revision: "1cd5208700acedef4ef93019b6cfc148b8522d45",
            files: HY_MT2_1_8B_Q8_FILES,
            installable: true,
        };
    }

    TranslationModelManifest {
        model_id: model.id,
        directory_name: model.id,
        source_label: model.name,
        source_url: "",
        revision: "",
        files: &[],
        installable: false,
    }
}

/// Permanent location for verified translation models.
///
/// Translation models are separate from TTS models and generated translated
/// documents: `<app-data>/translation/models/{model-id}`. Keeping these roots
/// split makes future cleanup and per-feature storage accounting simpler.
pub(crate) fn installed_translation_model_dir<R: Runtime>(
    app: &tauri::AppHandle<R>,
    manifest: TranslationModelManifest,
) -> Result<PathBuf, String> {
    Ok(translation_models_root(app)?.join(manifest.directory_name))
}

/// Legacy cache staging, retained for cleanup of older installs only.
/// New downloads stage beside their final directory to avoid cross-volume promotion.
pub(crate) fn translation_model_work_dir<R: Runtime>(
    app: &tauri::AppHandle<R>,
    manifest: TranslationModelManifest,
) -> Result<PathBuf, String> {
    let cache_dir = app
        .path()
        .app_cache_dir()
        .or_else(|_| app.path().app_data_dir())
        .map_err(|err| {
            format!("Failed to resolve cache dir for offline translation model install: {err}")
        })?;
    Ok(cache_dir
        .join("translation-model-installer")
        .join(manifest.directory_name))
}

pub(crate) fn resolve_translation_model_dir<R: Runtime>(
    app: &tauri::AppHandle<R>,
    manifest: TranslationModelManifest,
) -> Result<PathBuf, String> {
    let model_dir = installed_translation_model_dir(app, manifest)?;
    if manifest.is_installed(&model_dir) {
        return Ok(model_dir);
    }

    Err(format!(
        "Offline translation model {} is not installed. Checked: {}",
        manifest.model_id,
        model_dir.display()
    ))
}

pub(crate) fn directory_size(path: &Path) -> Result<u64, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(format!("Failed to inspect translation model storage: {e}")),
    };
    // Never follow a replaced cache/model entry into unrelated files or a directory cycle.
    if metadata.is_symlink() || (!metadata.is_file() && !metadata.is_dir()) {
        return Err("Invalid translation model storage type".into());
    }
    if metadata.is_file() {
        return Ok(metadata.len());
    }

    let mut total = 0u64;
    for entry in fs::read_dir(path).map_err(|err| {
        format!(
            "Failed to read translation model storage {}: {err}",
            path.display()
        )
    })? {
        let entry =
            entry.map_err(|err| format!("Failed to inspect translation model file: {err}"))?;
        total = total
            .checked_add(directory_size(&entry.path())?)
            .ok_or("Model storage size overflow")?;
    }
    Ok(total)
}

fn translation_models_root<R: Runtime>(app: &tauri::AppHandle<R>) -> Result<PathBuf, String> {
    let app_data = app.path().app_data_dir().map_err(|err| {
        format!("Failed to resolve app data dir for offline translation model: {err}")
    })?;
    Ok(app_data.join("translation").join("models"))
}

const OPUS_MT_ES_EN_CT2_FILES: &[TranslationModelFile] = &[
    TranslationModelFile {
        download_url: None,
        path: "config.json",
        bytes: 159,
        sha256: "0c2f6fa2057c7264d052fb4a62ba3476eeae70487acddfa8e779a53a00cbf44c",
    },
    TranslationModelFile {
        download_url: None,
        path: "generation_config.json",
        bytes: 293,
        sha256: "524dc91570823feb51fbd33a35e5aee3050a4441bdf65e66f6f5e945e5838e40",
    },
    TranslationModelFile {
        download_url: None,
        path: "model.bin",
        bytes: 155_502_501,
        sha256: "3a3b91dcb396ee7b682554e7d9f501909385c48b478a691bfe9bf9e3e32d3656",
    },
    TranslationModelFile {
        download_url: None,
        path: "shared_vocabulary.txt",
        bytes: 666_435,
        sha256: "77aee99211b7b8e569e0fb5b95dac01aba9f31bca2d1380b1fc6050797825ec6",
    },
    TranslationModelFile {
        download_url: None,
        path: "source.spm",
        bytes: 825_924,
        sha256: "e236ee6d866b635c0142114f8647f39831f9d92534aa2aad75c942f6a78ad0e3",
    },
    TranslationModelFile {
        download_url: None,
        path: "target.spm",
        bytes: 801_636,
        sha256: "4dd547c24816a335e7b0b2e63376a8f1b3cbfc671eda5ab808dd44fdadaa8791",
    },
    TranslationModelFile {
        download_url: None,
        path: "tokenizer_config.json",
        bytes: 44,
        sha256: "dd009536f1cd954180a3004be20e9963eaf3d68b485d062ea1b90d3b9ded2bc5",
    },
    TranslationModelFile {
        download_url: None,
        path: "vocab.json",
        bytes: 1_590_040,
        sha256: "257f346d7a6b2ecceafcca8ba05648ce2fd68dfaf105fb0e913dca7198f3f6d5",
    },
];

const OPUS_MT_FR_EN_CT2_FILES: &[TranslationModelFile] = &[
    TranslationModelFile {
        download_url: None,
        path: "config.json",
        bytes: 159,
        sha256: "0c2f6fa2057c7264d052fb4a62ba3476eeae70487acddfa8e779a53a00cbf44c",
    },
    TranslationModelFile {
        download_url: None,
        path: "generation_config.json",
        bytes: 293,
        sha256: "4cf5099aea4b599387562f1e52b8b872b997a51b64203647864239c3979ef17b",
    },
    TranslationModelFile {
        download_url: None,
        path: "model.bin",
        bytes: 149_872_839,
        sha256: "c32de81be5de6b9a5d03298173c724f02249371fb7ed6b6fd1ee7501578fe7fc",
    },
    TranslationModelFile {
        download_url: None,
        path: "shared_vocabulary.txt",
        bytes: 556_777,
        sha256: "e87446c025bbe57cc4f9ef90fa6e28ab303a2e31479b7355c2a2d3a6a3a43a0a",
    },
    TranslationModelFile {
        download_url: None,
        path: "source.spm",
        bytes: 802_397,
        sha256: "78d0e717c77053f1c4b856d8661d9cb87c64f083a35418c087b9146300e4f585",
    },
    TranslationModelFile {
        download_url: None,
        path: "target.spm",
        bytes: 778_395,
        sha256: "173e9f493a668fe396d599e28d414a201193094e6ffd7a4678e5aab0f6d3d838",
    },
    TranslationModelFile {
        download_url: None,
        path: "tokenizer_config.json",
        bytes: 42,
        sha256: "47de9ce87378593016432f8dc657202c03913ab3ce0c15d7f78d51edfc3ff9a3",
    },
    TranslationModelFile {
        download_url: None,
        path: "vocab.json",
        bytes: 1_339_166,
        sha256: "945c604346ce15ce4aff9001001e7f925e336d942c4087017f191871162cbdc4",
    },
];

const HY_MT2_1_8B_Q8_FILES: &[TranslationModelFile] = &[TranslationModelFile {
    download_url: None,
    path: "Hy-MT2-1.8B-Q8_0.gguf",
    bytes: 1_908_528_192,
    sha256: "5c3fe0b1408a5ceb0143184ef247b11b579c525f4b02b060e6c851bb76fef1a4",
}];

#[cfg(test)]
mod tests {
    use super::manifest_for;
    use crate::translation::models::find_planned_model;

    #[test]
    fn pinned_ct2_manifest_is_installable_after_downloader_lands() {
        let model = find_planned_model("opus-mt-es-en-ctranslate2").expect("model");
        let manifest = manifest_for(model);

        assert!(manifest.installable);
        assert_eq!(
            manifest.revision,
            "437f5ffc6c8544943c685ea405650e0d17cf6098"
        );
        assert!(manifest.source_url.contains(manifest.revision));
        assert_eq!(manifest.files.len(), 8);
        assert_eq!(manifest.total_bytes(), 159_387_032);
    }

    #[test]
    fn pinned_hy_mt2_manifest_is_installable() {
        let model = find_planned_model("hy-mt2-1.8b-q8").expect("model");
        let manifest = manifest_for(model);

        assert!(manifest.installable);
        assert!(manifest.source_url.contains(manifest.revision));
        assert_eq!(manifest.files.len(), 1);
        assert_eq!(manifest.total_bytes(), 1_908_528_192);
    }

    #[test]
    fn later_quality_candidates_remain_empty_manifests() {
        let model = find_planned_model("qwen3-8b").expect("model");
        let manifest = manifest_for(model);

        assert!(!manifest.installable);
        assert!(manifest.source_url.is_empty());
        assert!(manifest.revision.is_empty());
        assert!(manifest.files.is_empty());
    }
}
