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
