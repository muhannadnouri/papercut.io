//! Training-specific phoneme adapters; model metadata and inference remain shared.
//!
//! Keep this bridge in the app, not in a speech-library fork. Exact vocabulary/style
//! pins bind the transformations to the pair accepted in the Stage 4a listening review.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use misaki_rs::{fallback::FallbackError, Fallback, Language, G2P};
use regex::Regex;
use sha2::{Digest, Sha256};

use super::models::{KOKORO_7M_MODEL_ID, NABRA_7M_MODEL_ID};
use super::phonemizer::{phonemize, split_sentence};

const STYLE_SHA256: &str = "129d447a7c9bdce1d5b6cb5ecb789bbfd9f427b7775131a97dfa5638583b3b92";

static SYLLABIC_CONSONANT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\S)\x{0329}").expect("valid syllabic-consonant pattern"));

// This state follows the existing serialized engine slot, not a second model cache.
pub(super) struct PhonemeAdapter {
    english: Option<G2P>,
    data: PathBuf,
    vocabulary: HashSet<i32>,
}

impl PhonemeAdapter {
    /// Use identity, not the misleading shared af_msa voice prefix (Nabra is Arabic).
    pub(super) fn locale(model_id: &str) -> Option<&'static str> {
        match model_id {
            KOKORO_7M_MODEL_ID => Some("en-us"),
            NABRA_7M_MODEL_ID => Some("ar"),
            _ => None,
        }
    }

    /// Load once with the engine slot; reject mismatched packs before native inference.
    /// The supplied style rows are identical, so Sherpa's N vs reference N-1 needs no adapter.
    pub(super) fn load(model_id: &str, model_dir: &Path) -> Result<Option<Self>, String> {
        let token_sha = match model_id {
            KOKORO_7M_MODEL_ID => {
                "6ebb6bb288f20f3ae8d004d3c2ca27697da27c037d75e81a60e2a6a663f95425"
            }
            NABRA_7M_MODEL_ID => "5b43b8142c28b5c6f5d13d83caed696e3dabde376d5a0e9e67aceaf66f021da1",
            _ => return Ok(None),
        };
        let tokens = fs::read(model_dir.join("tokens.txt")).map_err(|err| err.to_string())?;
        let style = fs::read(model_dir.join("voices.bin")).map_err(|err| err.to_string())?;
        if format!("{:x}", Sha256::digest(&tokens)) != token_sha
            || format!("{:x}", Sha256::digest(&style)) != STYLE_SHA256
        {
            return Err("Incorrect 7M vocabulary or voice pack; reinstall this model".into());
        }
        let vocabulary = std::str::from_utf8(&tokens)
            .map_err(|err| err.to_string())?
            .lines()
            .map(|line| {
                let (symbol, _) = line.rsplit_once(' ').ok_or("Invalid 7M token table")?;
                let mut chars = symbol.chars();
                let codepoint = chars.next().ok_or("Empty 7M token")? as i32;
                if chars.next().is_some() {
                    return Err("Invalid 7M token symbol");
                }
                Ok(codepoint)
            })
            .collect::<Result<HashSet<_>, &str>>()?;
        let data = model_dir.join("espeak-ng-data");
        let english = (model_id == KOKORO_7M_MODEL_ID).then(|| {
            G2P::with_fallback(
                Language::EnglishUS,
                Some(Box::new(PiperFallback(data.clone()))),
            )
        });
        Ok(Some(Self {
            english,
            data,
            vocabulary,
        }))
    }

    /// Input is the existing app-normalized/optionally diacritized synthesis copy.
    /// Unknown words must resolve or fail; passing unknown phones to Sherpa would drop them.
    pub(super) fn phonemize(&self, text: &str) -> Result<Vec<Vec<i32>>, String> {
        let phones = if let Some(g2p) = &self.english {
            let (phones, tokens) = g2p
                .g2p(text)
                .map_err(|err| format!("7M English frontend: {err}"))?;
            if tokens.iter().any(|token| {
                token.text.chars().any(char::is_alphanumeric)
                    && token
                        .phonemes
                        .as_deref()
                        .is_none_or(|p| p.trim().is_empty())
            }) {
                return Err("7M English frontend could not pronounce a source word".into());
            }
            vec![phones]
        } else {
            phonemize(&self.data, text, "ar")?
                .iter()
                .map(|sentence| phone_text(sentence).map(|text| arabic_phones(&text)))
                .collect::<Result<Vec<_>, _>>()?
        };
        let mut sentences = Vec::new();
        for sentence in phones {
            sentences.extend(self.checked_sentences(&sentence)?);
        }
        if sentences.is_empty() {
            return Err("7M frontend returned no speech".into());
        }
        Ok(sentences)
    }

    /// Validate every symbol before word-safe splitting; never crop an oversized word.
    fn checked_sentences(&self, phones: &str) -> Result<Vec<Vec<i32>>, String> {
        if phones.trim().is_empty() {
            return Err("7M frontend returned no speech".into());
        }
        let codepoints: Vec<_> = phones.chars().map(|c| c as i32).collect();
        if let Some(unknown) = codepoints.iter().find(|p| !self.vocabulary.contains(p)) {
            return Err(format!(
                "7M frontend produced an unsupported phone U+{unknown:04X}"
            ));
        }
        split_sentence(&codepoints)
    }
}

struct PiperFallback(PathBuf);

impl Fallback for PiperFallback {
    /// Share the app's pinned, process-global Piper initialization and verified data lifetime.
    fn phonemize(&self, word: &str) -> Result<String, FallbackError> {
        let convert = || -> Result<String, String> {
            let sentences = phonemize(&self.0, word, "en-us")?;
            let phones = misaki_fallback_phones(&phone_text(&sentences.concat())?);
            if phones.trim().is_empty() {
                return Err("No pronunciation for unknown word".into());
            }
            Ok(phones)
        };
        convert().map_err(|error| FallbackError::Espeak {
            word: word.into(),
            error,
        })
    }
}

/// Treat invalid native codepoints as a recoverable frontend failure, not a panic.
fn phone_text(phones: &[i32]) -> Result<String, String> {
    phones
        .iter()
        .map(|p| char::from_u32(*p as u32).ok_or_else(|| "Invalid native phone codepoint".into()))
        .collect()
}

/// Mirror Misaki 0.9.4's US fallback conversions for Piper's untied IPA; differential tests qualify the bridge.
fn misaki_fallback_phones(raw: &str) -> String {
    let mut text = raw
        .trim()
        .trim_end_matches(['.', ',', '?', '!'])
        .to_string();
    for (old, new) in [
        ("ʔˌn\u{0329}", "ʔn"),
        ("ʔn\u{0329}", "ʔn"),
        ("aɪ", "I"),
        ("aʊ", "W"),
        ("dʒ", "ʤ"),
        ("eɪ", "A"),
        ("tʃ", "ʧ"),
        ("ɔɪ", "Y"),
        ("ʲo", "jo"),
        ("ʲə", "jə"),
        ("ʲ", ""),
        ("ɚ", "əɹ"),
        ("r", "ɹ"),
        ("x", "k"),
        ("ç", "k"),
        ("ɐ", "ə"),
        ("ɬ", "l"),
        ("\u{0303}", ""),
    ] {
        text = text.replace(old, new);
    }
    text = SYLLABIC_CONSONANT
        .replace_all(&text, "ᵊ$1")
        .replace('\u{0329}', "");
    for (old, new) in [
        ("e", "A"),
        ("oʊ", "O"),
        ("ɜːɹ", "ɜɹ"),
        ("ɜː", "ɜɹ"),
        ("ɪə", "iə"),
        ("ː", ""),
        ("o", "ɔ"),
        ("ɾ", "T"),
        ("ʔ", "t"),
    ] {
        text = text.replace(old, new);
    }
    text
}

/// Match the reference order: Misaki's IPA folds precede Arabic syllable-dot/marker cleanup.
fn arabic_phones(raw: &str) -> String {
    let mut text = raw.to_string();
    for (old, new) in [
        ("aɪ", "I"),
        ("aʊ", "W"),
        ("dz", "ʣ"),
        ("dʒ", "ʤ"),
        ("eɪ", "A"),
        ("oʊ", "O"),
        ("əʊ", "Q"),
        ("ss", "S"),
        ("ts", "ʦ"),
        ("tʃ", "ʧ"),
        ("ɔɪ", "Y"),
    ] {
        text = text.replace(old, new);
    }
    let chars: Vec<char> = text.chars().collect();
    chars
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            if matches!(
                *c,
                '\u{032a}' | '\u{02e4}' | '[' | ']' | '{' | '}' | '^' | '-'
            ) {
                return None;
            }
            if *c == '.'
                && i > 0
                && i + 1 < chars.len()
                && !chars[i - 1].is_whitespace()
                && !chars[i + 1].is_whitespace()
            {
                return None;
            }
            Some(*c)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::models::MODELS;
    use super::*;

    /// No current model may accidentally inherit the small-model transforms or Nabra's locale.
    #[test]
    fn only_the_new_identities_opt_in() {
        for model in MODELS {
            if [KOKORO_7M_MODEL_ID, NABRA_7M_MODEL_ID].contains(&model.id) {
                assert!(PhonemeAdapter::locale(model.id).is_some());
                continue;
            }
            assert_eq!(PhonemeAdapter::locale(model.id), None);
            assert!(PhonemeAdapter::load(model.id, Path::new("/missing"))
                .unwrap()
                .is_none());
        }
        assert_eq!(PhonemeAdapter::locale(KOKORO_7M_MODEL_ID), Some("en-us"));
        assert_eq!(PhonemeAdapter::locale(NABRA_7M_MODEL_ID), Some("ar"));
    }

    /// Unknown or overlong phones must fail before Sherpa can drop/crop source content.
    #[test]
    fn phone_validation_preserves_content_and_rejects_invalid_input() {
        let frontend = PhonemeAdapter {
            english: None,
            data: PathBuf::new(),
            vocabulary: "a ,".chars().map(|c| c as i32).collect(),
        };
        let text = "aa ".repeat(400);
        let parts = frontend.checked_sentences(&text).unwrap();
        assert_eq!(phone_text(&parts.concat()).unwrap(), text);
        assert!(parts.iter().all(|part| part.len() <= 350));
        assert!(frontend.checked_sentences(&"a".repeat(351)).is_err());
        for invalid in ["", "   ", "a❓", "aΩ", "a\0"] {
            assert!(frontend.checked_sentences(invalid).is_err(), "{invalid:?}");
        }
        assert!(phone_text(&[-1]).is_err());
        assert!(phone_text(&[0x11_0000]).is_err());
    }

    /// Retain the owner-reviewed OOV syllabic-L variant instead of changing accepted pronunciation here.
    #[test]
    fn piper_fallback_conversions_keep_the_reviewed_phone_contract() {
        assert_eq!(misaki_fallback_phones("dʒeɪmz tʃɔɪs."), "ʤAmz ʧYs");
        assert_eq!(
            misaki_fallback_phones("kjˌuːzˌiːˈɛksdˌʌbəljˌuːvˈiː."),
            "kjˌuzˌiˈɛksdˌʌbəljˌuvˈi"
        );
        assert_eq!(misaki_fallback_phones("əl\u{0329}."), "əᵊl");
    }

    /// Arabic cleanup must retain pharyngeals, mixed-language phones and accepted comma pauses.
    #[test]
    fn arabic_cleanup_preserves_language_content_and_pause_markers() {
        assert_eq!(
            arabic_phones("ʕa.li ħaˤq, pApɚkʌt dʒeɪmz."),
            "ʕali ħaq, pApɚkʌt ʤAmz."
        );
    }

    struct FailedFallback;
    impl Fallback for FailedFallback {
        /// Stand in for a native failure; no test should initialize real Piper implicitly.
        fn phonemize(&self, word: &str) -> Result<String, FallbackError> {
            Err(FallbackError::Espeak {
                word: word.into(),
                error: "test failure".into(),
            })
        }
    }

    /// Library fallback errors must remain recoverable, not turn unknown words into empty speech.
    #[test]
    fn unknown_word_failure_reaches_the_caller() {
        let frontend = PhonemeAdapter {
            english: Some(G2P::with_fallback(
                Language::EnglishUS,
                Some(Box::new(FailedFallback)),
            )),
            data: PathBuf::new(),
            vocabulary: HashSet::new(),
        };
        assert!(frontend
            .phonemize("Zyxophrenia")
            .unwrap_err()
            .contains("test failure"));
        assert!(frontend.phonemize("").is_err());
    }

    /// Even an upstream success containing an unknown marker must not count as complete speech.
    #[test]
    fn unresolved_word_is_rejected_without_fallback() {
        let frontend = PhonemeAdapter {
            english: Some(G2P::with_fallback(Language::EnglishUS, None)),
            data: PathBuf::new(),
            vocabulary: "❓ ".chars().map(|c| c as i32).collect(),
        };
        assert!(frontend
            .phonemize("Zyxophrenia")
            .unwrap_err()
            .contains("source word"));
    }

    /// A plausible file layout is insufficient: foreign/changed token and voice packs must fail closed.
    #[test]
    fn mismatched_assets_are_rejected() {
        let dir = std::env::temp_dir().join(format!(
            "papercut-7m-assets-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("tokens.txt"), "a 1\n").unwrap();
        fs::write(dir.join("voices.bin"), [0]).unwrap();
        for id in [KOKORO_7M_MODEL_ID, NABRA_7M_MODEL_ID] {
            assert!(PhonemeAdapter::load(id, &dir)
                .err()
                .unwrap()
                .contains("Incorrect 7M"));
        }
        fs::remove_dir_all(dir).unwrap();
    }
}
