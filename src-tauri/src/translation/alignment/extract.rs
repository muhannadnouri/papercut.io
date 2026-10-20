//! Fixed Awesome-align-compatible softmax intersection, independently implemented in Rust.
//! Reference: neulab/awesome-align@5f150d45bbe51e167daf0a84abebaeb07c3323d1,
//! BertGuideHead.forward (unscaled dot products, threshold > .001 in both directions).
//! See extract/LICENSE for the reference attribution. Scores are not calibrated confidence.
use std::collections::BTreeSet;

use super::{
    runtime::{PairEmbeddings, SessionError, WIDTH},
    MAX_TOKENS,
};
use crate::translation::inline_markup::provider::contract::{
    self, Rejection, Side, TokenRules, WordLinks,
};

pub(crate) const POLICY: &str =
    "awesome-align-softmax-5f150d45:f32:unscaled:intersection-gt-0.001:v1";
const THRESHOLD: f32 = 0.001;

/// Framing stays in the graph but not in either softmax denominator or returned word-link indices.
pub(super) fn word_links<'a>(
    pair: &PairEmbeddings<'a>,
    rules: &TokenRules<'_>,
    cancelled: impl Fn() -> bool,
) -> Result<WordLinks<'a>, SessionError> {
    let source = pair.source.encoding.evidence();
    let target = pair.target.encoding.evidence();
    for (embedding, evidence, side) in [
        (&pair.source, &source, Side::Source),
        (&pair.target, &target, Side::Target),
    ] {
        contract::validate_tokens(embedding.encoding.text(), rules, evidence, side)?;
        if evidence.tokens.len() > MAX_TOKENS
            || embedding.values.len() != evidence.tokens.len() * WIDTH
            || embedding.values.iter().any(|v| !v.is_finite())
        {
            return Err(SessionError::Output);
        }
    }
    let sw = source
        .tokens
        .iter()
        .filter_map(|t| t.word)
        .collect::<Vec<_>>();
    let tw = target
        .tokens
        .iter()
        .filter_map(|t| t.word)
        .collect::<Vec<_>>();
    let mut scores = Vec::with_capacity(sw.len() * tw.len());
    for s in pair
        .source
        .values
        .chunks_exact(WIDTH)
        .skip(1)
        .take(sw.len())
    {
        if cancelled() {
            return Err(SessionError::Cancelled);
        }
        for t in pair
            .target
            .values
            .chunks_exact(WIDTH)
            .skip(1)
            .take(tw.len())
        {
            let dot = s.iter().zip(t).map(|(s, t)| s * t).sum::<f32>();
            if !dot.is_finite() {
                return Err(SessionError::Output);
            }
            scores.push(dot);
        }
    }
    let links = intersect(&scores, &sw, &tw)?;
    if cancelled() {
        return Err(SessionError::Cancelled);
    }
    Ok(WordLinks::new(
        pair.source.encoding.text(),
        pair.target.encoding.text(),
        rules,
        &source,
        &target,
        links,
    )?)
}

/// Stable row/column softmax uses the published strict threshold, not cosine, argmax or adjacency repair.
fn intersect(
    scores: &[f32],
    source: &[usize],
    target: &[usize],
) -> Result<Vec<(usize, usize)>, SessionError> {
    let (rows, cols) = (source.len(), target.len());
    if rows == 0
        || cols == 0
        || rows > MAX_TOKENS - 2
        || cols > MAX_TOKENS - 2
        || scores.len() != rows * cols
        || scores.iter().any(|v| !v.is_finite())
    {
        return Err(SessionError::Output);
    }
    let row_max = scores
        .chunks_exact(cols)
        .map(|r| r.iter().copied().fold(f32::NEG_INFINITY, f32::max))
        .collect::<Vec<_>>();
    let col_max = (0..cols)
        .map(|j| {
            (0..rows)
                .map(|i| scores[i * cols + j])
                .fold(f32::NEG_INFINITY, f32::max)
        })
        .collect::<Vec<_>>();
    let row_sum = (0..rows)
        .map(|i| {
            (0..cols)
                .map(|j| (scores[i * cols + j] - row_max[i]).exp())
                .sum::<f32>()
        })
        .collect::<Vec<_>>();
    let col_sum = (0..cols)
        .map(|j| {
            (0..rows)
                .map(|i| (scores[i * cols + j] - col_max[j]).exp())
                .sum::<f32>()
        })
        .collect::<Vec<_>>();
    let mut links = BTreeSet::new();
    for i in 0..rows {
        for j in 0..cols {
            let score = scores[i * cols + j];
            if (score - row_max[i]).exp() / row_sum[i] > THRESHOLD
                && (score - col_max[j]).exp() / col_sum[j] > THRESHOLD
            {
                links.insert((source[i], target[j]));
                if links.len() > contract::MAX_LINKS {
                    return Err(Rejection::Limit.into());
                }
            }
        }
    }
    Ok(links.into_iter().collect())
}

#[cfg(test)]
mod tests;
