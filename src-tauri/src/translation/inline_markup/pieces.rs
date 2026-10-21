//! Source-owned sentence/literal planning and exact decoding; no alignment model, projection or persistence.
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

use super::protected::{ProtectedPlan, MAX_CODES, MAX_HTML_BYTES};
use crate::translation::{
    config::{DEFAULT_BATCH_SEGMENT_LIMIT, DEFAULT_MAX_SEGMENT_CHARS},
    segment::TranslationTextSegment,
};

const MAX_BOUNDARY_PROSE: usize = 128;
// Each MT piece has at most two whitespace joins; code endpoints add at most 2N+1 copied islands.
const MAX_BOUNDARY_UNITS: usize = MAX_BOUNDARY_PROSE * 3 + MAX_CODES * 2 + 1;

/// Providers translate each admitted input exactly once; hidden subdivisions must not be joined away.
pub(crate) trait ExactPieceTranslator {
    /// Include provider framing tokens in this check; errors must not contain document prose.
    fn source_fits(&self, source: &str) -> Result<bool, String>;
    /// Return one exact decoded target per input, rejecting decoder cutoffs before returning.
    fn translate_exact(&mut self, sources: &[&str]) -> Result<Vec<String>, String>;
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum Kind {
    Prose,
    Whitespace,
    Literal(usize),
    /// Sentence-policy punctuation/symbol fragments have no words to translate and can trigger hallucinations.
    Verbatim,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Unit {
    pub(crate) kind: Kind,
    /// Scalars in the retained segment source; adjacent units partition every source character.
    pub(crate) source: Range<usize>,
}

pub(crate) struct SourcePlan {
    /// The one source identity tree is retained intact; pieces never clone or clip authored identities.
    pub(crate) protected: ProtectedPlan,
    units: Vec<Unit>,
}

pub(crate) struct Translation<'a> {
    /// Borrow the producing plan, so parent identity, exact source and target pieces travel together.
    pub(crate) plan: &'a SourcePlan,
    pub(crate) text: String,
    /// One scalar range per source unit, including copied literals and explicit whitespace joins.
    pub(crate) targets: Vec<Range<usize>>,
}

impl SourcePlan {
    /// Keep whole-sentence context across styles; literals stay outside inference and every unit remains source-bound.
    pub(crate) fn from_sentences(
        segment: &TranslationTextSegment,
        protected: ProtectedPlan,
        translator: &dyn ExactPieceTranslator,
    ) -> Result<Self, String> {
        if segment.id.is_empty()
            || segment.id.len() > 256
            || segment.text != protected.source_text()
            || segment.source_end.checked_sub(segment.source_start)
                != Some(segment.text.chars().count())
            || segment.text.len() > MAX_HTML_BYTES
        {
            return Err("Exact-piece source binding mismatch".into());
        }
        if !protected.alignment_payload_fits(MAX_HTML_BYTES * 8) {
            return Err("Exact-piece source structure exceeds its bound".into());
        }
        let text = protected.source_text();
        let bytes = scalar_bytes(text);
        let graphemes = text
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .chain([text.len()])
            .collect::<Vec<_>>();
        let codes = protected.source_codes().collect::<Vec<_>>();
        let mut units = Vec::new();
        let mut cursor = 0;
        for code in codes.iter().filter(|c| c.literal && !c.range.is_empty()) {
            if code.range.start < cursor
                || code.range.end >= bytes.len()
                || graphemes.binary_search(&bytes[code.range.start]).is_err()
                || graphemes.binary_search(&bytes[code.range.end]).is_err()
            {
                return Err("Literal island has an invalid source boundary".into());
            }
            plan_prose(
                text,
                &bytes,
                cursor..code.range.start,
                translator,
                &mut units,
            )?;
            push(&mut units, Kind::Literal(code.id), code.range.clone())?;
            cursor = code.range.end;
        }
        plan_prose(
            text,
            &bytes,
            cursor..bytes.len() - 1,
            translator,
            &mut units,
        )?;
        // Whitespace trimming must not detach a combining mark from its original grapheme either.
        if units.iter().any(|u| {
            graphemes.binary_search(&bytes[u.source.start]).is_err()
                || graphemes.binary_search(&bytes[u.source.end]).is_err()
        }) {
            return Err("Exact-piece source boundary is inside a grapheme".into());
        }
        for code in codes.iter().filter(|c| c.range.is_empty()) {
            if code.range.start >= bytes.len()
                || graphemes.binary_search(&bytes[code.range.start]).is_err()
            {
                return Err("Attachment is inside a source grapheme".into());
            }
            if !units.iter().any(|u| {
                (u.source.start < code.range.start && code.range.start <= u.source.end)
                    || (code.range.start == 0 && u.source.start == 0)
            }) {
                return Err("Attachment has no source owner".into());
            }
        }
        Ok(Self { protected, units })
    }

    /// Only whole natural sentence scopes or exact copied ranges can retain target formatting.
    /// A tokenizer/budget cut is not evidence that half a sentence has the same semantic owner.
    pub(crate) fn formatting_needs_review(&self) -> bool {
        let text = self.protected.source_text();
        let mut sentence_edges = std::collections::BTreeSet::from([0, text.chars().count()]);
        let mut offset = 0;
        for sentence in text.split_sentence_bounds() {
            let start = offset + sentence.chars().take_while(|c| c.is_whitespace()).count();
            let end = offset + sentence.trim_end().chars().count();
            sentence_edges.extend([start, end]);
            offset += sentence.chars().count();
        }
        let supported = |point| {
            sentence_edges.contains(&point)
                && self.units.iter().any(|u| {
                    if u.kind != Kind::Prose {
                        u.source.start <= point && point <= u.source.end
                    } else {
                        point == u.source.start || point == u.source.end
                    }
                })
        };
        if self.units.iter().all(|u| u.kind != Kind::Prose) {
            return false; // Entirely copied content has exact correspondence, including punctuation and notes.
        }
        self.protected.source_codes().any(|code| {
            let copied = !code.range.is_empty()
                && self.units.iter().any(|u| {
                    u.kind != Kind::Prose
                        && u.source.start <= code.range.start
                        && code.range.end <= u.source.end
                });
            !copied && (!supported(code.range.start) || !supported(code.range.end))
        })
    }

    pub(crate) fn units(&self) -> &[Unit] {
        &self.units
    }
    /// Admit decoder inputs and exact outputs before reconstruction; never split a target independently.
    pub(crate) fn translate(
        &self,
        translator: &mut dyn ExactPieceTranslator,
        cancelled: impl Fn() -> bool,
    ) -> Result<Translation<'_>, String> {
        let source = self.protected.source_text();
        let bytes = scalar_bytes(source);
        let mut outputs = vec![None; self.units.len()];
        let prose = self
            .units
            .iter()
            .enumerate()
            .filter(|(_, u)| u.kind == Kind::Prose)
            .collect::<Vec<_>>();
        for chunk in prose.chunks(DEFAULT_BATCH_SEGMENT_LIMIT) {
            check_cancelled(&cancelled)?;
            let inputs = chunk
                .iter()
                .map(|(_, u)| &source[bytes[u.source.start]..bytes[u.source.end]])
                .collect::<Vec<_>>();
            for text in &inputs {
                if !translator.source_fits(text)? {
                    return Err("Exact-piece source exceeds the configured limits".into());
                }
            }
            let translated = translator.translate_exact(&inputs)?;
            check_cancelled(&cancelled)?;
            if translated.len() != chunk.len() {
                return Err("Exact-piece reply count mismatch".into());
            }
            for ((index, _), target) in chunk.iter().zip(translated) {
                if target.trim().is_empty() || target.len() > MAX_HTML_BYTES {
                    return Err("Exact-piece target is empty or exceeds its bound".into());
                }
                outputs[*index] = Some(target);
            }
        }
        let mut text = String::new();
        let mut targets = Vec::with_capacity(self.units.len());
        let mut position = 0;
        for (index, unit) in self.units.iter().enumerate() {
            let part = match unit.kind {
                Kind::Prose => outputs[index]
                    .as_deref()
                    .ok_or("Missing exact-piece target")?,
                _ => &source[bytes[unit.source.start]..bytes[unit.source.end]],
            };
            if text
                .len()
                .checked_add(part.len())
                .is_none_or(|n| n > MAX_HTML_BYTES)
            {
                return Err("Exact-piece target reconstruction exceeds its bound".into());
            }
            let end = position + part.chars().count();
            targets.push(position..end);
            text.push_str(part);
            position = end;
        }
        check_cancelled(&cancelled)?;
        let bytes = scalar_bytes(&text);
        let graphemes = text
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .chain([text.len()])
            .collect::<Vec<_>>();
        if targets.iter().any(|r| {
            graphemes.binary_search(&bytes[r.start]).is_err()
                || graphemes.binary_search(&bytes[r.end]).is_err()
        }) {
            return Err("Exact-piece target boundary is inside a grapheme".into());
        }
        Ok(Translation {
            plan: self,
            text,
            targets,
        })
    }
}

/// Sentence-first across styles, then bounded clause/whitespace cuts when the decoder budget requires it.
#[allow(clippy::too_many_arguments)]
fn plan_prose(
    text: &str,
    bytes: &[usize],
    range: Range<usize>,
    translator: &dyn ExactPieceTranslator,
    units: &mut Vec<Unit>,
) -> Result<(), String> {
    let max_units = MAX_BOUNDARY_UNITS;
    let mut pending = vec![range];
    let mut probes = 0;
    while let Some(range) = pending.pop() {
        if range.is_empty() {
            continue;
        }
        probes += 1;
        if probes > MAX_BOUNDARY_PROSE * 2 {
            return Err("Exact-piece planning exceeds its work bound".into());
        }
        let value = &text[bytes[range.start]..bytes[range.end]];
        if !value.chars().any(char::is_alphanumeric) {
            push(units, Kind::Verbatim, range)?;
            continue;
        }
        let trimmed = value.trim();
        if trimmed.is_empty() {
            push(units, Kind::Whitespace, range)?;
            continue;
        }
        let start = range.start
            + value[..value.len() - value.trim_start().len()]
                .chars()
                .count();
        let end = start + trimmed.chars().count();
        let sentence = cut_sentence(trimmed);
        if sentence.is_none()
            && end - start <= DEFAULT_MAX_SEGMENT_CHARS
            && translator.source_fits(trimmed)?
        {
            push(units, Kind::Whitespace, range.start..start)?;
            push(units, Kind::Prose, start..end)?;
            push(units, Kind::Whitespace, end..range.end)?;
        } else {
            let cut = sentence
                .or_else(|| cut_prose(trimmed))
                .ok_or("An indivisible prose unit exceeds a runtime limit")?;
            let cut = start + cut;
            pending.push(cut..range.end);
            pending.push(range.start..cut);
            if pending.len() + units.len() > max_units {
                return Err("Exact-piece planning exceeds its piece bound".into());
            }
        }
    }
    Ok(())
}

/// Balanced sentence, clause, then whitespace cuts bound repeated tokenization; no word/grapheme chopping.
fn cut_prose(text: &str) -> Option<usize> {
    if let Some(cut) = cut_sentence(text) {
        return Some(cut);
    }
    let clause = text
        .char_indices()
        .filter(|(_, c)| matches!(c, ',' | ';' | ':' | '،' | '؛'))
        .map(|(i, c)| i + c.len_utf8());
    let space = text
        .char_indices()
        .filter(|(_, c)| c.is_whitespace())
        .map(|(i, c)| i + c.len_utf8());
    nearest_cut(text, clause).or_else(|| nearest_cut(text, space))
}

/// Unicode sentence boundaries are model-independent and must not cut a source grapheme or invent a join.
fn cut_sentence(text: &str) -> Option<usize> {
    nearest_cut(
        text,
        text.split_sentence_bound_indices()
            .map(|(i, s)| i + s.len()),
    )
}

/// A balanced split bounds the existing work tree; return source scalars, never a rounded byte offset.
fn nearest_cut(text: &str, cuts: impl Iterator<Item = usize>) -> Option<usize> {
    let len = text.chars().count();
    let scalars = scalar_bytes(text);
    let graphemes = text
        .grapheme_indices(true)
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    cuts.filter(|i| {
        *i > 0
            && *i < text.len()
            && graphemes.binary_search(i).is_ok()
            && !text[..*i].trim().is_empty()
            && !text[*i..].trim().is_empty()
    })
    .min_by_key(|i| {
        scalars
            .binary_search(i)
            .expect("scalar cut")
            .abs_diff(len / 2)
    })
    .and_then(|cut| scalars.binary_search(&cut).ok())
}

/// Keep a bounded full partition; copied joins must not spend the boundary decoder's work allowance.
fn push(units: &mut Vec<Unit>, kind: Kind, range: Range<usize>) -> Result<(), String> {
    if range.is_empty() {
        return Ok(());
    }
    if units.last().is_some_and(|u| u.source.end != range.start) || range.start > range.end {
        return Err("Exact-piece source coverage mismatch".into());
    }
    if kind == Kind::Whitespace {
        if let Some(last) = units.last_mut().filter(|u| u.kind == Kind::Whitespace) {
            last.source.end = range.end;
            return Ok(());
        }
    }
    let max_units = MAX_BOUNDARY_UNITS;
    if units.len() >= max_units {
        return Err("Exact-piece planning exceeds its piece bound".into());
    }
    if kind == Kind::Prose
        && units.iter().filter(|u| u.kind == Kind::Prose).count() >= MAX_BOUNDARY_PROSE
    {
        return Err("Boundary planning exceeds its translation piece bound".into());
    }
    units.push(Unit {
        kind,
        source: range,
    });
    Ok(())
}

/// Include the terminal boundary so half-open scalar slices convert without rounding UTF-8 offsets.
fn scalar_bytes(text: &str) -> Vec<usize> {
    text.char_indices()
        .map(|(i, _)| i)
        .chain([text.len()])
        .collect()
}

fn check_cancelled(cancelled: &impl Fn() -> bool) -> Result<(), String> {
    if cancelled() {
        Err("Translation cancelled".into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;
