//! Text segmentation for translation jobs.
//!
//! Document parsers already preserve the safe HTML and sections. Translation
//! still needs bounded text payloads so native engines can batch work without
//! overflowing context windows or freezing the UI on very large books.

use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TranslationTextSegment {
    pub(crate) id: String,
    pub(crate) source_block_index: usize,
    /// Character offsets inside the whitespace-normalized source block.
    ///
    /// These are not raw HTML byte offsets. They match the normalized text that
    /// goes to the translation engine and later lets inline-markup rendering
    /// place formatting spans without searching repeated phrases.
    pub(crate) source_start: usize,
    pub(crate) source_end: usize,
    pub(crate) text: String,
}

/// Split document text blocks into stable, bounded translation segments.
///
/// The segment ids are deterministic from source block order, which gives the
/// future job runner a cheap cache key for resume/retry. This is deliberately a
/// text-only primitive; HTML element mapping and anchor preservation should sit
/// above it so format-specific logic stays out of engine code.
pub(crate) fn segment_text_blocks<I, S>(
    blocks: I,
    max_chars: usize,
) -> Result<Vec<TranslationTextSegment>, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    if max_chars == 0 {
        return Err("Translation segment size must be greater than zero".into());
    }

    let mut segments = Vec::new();
    for (block_index, block) in blocks.into_iter().enumerate() {
        let normalized = normalize_translation_whitespace(block.as_ref());
        if normalized.is_empty() {
            continue;
        }
        let parts = split_block_into_segments(&normalized, max_chars)?;
        if parts.is_empty() {
            return Err(format!(
                "Translation planner could not segment source block {}",
                block_index + 1
            ));
        }
        for (part_index, part) in parts.into_iter().enumerate() {
            segments.push(TranslationTextSegment {
                id: format!("b{block_index}:s{part_index}"),
                source_block_index: block_index,
                source_start: part.start,
                source_end: part.end,
                text: part.text,
            });
        }
    }

    Ok(segments)
}

fn normalize_translation_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TranslationTextPart {
    start: usize,
    end: usize,
    text: String,
}

/// Split one normalized block and retain char offsets inside that normalized block.
///
/// Renderer alignment later uses the same whitespace-normalized source text,
/// so offsets are produced as the text is split. We avoid rebuilding strings
/// and searching for them again because URLs, initials, and citations can make
/// punctuation-based chunks differ from the original source text.
fn split_block_into_segments(
    text: &str,
    max_chars: usize,
) -> Result<Vec<TranslationTextPart>, String> {
    if text.chars().count() <= max_chars {
        return Ok(vec![TranslationTextPart {
            start: 0,
            end: text.chars().count(),
            text: text.to_string(),
        }]);
    }

    split_block_text_into_segments(text, max_chars)
}

fn split_block_text_into_segments(
    text: &str,
    max_chars: usize,
) -> Result<Vec<TranslationTextPart>, String> {
    let mut segments = Vec::<TranslationTextPart>::new();
    let mut current: Option<PartBuilder> = None;

    for sentence in sentence_like_parts(text) {
        if sentence.char_len > max_chars {
            push_current_part(text, &mut segments, &mut current);
            segments.extend(split_oversized_part(text, sentence, max_chars)?);
            continue;
        }

        let proposed_len = current
            .as_ref()
            .map(|builder| builder.char_len + sentence.char_len + 1)
            .unwrap_or(sentence.char_len);
        if proposed_len > max_chars {
            push_current_part(text, &mut segments, &mut current);
        }
        append_range(&mut current, sentence);
    }

    push_current_part(text, &mut segments, &mut current);
    Ok(segments)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SourceRange {
    byte_start: usize,
    byte_end: usize,
    char_start: usize,
    char_end: usize,
    char_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PartBuilder {
    byte_start: usize,
    byte_end: usize,
    char_start: usize,
    char_end: usize,
    char_len: usize,
}

impl PartBuilder {
    fn from_range(range: SourceRange) -> Self {
        Self {
            byte_start: range.byte_start,
            byte_end: range.byte_end,
            char_start: range.char_start,
            char_end: range.char_end,
            char_len: range.char_len,
        }
    }

    fn extend(&mut self, range: SourceRange) {
        let gap_chars = range.char_start.saturating_sub(self.char_end);
        self.byte_end = range.byte_end;
        self.char_end = range.char_end;
        self.char_len += gap_chars + range.char_len;
    }

    fn into_part(self, source: &str) -> TranslationTextPart {
        TranslationTextPart {
            start: self.char_start,
            end: self.char_end,
            text: source[self.byte_start..self.byte_end].trim().to_string(),
        }
    }
}

fn sentence_like_parts(text: &str) -> Vec<SourceRange> {
    let mut ranges = Vec::new();
    let mut next_char_start = 0usize;

    for (byte_start, sentence) in text.split_sentence_bound_indices() {
        let raw_char_len = sentence.chars().count();
        if let Some(range) = trim_source_range(
            text,
            byte_start,
            byte_start + sentence.len(),
            next_char_start,
        ) {
            ranges.push(range);
        }
        next_char_start += raw_char_len;
    }
    ranges
}

fn split_oversized_part(
    source: &str,
    range: SourceRange,
    max_chars: usize,
) -> Result<Vec<TranslationTextPart>, String> {
    let mut segments = Vec::<TranslationTextPart>::new();
    let mut current: Option<PartBuilder> = None;

    for word in word_ranges(source, range) {
        if word.char_len > max_chars {
            push_current_part(source, &mut segments, &mut current);
            segments.extend(split_long_word(source, word, max_chars)?);
            continue;
        }

        let proposed_len = current
            .as_ref()
            .map(|builder| builder.char_len + word.char_len + 1)
            .unwrap_or(word.char_len);
        if proposed_len > max_chars {
            push_current_part(source, &mut segments, &mut current);
        }
        append_range(&mut current, word);
    }

    push_current_part(source, &mut segments, &mut current);
    Ok(segments)
}

fn split_long_word(
    source: &str,
    range: SourceRange,
    max_chars: usize,
) -> Result<Vec<TranslationTextPart>, String> {
    let mut segments = Vec::new();
    let mut current_start_byte = range.byte_start;
    let mut current_start_char = range.char_start;
    let mut current_chars = 0usize;

    for (relative_byte, ch) in source[range.byte_start..range.byte_end].char_indices() {
        if current_chars == max_chars {
            let byte_end = range.byte_start + relative_byte;
            segments.push(TranslationTextPart {
                start: current_start_char,
                end: current_start_char + current_chars,
                text: source[current_start_byte..byte_end].to_string(),
            });
            current_start_byte = byte_end;
            current_start_char += current_chars;
            current_chars = 0;
        }
        let _ = ch;
        current_chars += 1;
    }
    if current_chars > 0 {
        segments.push(TranslationTextPart {
            start: current_start_char,
            end: current_start_char + current_chars,
            text: source[current_start_byte..range.byte_end].to_string(),
        });
    }

    Ok(segments)
}

fn append_range(current: &mut Option<PartBuilder>, range: SourceRange) {
    if let Some(builder) = current {
        builder.extend(range);
    } else {
        *current = Some(PartBuilder::from_range(range));
    }
}

fn push_current_part(
    source: &str,
    segments: &mut Vec<TranslationTextPart>,
    current: &mut Option<PartBuilder>,
) {
    if let Some(builder) = current.take() {
        segments.push(builder.into_part(source));
    }
}

fn trim_source_range(
    text: &str,
    byte_start: usize,
    byte_end: usize,
    char_start: usize,
) -> Option<SourceRange> {
    let mut start_byte = byte_start;
    let mut end_byte = byte_end;
    let mut start_trim_chars = 0usize;
    let mut end_trim_chars = 0usize;

    while start_byte < end_byte {
        let ch = text[start_byte..end_byte].chars().next()?;
        if !ch.is_whitespace() {
            break;
        }
        start_byte += ch.len_utf8();
        start_trim_chars += 1;
    }

    while start_byte < end_byte {
        let ch = text[start_byte..end_byte].chars().next_back()?;
        if !ch.is_whitespace() {
            break;
        }
        end_byte -= ch.len_utf8();
        end_trim_chars += 1;
    }

    if start_byte >= end_byte {
        return None;
    }

    let raw_char_len = text[byte_start..byte_end].chars().count();
    let char_start = char_start + start_trim_chars;
    let char_len = raw_char_len
        .saturating_sub(start_trim_chars)
        .saturating_sub(end_trim_chars);
    Some(SourceRange {
        byte_start: start_byte,
        byte_end: end_byte,
        char_start,
        char_end: char_start + char_len,
        char_len,
    })
}

fn word_ranges(source: &str, range: SourceRange) -> Vec<SourceRange> {
    let mut char_cursor = range.char_start;
    source[range.byte_start..range.byte_end]
        .split_word_bound_indices()
        .filter_map(|(relative_byte, word)| {
            let word_char_len = word.chars().count();
            let char_start = char_cursor;
            char_cursor += word_char_len;
            if word.trim().is_empty() {
                return None;
            }
            let byte_start = range.byte_start + relative_byte;
            let byte_end = byte_start + word.len();
            Some(SourceRange {
                byte_start,
                byte_end,
                char_start,
                char_end: char_start + word_char_len,
                char_len: word_char_len,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::segment_text_blocks;

    #[test]
    fn skips_empty_blocks_and_assigns_stable_ids() {
        let segments = segment_text_blocks(["   ", "One. Two."], 20).expect("segments");

        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].id, "b1:s0");
        assert_eq!(segments[0].source_block_index, 1);
        assert_eq!(segments[0].source_start, 0);
        assert_eq!(segments[0].source_end, 9);
        assert_eq!(segments[0].text, "One. Two.");
    }

    #[test]
    fn splits_sentence_like_boundaries_under_limit() {
        let segments = segment_text_blocks(["Alpha. Beta. Gamma."], 13).expect("segments");
        let offsets: Vec<_> = segments
            .iter()
            .map(|segment| (segment.source_start, segment.source_end))
            .collect();
        let texts: Vec<_> = segments.into_iter().map(|segment| segment.text).collect();

        assert_eq!(texts, vec!["Alpha. Beta.", "Gamma."]);
        assert_eq!(offsets, vec![(0, 12), (13, 19)]);
    }

    #[test]
    fn splits_long_unpunctuated_text_by_words() {
        let segments = segment_text_blocks(["alpha beta gamma delta"], 12).expect("segments");
        let texts: Vec<_> = segments.into_iter().map(|segment| segment.text).collect();

        assert_eq!(texts, vec!["alpha beta", "gamma delta"]);
    }

    #[test]
    fn splits_oversized_words_on_char_boundaries() {
        let segments = segment_text_blocks(["abcdef"], 2).expect("segments");
        let texts: Vec<_> = segments.into_iter().map(|segment| segment.text).collect();

        assert_eq!(texts, vec!["ab", "cd", "ef"]);
    }

    #[test]
    fn keeps_long_paragraph_segments_under_opus_mt_char_cap() {
        let paragraph = "The team reviewed digital catalogs, compared archive notes, and prepared a new guide for readers. ".repeat(18);
        let max_chars = 900;
        let segments = segment_text_blocks([paragraph], max_chars).expect("segments");

        assert!(segments.len() > 1);
        assert!(segments
            .iter()
            .all(|segment| { segment.text.chars().count() <= max_chars }));
    }

    #[test]
    fn preserves_url_heavy_reference_block_instead_of_dropping_it() {
        let paragraph = "A reading note cites https://www.example.org/subject/archive/report-2.pdf and continues with a long comment about libraries, revised editions, cross references, and editorial criteria. A second sentence forces the planner to split the block without losing original offsets.";
        let segments = segment_text_blocks([paragraph], 120).expect("segments");

        assert!(segments.len() > 1);
        assert!(segments
            .iter()
            .all(|segment| segment.source_block_index == 0));
        assert!(segments
            .iter()
            .all(|segment| segment.text.chars().count() <= 120));
        assert_eq!(
            segments
                .iter()
                .map(|segment| segment.text.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            paragraph
        );
        for segment in &segments {
            assert_eq!(
                char_slice(paragraph, segment.source_start, segment.source_end),
                segment.text
            );
        }
    }

    #[test]
    fn keeps_later_blocks_aligned_after_url_heavy_reference_block() {
        let blocks = [
            "Short intro.",
            "Technical reference https://www.example.org/a/b/c.final.pdf with enough extra text to force more than one segment inside the same block.",
            "Short close.",
        ];
        let segments = segment_text_blocks(blocks, 80).expect("segments");

        assert!(segments.iter().any(|segment| segment.id.starts_with("b1:")));
        assert_eq!(
            segments.last().map(|segment| segment.id.as_str()),
            Some("b2:s0")
        );
        assert_eq!(
            segments.last().map(|segment| segment.text.as_str()),
            Some("Short close.")
        );
    }

    #[test]
    fn rejects_zero_limit() {
        let error = segment_text_blocks(["text"], 0).expect_err("zero limit should fail");

        assert!(error.contains("greater than zero"));
    }

    fn char_slice(text: &str, start: usize, end: usize) -> String {
        text.chars().skip(start).take(end - start).collect()
    }
}
