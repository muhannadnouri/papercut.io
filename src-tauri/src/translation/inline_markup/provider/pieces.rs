//! Exact decoder units and copied literals, shared by boundary jobs and the disabled strict candidate.
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

use super::super::protected::{ProtectedPlan, ProtectedSource, MAX_HTML_BYTES};
use super::contract::Side;
use crate::translation::{
    config::{DEFAULT_BATCH_SEGMENT_LIMIT, DEFAULT_MAX_SEGMENT_CHARS},
    segment::TranslationTextSegment,
};

const MAX_UNITS: usize = 128;

mod projection;
pub(crate) use projection::Projection;
mod cached;
pub(crate) use cached::MappedSegment;

/// One job-owned backend supplies checked maps; engines and cache code never interpret embeddings.
pub(crate) trait PieceAligner {
    fn versions(&self, model: &str) -> Result<super::contract::Versions, String>;
    fn text_fits(&self, text: &str, side: Side) -> Result<bool, String>;
    fn map(
        &mut self,
        translation: &Translation<'_>,
    ) -> Result<Vec<super::contract::NodeMapping>, String>;
}

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
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Unit {
    pub(crate) kind: Kind,
    /// Scalars in the retained segment source; adjacent units partition every source character.
    pub(crate) source: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Attachment {
    pub(crate) source_id: usize,
    pub(crate) parent: Option<usize>,
    pub(crate) offset: usize,
    /// A cut belongs to the preceding unit; source start belongs to the first unit.
    pub(crate) owner: usize,
}

pub(crate) struct SourcePlan {
    pub(crate) segment_id: String,
    pub(crate) source_ordinal: usize,
    pub(crate) source_start: usize,
    /// The one source identity tree is retained intact; pieces never clone or clip authored identities.
    pub(crate) protected: ProtectedPlan,
    units: Vec<Unit>,
    attachments: Vec<Attachment>,
}

pub(crate) struct Translation<'a> {
    /// Borrow the producing plan, so parent identity, exact source and target pieces travel together.
    pub(crate) plan: &'a SourcePlan,
    pub(crate) text: String,
    /// One scalar range per source unit, including copied literals and explicit whitespace joins.
    pub(crate) targets: Vec<Range<usize>>,
}

impl SourcePlan {
    /// Start from the same DOM-normalized segment and reject a pre-existing cut through an authored identity.
    pub(crate) fn new(
        segment: &TranslationTextSegment,
        source: &ProtectedSource,
        translator: &dyn ExactPieceTranslator,
        aligner_fits: impl Fn(&str, Side) -> Result<bool, String>,
    ) -> Result<Self, String> {
        if segment.id.is_empty() || segment.id.len() > 256 || segment.text.len() > MAX_HTML_BYTES {
            return Err("Exact-piece source exceeds its bound".into());
        }
        let protected = source.fragment(segment.source_start..segment.source_end, &segment.text)?;
        Self::from_protected(segment, protected, translator, aligner_fits)
    }

    /// The runner already retains each bounded DOM fragment; do not extract or clip its identities again.
    pub(crate) fn from_protected(
        segment: &TranslationTextSegment,
        protected: ProtectedPlan,
        translator: &dyn ExactPieceTranslator,
        aligner_fits: impl Fn(&str, Side) -> Result<bool, String>,
    ) -> Result<Self, String> {
        Self::plan(segment, protected, translator, aligner_fits, false)
    }

    /// Boundary translation cuts at every source shell/attachment; it needs no semantic aligner.
    pub(crate) fn from_boundaries(
        segment: &TranslationTextSegment,
        protected: ProtectedPlan,
        translator: &dyn ExactPieceTranslator,
    ) -> Result<Self, String> {
        Self::plan(segment, protected, translator, |_, _| Ok(true), true)
    }

    /// Both policies retain the same size, coverage, literal, grapheme and tokenizer checks.
    fn plan(
        segment: &TranslationTextSegment,
        protected: ProtectedPlan,
        translator: &dyn ExactPieceTranslator,
        aligner_fits: impl Fn(&str, Side) -> Result<bool, String>,
        boundaries: bool,
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
        if boundaries {
            let mut cuts = vec![0, bytes.len() - 1];
            for code in &codes {
                cuts.extend([code.range.start, code.range.end]);
            }
            cuts.sort_unstable();
            cuts.dedup();
            for range in cuts.windows(2).map(|p| p[0]..p[1]) {
                if let Some(code) = codes.iter().find(|c| c.literal && c.range == range) {
                    push(&mut units, Kind::Literal(code.id), range)?;
                } else {
                    plan_prose(text, &bytes, range, translator, &aligner_fits, &mut units)?;
                }
            }
        } else {
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
                    &aligner_fits,
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
                &aligner_fits,
                &mut units,
            )?;
        }
        // Whitespace trimming must not detach a combining mark from its original grapheme either.
        if units.iter().any(|u| {
            graphemes.binary_search(&bytes[u.source.start]).is_err()
                || graphemes.binary_search(&bytes[u.source.end]).is_err()
        }) {
            return Err("Exact-piece source boundary is inside a grapheme".into());
        }
        let mut attachments = Vec::new();
        for code in codes.iter().filter(|c| c.range.is_empty()) {
            if code.range.start >= bytes.len()
                || graphemes.binary_search(&bytes[code.range.start]).is_err()
            {
                return Err("Attachment is inside a source grapheme".into());
            }
            let owner = units
                .iter()
                .position(|u| {
                    (u.source.start < code.range.start && code.range.start <= u.source.end)
                        || (code.range.start == 0 && u.source.start == 0)
                })
                .ok_or("Attachment has no source owner")?;
            attachments.push(Attachment {
                source_id: code.id,
                parent: code.parent,
                offset: code.range.start,
                owner,
            });
        }
        Ok(Self {
            segment_id: segment.id.clone(),
            source_ordinal: segment.source_block_index,
            source_start: segment.source_start,
            protected,
            units,
            attachments,
        })
    }

    pub(crate) fn units(&self) -> &[Unit] {
        &self.units
    }
    pub(crate) fn attachments(&self) -> &[Attachment] {
        &self.attachments
    }

    /// Validate both tokenizers before inference and target admission afterward; never split a target independently.
    pub(crate) fn translate(
        &self,
        translator: &mut dyn ExactPieceTranslator,
        aligner_fits: impl Fn(&str, Side) -> Result<bool, String>,
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
                if !translator.source_fits(text)? || !aligner_fits(text, Side::Source)? {
                    return Err("Exact-piece source exceeds the configured limits".into());
                }
            }
            let translated = translator.translate_exact(&inputs)?;
            check_cancelled(&cancelled)?;
            if translated.len() != chunk.len() {
                return Err("Exact-piece reply count mismatch".into());
            }
            for ((index, _), target) in chunk.iter().zip(translated) {
                if target.trim().is_empty()
                    || target.len() > MAX_HTML_BYTES
                    || !aligner_fits(&target, Side::Target)?
                {
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

/// Split only over-budget prose; formatting boundaries never enter the candidate cut list.
fn plan_prose(
    text: &str,
    bytes: &[usize],
    range: Range<usize>,
    translator: &dyn ExactPieceTranslator,
    aligner_fits: &impl Fn(&str, Side) -> Result<bool, String>,
    units: &mut Vec<Unit>,
) -> Result<(), String> {
    let mut pending = vec![range];
    let mut probes = 0;
    while let Some(range) = pending.pop() {
        if range.is_empty() {
            continue;
        }
        probes += 1;
        if probes > MAX_UNITS * 2 {
            return Err("Exact-piece planning exceeds its work bound".into());
        }
        let value = &text[bytes[range.start]..bytes[range.end]];
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
        if end - start <= DEFAULT_MAX_SEGMENT_CHARS
            && translator.source_fits(trimmed)?
            && aligner_fits(trimmed, Side::Source)?
        {
            push(units, Kind::Whitespace, range.start..start)?;
            push(units, Kind::Prose, start..end)?;
            push(units, Kind::Whitespace, end..range.end)?;
        } else {
            let cut =
                cut_prose(trimmed).ok_or("An indivisible prose unit exceeds a runtime limit")?;
            let cut = start + cut;
            pending.push(cut..range.end);
            pending.push(range.start..cut);
            if pending.len() + units.len() > MAX_UNITS {
                return Err("Exact-piece planning exceeds its piece bound".into());
            }
        }
    }
    Ok(())
}

/// Balanced sentence, clause, then whitespace cuts bound repeated tokenization; no word/grapheme chopping.
fn cut_prose(text: &str) -> Option<usize> {
    let len = text.chars().count();
    let scalars = scalar_bytes(text);
    let graphemes = text
        .grapheme_indices(true)
        .map(|(i, _)| i)
        .collect::<Vec<_>>();
    let sentence = text
        .split_sentence_bound_indices()
        .map(|(i, s)| i + s.len());
    let clause = text
        .char_indices()
        .filter(|(_, c)| matches!(c, ',' | ';' | ':' | '،' | '؛'))
        .map(|(i, c)| i + c.len_utf8());
    let space = text
        .char_indices()
        .filter(|(_, c)| c.is_whitespace())
        .map(|(i, c)| i + c.len_utf8());
    for cuts in [
        sentence.collect::<Vec<_>>(),
        clause.collect(),
        space.collect(),
    ] {
        if let Some(cut) = cuts
            .into_iter()
            .filter(|i| {
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
        {
            return scalars.binary_search(&cut).ok();
        }
    }
    None
}

/// Units form a complete partition; keep source whitespace joins explicit rather than inventing target spaces.
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
    if units.len() >= MAX_UNITS {
        return Err("Exact-piece planning exceeds its piece bound".into());
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
