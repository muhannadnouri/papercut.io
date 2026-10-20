//! Checked word correspondence and conservative scope construction; never HTML or semantic certification.
use super::*;

pub(crate) const POLICY: &str = "word-ownership-v1:whole-unit-or-complete-links:no-hull";

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
