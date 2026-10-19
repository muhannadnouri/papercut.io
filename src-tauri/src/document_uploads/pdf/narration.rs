// Reconstruct logical narration text from PDF.js page fragments.

use serde::Serialize;

use super::page_text::{PageTextBlock, PageTextLayer};

const SAME_LINE_Y_TOLERANCE: f32 = 0.45;
const WRAPPED_LINE_GAP_FACTOR: f32 = 0.9;
const WRAPPED_LINE_GAP_PADDING: f32 = 2.0;
const PAGE_EDGE_RATIO: f32 = 0.2;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PdfNarrationSegment {
    pub(crate) text: String,
    pub(crate) source_runs: Vec<PdfNarrationSourceRun>,
}

/// Maps UTF-16 offsets in reconstructed text back to one PDF.js text item.
///
/// Frontend source spans also use JavaScript string offsets, so counting UTF-16
/// here avoids corrupting later highlights for Arabic, CJK, and supplementary
/// Unicode characters. Ligatures remain isolated runs because one source glyph
/// can expand to multiple narration characters.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PdfNarrationSourceRun {
    pub(crate) page_index: u32,
    pub(crate) block_order: u32,
    pub(crate) start_offset: u32,
    pub(crate) end_offset: u32,
    pub(crate) source_start_offset: u32,
    pub(crate) source_end_offset: u32,
}

#[derive(Clone, Debug)]
struct SourcePiece {
    page_index: u32,
    block_order: u32,
    text: String,
}

#[derive(Clone, Debug)]
struct TextLine {
    page_index: u32,
    page_height: f32,
    top: f32,
    bottom: f32,
    height: f32,
    pieces: Vec<SourcePiece>,
}

impl TextLine {
    fn text(&self) -> String {
        self.pieces
            .iter()
            .map(|piece| piece.text.as_str())
            .collect()
    }
}

#[derive(Default)]
struct SegmentBuilder {
    text: String,
    source_runs: Vec<PdfNarrationSourceRun>,
    pending_space: bool,
    text_offset: u32,
}

/// Rebuild paragraphs before shared TTS chunking so PDF visual line wrapping
/// does not become an audible model restart. Extraction order stays canonical:
/// geometry is used only to recognize lines, paragraph gaps, and page seams.
/// Reading through a callback keeps memory bounded to one full page layer plus
/// the compact reconstructed text and source runs.
pub(crate) fn reconstruct_narration_segments<E>(
    page_count: u32,
    mut read_page: impl FnMut(u32) -> Result<PageTextLayer, E>,
) -> Result<Vec<PdfNarrationSegment>, E> {
    let mut segments = Vec::new();
    let mut current = Vec::new();

    for page_index in 0..page_count {
        for line in page_lines(read_page(page_index)?) {
            if current
                .last()
                .is_some_and(|previous| !continues_paragraph(previous, &line))
            {
                push_segment(&mut segments, std::mem::take(&mut current));
            }
            current.push(line);
        }
    }
    push_segment(&mut segments, current);
    Ok(segments)
}

/// Preserve visual line boundaries for page-level FTS text while reusing the
/// same inline-fragment and whitespace normalization as PDF narration.
#[cfg(test)]
pub(crate) fn reconstruct_search_text(page: PageTextLayer) -> String {
    search_text_lines(page, true).0
}

// Verify old derived text before repairing it from a saved page layer.
pub(crate) fn legacy_search_text(page: PageTextLayer) -> String {
    search_text_lines(page, false).0
}

fn search_text_lines(
    page: PageTextLayer,
    unicode_wraps: bool,
) -> (String, Vec<(TextLine, std::ops::Range<usize>)>) {
    let lines = page_lines(page);
    let mut text = String::new();
    let mut mapped: Vec<(TextLine, std::ops::Range<usize>)> = Vec::new();

    for (index, line) in lines.iter().enumerate() {
        let mut builder = SegmentBuilder::default();
        builder.push_line(line.clone());
        if builder.text.is_empty() {
            continue;
        }

        if !text.is_empty() {
            if index > 0
                && is_soft_hyphenated_wrap(&lines[index - 1], line)
                && (unicode_wraps || lines[index - 1].text().trim_end().ends_with('-'))
            {
                text.pop();
                if let Some((_, range)) = mapped.last_mut() {
                    range.end = text.len();
                }
            } else {
                text.push('\n');
            }
        }
        let start = text.len();
        text.push_str(&builder.text);
        mapped.push((line.clone(), start..text.len()));
    }

    (text, mapped)
}

/// Page-edge repetition is document-local evidence; page numbers alone cannot identify a header.
pub(crate) fn edge_keys(page: PageTextLayer) -> std::collections::HashSet<String> {
    page_lines(page)
        .iter()
        .filter(|line| edge(line))
        .map(|line| edge_key(&line.text()))
        .filter(|key| key.chars().count() >= 4)
        .collect()
}

fn edge(line: &TextLine) -> bool {
    line.top < line.page_height * 0.07 || line.bottom > line.page_height * 0.92
}

fn edge_key(text: &str) -> String {
    text.chars()
        .filter(|ch| !ch.is_numeric())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Classify derived spans without sorting columns or changing canonical text/reader offsets.
/// Small bottom notes, repeated edge furniture and dense navigation/citation lists stay literal-searchable.
pub(crate) fn search_content(
    page: PageTextLayer,
    repeated: &std::collections::HashSet<String>,
) -> (String, Vec<std::ops::Range<usize>>) {
    let (text, mapped) = search_text_lines(page, true);
    // shortcut: small bottom captions can resemble notes; refine against annotated layout failures.
    let mut heights = mapped
        .iter()
        .filter(|(line, _)| !edge(line))
        .map(|(line, _)| line.height)
        .collect::<Vec<_>>();
    heights.sort_by(f32::total_cmp);
    let typical = heights.get(heights.len() / 2).copied().unwrap_or(0.0);
    let contents_lines = mapped
        .iter()
        .filter(|(line, _)| {
            let value = line.text();
            value
                .split_whitespace()
                .last()
                .is_some_and(|last| last.chars().all(|ch| ch.is_numeric()))
                && value.split_whitespace().count() >= 2
        })
        .count();
    // A numeric table is not navigation: require repeated dot leaders as well as page-like endings.
    let leaders = text.matches(". . .").count() + text.matches("...").count();
    let toc = contents_lines >= 8 && contents_lines * 2 >= mapped.len() && leaders >= 2;
    let values = mapped
        .iter()
        .map(|(line, _)| line.text())
        .collect::<Vec<_>>();
    let citations = values
        .iter()
        .enumerate()
        .filter_map(|(index, value)| {
            let number = value
                .trim_start()
                .strip_prefix('[')
                .and_then(|rest| rest.split_once(']'))
                .filter(|(number, _)| {
                    !number.is_empty() && number.chars().all(|ch| ch.is_ascii_digit())
                })
                .and_then(|(number, _)| number.parse::<u32>().ok());
            number.map(|number| (index, number))
        })
        .collect::<Vec<_>>();
    let links = text.matches("https://").count()
        + text.matches("http://").count()
        + text.matches("doi:").count();
    let printed = citations
        .iter()
        .enumerate()
        .filter(|(position, (start, _))| {
            let end = citations
                .get(position + 1)
                .map_or(values.len(), |next| next.0);
            printed_reference(&values[*start..end].join(" "))
        })
        .count();
    let sequential = citations
        .windows(2)
        .all(|pair| pair[0].1.checked_add(1) == Some(pair[1].1));
    let reference_start = (citations.len() >= 3
        && (links >= 2 || (sequential && printed >= 3 && printed * 2 >= citations.len())))
    .then(|| {
        if citations[0].0 <= 1 {
            0
        } else {
            citations[0].0
        }
    });
    let index_page = subject_index(&values);
    let mut ranges: Vec<std::ops::Range<usize>> = Vec::new();
    for (index, (line, range)) in mapped.iter().enumerate() {
        let value = line.text();
        let note = line.top > line.page_height * 0.7 && line.height < typical * 0.85;
        let furniture = edge(line)
            && (repeated.contains(&edge_key(&value))
                || value.trim().chars().all(|ch| ch.is_numeric()));
        if toc
            || index_page
            || note
            || furniture
            || reference_start.is_some_and(|start| index >= start)
        {
            continue;
        }
        if let Some(previous) = ranges.last_mut() {
            if text[previous.end..range.start].trim().is_empty() {
                previous.end = range.end;
                continue;
            }
        }
        ranges.push(range.clone());
    }
    (text, ranges)
}

/// Printed references need a dated, quoted title after an author separator, not merely numbers or domains.
fn printed_reference(text: &str) -> bool {
    let Some((_, title)) = text.split_once(':') else {
        return false;
    };
    let quoted = (title.contains('“') && title.contains('”')) || title.matches('"').count() >= 2;
    quoted
        && text
            .split(|ch: char| !ch.is_ascii_digit())
            .any(|number| number.len() == 4 && matches!(number.as_bytes()[0], b'1' | b'2'))
}

/// Combine independent page-list cues; numbers alone cannot distinguish an index from a table.
fn subject_index(lines: &[String]) -> bool {
    // Join only cued wraps for classification; canonical text and source locators stay untouched.
    let mut logical: Vec<String> = Vec::new();
    for line in lines {
        let value = line.trim();
        if let Some(previous) = logical.last_mut() {
            let wrapping = (page_references(value)
                && (index_entry(previous).is_none() || previous.ends_with(',')))
                || previous.ends_with(['-', '‐', '\u{ad}'])
                || (previous.contains('(') && !previous.contains(')'));
            if wrapping {
                let joined = format!("{previous} {value}");
                if index_entry(&joined).is_some() || index_cross_reference(&joined) {
                    *previous = joined;
                    continue;
                }
            }
        }
        logical.push(value.to_owned());
    }
    let entries = logical
        .iter()
        .filter_map(|line| index_entry(line))
        .collect::<Vec<_>>();
    let cross_references = logical
        .iter()
        .filter(|line| index_cross_reference(line))
        .count();
    // shortcut: qualified dense indexes only; uncued wraps and other reference syntax need annotated layouts.
    entries.len() >= 8
        && (entries.len() + cross_references) * 2 >= logical.len()
        && !logical.iter().any(|line| {
            line.split_whitespace().count() > 10
                || line
                    .trim_end()
                    .ends_with(['.', '?', '!', '؟', '。', '！', '？'])
                || (index_entry(line).is_none()
                    && !index_cross_reference(line)
                    && line
                        .split_whitespace()
                        .filter(|word| word.chars().all(|ch| ch.is_ascii_digit()))
                        .count()
                        >= 2)
        })
        && entries.iter().filter(|(_, multiple)| *multiple).count() >= 2
        && entries
            .iter()
            .filter(|(label, _)| label.contains('(') && label.contains(')'))
            .count()
            >= 2
        && entries
            .windows(2)
            .filter(|pair| pair[0].0.to_lowercase() <= pair[1].0.to_lowercase())
            .count()
            * 2
            >= entries.len() - 1
}

/// Parenthesized publisher cross-references count as navigation only on an independently qualified index page.
fn index_cross_reference(text: &str) -> bool {
    let Some((label, reference)) = text.strip_suffix(')').and_then(|text| text.split_once('('))
    else {
        return false;
    };
    let Some(target) = reference
        .strip_prefix("see ")
        .or_else(|| reference.strip_prefix("See "))
    else {
        return false;
    };
    let label = label.trim();
    let target = target.strip_prefix("also ").unwrap_or(target);
    (label.is_empty()
        || (label.starts_with(char::is_alphabetic) && label.split_whitespace().count() <= 8))
        && target.starts_with(char::is_alphabetic)
        && !text.chars().any(|ch| {
            ch.is_ascii_digit()
                || matches!(ch, '.' | ':' | ';' | '!' | '?' | '؟' | '。' | '！' | '？')
        })
        && !reference.contains(['(', ')'])
}

/// Validate single page numbers and comma/range lists; whitespace-only numeric columns remain tables.
fn page_references(pages: &str) -> bool {
    !pages.is_empty()
        && pages.split([',', '-', '–', '—']).all(|part| {
            let part = part.trim();
            !part.is_empty()
                && part.chars().all(|ch| ch.is_ascii_digit())
                && part.parse::<u32>().is_ok_and(|page| page > 0)
        })
}

/// Separate a short subject label from comma/range page references; retain original UTF-8 text untouched.
fn index_entry(text: &str) -> Option<(&str, bool)> {
    let text = text.trim();
    let start = text
        .char_indices()
        .rev()
        .find(|(_, ch)| !ch.is_ascii_digit() && !matches!(ch, ' ' | ',' | '-' | '–' | '—'))
        .map_or(0, |(index, ch)| index + ch.len_utf8());
    let (label, pages) = text.split_at(start);
    let label = label.trim_end_matches([' ', ',']);
    let pages = pages.trim_matches([' ', ',']);
    if !label.starts_with(char::is_alphabetic)
        || label.split_whitespace().count() > 8
        || label
            .chars()
            .any(|ch| ch.is_ascii_digit() || matches!(ch, '.' | ':' | ';' | '!' | '?'))
        || !page_references(pages)
    {
        return None;
    }
    Some((label, pages.contains([',', '-', '–', '—'])))
}

/// Group adjacent PDF.js items into visual lines while honoring explicit EOL
/// markers. This rejoins inline font/ligature fragments without reordering
/// columns or trusting whitespace-only extraction items as narration.
fn page_lines(mut page: PageTextLayer) -> Vec<TextLine> {
    page.blocks.sort_by_key(|block| block.order);
    let mut lines = Vec::new();
    let mut current: Option<TextLine> = None;

    for block in page.blocks {
        let explicit_break = block.text.ends_with('\r') || block.text.ends_with('\n');
        let text = block
            .text
            .trim_end_matches(|character| matches!(character, '\r' | '\n'))
            .to_string();

        if current
            .as_ref()
            .is_some_and(|line| !same_visual_line(line, &block))
        {
            push_line(&mut lines, current.take());
        }

        if !text.is_empty() {
            let top = block.bounds[1];
            let bottom = top + block.bounds[3];
            let line = current.get_or_insert_with(|| TextLine {
                page_index: page.page_index,
                page_height: page.height,
                top,
                bottom,
                height: block.bounds[3],
                pieces: Vec::new(),
            });
            line.top = line.top.min(top);
            line.bottom = line.bottom.max(bottom);
            line.height = line.height.max(block.bounds[3]);
            line.pieces.push(SourcePiece {
                page_index: page.page_index,
                block_order: block.order,
                text,
            });
        }

        if explicit_break {
            push_line(&mut lines, current.take());
        }
    }

    push_line(&mut lines, current);
    lines
}

fn same_visual_line(line: &TextLine, block: &PageTextBlock) -> bool {
    let line_center = (line.top + line.bottom) / 2.0;
    let block_center = block.bounds[1] + block.bounds[3] / 2.0;
    (line_center - block_center).abs() <= line.height.max(block.bounds[3]) * SAME_LINE_Y_TOLERANCE
}

fn push_line(lines: &mut Vec<TextLine>, line: Option<TextLine>) {
    if let Some(line) = line.filter(|line| {
        line.pieces
            .iter()
            .any(|piece| !piece.text.trim().is_empty())
    }) {
        lines.push(line);
    }
}

/// Keep normal leading together, but split on paragraph spacing or a backwards
/// vertical jump such as moving from the bottom of one column to the next.
/// Cross-page joins are deliberately conservative and require prose at both
/// page edges plus an unfinished prior sentence.
fn continues_paragraph(previous: &TextLine, next: &TextLine) -> bool {
    if previous.page_index == next.page_index {
        if next.top + previous.height.min(next.height) < previous.top {
            return false;
        }
        let gap = (next.top - previous.bottom).max(0.0);
        return gap
            <= previous.height.max(next.height) * WRAPPED_LINE_GAP_FACTOR
                + WRAPPED_LINE_GAP_PADDING;
    }

    next.page_index == previous.page_index + 1
        && previous.bottom >= previous.page_height * (1.0 - PAGE_EDGE_RATIO)
        && next.top <= next.page_height * PAGE_EDGE_RATIO
        && previous.text().split_whitespace().count() >= 4
        && next.text().split_whitespace().count() >= 2
        && !ends_sentence(&previous.text())
}

/// Match PDF.js's conservative broken-word rule at genuine visual line
/// boundaries. Same-line compounds retain their hyphen.
fn is_soft_hyphenated_wrap(previous: &TextLine, next: &TextLine) -> bool {
    if !continues_paragraph(previous, next) {
        return false;
    }

    let previous_text = previous.text();
    let mut previous_chars = previous_text.trim_end().chars().rev();
    if !matches!(previous_chars.next(), Some('-' | '\u{2010}' | '\u{2011}')) {
        return false;
    }
    let Some(before) = previous_chars.next() else {
        return false;
    };
    let Some(after) = next.text().trim_start().chars().next() else {
        return false;
    };

    (before.is_lowercase() && after.is_lowercase())
        || (before.is_uppercase() && after.is_alphabetic())
}

fn ends_sentence(text: &str) -> bool {
    text.trim_end()
        .trim_end_matches(|character| {
            matches!(
                character,
                '"' | '\'' | '”' | '’' | '»' | ')' | ']' | '）' | '】' | '」' | '』' | '》'
            )
        })
        .chars()
        .last()
        .is_some_and(|character| {
            matches!(
                character,
                '.' | '!' | '?' | '؟' | '。' | '！' | '？' | '।' | '॥'
            )
        })
}

fn push_segment(segments: &mut Vec<PdfNarrationSegment>, lines: Vec<TextLine>) {
    let mut builder = SegmentBuilder::default();
    for line in lines {
        builder.push_line(line);
    }
    if !builder.text.is_empty() {
        segments.push(PdfNarrationSegment {
            text: builder.text,
            source_runs: builder.source_runs,
        });
    }
}

impl SegmentBuilder {
    fn push_line(&mut self, line: TextLine) {
        if !self.text.is_empty() {
            self.pending_space = true;
        }
        for piece in line.pieces {
            self.push_piece(piece);
        }
    }

    /// Collapse layout whitespace while retaining source offsets for every
    /// visible run. Spaces inserted between wrapped lines intentionally have no
    /// source run; later highlighting simply bridges to the next mapped glyph.
    fn push_piece(&mut self, piece: SourcePiece) {
        let mut source_offset = 0u32;
        for character in piece.text.chars() {
            let source_units = character.len_utf16() as u32;
            if character.is_whitespace() {
                self.pending_space = !self.text.is_empty();
                source_offset += source_units;
                continue;
            }

            if self.pending_space {
                self.text.push(' ');
                self.text_offset += 1;
                self.pending_space = false;
            }

            let start_offset = self.text_offset;
            let output_units = if let Some(expanded) = expanded_ligature(character) {
                self.text.push_str(expanded);
                expanded.encode_utf16().count() as u32
            } else {
                self.text.push(character);
                source_units
            };
            self.text_offset += output_units;
            let end_offset = self.text_offset;
            self.push_source_run(
                &piece,
                start_offset,
                end_offset,
                source_offset,
                source_offset + source_units,
                output_units == source_units,
            );
            source_offset += source_units;
        }
    }

    fn push_source_run(
        &mut self,
        piece: &SourcePiece,
        start_offset: u32,
        end_offset: u32,
        source_start_offset: u32,
        source_end_offset: u32,
        merge_allowed: bool,
    ) {
        if merge_allowed {
            if let Some(previous) = self.source_runs.last_mut().filter(|previous| {
                previous.page_index == piece.page_index
                    && previous.block_order == piece.block_order
                    && previous.end_offset == start_offset
                    && previous.source_end_offset == source_start_offset
                    && previous.end_offset - previous.start_offset
                        == previous.source_end_offset - previous.source_start_offset
            }) {
                previous.end_offset = end_offset;
                previous.source_end_offset = source_end_offset;
                return;
            }
        }

        self.source_runs.push(PdfNarrationSourceRun {
            page_index: piece.page_index,
            block_order: piece.block_order,
            start_offset,
            end_offset,
            source_start_offset,
            source_end_offset,
        });
    }
}

fn expanded_ligature(character: char) -> Option<&'static str> {
    Some(match character {
        'ﬀ' => "ff",
        'ﬁ' => "fi",
        'ﬂ' => "fl",
        'ﬃ' => "ffi",
        'ﬄ' => "ffl",
        'ﬅ' => "ft",
        'ﬆ' => "st",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derived_body_ranges_keep_source_order_but_exclude_bottom_notes_and_furniture() {
        let page = layer(
            0,
            vec![
                block(0, "i. A footnote citation.\n", 700.0, 8.0),
                block(1, "港 Electric ferries serve the island.\n", 100.0, 12.0),
                block(2, "Tickets are available at the harbor.\n", 116.0, 12.0),
                block(3, "Harbor Guide | 12", 760.0, 10.0),
            ],
        );
        let repeated = std::collections::HashSet::from(["Harbor Guide |".into()]);
        let canonical = reconstruct_search_text(page.clone());
        let (text, ranges) = search_content(page, &repeated);
        assert_eq!(text, canonical);
        assert!(text.starts_with("i. A footnote"));
        let body = crate::document_uploads::search_content::body_text(&text, &ranges);
        assert!(body.starts_with("港 Electric ferries"));
        assert!(!body.contains("footnote") && !body.contains("Harbor Guide"));
        assert!(crate::document_uploads::search_content::ranges(
            &text,
            Some(&serde_json::to_string(&ranges).unwrap())
        )
        .is_ok());
    }

    #[test]
    fn navigation_and_citation_lists_are_auxiliary_but_numeric_tables_are_not() {
        let table = (0..10)
            .map(|i| {
                block(
                    i,
                    &format!("Route {i} {}\n", i + 10),
                    100.0 + i as f32 * 16.0,
                    12.0,
                )
            })
            .collect::<Vec<_>>();
        let (_, ranges) = search_content(layer(0, table.clone()), &Default::default());
        assert!(!ranges.is_empty());
        let toc = (0..10)
            .map(|i| {
                block(
                    i,
                    &format!("Route {i} . . . . . {}\n", i + 10),
                    100.0 + i as f32 * 16.0,
                    12.0,
                )
            })
            .collect();
        assert!(search_content(layer(0, toc), &Default::default())
            .1
            .is_empty());
        let references = (0..4)
            .map(|i| {
                block(
                    i,
                    &format!("[{}] Author. https://example.org/{i}\n", i + 1),
                    100.0 + i as f32 * 16.0,
                    12.0,
                )
            })
            .collect();
        assert!(search_content(layer(0, references), &Default::default())
            .1
            .is_empty());
        assert_eq!(
            edge_keys(layer(0, vec![block(0, "Harbor Guide | 14", 760.0, 10.0)])),
            edge_keys(layer(1, vec![block(0, "Harbor Guide | 15", 760.0, 10.0)]))
        );
    }

    #[test]
    fn printed_references_keep_preceding_prose_and_original_offsets() {
        let references = [
            "[21] A. Reed: “Harbor Lighting,” Printed Journal, 2018.",
            "[22] B. Moss: \"Port Maps,\" Press, 2019.",
            "[23] C. Stone: “Tide Charts,” maps.example.test, 2021.",
        ];
        let mut blocks = vec![
            block(0, "港 Ferries run every day.\n", 100.0, 12.0),
            block(1, "Tickets remain valid after sunset.\n", 116.0, 12.0),
        ];
        blocks.extend(references.iter().enumerate().map(|(i, text)| {
            block(
                i as u32 + 2,
                &format!("{text}\n"),
                160.0 + i as f32 * 16.0,
                12.0,
            )
        }));
        let page = layer(0, blocks);
        let canonical = reconstruct_search_text(page.clone());
        let (text, ranges) = search_content(page, &Default::default());
        assert_eq!(text, canonical);
        assert_eq!(
            crate::document_uploads::search_content::body_text(&text, &ranges),
            "港 Ferries run every day.\nTickets remain valid after sunset."
        );
        assert!(text.contains(references[2]));
        assert_eq!(ranges, vec![0..text.find("\n[21]").unwrap()]);
        // Numeric rows and dated prose quotations do not have the author/title structure.
        for lines in [
            vec![
                "[1] Route 2018 12",
                "[2] Route 2019 13",
                "[3] Route 2020 14",
            ],
            vec![
                "[1] In 2018, “tides changed.”",
                "[2] In 2019, \"winds changed.\"",
                "[3] In 2020, “routes changed.”",
            ],
        ] {
            let blocks = lines
                .iter()
                .enumerate()
                .map(|(i, text)| {
                    block(
                        i as u32,
                        &format!("{text}\n"),
                        100.0 + i as f32 * 16.0,
                        12.0,
                    )
                })
                .collect();
            let (text, ranges) = search_content(layer(0, blocks), &Default::default());
            assert_eq!(ranges, vec![0..text.len()]);
        }
    }

    #[test]
    fn qualified_subject_indexes_need_density_order_and_multiple_page_lists() {
        let entries = [
            "Anchors (equipment), 12, 24",
            "Berths (allocation), 15–18",
            "Cargo handling, 19, 21",
            "Dock schedules, 30",
            "Electric ferries, 35",
            "Freight routes, 41",
            "Harbor maps, 50",
            "Island service, 52",
        ];
        let make_page = |lines: &[&str]| {
            layer(
                0,
                lines
                    .iter()
                    .enumerate()
                    .map(|(i, text)| {
                        block(
                            i as u32,
                            &format!("{text}\n"),
                            100.0 + i as f32 * 16.0,
                            12.0,
                        )
                    })
                    .collect(),
            )
        };
        let page = make_page(&entries);
        let canonical = reconstruct_search_text(page.clone());
        let (text, ranges) = search_content(page, &Default::default());
        assert_eq!(text, canonical);
        assert!(ranges.is_empty());
        // Similar short lists remain main content without the independent index cues.
        for lines in [
            entries[..4].to_vec(),
            vec![
                "Anchors 12 24",
                "Berths 15 18",
                "Cargo 19 21",
                "Docks 30 32",
                "Ferries 35 39",
                "Freight 41 44",
                "Harbors 50 51",
                "Islands 52 53",
            ],
            {
                let mut lines = entries.to_vec();
                lines.push("This sentence explains the routes.");
                lines
            },
            {
                let mut lines = entries.to_vec();
                lines.reverse();
                lines
            },
            vec![
                "Equipment 2020 2021",
                "Anchors (count), 12, 24",
                "Berths (count), 15, 18",
                "Cargo (tons), 19, 21",
                "Docks (count), 30, 32",
                "Ferries (count), 35, 39",
                "Freight (tons), 41, 44",
                "Harbors (count), 50, 51",
                "Islands (count), 52, 53",
            ],
        ] {
            let (text, ranges) = search_content(make_page(&lines), &Default::default());
            assert_eq!(ranges, vec![0..text.len()]);
        }
        assert_eq!(
            index_entry("港 (航路), 12—14, 20"),
            Some(("港 (航路)", true))
        );
        for invalid in [
            "Cargo 0",
            "Cargo 12,,14",
            "Cargo -12",
            "Cargo 12-",
            "Route 2 12",
        ] {
            assert!(index_entry(invalid).is_none(), "{invalid}");
        }
    }

    #[test]
    fn wrapped_index_entries_and_cross_references_keep_literal_source_text() {
        let lines = [
            "Anchors (equipment),",
            "12, 24",
            "Anchor lights (see Beacons)",
            "Berths (allocation), 15–",
            "18",
            "Berth maps (see Harbor maps)",
            "Cargo handling, 19, 21",
            "Cargo lifts (see Cranes)",
            "Coastal routes (see",
            "Ferry routes)",
            "Dock schedules, 30",
            "Dock signs (see Beacons)",
            "Electric ferries, 35",
            "Electric motors (see Propulsion)",
            "Freight trans-",
            "port, 41",
            "Freight ships (see Vessels)",
            "Harbor maps, 50",
            "(see also Navigation)",
            "Island service, 52",
            "Island terminals (see Berths)",
            "港 (航路), 61—63, 70",
        ];
        let page = |lines: &[&str]| {
            layer(
                0,
                lines
                    .iter()
                    .enumerate()
                    .map(|(i, text)| {
                        block(
                            i as u32,
                            &format!("{text}\n"),
                            100.0 + i as f32 * 12.0,
                            12.0,
                        )
                    })
                    .collect(),
            )
        };
        let canonical = reconstruct_search_text(page(&lines));
        let (text, ranges) = search_content(page(&lines), &Default::default());
        assert_eq!(text, canonical);
        assert!(text.contains("12, 24") && text.contains("港 (航路)"));
        assert!(ranges.is_empty(), "wrapped navigation must be auxiliary");

        for extra in [
            "This sentence explains the ferry routes.",
            "هذه جملة تشرح مسارات العبارات؟",
            "港の航路を説明します。",
            "Equipment 2020 2021",
            "These operational observations describe the routes and their maintenance requirements in detail",
        ] {
            let mut mixed = lines.to_vec();
            mixed.push(extra);
            let (text, ranges) = search_content(page(&mixed), &Default::default());
            assert_eq!(ranges, vec![0..text.len()], "retain ambiguous page: {extra}");
        }
        let mut malformed = lines.to_vec();
        malformed[1] = "0, 24";
        malformed[4] = "18,,19";
        // Losing both qualified entries must not turn arbitrary wrapped numbers into navigation.
        let (text, ranges) = search_content(page(&malformed), &Default::default());
        assert_eq!(ranges, vec![0..text.len()]);

        let mut columns = lines.to_vec();
        columns.extend(["Equipment", "2020 2021"]);
        let (text, ranges) = search_content(page(&columns), &Default::default());
        assert_eq!(ranges, vec![0..text.len()]);
        let references = ["Anchors (see Berths)"; 12];
        let (text, ranges) = search_content(page(&references), &Default::default());
        assert_eq!(
            ranges,
            vec![0..text.len()],
            "references alone are insufficient"
        );
        for invalid in [
            "(see )",
            "(see also )",
            "(see Routes. More prose)",
            "Dock 2020 (see Maps)",
            "(see Routes",
        ] {
            assert!(!index_cross_reference(invalid), "{invalid}");
        }
    }

    #[test]
    fn rebuilds_wrapped_prose_ligatures_and_page_seams() {
        let pages = vec![
            layer(
                0,
                vec![
                    block(0, "Heading\n", 40.0, 20.0),
                    block(1, "A paragraph wraps at the\n", 100.0, 12.0),
                    block(2, "exempli", 117.0, 12.0),
                    block(3, "ﬁ", 117.0, 12.0),
                    block(4, "ed result.\n", 117.0, 12.0),
                    block(5, "A new paragraph starts here.\n", 150.0, 12.0),
                    block(6, "This sentence continues onto the\n", 770.0, 12.0),
                ],
            ),
            layer(
                1,
                vec![
                    block(0, "next page without a seam.\n", 30.0, 12.0),
                    block(1, "Another paragraph.\n", 65.0, 12.0),
                ],
            ),
        ];

        let segments = reconstruct(pages);
        assert_eq!(
            segments
                .iter()
                .map(|segment| segment.text.as_str())
                .collect::<Vec<_>>(),
            vec![
                "Heading",
                "A paragraph wraps at the exemplified result.",
                "A new paragraph starts here.",
                "This sentence continues onto the next page without a seam.",
                "Another paragraph.",
            ]
        );
        let ligature_run = segments[1]
            .source_runs
            .iter()
            .find(|run| run.block_order == 3)
            .expect("ligature source run");
        assert_eq!(ligature_run.end_offset - ligature_run.start_offset, 2);
        assert_eq!(
            ligature_run.source_end_offset - ligature_run.source_start_offset,
            1
        );
    }

    #[test]
    fn keeps_extraction_order_and_splits_a_column_jump() {
        let segments = reconstruct(vec![layer(
            0,
            vec![
                block(0, "العَرَبِيَّة हिन्दी\n", 100.0, 12.0),
                block(1, "简体中文\n", 117.0, 12.0),
                block(2, "Second column\n", 80.0, 12.0),
            ],
        )]);

        assert_eq!(segments[0].text, "العَرَبِيَّة हिन्दी 简体中文");
        assert_eq!(segments[1].text, "Second column");
        assert!(segments[0]
            .source_runs
            .windows(2)
            .all(|runs| runs[0].start_offset <= runs[1].start_offset));
    }

    #[test]
    fn preserves_visual_lines_in_search_text_without_splitting_inline_fragments() {
        let text = reconstruct_search_text(layer(
            0,
            vec![
                block(0, "Inline ", 100.0, 12.0),
                block(1, "formatting", 100.0, 12.0),
                block(2, "continues below", 117.0, 12.0),
            ],
        ));

        assert_eq!(text, "Inline formatting\ncontinues below");
    }

    #[test]
    fn rejoins_soft_hyphens_but_preserves_same_line_compounds() {
        let text = reconstruct_search_text(layer(
            0,
            vec![
                block(0, "Visible high-\n", 100.0, 12.0),
                block(1, "lights remain searchable.\n", 117.0, 12.0),
                block(2, "A state-owned archive.\n", 150.0, 12.0),
            ],
        ));

        assert_eq!(
            text,
            "Visible highlights remain searchable.\nA state-owned archive."
        );
    }

    #[test]
    fn unicode_hyphen_wraps_join_only_at_continuing_visual_lines() {
        for hyphen in ['-', '\u{2010}', '\u{2011}'] {
            let page = layer(
                0,
                vec![
                    block(0, &format!("港 A high{hyphen}\n"), 100.0, 12.0),
                    block(1, "light guides the ferry.\n", 117.0, 12.0),
                    block(2, &format!("A state{hyphen}owned archive."), 150.0, 12.0),
                ],
            );
            let (text, ranges) = search_content(page.clone(), &Default::default());
            assert_eq!(
                text,
                format!("港 A highlight guides the ferry.\nA state{hyphen}owned archive.")
            );
            assert!(ranges.iter().all(|range| text.get(range.clone()).is_some()));
            if hyphen != '-' {
                assert!(legacy_search_text(page).contains(&format!("high{hyphen}\nlight")));
            }
            for (top, following) in [(150.0, "light"), (117.0, "Light")] {
                let separated = reconstruct_search_text(layer(
                    0,
                    vec![
                        block(0, &format!("A high{hyphen}"), 100.0, 12.0),
                        block(1, following, top, 12.0),
                    ],
                ));
                assert!(separated.contains(&format!("high{hyphen}\n{following}")));
            }
        }
        for dash in ['\u{2013}', '\u{2014}'] {
            assert!(reconstruct_search_text(layer(
                0,
                vec![
                    block(0, &format!("A word{dash}"), 100.0, 12.0),
                    block(1, "continuation", 117.0, 12.0),
                ]
            ))
            .contains(&format!("word{dash}\ncontinuation")));
        }
    }

    fn layer(page_index: u32, blocks: Vec<PageTextBlock>) -> PageTextLayer {
        PageTextLayer {
            schema_version: 1,
            page_index,
            width: 600.0,
            height: 800.0,
            blocks,
        }
    }

    fn block(order: u32, text: &str, top: f32, height: f32) -> PageTextBlock {
        PageTextBlock {
            text: text.into(),
            bounds: [40.0, top, 500.0, height],
            order,
            confidence: None,
        }
    }

    fn reconstruct(pages: Vec<PageTextLayer>) -> Vec<PdfNarrationSegment> {
        let page_count = pages.len() as u32;
        let mut pages = pages.into_iter();
        reconstruct_narration_segments(page_count, |_| {
            Ok::<_, ()>(pages.next().expect("fixture page"))
        })
        .expect("reconstruct narration")
    }
}
