use std::{fs, io, path::Path};

pub(crate) fn copy_file_atomically(src: &Path, dst: &Path) -> io::Result<()> {
    let mut name = dst
        .file_name()
        .ok_or(io::ErrorKind::InvalidInput)?
        .to_os_string();
    // Build-script calls are sequential; different build processes need distinct staging files.
    name.push(format!(".{}.part", std::process::id()));
    let staged = dst.with_file_name(name);
    let result = fs::copy(src, &staged).and_then(|_| fs::rename(&staged, dst));
    if result.is_err() {
        let _ = fs::remove_file(staged);
    }
    result
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{fs::File, io::Read, os::unix::fs::MetadataExt};

    #[test]
    fn replacement_preserves_open_inode_and_failed_copy_preserves_destination() {
        let dir =
            std::env::temp_dir().join(format!("papercut-runtime-copy-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let src = dir.join("source");
        let dst = dir.join("library.so");
        fs::write(&src, b"new library").unwrap();
        fs::write(&dst, b"old library").unwrap();
        let mut open = File::open(&dst).unwrap();
        let inode = open.metadata().unwrap().ino();
        copy_file_atomically(&src, &dst).unwrap();
        assert_ne!(fs::metadata(&dst).unwrap().ino(), inode);
        let mut old = Vec::new();
        open.read_to_end(&mut old).unwrap();
        assert_eq!(old, b"old library");
        assert_eq!(fs::read(&dst).unwrap(), b"new library");
        assert!(copy_file_atomically(&dir.join("missing"), &dst).is_err());
        assert_eq!(fs::read(&dst).unwrap(), b"new library");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 2);
        fs::remove_dir_all(dir).unwrap();
    }
}
