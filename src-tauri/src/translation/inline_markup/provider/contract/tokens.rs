//! Tokenizer evidence stays separate from aligner output; no tokenizer runtime is loaded here.
use std::ops::Range;

use super::{checked_range, Rejection, Side, MAX_HTML_BYTES, MAX_TOKENS};
use crate::translation::inline_markup::provider::scalar_range_from_utf8;

pub(crate) struct TokenRules<'a> {
    pub(crate) identity: &'a str,
    pub(crate) unknown: u32,
    pub(crate) reserved: &'a [u32],
    /// V1 checks one unpadded pair with a leading/trailing framing token.
    pub(crate) prefix: u32,
    pub(crate) suffix: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct Token {
    pub(crate) id: u32,
    pub(crate) word: Option<usize>,
    /// Raw-text UTF-8 offsets, not offsets into a separately split/normalized string.
    pub(crate) bytes: Range<usize>,
    pub(crate) special: bool,
}

pub(crate) struct TokenizedText<'a> {
    pub(crate) normalized: &'a str,
    pub(crate) tokens: &'a [Token],
}

pub(super) struct Word {
    pub(super) range: Range<usize>,
    pub(super) lexical: bool,
}

/// Validate trusted tokenizer evidence before deriving word ranges; model text cannot label its own framing.
pub(super) fn check(
    text: &str,
    rules: &TokenRules<'_>,
    evidence: &TokenizedText<'_>,
    side: Side,
) -> Result<Vec<Word>, Rejection> {
    if evidence.tokens.len() > MAX_TOKENS || evidence.normalized.len() > MAX_HTML_BYTES {
        return Err(Rejection::Limit);
    }
    if rules.reserved.len() > 64
        || ![rules.unknown, rules.prefix, rules.suffix]
            .iter()
            .all(|id| rules.reserved.contains(id))
        || rules.prefix == rules.suffix
        || rules.unknown == rules.prefix
        || rules.unknown == rules.suffix
    {
        return Err(Rejection::TokenProfile);
    }
    if !text
        .chars()
        .filter(|c| !c.is_whitespace())
        .eq(evidence.normalized.chars().filter(|c| !c.is_whitespace()))
    {
        return Err(Rejection::Normalization(side));
    }
    if evidence.tokens.len() < 3 {
        return Err(Rejection::Framing(side));
    }
    let mut covered = vec![false; text.len()];
    let mut words: Vec<Range<usize>> = Vec::new();
    let mut end = 0;
    for (index, token) in evidence.tokens.iter().enumerate() {
        if index == 0 || index + 1 == evidence.tokens.len() {
            let expected = if index == 0 {
                rules.prefix
            } else {
                rules.suffix
            };
            if token.id != expected
                || token.word.is_some()
                || !token.special
                || token.bytes != (0..0)
            {
                return Err(Rejection::Framing(side));
            }
            continue;
        }
        if token.id == rules.unknown {
            return Err(Rejection::UnknownToken(side));
        }
        if rules.reserved.contains(&token.id) {
            return Err(Rejection::ReservedToken(side));
        }
        let word = token.word.ok_or(Rejection::TokenRange(side))?;
        if token.special
            || token.bytes.start < end
            || token.bytes.is_empty()
            || text.get(token.bytes.clone()).is_none()
        {
            return Err(Rejection::TokenRange(side));
        }
        // Subword boundaries may split a combining sequence; only the merged word must be a whole grapheme.
        covered[token.bytes.clone()].fill(true);
        if word == words.len() {
            words.push(token.bytes.clone());
        } else if word.checked_add(1) == Some(words.len()) && token.bytes.start == end {
            words[word].end = token.bytes.end;
        } else {
            return Err(Rejection::TokenRange(side));
        }
        end = token.bytes.end;
    }
    if text
        .char_indices()
        .any(|(i, c)| !c.is_whitespace() && covered[i..i + c.len_utf8()].iter().any(|b| !b))
    {
        return Err(Rejection::TokenCoverage(side));
    }
    words
        .into_iter()
        .map(|bytes| {
            let lexical = text[bytes.clone()]
                .chars()
                .any(|c| c.is_alphanumeric() || unicode_normalization::char::is_combining_mark(c));
            let scalars =
                scalar_range_from_utf8(text, bytes).map_err(|_| Rejection::TokenRange(side))?;
            checked_range(text, &scalars, side)?;
            Ok(Word {
                range: scalars,
                lexical,
            })
        })
        .collect()
}
