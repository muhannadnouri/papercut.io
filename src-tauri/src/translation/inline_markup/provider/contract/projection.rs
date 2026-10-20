//! Checked word correspondence and conservative scope construction; never HTML or semantic certification.
use super::*;

pub(crate) const POLICY: &str =
    "word-ownership-v2:whole-unit-or-complete-links:no-hull:unique-anchor-gap-v1:source-dom-v1";

/// Exact input borrows bind word IDs to their original strings, not a second sentence/token split.
pub(crate) struct WordLinks<'a> {
    source: &'a str,
    target: &'a str,
    source_words: Vec<Word>,
    target_words: Vec<Word>,
    links: Vec<(usize, usize)>,
}

impl<'a> WordLinks<'a> {
    /// A model may supply links only; framing, offsets and word ownership come from checked tokenizer evidence.
    pub(crate) fn new(
        source: &'a str,
        target: &'a str,
        rules: &TokenRules<'_>,
        source_tokens: &TokenizedText<'_>,
        target_tokens: &TokenizedText<'_>,
        links: Vec<(usize, usize)>,
    ) -> Result<Self, Rejection> {
        if source.len() > MAX_HTML_BYTES || target.len() > MAX_HTML_BYTES || links.len() > MAX_LINKS
        {
            return Err(Rejection::Limit);
        }
        let source_words = tokens::check(source, rules, source_tokens, Side::Source)?;
        let target_words = tokens::check(target, rules, target_tokens, Side::Target)?;
        let mut seen = BTreeSet::new();
        for &(s, t) in &links {
            if s >= source_words.len() || t >= target_words.len() || !seen.insert((s, t)) {
                return Err(Rejection::Link);
            }
        }
        Ok(Self {
            source,
            target,
            source_words,
            target_words,
            links,
        })
    }

    pub(crate) fn links(&self) -> &[(usize, usize)] {
        &self.links
    }

    /// Adjacent lexical words must have unique reciprocal anchors; fused/missing/reversed gaps reject.
    /// Only the intervening punctuation may move a note relative to quotes, never another target word.
    pub(in crate::translation::inline_markup::provider) fn attachment(
        &self,
        offset: usize,
    ) -> Result<usize, Rejection> {
        if offset > self.source.chars().count()
            || self
                .source_words
                .iter()
                .any(|w| w.lexical && w.range.start < offset && offset < w.range.end)
        {
            return Err(Rejection::AttachmentRequiresEvidence);
        }
        let left = self
            .source_words
            .iter()
            .enumerate()
            .rfind(|(_, w)| w.lexical && w.range.end <= offset);
        let right = self
            .source_words
            .iter()
            .enumerate()
            .find(|(_, w)| w.lexical && offset <= w.range.start);
        if left.is_none() && right.is_none() {
            return Err(Rejection::AttachmentRequiresEvidence);
        }
        let anchor = |s: usize| -> Result<&Word, Rejection> {
            let targets = self
                .links
                .iter()
                .filter(|(i, _)| *i == s)
                .map(|(_, t)| *t)
                .collect::<Vec<_>>();
            let [t] = targets.as_slice() else {
                return Err(Rejection::AttachmentRequiresEvidence);
            };
            if !self.target_words[*t].lexical || self.links.iter().any(|(i, j)| j == t && *i != s) {
                return Err(Rejection::AttachmentRequiresEvidence);
            }
            Ok(&self.target_words[*t])
        };
        let sl = left.map_or(0, |(_, w)| w.range.end);
        let sr = right.map_or(self.source.chars().count(), |(_, w)| w.range.start);
        let tl = left
            .map(|(i, _)| anchor(i).map(|w| w.range.end))
            .transpose()?
            .unwrap_or(0);
        let tr = right
            .map(|(i, _)| anchor(i).map(|w| w.range.start))
            .transpose()?
            .unwrap_or(self.target.chars().count());
        if sl > offset || offset > sr || tl > tr {
            return Err(Rejection::AttachmentRequiresEvidence);
        }
        let gap = |text: &str, start, end| {
            text.chars()
                .skip(start)
                .take(end - start)
                .collect::<String>()
        };
        let delta = crate::translation::inline_markup::protected::boundaries::map_gap(
            &gap(self.source, sl, sr),
            offset - sl,
            &gap(self.target, tl, tr),
        )
        .map_err(|_| Rejection::AttachmentRequiresEvidence)?;
        Ok(tl + delta)
    }

    /// Reject stale/wrong-pair results even if the provider returns a plausible correspondence graph.
    pub(in crate::translation::inline_markup::provider) fn matches(
        &self,
        source: &str,
        target: &str,
    ) -> bool {
        self.source == source && self.target == target
    }

    /// Derive minimal word runs, then reuse the same identity/hierarchy/coverage guards as supplied replies.
    pub(in crate::translation::inline_markup::provider) fn project(
        &self,
        codes: &[SourceCode],
    ) -> Result<Vec<NodeMapping>, Rejection> {
        if codes.iter().any(|c| c.range.is_empty()) {
            return Err(Rejection::AttachmentRequiresEvidence);
        }
        if codes.iter().any(|c| c.literal) {
            return Err(Rejection::LiteralRequiresIsolation);
        }
        let whole = !codes.is_empty()
            && codes
                .iter()
                .all(|c| c.range == (0..self.source.chars().count()));
        if !whole {
            check_source_scope(codes, &self.source_words, &self.links)?;
        }
        let mut nodes = Vec::with_capacity(codes.len());
        let target_bytes = self
            .target
            .char_indices()
            .map(|(i, _)| i)
            .chain([self.target.len()])
            .collect::<Vec<_>>();
        for code in codes {
            let mut targets: Vec<Range<usize>> = Vec::new();
            if whole {
                targets.push(0..self.target.chars().count());
            } else {
                let mut owned = vec![false; self.target_words.len()];
                for &(s, t) in &self.links {
                    if contains(&code.range, &self.source_words[s].range) {
                        owned[t] = true;
                    }
                }
                for (t, word) in self.target_words.iter().enumerate() {
                    if owned[t] {
                        // Only whitespace between adjacent owned words may join a run; punctuation needs its own link.
                        if let Some(last) = targets.last_mut() {
                            let gap = &self.target
                                [target_bytes[last.end]..target_bytes[word.range.start]];
                            if gap.chars().all(char::is_whitespace) {
                                last.end = word.range.end;
                                continue;
                            }
                        }
                        targets.push(word.range.clone());
                    }
                }
                if targets.is_empty() {
                    return Err(Rejection::UnresolvedSource);
                }
            }
            nodes.push(NodeMapping {
                source_id: code.id,
                targets,
            });
        }
        check_nodes(codes, self.source, self.target, &nodes)?;
        if !whole {
            check_scope(
                codes,
                &self.source_words,
                &self.target_words,
                &self.links,
                &nodes,
            )?;
        }
        Ok(nodes)
    }
}

#[cfg(test)]
mod tests;
