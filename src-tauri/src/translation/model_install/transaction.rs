//! Destination-volume staging and recoverable replacement; no network, app state or document writes.
use std::{
    fs, io,
    path::{Path, PathBuf},
};

use super::super::model_store::{directory_size, TranslationModelFile, TranslationModelManifest};

pub(super) struct Paths {
    pub(super) final_dir: PathBuf,
    staging: PathBuf,
    previous: PathBuf,
}

impl Paths {
    /// Siblings keep both renames on the destination filesystem, including separate XDG cache volumes.
    pub(super) fn new(final_dir: PathBuf) -> Result<Self, String> {
        let name = final_dir
            .file_name()
            .and_then(|v| v.to_str())
            .ok_or("Invalid model directory")?;
        if !super::super::model_store::verified::valid_name(name) {
            return Err("Invalid model directory".into());
        }
        let parent = final_dir.parent().ok_or("Missing model parent")?;
        Ok(Self {
            staging: parent.join(format!(".{name}.installing")),
            previous: parent.join(format!(".{name}.previous")),
            final_dir,
        })
    }

    /// Retry recovery chooses a verified new pack, otherwise restores the untouched previous directory.
    fn recover(&self, manifest: TranslationModelManifest) -> Result<(), String> {
        if exists(&self.previous)? {
            if exists(&self.final_dir)? && manifest.verify_files(&self.final_dir).is_ok() {
                remove(&self.previous)?;
            } else {
                remove(&self.final_dir)?;
                fs::rename(&self.previous, &self.final_dir)
                    .map_err(|e| format!("Could not restore previous model: {e}"))?;
            }
        }
        remove(&self.staging)
    }

    /// Preserve the previous pack through a failed rename; failed rollback leaves it for the next retry.
    // shortcut: process-interruption recovery; add directory durability before promising power-loss safety.
    fn promote(
        &self,
        rename: &mut impl FnMut(&Path, &Path) -> io::Result<()>,
    ) -> Result<(), String> {
        let had_previous = exists(&self.final_dir)?;
        if had_previous {
            rename(&self.final_dir, &self.previous)
                .map_err(|e| format!("Could not retain previous model: {e}"))?;
        }
        if let Err(error) = rename(&self.staging, &self.final_dir) {
            if had_previous {
                rename(&self.previous, &self.final_dir).map_err(|rollback| format!("Model promotion failed: {error}; previous model retained for recovery: {rollback}"))?;
            }
            return Err(format!("Model promotion failed: {error}"));
        }
        // If cleanup fails, retry recovery verifies the new final before deleting the backup.
        remove(&self.previous)
    }

    /// Explicit removal includes interrupted installs/backups, never neighboring models or documents.
    pub(super) fn remove(&self) -> Result<(bool, u64), String> {
        let mut bytes = 0u64;
        let mut removed = false;
        for path in [&self.final_dir, &self.staging, &self.previous] {
            if exists(path)? {
                removed = true;
                bytes = bytes
                    .checked_add(directory_size(path)?)
                    .ok_or("Model storage size overflow")?;
            }
        }
        for path in [&self.staging, &self.previous, &self.final_dir] {
            remove(path)?;
        }
        Ok((removed, bytes))
    }
}

/// Only manifest-owned regular directories are eligible for recursive cleanup.
fn exists(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => Ok(true),
        Ok(_) => Err("Invalid model directory type".into()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(format!("Could not inspect model directory: {e}")),
    }
}

fn remove(path: &Path) -> Result<(), String> {
    if exists(path)? {
        fs::remove_dir_all(path).map_err(|e| format!("Could not remove model directory: {e}"))?;
    }
    Ok(())
}

/// Reclaim only an obsolete installer-owned cache directory, without following links.
pub(super) fn clear_legacy(path: &Path) -> Result<(bool, u64), String> {
    if !exists(path)? {
        return Ok((false, 0));
    }
    let bytes = directory_size(path)?;
    remove(path)?;
    Ok((true, bytes))
}

/// Run under the caller's model-operation lease. Closures expose actual I/O boundaries for failure tests.
pub(super) fn install(
    paths: &Paths,
    manifest: TranslationModelManifest,
    mut download: impl FnMut(&TranslationModelFile, &Path, &mut u64) -> Result<(), String>,
    check_space: impl FnOnce(&Path, u64) -> Result<(), String>,
    mut rename: impl FnMut(&Path, &Path) -> io::Result<()>,
) -> Result<u64, String> {
    manifest.validate()?;
    if paths.final_dir.file_name().and_then(|n| n.to_str()) != Some(manifest.directory_name) {
        return Err("Model inventory and destination do not match".into());
    }
    paths.recover(manifest)?;
    if manifest.verify_files(&paths.final_dir).is_ok() {
        return directory_size(&paths.final_dir);
    }
    let parent = paths.final_dir.parent().ok_or("Missing model parent")?;
    fs::create_dir_all(parent).map_err(|e| format!("Could not create model parent: {e}"))?;
    check_space(parent, manifest.total_bytes())?;
    fs::create_dir(&paths.staging).map_err(|e| format!("Could not stage model: {e}"))?;
    let _cleanup = WorkDirGuard::new(paths.staging.clone());
    let mut total = 0;
    for file in manifest.files {
        download(file, &paths.staging.join(file.path), &mut total)?;
    }
    manifest.verify_files(&paths.staging)?;
    for file in manifest.files {
        // Windows FlushFileBuffers requires a writable handle; do not truncate verified bytes.
        fs::OpenOptions::new()
            .write(true)
            .open(paths.staging.join(file.path))
            .and_then(|file| file.sync_all())
            .map_err(|e| format!("Could not sync staged model: {e}"))?;
    }
    paths.promote(&mut rename)?;
    directory_size(&paths.final_dir)
}

/// Failed downloads leave no partial staging tree; the backup is deliberately owned by recovery instead.
pub(super) struct WorkDirGuard(PathBuf);
impl WorkDirGuard {
    pub(super) fn new(path: PathBuf) -> Self {
        Self(path)
    }
}
impl Drop for WorkDirGuard {
    fn drop(&mut self) {
        let _ = remove(&self.0);
    }
}

#[cfg(test)]
mod tests;
