//! Authored English word forms for explicit, conservative typo suggestions.

use std::collections::HashSet;

pub(super) fn words(text: &str) -> HashSet<String> {
    text.split(|ch: char| !ch.is_ascii_alphabetic())
        .filter(|word| (5..=24).contains(&word.len()))
        .map(str::to_ascii_lowercase)
        .collect()
}

pub(super) fn eligible_query_word(word: &str) -> bool {
    (5..=24).contains(&word.len()) && word.bytes().all(|ch| ch.is_ascii_lowercase())
}

/// ASCII-only one insertion, deletion, or substitution; no fuzzy Unicode
/// folding and no full edit-distance matrix for an explicitly narrow feature.
pub(super) fn one_edit_apart(left: &str, right: &str) -> bool {
    let a = left.as_bytes();
    let b = right.as_bytes();
    if a.len().abs_diff(b.len()) > 1 || a == b {
        return false;
    }
    let (mut i, mut j, mut edits) = (0, 0, 0);
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            i += 1;
            j += 1;
            continue;
        }
        edits += 1;
        if edits > 1 {
            return false;
        }
        match a.len().cmp(&b.len()) {
            std::cmp::Ordering::Less => j += 1,
            std::cmp::Ordering::Greater => i += 1,
            std::cmp::Ordering::Equal => {
                i += 1;
                j += 1;
            }
        }
    }
    edits + (a.len() - i).max(b.len() - j) == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_one_ascii_edit_and_authored_words() {
        assert!(one_edit_apart("enviroment", "environment"));
        assert!(one_edit_apart("collonialism", "colonialism"));
        assert!(!one_edit_apart("enviroment", "environmental"));
        assert!(!one_edit_apart("environment", "environment"));
        assert!(!eligible_query_word("Arabicالعربية"));
        assert!(!eligible_query_word("ABC123"));
        assert_eq!(
            words("Environment, collonialism. 123 HTML العربية"),
            HashSet::from(["environment".into(), "collonialism".into()])
        );
    }
}
