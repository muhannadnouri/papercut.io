//! Trusted auxiliary install pins; separate from the translation engine catalog.
use std::path::Path;

use super::AdmissionError;
use crate::translation::model_store::{verified, TranslationModelFile, TranslationModelManifest};

pub(super) const REPOSITORY: &str = "cstr/awesome-align-onnx-int8";
pub(super) const REVISION: &str = "e803a2f501474044099b539160aaca461976ecd0";
pub(super) const TOKENIZER_ID: &str = "mbert-raw-v1:tokenizers-0.23.2:bf1b59b7b11c95f194f51708d918eea378e09d05f84c0e1656dc5180e8117088";
pub(in crate::translation) const MODEL_KEY: &str = "alignment-awesome-align-int8";
const PACK_LIMIT: u64 = 200 * 1024 * 1024;

// Raw developer admission keeps these seven pins; installer admission additionally requires both notices.
pub(super) const FILES: &[TranslationModelFile] = &[
    TranslationModelFile {
        path: "model.onnx",
        download_url: None,
        bytes: 149709873,
        sha256: "beb17c2e9520155f7da7bb94a1fb8e74f5e528acdbd8f4415854c99a3d84f429",
    },
    TranslationModelFile {
        path: "tokenizer.json",
        download_url: None,
        bytes: 2919362,
        sha256: "bf1b59b7b11c95f194f51708d918eea378e09d05f84c0e1656dc5180e8117088",
    },
    TranslationModelFile {
        path: "config.json",
        download_url: None,
        bytes: 753,
        sha256: "6a08ece128ddd8c14d9117565ec27416d6da7d574ba24121c3d19468305311c9",
    },
    TranslationModelFile {
        path: "tokenizer_config.json",
        download_url: None,
        bytes: 1273,
        sha256: "470cff6e0353b08e2a6e9b4f61729ecdc47ccb3ced335fa5520e9ce334572d59",
    },
    TranslationModelFile {
        path: "special_tokens_map.json",
        download_url: None,
        bytes: 125,
        sha256: "b6d346be366a7d1d48332dbc9fdf3bf8960b5d879522b7799ddba59e76237ee3",
    },
    TranslationModelFile {
        path: "vocab.txt",
        download_url: None,
        bytes: 995526,
        sha256: "fe0fda7c425b48c516fc8f160d594c8022a0808447475c1a7c6d6479763f310c",
    },
    TranslationModelFile {
        path: "README.md",
        download_url: None,
        bytes: 11927,
        sha256: "e358416e1e88ba918f506b4988ad6ec29d85c0964bd988d9e6cd9473c24df9fa",
    },
];

const INSTALL_FILES: &[TranslationModelFile] = &[
    FILES[0],
    FILES[1],
    FILES[2],
    FILES[3],
    FILES[4],
    FILES[5],
    FILES[6],
    TranslationModelFile {
        path: "LICENSE-APACHE-2.0.txt",
        download_url: None,
        bytes: 11358,
        sha256: "cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30",
    },
    TranslationModelFile {
        path: "ATTRIBUTION.md",
        download_url: None,
        bytes: 2235,
        sha256: "071418670638baed08e3932a375dec40fb374ec4073c076dc95db1b9f625b45f",
    },
];

/// Only this feature-gated key grants auxiliary install admission; it never enters models.rs.
pub(in crate::translation) fn manifest_by_id(id: &str) -> Option<TranslationModelManifest> {
    (id == MODEL_KEY).then_some(TranslationModelManifest {
        model_id: MODEL_KEY, directory_name: MODEL_KEY, source_label: REPOSITORY,
        source_url: "https://huggingface.co/cstr/awesome-align-onnx-int8/tree/e803a2f501474044099b539160aaca461976ecd0",
        revision: REVISION, files: INSTALL_FILES, installable: true,
    })
}

/// App-owned legal bytes need no hosting service or network request; staged hashes still verify them.
pub(in crate::translation) fn notice_bytes(path: &str) -> Option<&'static [u8]> {
    match path {
        "LICENSE-APACHE-2.0.txt" => Some(include_bytes!("notices/LICENSE-APACHE-2.0.txt")),
        "ATTRIBUTION.md" => Some(include_bytes!("notices/ATTRIBUTION.md")),
        _ => None,
    }
}

/// Verify every required local file; retain only the tokenizer snapshot, not the weights.
/// The runtime separately verifies the graph bytes it loads under a model-use lease.
pub(super) fn tokenizer_snapshot(dir: &Path) -> Result<Vec<u8>, AdmissionError> {
    checked_snapshot(dir, FILES)
}

/// Check inventory bounds before I/O; missing later files must invalidate even a valid tokenizer snapshot.
fn checked_snapshot(dir: &Path, files: &[TranslationModelFile]) -> Result<Vec<u8>, AdmissionError> {
    snapshots(dir, files, false, &|| false).map(|(tokenizer, _)| tokenizer)
}

/// Runtime loads exactly these verified bytes, not a subsequently reopened filename.
#[cfg(feature = "native-translation-alignment-runtime")]
pub(super) fn runtime_snapshot(
    dir: &Path,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<u8>, Vec<u8>), AdmissionError> {
    snapshots(dir, FILES, true, cancelled)
}

/// Installed jobs require legal files in the same cancellable admission pass as the graph bytes.
#[cfg(feature = "native-translation-alignment-runtime")]
pub(super) fn installed_runtime_snapshot(
    dir: &Path,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<u8>, Vec<u8>), AdmissionError> {
    snapshots(dir, INSTALL_FILES, true, cancelled)
}

/// One inventory pass; only runtime admission retains weights, and no partial pack escapes.
fn snapshots(
    dir: &Path,
    files: &[TranslationModelFile],
    retain_weights: bool,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<u8>, Vec<u8>), AdmissionError> {
    let total = files
        .iter()
        .try_fold(0u64, |sum, p| sum.checked_add(p.bytes));
    if total.is_none_or(|n| n > PACK_LIMIT) {
        return Err(AdmissionError::Assets(verified::AssetError::Size));
    }
    let mut tokenizer = None;
    let mut weights = Vec::new();
    for pin in files {
        if cancelled() {
            return Err(verified::AssetError::Cancelled.into());
        }
        if retain_weights && pin.path == "model.onnx" {
            weights = verified::read_cancellable(dir, pin, PACK_LIMIT, cancelled)?;
            continue;
        }
        if pin.path == "tokenizer.json" {
            if tokenizer.is_some() {
                return Err(AdmissionError::Profile);
            }
            tokenizer = Some(verified::read(dir, pin, 4 * 1024 * 1024)?);
        } else {
            verified::verify(dir, pin, PACK_LIMIT)?;
        }
    }
    if cancelled() {
        return Err(verified::AssetError::Cancelled.into());
    }
    Ok((tokenizer.ok_or(AdmissionError::Profile)?, weights))
}

#[cfg(test)]
mod tests;
