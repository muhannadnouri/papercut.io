//! Pinned Bergamot binary delivery. No document writes or caller-supplied paths.

use std::path::Path;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::translation::model_store::{TranslationModelFile, TranslationModelManifest};

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Asset {
    Wasm,
    Model,
    Lex,
    Vocab,
}

impl Asset {
    fn path(self) -> &'static str {
        match self {
            Self::Wasm => "runtime.wasm",
            Self::Model => "model.bin",
            Self::Lex => "lex.bin",
            Self::Vocab => "vocab.spm",
        }
    }
}

pub(crate) fn manifest_by_id(id: &str) -> Option<TranslationModelManifest> {
    let (id, files) = match id {
        "bergamot-fr-en-worker" => ("bergamot-fr-en-worker", FR_FILES),
        "bergamot-es-en-worker" => ("bergamot-es-en-worker", ES_FILES),
        _ => return None,
    };
    Some(TranslationModelManifest {
        model_id: id,
        directory_name: id,
        source_label: "Mozilla Remote Settings (Bergamot)",
        source_url: "https://firefox-source-docs.mozilla.org/toolkit/components/translations/resources/03_bergamot.html",
        revision: "wasm-3.0-models-2.0",
        files,
        installable: true,
    })
}

pub(crate) fn manifest_for_language(language: &str) -> Result<TranslationModelManifest, String> {
    let id = match language {
        "fr" => "bergamot-fr-en-worker",
        "es" => "bergamot-es-en-worker",
        _ => return Err("Bergamot worker does not support this language pair".into()),
    };
    manifest_by_id(id).ok_or_else(|| "Bergamot worker manifest is unavailable".into())
}

/// Presence alone cannot make a corrupt or replaced worker install appear valid.
pub(crate) fn verify_install(dir: &Path, manifest: TranslationModelManifest) -> Result<(), String> {
    for pin in manifest.files {
        read_verified(dir, pin)?;
    }
    Ok(())
}

/// Read one bounded, immutable snapshot; the worker independently verifies it again.
pub(crate) fn read_asset(
    dir: &Path,
    manifest: TranslationModelManifest,
    asset: Asset,
) -> Result<Vec<u8>, String> {
    let pin = manifest
        .files
        .iter()
        .find(|pin| pin.path == asset.path())
        .ok_or("Unsupported Bergamot asset")?;
    read_verified(dir, pin)
}

fn read_verified(dir: &Path, pin: &TranslationModelFile) -> Result<Vec<u8>, String> {
    use crate::translation::model_store::verified::{self, AssetError};
    verified::read(dir, pin, 32 * 1024 * 1024).map_err(|error| {
        match error {
            AssetError::Size => "Invalid Bergamot asset size limit",
            AssetError::Path => "Bergamot asset has an invalid location",
            AssetError::Missing => "Bergamot assets are not installed",
            AssetError::Read => "Could not read Bergamot asset",
            AssetError::Integrity => "Bergamot asset integrity check failed; reinstall its files",
        }
        .into()
    })
}

const FR_FILES: &[TranslationModelFile] = &[
    TranslationModelFile {
        path: "runtime.wasm",
        download_url: Some("https://firefox-settings-attachments.cdn.mozilla.net/main-workspace/translations-wasm/05082c31-aee8-4249-9e01-c1865afd7520.wasm"),
        bytes: 4960506,
        sha256: "a3a89d9ad0a4ed8f27bf3e403701b23f5709816f6376438503f2fa5b0182c2dc",
    },
    TranslationModelFile {
        path: "model.bin",
        download_url: Some("https://firefox-settings-attachments.cdn.mozilla.net/main-workspace/translations-models/8ef78b1c-df6c-49fc-8cae-cf8c0c2a6630.bin"),
        bytes: 31561787,
        sha256: "15f997bc0d13808b0b0fbd0786e684a3c8a52adcd8071844b76123fdacbf2b90",
    },
    TranslationModelFile {
        path: "lex.bin",
        download_url: Some("https://firefox-settings-attachments.cdn.mozilla.net/main-workspace/translations-models/c4d99496-81bf-480f-b0bb-3aebacdcbe8e.bin"),
        bytes: 4824120,
        sha256: "87c6752ea908f5f0347c10ac0cf7d80d9c2f4f20c81c90168f3e8230b56d4440",
    },
    TranslationModelFile {
        path: "vocab.spm",
        download_url: Some("https://firefox-settings-attachments.cdn.mozilla.net/main-workspace/translations-models/11ecb9da-60dd-4b39-aa12-27883099294b.spm"),
        bytes: 814404,
        sha256: "783abf3abe075afdf8d85d233994bef2c3a064e935ab1bed946820aff6ac002a",
    },
];

const ES_FILES: &[TranslationModelFile] = &[
    TranslationModelFile {
        path: "runtime.wasm",
        download_url: Some("https://firefox-settings-attachments.cdn.mozilla.net/main-workspace/translations-wasm/05082c31-aee8-4249-9e01-c1865afd7520.wasm"),
        bytes: 4960506,
        sha256: "a3a89d9ad0a4ed8f27bf3e403701b23f5709816f6376438503f2fa5b0182c2dc",
    },
    TranslationModelFile {
        path: "model.bin",
        download_url: Some("https://firefox-settings-attachments.cdn.mozilla.net/main-workspace/translations-models/d7256ea9-7731-4f15-866c-62998eef93b9.bin"),
        bytes: 31561787,
        sha256: "4aed7734152ae0045d1a69ae49c86cfda18f53c61f90e95e1d1de1c7c7c3b033",
    },
    TranslationModelFile {
        path: "lex.bin",
        download_url: Some("https://firefox-settings-attachments.cdn.mozilla.net/main-workspace/translations-models/a55a9cf2-8a8c-4942-a528-91e93d081746.bin"),
        bytes: 4636248,
        sha256: "e2610211d3b9577d012638fe7e7e74ed7b4b708ce96b9e792e67c282a6492daa",
    },
    TranslationModelFile {
        path: "vocab.spm",
        download_url: Some("https://firefox-settings-attachments.cdn.mozilla.net/main-workspace/translations-models/8695cc3d-7897-4c29-808b-bc983a93d1cd.spm"),
        bytes: 816054,
        sha256: "5ae254fa9b15aa182e70fd2a6186b1333c63a29a48043a9224c6aa4fcac058ad",
    },
];

#[cfg(test)]
mod tests;

/// Bind resume data to the exact assets and shipped worker/loader implementation.
pub(crate) fn provider_identity(model_id: &str) -> Option<String> {
    let manifest = manifest_by_id(model_id)?;
    let mut hash = Sha256::new();
    for file in manifest.files {
        hash.update(file.sha256);
    }
    hash.update(include_bytes!(
        "../../../../public/translation/bergamot/worker.js"
    ));
    hash.update(include_bytes!(
        "../../../../public/translation/bergamot/bergamot-translator.js"
    ));
    Some(format!(
        "bergamot-{}:{:x}",
        crate::translation::inline_markup::protected::MAPPING_VERSION,
        hash.finalize()
    ))
}
