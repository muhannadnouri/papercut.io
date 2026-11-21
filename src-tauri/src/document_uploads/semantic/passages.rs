//! Natural boundaries inside one anchored body/auxiliary region; never join sources.

use super::store::OVERLAP_TOKENS;
use std::ops::Range;

/// Prefer complete sentences after half a window, keeping at most half a window of repeated context.
/// Token/character ceilings win when a sentence or unbroken word is too long.
pub(crate) fn windows(
    text: &str,
    offsets: &[(usize, usize)],
    token_budget: usize,
    character_budget: usize,
) -> Result<Vec<Range<usize>>, String> {
    if token_budget == 0 || character_budget == 0 || offsets.is_empty() {
        return Err("Semantic passage has no token/character room".into());
    }
    let mut previous = (0, 0);
    for &(start, end) in offsets {
        if start < previous.0 || end < previous.1 || start >= end || text.get(start..end).is_none()
        {
            return Err("Invalid semantic tokenizer offsets".into());
        }
        previous = (start, end);
    }
    let boundaries = natural_boundaries(text);
    let mut result = Vec::new();
    let mut first = 0;
    while first < offsets.len() {
        let start = offsets[first].0;
        let character_end = text[start..]
            .char_indices()
            .nth(character_budget)
            .map_or(text.len(), |(byte, _)| start + byte);
        let last = (first + token_budget)
            .min(offsets.len())
            .min(offsets.partition_point(|(_, end)| *end <= character_end));
        if last <= first {
            return Err("Semantic token exceeds the character budget".into());
        }
        let hard_end = offsets[last - 1].1;
        let minimum_end = offsets[first + (last - first) / 2].1;
        let end = if hard_end == offsets.last().unwrap().1 {
            hard_end
        } else {
            boundaries[boundaries.partition_point(|(end, _)| *end < minimum_end)
                ..boundaries.partition_point(|(end, _)| *end <= hard_end)]
                .last()
                .filter(|(end, _)| *end > start)
                .map_or_else(
                    || word_end(text, start, hard_end, minimum_end),
                    |(end, _)| *end,
                )
        };
        result.push(start..end);
        let used = offsets.partition_point(|(_, token_end)| *token_end <= end);
        if used == offsets.len() {
            break;
        }
        // Character-limited windows must advance instead of repeatedly overlapping one token.
        let overlap = OVERLAP_TOKENS.min((used - first) / 3);
        let target = offsets[used - overlap].0;
        let minimum_start = offsets[first + (used - first) / 2].0;
        let next = boundaries[boundaries.partition_point(|(_, next)| *next < minimum_start)
            ..boundaries.partition_point(|(_, next)| *next <= target)]
            .iter()
            .rev()
            .find(|(_, next)| *next > start && *next < end)
            .map_or_else(
                || word_start(text, minimum_start, target),
                |(_, next)| *next,
            );
        // SentencePiece tokens can include preceding whitespace; keep the token containing the start.
        let next_first = offsets
            .partition_point(|(_, token_end)| *token_end <= next)
            .max(first + (used - first) / 2);
        first = if next_first > first && next_first < used {
            next_first
        } else {
            used
        };
    }
    Ok(result)
}

/// Closing quotes belong to the sentence; whitespace belongs to neither neighbor.
fn natural_boundaries(text: &str) -> Vec<(usize, usize)> {
    let mut boundaries = Vec::new();
    for (byte, ch) in text.char_indices() {
        let paragraph = ch == '\n'
            && (text[byte..].starts_with("\n\n") || text[byte..].starts_with("\n\r\n"))
            && !text[..byte].trim_end_matches('\r').ends_with('\n');
        if !paragraph && !matches!(ch, '.' | '!' | '?' | '؟' | '。' | '！' | '？') {
            continue;
        }
        let end = if paragraph {
            text[..byte].trim_end().len()
        } else {
            let punctuation_end = byte + ch.len_utf8();
            let closers = text[punctuation_end..]
                .chars()
                .take_while(|ch| {
                    matches!(
                        ch,
                        '"' | '\'' | '”' | '’' | '»' | ')' | ']' | '」' | '』' | '）' | '】'
                    )
                })
                .map(char::len_utf8)
                .sum::<usize>();
            punctuation_end + closers
        };
        let tail = &text[end..];
        // shortcut: punctuation is not linguistic segmentation; add language-aware rules only for measured boundary failures.
        if !paragraph && ch == '.' && !tail.is_empty() && !tail.starts_with(char::is_whitespace) {
            continue;
        }
        let next = text.len() - tail.trim_start().len();
        if end > 0 {
            boundaries.push((end, next));
        }
    }
    boundaries.sort_unstable();
    boundaries.dedup();
    boundaries
}

fn word_end(text: &str, start: usize, hard_end: usize, minimum: usize) -> usize {
    if text[hard_end..].starts_with(char::is_whitespace) {
        return hard_end;
    }
    text[start..hard_end]
        .rfind(char::is_whitespace)
        .map(|byte| start + text[start..start + byte].trim_end().len())
        .filter(|end| *end > start && *end >= minimum)
        .unwrap_or(hard_end)
}

fn word_start(text: &str, minimum: usize, target: usize) -> usize {
    text[minimum..target]
        .rfind(char::is_whitespace)
        .map(|byte| {
            let tail = &text[minimum + byte..target];
            target - tail.trim_start().len()
        })
        .unwrap_or(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn words(text: &str) -> Vec<(usize, usize)> {
        let mut end = 0;
        text.split_whitespace()
            .map(|word| {
                let start = end + text[end..].find(word).unwrap();
                end = start + word.len();
                (start, end)
            })
            .collect()
    }
    #[test]
    fn sentences_keep_closing_quotes_and_overlap_complete_context() {
        let text = "The harbor runs electric ferries daily. Riders board near the old station. “Tickets remain valid for the whole journey.” Boats also stop at the island pier. Visitors return before the last evening sailing.";
        let words = words(text);
        let mut whitespace_tokens = words.clone();
        for index in 1..whitespace_tokens.len() {
            whitespace_tokens[index].0 = words[index - 1].1;
        }
        for offsets in [&words, &whitespace_tokens] {
            let spans = windows(text, offsets, 18, 4096).unwrap();
            assert!(spans.len() > 1);
            assert!(spans
                .iter()
                .all(|span| text[span.clone()].ends_with(['.', '”'])));
            assert!(text[spans[1].clone()]
                .trim_start()
                .starts_with("Riders board"));
            assert!(spans[1].start < spans[0].end);
            assert!(spans
                .windows(2)
                .all(|pair| pair[0].end >= pair[1].start && pair[0].start < pair[1].start));
            assert_eq!(spans.last().unwrap().end, text.len());
        }
    }
    #[test]
    fn paragraph_and_multilingual_boundaries_use_original_utf8_offsets() {
        let text = "Travel costs 3.14 units at example.test.\n\n港口有电动渡船。乘客在车站买票！🙂 الحافلات تصل إلى الميناء؟ ثم يعود الركاب.";
        let boundaries = natural_boundaries(text);
        assert!(!boundaries
            .iter()
            .any(|(end, _)| *end == text.find("3.14").unwrap() + 2));
        assert!(!boundaries
            .iter()
            .any(|(end, _)| *end == text.find("example.test").unwrap() + 8));
        for ending in ["test.", "渡船。", "买票！", "الميناء؟"] {
            let end = text.find(ending).unwrap() + ending.len();
            assert!(boundaries.iter().any(|(byte, _)| *byte == end));
        }
        for separator in ["\n\n", "\r\n\r\n"] {
            let paragraphs = format!("First paragraph{separator}Second paragraph");
            assert_eq!(
                natural_boundaries(&paragraphs),
                vec![(15, 15 + separator.len())]
            );
        }
        let offsets = text
            .char_indices()
            .filter(|(_, ch)| !ch.is_whitespace())
            .map(|(start, ch)| (start, start + ch.len_utf8()))
            .collect::<Vec<_>>();
        let spans = windows(text, &offsets, 35, 35).unwrap();
        assert!(spans
            .iter()
            .all(|span| text.get(span.clone()).unwrap().chars().count() <= 35));
        assert!(spans.windows(2).all(|pair| pair[0].end >= pair[1].start));
        assert_eq!(spans.last().unwrap().end, text.len());
    }
    #[test]
    fn oversized_unbroken_text_still_advances_and_invalid_offsets_fail() {
        let text = "abcdefghij".repeat(20);
        let offsets = text
            .char_indices()
            .map(|(start, _)| (start, start + 1))
            .collect::<Vec<_>>();
        let spans = windows(&text, &offsets, 20, 8).unwrap();
        assert!(spans.iter().all(|span| span.len() <= 8));
        assert!(spans
            .windows(2)
            .all(|pair| pair[0].start < pair[1].start && pair[0].end >= pair[1].start));
        assert_eq!(spans.last().unwrap().end, text.len());
        assert!(windows("港", &[(1, 3)], 10, 10).is_err());
        assert!(windows("text", &[(2, 4), (0, 2)], 10, 10).is_err());
        assert!(windows("text", &[(0, 4)], 10, 2).is_err());
        let repeated_offsets = [(0, 4); 6];
        assert_eq!(
            windows("a. b", &repeated_offsets, 3, 10).unwrap(),
            vec![0..4]
        );
    }
}
