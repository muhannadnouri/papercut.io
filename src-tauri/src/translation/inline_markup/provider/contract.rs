//! Pure evidence validation, not semantic approval or a runnable alignment provider.
//!
//! Trusted planning/tokenizer adapters supply exact text and provenance; a model
//! supplies links/ranges only. No HTML, inference, IPC, cache or storage lives here.
use std::{collections::BTreeSet, ops::Range};

use unicode_segmentation::UnicodeSegmentation;

use super::super::protected::{ProtectedPlan, SourceCode, MAX_CODES, MAX_HTML_BYTES};
use tokens::Word;

mod tokens;
pub(crate) use tokens::{Token, TokenRules, TokenizedText};
mod projection;
pub(crate) use projection::WordLinks;
pub(crate) use projection::POLICY as PROJECTION_POLICY;

/// Admit real tokenizer output before inference; pair checking repeats this at the mapping boundary.
pub(crate) fn validate_tokens(
    text: &str,
    rules: &TokenRules<'_>,
    evidence: &TokenizedText<'_>,
    side: Side,
) -> Result<(), Rejection> {
    tokens::check(text, rules, evidence, side).map(|_| ())
}

const CONTRACT_VERSION: u32 = 2;
const MAX_TOKENS: usize = 512;
pub(crate) const MAX_LINKS: usize = MAX_TOKENS * 8;
pub(super) const MAX_MAPPED_RANGES: usize = MAX_TOKENS;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Versions {
    /// Each identity includes the pinned model/tokenizer revision, not a display name.
    pub(crate) translator: String,
    pub(crate) aligner: String,
    pub(crate) tokenizer: String,
    pub(crate) normalization: String,
    /// Includes extraction configuration/thresholds and projection semantics.
    pub(crate) policy: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Binding {
    pub(crate) version: u32,
    pub(crate) versions: Versions,
    pub(crate) segment_id: String,
    pub(crate) source_ordinal: usize,
    /// Parent coordinates are scalars; all reply ranges below are pair-local scalars.
    pub(crate) source_range: Range<usize>,
    pub(crate) target_range: Range<usize>,
    pub(crate) source_plan: String,
    pub(crate) source: String,
    pub(crate) target: String,
}

pub(crate) struct Pair<'a> {
    pub(crate) plan: &'a ProtectedPlan,
    pub(crate) segment_id: &'a str,
    pub(crate) source_ordinal: usize,
    pub(crate) source_start: usize,
    pub(crate) target_start: usize,
    pub(crate) target: &'a str,
    pub(crate) versions: &'a Versions,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NodeMapping {
    pub(crate) source_id: usize,
    pub(crate) targets: Vec<Range<usize>>,
}

#[derive(Debug, Clone)]
pub(crate) struct Reply {
    pub(crate) binding: Binding,
    /// Word indices into the checked tokenizer evidence; many-to-many links are legal.
    pub(crate) links: Vec<(usize, usize)>,
    pub(crate) nodes: Vec<NodeMapping>,
    /// A known blocker must survive even a complete graph; None never means semantic approval.
    pub(crate) unresolved: Option<ScopeIssue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScopeIssue {
    Auxiliary,
    FusedMeaning,
    Attachment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
    Source,
    Target,
}

/// Errors contain coordinates/categories only, so diagnostics cannot echo private prose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Rejection {
    Limit,
    Binding,
    TokenProfile,
    Framing(Side),
    UnknownToken(Side),
    ReservedToken(Side),
    Normalization(Side),
    TokenRange(Side),
    TokenCoverage(Side),
    Grapheme(Side),
    Link,
    Identity,
    Hierarchy,
    IdentitySplit,
    MappingRange,
    ScopeMismatch,
    UnresolvedSource,
    UnresolvedTarget,
    MixedOwnership,
    PartialWord,
    LiteralRequiresIsolation,
    AttachmentRequiresEvidence,
    Reconstruction,
    Unresolved(ScopeIssue),
}

/// Passing checks is structural evidence only; it is not renderer HTML or a semantic confidence score.
#[derive(Debug)]
pub(crate) struct CheckedMapping<'a> {
    nodes: &'a [NodeMapping],
}

impl CheckedMapping<'_> {
    /// Only successful validation constructs this view; consumers cannot forge checked evidence.
    pub(crate) fn nodes(&self) -> &[NodeMapping] {
        self.nodes
    }
}

impl Pair<'_> {
    /// Bind exact strings/DOM/settings and checked parent offsets before any model reply is accepted.
    pub(crate) fn binding(&self) -> Result<Binding, Rejection> {
        let source = self.plan.source_text();
        if source.len() > MAX_HTML_BYTES
            || self.target.len() > MAX_HTML_BYTES
            || !self.plan.alignment_payload_fits(MAX_HTML_BYTES * 8)
        {
            return Err(Rejection::Limit);
        }
        for key in [
            self.segment_id,
            &self.versions.translator,
            &self.versions.aligner,
            &self.versions.tokenizer,
            &self.versions.normalization,
            &self.versions.policy,
        ] {
            if key.trim().is_empty() || key.len() > 256 {
                return Err(Rejection::Binding);
            }
        }
        if source.trim().is_empty() || self.target.trim().is_empty() {
            return Err(Rejection::Binding);
        }
        let source_end = self
            .source_start
            .checked_add(source.chars().count())
            .ok_or(Rejection::Binding)?;
        let target_end = self
            .target_start
            .checked_add(self.target.chars().count())
            .ok_or(Rejection::Binding)?;
        Ok(Binding {
            version: CONTRACT_VERSION,
            versions: self.versions.clone(),
            segment_id: self.segment_id.into(),
            source_ordinal: self.source_ordinal,
            source_range: self.source_start..source_end,
            target_range: self.target_start..target_end,
            source_plan: self.plan.alignment_identity(),
            source: source.into(),
            target: self.target.into(),
        })
    }

    /// Validate all evidence atomically; tokenizer evidence must come from a verified adapter, not MT.
    pub(crate) fn check<'a>(
        &self,
        rules: &TokenRules<'_>,
        source: &TokenizedText<'_>,
        target: &TokenizedText<'_>,
        reply: &'a Reply,
    ) -> Result<CheckedMapping<'a>, Rejection> {
        if reply.links.len() > MAX_LINKS
            || reply.nodes.len() > MAX_CODES
            || reply
                .nodes
                .iter()
                .try_fold(0usize, |sum, n| sum.checked_add(n.targets.len()))
                .is_none_or(|sum| sum > MAX_TOKENS)
        {
            return Err(Rejection::Limit);
        }
        if reply.binding != self.binding()? {
            return Err(Rejection::Binding);
        }
        if let Some(issue) = reply.unresolved {
            return Err(Rejection::Unresolved(issue));
        }
        if rules.identity != self.versions.tokenizer {
            return Err(Rejection::TokenProfile);
        }
        let sw = tokens::check(self.plan.source_text(), rules, source, Side::Source)?;
        let tw = tokens::check(self.target, rules, target, Side::Target)?;
        let codes: Vec<_> = self.plan.source_codes().collect();
        let mut seen = BTreeSet::new();
        for &(s, t) in &reply.links {
            if s >= sw.len() || t >= tw.len() || !seen.insert((s, t)) {
                return Err(Rejection::Link);
            }
        }
        check_nodes(&codes, self.plan.source_text(), self.target, &reply.nodes)?;
        if codes.iter().any(|c| c.range.is_empty()) {
            return Err(Rejection::AttachmentRequiresEvidence);
        }
        if codes.iter().any(|c| c.literal) {
            return Err(Rejection::LiteralRequiresIsolation);
        }
        // A uniform source-owned wrapper needs no guessed word-level scope, including auxiliaries.
        let whole_unit = !codes.is_empty()
            && codes
                .iter()
                .all(|c| c.range == (0..self.plan.source_text().chars().count()));
        if whole_unit {
            let full = 0..self.target.chars().count();
            if reply
                .nodes
                .iter()
                .any(|n| n.targets.as_slice() != [full.clone()])
            {
                return Err(Rejection::ScopeMismatch);
            }
        } else {
            check_scope(&codes, &sw, &tw, &reply.links, &reply.nodes)?;
        }
        Ok(CheckedMapping {
            nodes: &reply.nodes,
        })
    }
}

/// Convert scalars once per range, then reject splits inside a user-perceived character.
fn checked_range(text: &str, range: &Range<usize>, side: Side) -> Result<Range<usize>, Rejection> {
    let bytes: Vec<_> = text
        .char_indices()
        .map(|(i, _)| i)
        .chain([text.len()])
        .collect();
    if range.start > range.end || range.end >= bytes.len() {
        return Err(Rejection::MappingRange);
    }
    let result = bytes[range.start]..bytes[range.end];
    for position in [result.start, result.end] {
        if position != text.len() && !text.grapheme_indices(true).any(|(i, _)| i == position) {
            return Err(Rejection::Grapheme(side));
        }
    }
    Ok(result)
}

/// Scalar containment permits equal boundaries for nested wrappers, not crossing siblings.
fn contains(outer: &Range<usize>, inner: &Range<usize>) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

/// Check exact identity coverage, disjoint repeatable styles and containment without constructing HTML.
fn check_nodes(
    codes: &[SourceCode],
    source: &str,
    target: &str,
    nodes: &[NodeMapping],
) -> Result<(), Rejection> {
    if nodes.len() > MAX_CODES
        || nodes
            .iter()
            .try_fold(0usize, |n, node| n.checked_add(node.targets.len()))
            .is_none_or(|n| n > MAX_TOKENS)
    {
        return Err(Rejection::Limit);
    }
    // Precompute ancestry once; disjoint target ranges must not repeatedly walk the source tree.
    let mut ancestors = BTreeSet::new();
    for code in codes {
        let mut parent = code.parent;
        while let Some(id) = parent {
            ancestors.insert((id, code.id));
            parent = codes.iter().find(|c| c.id == id).and_then(|c| c.parent);
        }
    }
    let mut seen = BTreeSet::new();
    let mut ranges = Vec::new();
    for node in nodes {
        let code = codes
            .iter()
            .find(|c| c.id == node.source_id)
            .ok_or(Rejection::Identity)?;
        if !seen.insert(code.id) || node.targets.is_empty() {
            return Err(Rejection::Identity);
        }
        checked_range(source, &code.range, Side::Source)?;
        if !code.repeatable && node.targets.len() != 1 {
            return Err(Rejection::IdentitySplit);
        }
        let mut previous_end = None;
        for range in &node.targets {
            checked_range(target, range, Side::Target)?;
            if range.is_empty() != code.range.is_empty()
                || previous_end.is_some_and(|end| end > range.start)
            {
                return Err(Rejection::MappingRange);
            }
            previous_end = Some(range.end);
            if let Some(parent) = code.parent {
                if !nodes
                    .iter()
                    .find(|n| n.source_id == parent)
                    .is_some_and(|p| p.targets.iter().any(|r| contains(r, range)))
                {
                    return Err(Rejection::Hierarchy);
                }
            }
            ranges.push((code.id, range));
        }
    }
    if seen.len() != codes.len() {
        return Err(Rejection::Identity);
    }
    for (i, (a, ar)) in ranges.iter().enumerate() {
        for (b, br) in &ranges[i + 1..] {
            // Points occupy no text; reconstruction must split repeatable siblings or reject identity splits.
            if !ar.is_empty()
                && !br.is_empty()
                && ar.start < br.end
                && br.start < ar.end
                && !(ancestors.contains(&(*a, *b)) && contains(ar, br)
                    || ancestors.contains(&(*b, *a)) && contains(br, ar))
            {
                return Err(Rejection::Hierarchy);
            }
        }
    }
    Ok(())
}

/// Recheck the combined tree after piece-local projection; no clipped tree grants final identity approval.
pub(in crate::translation::inline_markup) fn check_projected_nodes(
    plan: &ProtectedPlan,
    target: &str,
    nodes: &[NodeMapping],
) -> Result<(), Rejection> {
    check_nodes(
        &plan.source_codes().collect::<Vec<_>>(),
        plan.source_text(),
        target,
        nodes,
    )
}

/// Coverage rejects known gaps, not semantic mistakes: complete incorrect graphs can still pass.
fn check_scope(
    codes: &[SourceCode],
    source: &[Word],
    target: &[Word],
    links: &[(usize, usize)],
    nodes: &[NodeMapping],
) -> Result<(), Rejection> {
    let owners = |word: &Range<usize>| -> Vec<usize> {
        codes
            .iter()
            .filter(|c| contains(&c.range, word))
            .map(|c| c.id)
            .collect()
    };
    check_source_scope(codes, source, links)?;
    for (t, word) in target.iter().enumerate() {
        let from: Vec<_> = links
            .iter()
            .filter(|(_, j)| *j == t)
            .map(|(i, _)| owners(&source[*i].range))
            .collect();
        if from.is_empty() && word.lexical {
            return Err(Rejection::UnresolvedTarget);
        }
        if from.iter().any(|owner| owner != &from[0]) {
            return Err(Rejection::MixedOwnership);
        }
        let word = &word.range;
        let actual: Vec<_> = nodes
            .iter()
            .filter(|n| n.targets.iter().any(|r| contains(r, word)))
            .map(|n| n.source_id)
            .collect();
        let mut actual = actual;
        actual.sort_unstable();
        let mut expected = from.first().cloned().unwrap_or_default();
        expected.sort_unstable();
        if actual != expected {
            return Err(Rejection::ScopeMismatch);
        }
        if nodes.iter().any(|n| {
            n.targets
                .iter()
                .any(|r| r.start < word.end && word.start < r.end && !contains(r, word))
        }) {
            return Err(Rejection::PartialWord);
        }
    }
    Ok(())
}

/// Reject partial words and absent required source links before trying to construct target scopes.
fn check_source_scope(
    codes: &[SourceCode],
    source: &[Word],
    links: &[(usize, usize)],
) -> Result<(), Rejection> {
    for (s, word) in source.iter().enumerate() {
        let word = &word.range;
        if codes.iter().any(|c| {
            c.range.start < word.end && word.start < c.range.end && !contains(&c.range, word)
        }) {
            return Err(Rejection::PartialWord);
        }
        if codes.iter().any(|c| contains(&c.range, word)) && !links.iter().any(|(i, _)| *i == s) {
            return Err(Rejection::UnresolvedSource);
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests;
