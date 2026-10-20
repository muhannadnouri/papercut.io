//! Shared local asset admission. Only verified snapshots leave this boundary.
use std::{fs, io::Read, path::Path};

use sha2::{Digest, Sha256};

use super::TranslationModelFile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AssetError {
    Path,
    Missing,
    Size,
    Read,
    Integrity,
    Cancelled,
}

/// Keep tokenizer/worker bytes from the same bounded read whose digest was checked.
pub(crate) fn read(
    dir: &Path,
    pin: &TranslationModelFile,
    limit: u64,
) -> Result<Vec<u8>, AssetError> {
    checked(dir, pin, limit, true, &mut || false)
}

/// Check cancellation at each bounded read; never return a partially verified snapshot.
#[cfg(any(feature = "native-translation-alignment", test))]
pub(crate) fn read_cancellable(
    dir: &Path,
    pin: &TranslationModelFile,
    limit: u64,
    mut cancelled: impl FnMut() -> bool,
) -> Result<Vec<u8>, AssetError> {
    checked(dir, pin, limit, true, &mut cancelled)
}

/// Stream large weights without retaining a second model-sized buffer; this grants no future load lease.
#[cfg(any(feature = "native-translation-alignment", test))]
pub(crate) fn verify(dir: &Path, pin: &TranslationModelFile, limit: u64) -> Result<(), AssetError> {
    checked(dir, pin, limit, false, &mut || false).map(|_| ())
}

/// Paths are flat manifest-owned names, never caller input or a URL-derived filename.
fn checked(
    dir: &Path,
    pin: &TranslationModelFile,
    limit: u64,
    retain: bool,
    cancelled: &mut dyn FnMut() -> bool,
) -> Result<Vec<u8>, AssetError> {
    if cancelled() {
        return Err(AssetError::Cancelled);
    }
    if pin.bytes == 0 || pin.bytes > limit || pin.bytes.checked_add(1).is_none() {
        return Err(AssetError::Size);
    }
    let mut parts = Path::new(pin.path).components();
    if !matches!(parts.next(), Some(std::path::Component::Normal(_)))
        || parts.next().is_some()
        || pin.path.contains('\\')
    {
        return Err(AssetError::Path);
    }
    if !fs::symlink_metadata(dir)
        .map_err(|_| AssetError::Missing)?
        .is_dir()
    {
        return Err(AssetError::Path);
    }
    let root = dir.canonicalize().map_err(|_| AssetError::Missing)?;
    let path = dir.join(pin.path);
    let meta = fs::symlink_metadata(&path).map_err(|_| AssetError::Missing)?;
    if !meta.is_file()
        || path.canonicalize().map_err(|_| AssetError::Path)?.parent() != Some(root.as_path())
    {
        return Err(AssetError::Path);
    }
    if meta.len() != pin.bytes {
        return Err(AssetError::Size);
    }
    let file = fs::File::open(path).map_err(|_| AssetError::Read)?;
    let opened = file.metadata().map_err(|_| AssetError::Read)?;
    if !opened.is_file() || opened.len() != pin.bytes {
        return Err(AssetError::Size);
    }
    let mut input = file.take(pin.bytes + 1);
    let mut bytes = Vec::new();
    if retain {
        bytes
            .try_reserve_exact(usize::try_from(pin.bytes).map_err(|_| AssetError::Size)?)
            .map_err(|_| AssetError::Read)?;
    }
    let mut hash = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0; 64 * 1024];
    loop {
        if cancelled() {
            return Err(AssetError::Cancelled);
        }
        let count = input.read(&mut buffer).map_err(|_| AssetError::Read)?;
        if count == 0 {
            break;
        }
        total += count as u64;
        hash.update(&buffer[..count]);
        if retain {
            bytes.extend_from_slice(&buffer[..count]);
        }
    }
    if cancelled() {
        return Err(AssetError::Cancelled);
    }
    if total != pin.bytes || format!("{:x}", hash.finalize()) != pin.sha256 {
        return Err(AssetError::Integrity);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests;
