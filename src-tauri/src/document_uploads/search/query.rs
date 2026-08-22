//! User-query normalization and FTS5 expression construction.
//!
//! This module owns syntax only. It does not know about SQLite connections,
//! result ranking, evidence, or document scopes.

use std::collections::HashSet;

const MAX_COMPARISON_TERMS: usize = 6;

/// Turn a broad/fuzzy query into safe FTS5 terms for candidate lookup.
pub(super) fn fts_fuzzy_terms(query: &str) -> Vec<String> {
    fts_terms(query, 12)
}

#[cfg(test)]
pub(super) fn fts_fuzzy_queries(query: &str) -> Vec<String> {
    fts_fuzzy_terms(query)
        .iter()
        .map(|term| fts_alias_query(term))
        .collect()
}

/// Pair each distinct display term with its FTS alias expression only for the
/// small broad queries whose per-term coverage is shown in result cards.
pub(super) fn comparison_terms(
    terms: &[String],
    queries: &[String],
    broad_search: bool,
) -> Vec<(String, String)> {
    if !broad_search {
        return Vec::new();
    }

    let mut seen = HashSet::new();
    let unique = terms
        .iter()
        .cloned()
        .zip(queries.iter().cloned())
        .filter(|(term, _)| seen.insert(term.to_lowercase()))
        .collect::<Vec<_>>();
    if (2..=MAX_COMPARISON_TERMS).contains(&unique.len()) {
        unique
    } else {
        Vec::new()
    }
}

/// Convert each user phrase to one safely quoted FTS5 phrase. The larger bound
/// preserves normal quotations while keeping pathological pasted input finite.
pub(super) fn fts_phrase_queries(phrases: &[String]) -> Vec<String> {
    phrases
        .iter()
        .filter_map(|phrase| {
            let terms = fts_terms(phrase, 128);
            (!terms.is_empty()).then(|| fts_alias_query(&terms.join(" ")))
        })
        .collect()
}

/// Extract bounded FTS-safe tokens while retaining internal hyphens so the
/// alias builder can distinguish punctuation from copied PDF line wrapping.
pub(super) fn fts_terms(query: &str, limit: usize) -> Vec<String> {
    collapse_hyphen_spacing(&normalize_search_punctuation(query))
        .split_whitespace()
        .map(|part| part.trim_matches(|ch: char| !ch.is_alphanumeric() && ch != '_' && ch != '-'))
        .filter(|part| !part.is_empty())
        .take(limit)
        .map(|term| term.replace('"', ""))
        .collect()
}

/// Case-fold the canonical display copy for literal comparison only; callers
/// that map matches back to an excerpt keep the display copy for offset checks.
pub(super) fn normalize_exact_text(text: &str) -> String {
    normalize_exact_display(text).to_lowercase()
}

/// Normalize punctuation, PDF line wrapping, and whitespace without
/// lowercasing, since Unicode case folding can change character counts.
pub(super) fn normalize_exact_display(text: &str) -> String {
    let punctuation = normalize_search_punctuation(text);
    remove_internal_word_hyphens(&collapse_hyphen_spacing(&punctuation))
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_search_punctuation(text: &str) -> String {
    text.replace(['\u{2018}', '\u{2019}'], "'")
        .replace(['\u{201c}', '\u{201d}'], "\"")
        .replace(
            [
                '\u{2010}', '\u{2011}', '\u{2012}', '\u{2013}', '\u{2014}', '\u{2015}',
            ],
            "-",
        )
}

/// Treat each of the first four internal hyphens independently as punctuation
/// or a PDF line wrap. The cap prevents pasted input from expanding into an
/// unbounded FTS expression while covering normal multi-compound phrases.
pub(super) fn fts_alias_query(text: &str) -> String {
    let aliases = internal_hyphen_aliases(text);
    if aliases.len() == 1 {
        return quote_fts_term(text);
    }
    format!(
        "({})",
        aliases
            .iter()
            .map(|alias| quote_fts_term(alias))
            .collect::<Vec<_>>()
            .join(" OR ")
    )
}

/// Generate independent kept/removed forms for only the first few internal
/// hyphens; the final fully joined form also covers compounds beyond that cap.
fn internal_hyphen_aliases(text: &str) -> Vec<String> {
    const MAX_VARIABLE_HYPHENS: usize = 4;

    let characters = text.chars().collect::<Vec<_>>();
    let positions = characters
        .iter()
        .enumerate()
        .filter_map(|(index, character)| {
            (*character == '-'
                && index > 0
                && index + 1 < characters.len()
                && characters[index - 1].is_alphabetic()
                && characters[index + 1].is_alphabetic())
            .then_some(index)
        })
        .take(MAX_VARIABLE_HYPHENS)
        .collect::<Vec<_>>();
    if positions.is_empty() {
        return vec![text.to_string()];
    }

    let mut aliases = Vec::with_capacity(1 << positions.len());
    for mask in 0..(1 << positions.len()) {
        let alias = characters
            .iter()
            .enumerate()
            .filter_map(|(index, character)| {
                let position = positions.iter().position(|candidate| *candidate == index);
                let removed = position.is_some_and(|position| mask & (1 << position) != 0);
                (!removed).then_some(*character)
            })
            .collect::<String>();
        if !aliases.contains(&alias) {
            aliases.push(alias);
        }
    }

    let fully_joined = remove_internal_word_hyphens(text);
    if !aliases.contains(&fully_joined) {
        aliases.push(fully_joined);
    }
    aliases
}

/// Collapse only whitespace between a letter-ending hyphen and the next
/// letter, turning copied PDF text such as `high- lights` into one query term.
fn collapse_hyphen_spacing(text: &str) -> String {
    let characters = text.chars().collect::<Vec<_>>();
    let mut output = String::with_capacity(text.len());
    let mut index = 0;

    while index < characters.len() {
        let character = characters[index];
        let follows_letter = output.chars().last().is_some_and(char::is_alphabetic);
        output.push(character);
        index += 1;
        if character != '-' || !follows_letter {
            continue;
        }

        let whitespace_start = index;
        while index < characters.len() && characters[index].is_whitespace() {
            index += 1;
        }
        if index == whitespace_start
            || index >= characters.len()
            || !characters[index].is_alphabetic()
        {
            for character in &characters[whitespace_start..index] {
                output.push(*character);
            }
        }
    }

    output
}

/// Build the canonical alias used by dehyphenated PDF search projections.
fn remove_internal_word_hyphens(text: &str) -> String {
    let characters = text.chars().collect::<Vec<_>>();
    characters
        .iter()
        .enumerate()
        .filter_map(|(index, character)| {
            let internal_hyphen = *character == '-'
                && index > 0
                && index + 1 < characters.len()
                && characters[index - 1].is_alphabetic()
                && characters[index + 1].is_alphabetic();
            (!internal_hyphen).then_some(*character)
        })
        .collect()
}

pub(super) fn fts_and_query(queries: &[String]) -> String {
    queries.join(" AND ")
}

pub(super) fn fts_or_query(queries: &[String]) -> String {
    queries.join(" OR ")
}

fn quote_fts_term(term: &str) -> String {
    format!("\"{}\"", term.replace('"', ""))
}
