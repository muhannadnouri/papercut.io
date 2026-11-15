//! App ownership of the published Piper C API; upstream speech-library source stays unchanged.

use std::ffi::{c_char, c_void, CStr, CString};
use std::fs;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::OnceLock;

use sha2::{Digest, Sha256};

const DATA_TREE_SHA256: &str = "43ebef9ebca9edcb42dd5d49a7935038731af4c6450a677dc0cae732dae0ce6a";
const MAX_PHONEMES: usize = 350;
static INITIALIZED: OnceLock<Result<PathBuf, String>> = OnceLock::new();

extern "C" {
    fn PiperPhonemizeGetVersionStr() -> *const c_char;
    fn PiperPhonemizeInitialize(data: *const c_char) -> i32;
    fn PiperPhonemizeText(text: *const c_char, voice: *const c_char) -> *mut c_void;
    fn PiperPhonemizeResultGetNumSentences(result: *const c_void) -> i32;
    fn PiperPhonemizeResultGetNumPhonemes(result: *const c_void, index: i32) -> i32;
    fn PiperPhonemizeResultGetPhonemes(result: *const c_void, index: i32) -> *const i32;
    fn PiperPhonemizeDestroyResult(result: *mut c_void);
}

struct PhonemeResult(NonNull<c_void>);

impl Drop for PhonemeResult {
    fn drop(&mut self) {
        // SAFETY: this unique handle came from PiperPhonemizeText and is released exactly once.
        unsafe { PiperPhonemizeDestroyResult(self.0.as_ptr()) };
    }
}

/// Verify the existing model-pack data before initialization; filenames and contents both affect the digest.
fn verify_data(root: &Path) -> Result<(), String> {
    fn walk(root: &Path, dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
        for entry in
            fs::read_dir(dir).map_err(|err| format!("Cannot read phonemizer data: {err}"))?
        {
            let entry = entry.map_err(|err| err.to_string())?;
            let kind = entry.file_type().map_err(|err| err.to_string())?;
            if kind.is_dir() {
                walk(root, &entry.path(), files)?;
            } else if kind.is_file() {
                files.push(
                    entry
                        .path()
                        .strip_prefix(root)
                        .map_err(|err| err.to_string())?
                        .to_path_buf(),
                );
            } else {
                return Err(
                    "Unexpected link in phonemizer data; repair the voice model and restart".into(),
                );
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    walk(root, root, &mut files)?;
    files.sort_by_key(|path| path.to_string_lossy().replace('\\', "/"));
    let mut tree = Sha256::new();
    for path in files {
        let digest = Sha256::digest(fs::read(root.join(&path)).map_err(|err| err.to_string())?);
        tree.update(format!(
            "{}\t{digest:x}\n",
            path.to_string_lossy().replace('\\', "/")
        ));
    }
    if format!("{:x}", tree.finalize()) != DATA_TREE_SHA256 {
        return Err(
            "Phonemizer data checksum mismatch; repair the voice model and restart the app".into(),
        );
    }
    Ok(())
}

/// Cache failure as well as success: upstream call_once can return zero after a failed first initialization.
fn initialize(data: &Path) -> Result<&Path, String> {
    let retained = INITIALIZED.get_or_init(|| {
        verify_data(data)?;
        let path = CString::new(data.to_string_lossy().as_bytes()).map_err(|err| err.to_string())?;
        // SAFETY: version is a static C string; initialization borrows our NUL-terminated path during this call.
        unsafe {
            let version = PiperPhonemizeGetVersionStr();
            if version.is_null() || CStr::from_ptr(version).to_bytes() != b"1.5.9" {
                return Err("Incorrect native Piper phonemizer version; rebuild/reinstall the app".into());
            }
            if PiperPhonemizeInitialize(path.as_ptr()) <= 0 {
                return Err("Native phonemizer initialization failed; repair the voice model and restart the app".into());
            }
        }
        Ok(data.to_path_buf())
    }).as_ref().map_err(Clone::clone)?;
    if !retained.is_dir() {
        return Err("Initialized phonemizer data was removed; reinstall the voice model and restart the app".into());
    }
    Ok(retained)
}

/// Copy sentences while the native result owns their buffers; current synthesis already serializes engine use.
pub(super) fn phonemize(data: &Path, text: &str, locale: &str) -> Result<Vec<Vec<i32>>, String> {
    initialize(data)?;
    let text = CString::new(text).map_err(|err| format!("Invalid synthesis text: {err}"))?;
    let locale = CString::new(locale).map_err(|err| err.to_string())?;
    // SAFETY: both strings live through the call; RAII retains the returned handle until all buffers are copied.
    let result = PhonemeResult(
        NonNull::new(unsafe { PiperPhonemizeText(text.as_ptr(), locale.as_ptr()) })
            .ok_or("Native phonemization failed")?,
    );
    let mut sentences = Vec::new();
    // SAFETY: all getters borrow this live result; non-empty buffers are checked before constructing slices.
    unsafe {
        let count = PiperPhonemizeResultGetNumSentences(result.0.as_ptr());
        if count < 0 {
            return Err("Invalid native phoneme sentence count".into());
        }
        for index in 0..count {
            let n = PiperPhonemizeResultGetNumPhonemes(result.0.as_ptr(), index);
            if n < 0 {
                return Err("Invalid native phoneme count".into());
            }
            if n == 0 {
                continue;
            }
            let ptr = PiperPhonemizeResultGetPhonemes(result.0.as_ptr(), index);
            if ptr.is_null() {
                return Err("Missing native phoneme buffer".into());
            }
            sentences.extend(split_sentence(std::slice::from_raw_parts(ptr, n as usize))?);
        }
    }
    Ok(sentences)
}

/// Stay below Sherpa's count/style boundaries without truncating or bisecting a spoken word.
fn split_sentence(phonemes: &[i32]) -> Result<Vec<Vec<i32>>, String> {
    let mut parts = Vec::new();
    let mut rest = phonemes;
    while rest.len() > MAX_PHONEMES {
        let end = rest[..MAX_PHONEMES]
            .iter()
            .rposition(|p| *p == 32)
            .ok_or("A spoken word exceeds the phoneme limit; shorten this source passage")?
            + 1;
        parts.push(rest[..end].to_vec());
        rest = &rest[end..];
    }
    if !rest.is_empty() {
        parts.push(rest.to_vec());
    }
    Ok(parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_sentences_preserve_every_phoneme_and_split_at_word_boundaries() {
        let input: Vec<i32> = "word ".repeat(180).chars().map(|c| c as i32).collect();
        let parts = split_sentence(&input).unwrap();
        assert_eq!(parts.concat(), input);
        assert!(parts.iter().all(|part| part.len() <= MAX_PHONEMES));
        assert!(parts[..parts.len() - 1]
            .iter()
            .all(|part| part.last() == Some(&32)));
        assert!(split_sentence(&vec![97; MAX_PHONEMES + 1]).is_err());
    }

    #[test]
    fn missing_or_incomplete_data_is_rejected_before_native_initialization() {
        assert!(verify_data(&std::env::temp_dir().join("papercut-no-espeak-data")).is_err());
    }
}
