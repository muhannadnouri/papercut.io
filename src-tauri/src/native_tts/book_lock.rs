//! One OS-backed writer lease per audiobook, including save, import and delete.

use std::fs::{self, File, OpenOptions};
use std::path::Path;

/// Keep the returned file alive through all writes; dropping it releases the writer lease.
/// Never unlink lease files: another process may already hold the previous inode open.
pub(crate) fn lock_audiobook_directory(dir: &Path) -> Result<File, String> {
    let key = dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if key.len() != 16 || !key.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("Invalid audiobook storage key".into());
    }
    let parent = dir.parent().ok_or("Missing audiobook parent directory")?;
    fs::create_dir_all(parent)
        .map_err(|err| format!("Failed to create audiobook directory: {err}"))?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(parent.join(format!(".{key}.lock")))
        .map_err(|err| format!("Failed to open audiobook writer lock: {err}"))?;
    fs4::FileExt::try_lock(&file)
        .map_err(|err| format!("Audiobook operation already in progress: {err}"))?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writer_lease_is_exclusive_and_released_on_drop() {
        let root = std::env::temp_dir().join(format!("papercut-lease-{}", std::process::id()));
        let dir = root.join("0123456789abcdef");
        let first = lock_audiobook_directory(&dir).unwrap();
        assert!(lock_audiobook_directory(&dir)
            .unwrap_err()
            .contains("already in progress"));
        drop(first);
        drop(lock_audiobook_directory(&dir).unwrap());
        assert!(lock_audiobook_directory(&root.join("../escape")).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
