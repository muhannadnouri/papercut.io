//! Default-off alignment assets/tokenizer and job-owned CPU session; public starts remain translator-only.
use std::path::Path;

use tokenizers::{NormalizedString, Normalizer, Tokenizer};

use super::inline_markup::provider::contract::{
    self, CheckedMapping, Pair, Rejection, Reply, Side, Token, TokenRules, TokenizedText,
};
use super::model_store::verified::AssetError;

pub(super) mod assets;
#[cfg(feature = "native-translation-alignment-runtime")]
mod extract;
#[cfg(feature = "native-translation-alignment-runtime")]
pub(crate) mod runtime;

/// Engineering admission cap, including framing; not a measured low-power performance claim.
const MAX_TOKENS: usize = 256;
const MAX_TEXT_BYTES: usize = 32 * 1024;
const RESERVED: &[u32] = &[0, 100, 101, 102, 103];

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum AdmissionError {
    Assets(AssetError),
    Profile,
    Tokenizer(Side),
    Evidence(Rejection),
}

impl From<AssetError> for AdmissionError {
    fn from(error: AssetError) -> Self {
        Self::Assets(error)
    }
}
impl From<Rejection> for AdmissionError {
    fn from(error: Rejection) -> Self {
        Self::Evidence(error)
    }
}

/// Only a verified local tokenizer snapshot constructs the production adapter.
pub(crate) struct CandidateTokenizer {
    tokenizer: Tokenizer,
}

/// Keep raw input and original-byte evidence together; do not log or serialize this private prose.
pub(crate) struct EncodedText<'a> {
    text: &'a str,
    normalized: String,
    tokens: Vec<Token>,
}

impl<'a> EncodedText<'a> {
    pub(crate) fn text(&self) -> &'a str {
        self.text
    }

    /// Word IDs/offsets come from one raw encoding, never from separately tokenized words.
    pub(crate) fn evidence(&self) -> TokenizedText<'_> {
        TokenizedText {
            normalized: &self.normalized,
            tokens: &self.tokens,
        }
    }
}

impl CandidateTokenizer {
    /// Planning treats only size overflow as splittable; unknown tokens and provenance failures still reject.
    pub(crate) fn text_fits(&self, text: &str, side: Side) -> Result<bool, AdmissionError> {
        match self.encode(text, side) {
            Ok(_) => Ok(true),
            Err(AdmissionError::Evidence(Rejection::Limit)) => Ok(false),
            Err(error) => Err(error),
        }
    }
    /// No network, installation or weight loading; replaced files are rechecked on each admission.
    pub(crate) fn load(dir: &Path) -> Result<Self, AdmissionError> {
        Self::from_snapshot(&assets::tokenizer_snapshot(dir)?)
    }

    /// The snapshot's pin establishes identity; profile checks prohibit hidden padding/truncation.
    fn from_snapshot(bytes: &[u8]) -> Result<Self, AdmissionError> {
        let tokenizer = Tokenizer::from_bytes(bytes).map_err(|_| AdmissionError::Profile)?;
        if tokenizer.get_padding().is_some()
            || tokenizer.get_truncation().is_some()
            || tokenizer.get_normalizer().is_none()
            || ["[PAD]", "[UNK]", "[CLS]", "[SEP]", "[MASK]"]
                .iter()
                .zip(RESERVED)
                .any(|(text, id)| tokenizer.token_to_id(text) != Some(*id))
        {
            return Err(AdmissionError::Profile);
        }
        Ok(Self { tokenizer })
    }

    pub(crate) fn identity(&self) -> &'static str {
        assets::TOKENIZER_ID
    }

    pub(crate) fn rules(&self) -> TokenRules<'static> {
        TokenRules {
            identity: assets::TOKENIZER_ID,
            unknown: 100,
            reserved: RESERVED,
            prefix: 101,
            suffix: 102,
        }
    }

    /// Encode exact raw text once, with no truncation, then validate provenance before any inference.
    pub(crate) fn encode<'a>(
        &self,
        text: &'a str,
        side: Side,
    ) -> Result<EncodedText<'a>, AdmissionError> {
        if text.len() > MAX_TEXT_BYTES {
            return Err(Rejection::Limit.into());
        }
        if text.trim().is_empty() {
            return Err(Rejection::Binding.into());
        }
        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|_| AdmissionError::Tokenizer(side))?;
        if encoding.len() > MAX_TOKENS {
            return Err(Rejection::Limit.into());
        }
        let sequences = encoding.get_sequence_ids();
        if !encoding.get_overflowing().is_empty()
            || encoding.get_type_ids().iter().any(|id| *id != 0)
            || encoding.get_attention_mask().iter().any(|mask| *mask != 1)
        {
            return Err(Rejection::Framing(side).into());
        }
        let mut tokens = Vec::with_capacity(encoding.len());
        for index in 0..encoding.len() {
            let (start, end) = encoding.get_offsets()[index];
            let frame = index == 0 || index + 1 == encoding.len();
            if sequences[index] != if frame { None } else { Some(0) } {
                return Err(Rejection::Framing(side).into());
            }
            tokens.push(Token {
                id: encoding.get_ids()[index],
                word: encoding.get_word_ids()[index].map(|id| id as usize),
                bytes: start..end,
                special: encoding.get_special_tokens_mask()[index] == 1,
            });
        }
        // Run the pinned normalizer on the whole raw string to detect deletions inside offset envelopes.
        let mut normalized = NormalizedString::from(text);
        self.tokenizer
            .get_normalizer()
            .ok_or(AdmissionError::Profile)?
            .normalize(&mut normalized)
            .map_err(|_| AdmissionError::Tokenizer(side))?;
        let result = EncodedText {
            text,
            normalized: normalized.get().into(),
            tokens,
        };
        contract::validate_tokens(text, &self.rules(), &result.evidence(), side)?;
        Ok(result)
    }

    /// Bind both actual pair strings to real tokenizer evidence; passing is structural, not semantic approval.
    pub(crate) fn check_pair<'a>(
        &self,
        pair: &Pair<'_>,
        reply: &'a Reply,
    ) -> Result<CheckedMapping<'a>, AdmissionError> {
        if pair.versions.tokenizer != self.identity() {
            return Err(Rejection::TokenProfile.into());
        }
        let source = self.encode(pair.plan.source_text(), Side::Source)?;
        let target = self.encode(pair.target, Side::Target)?;
        pair.check(&self.rules(), &source.evidence(), &target.evidence(), reply)
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests;
