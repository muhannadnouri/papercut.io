//! Inline-markup alignment helpers for translated HTML rendering.
//!
//! Translation engines consume plain text, but reader output needs to preserve
//! author emphasis where it can be placed safely. This module owns the text
//! normalization, source-span collection, fragment-offset matching, and
//! conservative source-to-target span matching used by `render.rs`.

use kuchikiki::NodeRef;

use super::dom::DomPlan;
use super::storage::{PersistTranslationFragment, PersistTranslationInlinePhrase};

const MAX_INLINE_PHRASE_CHARS: usize = 120;

/// Version of the inline-alignment strategy (probes, folding, projection).
///
/// Bump this when matching/projection semantics change so resume caches that
/// start persisting alignment-derived data cannot silently mix strategies.
pub(crate) const INLINE_ALIGNMENT_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InlinePhraseProbe {
    pub(crate) source_start: usize,
    pub(crate) source_end: usize,
    pub(crate) text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InlineFormattingSpan {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InlinePreservedMarker {
    pub(crate) source_offset: usize,
    pub(crate) html: String,
    pub(crate) source_anchor_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProjectedInlineSpan {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InlineSourceModel {
    pub(crate) text: String,
    pub(crate) spans: Vec<InlineFormattingSpan>,
    pub(crate) markers: Vec<InlinePreservedMarker>,
}

pub(crate) fn is_inline_formatting_element(node: &NodeRef) -> bool {
    let Some(element) = node.as_element() else {
        return false;
    };
    matches!(
        element.name.local.as_ref(),
        "b" | "strong" | "i" | "em" | "u" | "s" | "sub" | "sup" | "code" | "mark"
    )
}

/// Extract one marker-aware inline model from a readable source block.
///
/// This is the document-translation equivalent of XLIFF inline-code
/// extraction: prose text goes to MT, formatting spans and note/backlink markers
/// stay outside the model and are merged back during render.
pub(crate) fn source_inline_model(node: &NodeRef) -> InlineSourceModel {
    let mut text = String::new();
    let mut spans = Vec::new();
    let mut markers = Vec::new();
    collect_inline_model(
        node,
        &mut Vec::new(),
        &mut text,
        &mut spans,
        &mut markers,
        &mut false,
    );
    InlineSourceModel {
        text,
        spans: coalesce_inline_spans(spans),
        markers,
    }
}

pub(crate) fn is_nontranslatable_inline_marker(node: &NodeRef) -> bool {
    let Some(element) = node.as_element() else {
        return false;
    };
    if element.name.local.as_ref() != "a" {
        return false;
    }
    let attrs = element.attributes.borrow();
    matches!(attrs.get("role"), Some("doc-noteref" | "doc-backlink"))
}

/// Resolve one translated fragment back to the normalized source text.
///
/// Prefer planner-provided char offsets because repeated sentences/phrases can
/// appear in long political texts. If the source DOM diverged from uploaded
/// section text, fall back to search so old/no-offset fragments still render.
pub(crate) fn fragment_char_range(
    source_text: &str,
    fragment: &PersistTranslationFragment,
    search_start: usize,
) -> Option<(usize, usize, usize)> {
    if fragment.source_end > fragment.source_start
        && fragment.source_end <= source_text.chars().count()
        && char_slice(source_text, fragment.source_start, fragment.source_end)
            .is_some_and(|value| value == normalize_fragment_text(&fragment.source_text))
    {
        return Some((
            fragment.source_start,
            fragment.source_end,
            byte_index_for_char(source_text, fragment.source_end),
        ));
    }
    find_fragment_char_range(source_text, &fragment.source_text, search_start)
}

pub(crate) fn local_spans_for_fragment(
    spans: &[InlineFormattingSpan],
    fragment_start: usize,
    fragment_end: usize,
) -> Vec<InlineFormattingSpan> {
    spans
        .iter()
        .filter_map(|span| {
            let start = span.start.max(fragment_start);
            let end = span.end.min(fragment_end);
            (start < end).then(|| InlineFormattingSpan {
                start: start - fragment_start,
                end: end - fragment_start,
                tags: span.tags.clone(),
            })
        })
        .collect()
}

/// Project spans independently and keep every safe non-overlapping result.
///
/// Without provider alignment, only unique carry-over terms or exact phrase
/// hints can be placed. Never infer emphasis from relative character position;
/// uncertain spans remain plain text. Hints still need semantic review.
pub(crate) fn projected_translated_spans_best_effort(
    spans: &[InlineFormattingSpan],
    source_text: &str,
    translated_text: &str,
    translation_hints: &[PersistTranslationInlinePhrase],
) -> Vec<ProjectedInlineSpan> {
    let mut projected = spans
        .iter()
        .filter_map(|span| {
            exact_translated_span(span, source_text, translated_text, translation_hints)
        })
        .filter(|span| span.start < span.end)
        .collect::<Vec<_>>();
    projected.sort_by_key(|span| (span.start, span.end));

    // Distinct source spans that project onto overlapping target ranges are
    // alignment failures, not nesting: keep the first and drop the rest so
    // fabricated combined emphasis never lands on translated words.
    let mut accepted: Vec<ProjectedInlineSpan> = Vec::new();
    for span in projected {
        if let Some(previous) = accepted.last() {
            if previous.end > span.start {
                continue;
            }
        }
        accepted.push(span);
    }
    accepted
}

/// Place formatting on an unchanged target phrase when that is unambiguous.
///
/// MT often carries proper nouns, quoted labels, and technical loanwords across
/// unchanged. Matching those exact phrases beats proportional projection for
/// terms like unique project names; requiring one bounded match avoids styling the wrong
/// copy when a phrase repeats.
fn exact_translated_span(
    span: &InlineFormattingSpan,
    source_text: &str,
    translated_text: &str,
    translation_hints: &[PersistTranslationInlinePhrase],
) -> Option<ProjectedInlineSpan> {
    if span.start == 0 && span.end == source_text.chars().count() {
        return Some(ProjectedInlineSpan {
            start: 0,
            end: translated_text.len(),
            tags: span.tags.clone(),
        });
    }
    let source_phrase = char_slice(source_text, span.start, span.end)?;
    let phrase = source_phrase.trim();
    if !phrase_is_specific_enough(phrase) {
        return None;
    }
    let hinted_phrase = translation_hints
        .iter()
        .find(|hint| folded_phrase_key(&hint.source_text) == folded_phrase_key(phrase))
        .map(|hint| trim_phrase_noise(hint.text.trim()))
        .filter(|value| !value.is_empty());
    let (start, end) = hinted_phrase
        .and_then(|hint| {
            find_unique_phrase_byte_range(translated_text, hint)
                .or_else(|| find_unique_phrase_byte_range_folded(translated_text, hint))
        })
        .or_else(|| find_unique_phrase_byte_range(translated_text, phrase))
        .or_else(|| find_unique_phrase_byte_range_folded(translated_text, phrase))?;
    Some(ProjectedInlineSpan {
        start,
        end,
        tags: span.tags.clone(),
    })
}

/// Collect emphasized source phrases per readable block for optional repair.
///
/// The job runner can translate these small phrases as probes, then renderer
/// can exact-match the translated phrase. These hints are best effort, not semantic
/// alignment; ambiguous or absent matches remain plain text.
pub(crate) fn inline_phrase_probes_by_block(view_html: &str) -> Vec<Vec<InlinePhraseProbe>> {
    if view_html.trim().is_empty() {
        return Vec::new();
    }
    DomPlan::new(view_html)
        .blocks
        .into_iter()
        .map(|block| {
            block
                .inline
                .spans
                .into_iter()
                .filter_map(|span| inline_phrase_probe(&block.inline.text, span))
                .collect()
        })
        .collect()
}

fn collect_inline_model(
    node: &NodeRef,
    active_tags: &mut Vec<String>,
    text: &mut String,
    spans: &mut Vec<InlineFormattingSpan>,
    markers: &mut Vec<InlinePreservedMarker>,
    pending_space: &mut bool,
) {
    if is_nontranslatable_inline_marker(node) {
        if let Some(html) = serialize_node(node) {
            markers.push(InlinePreservedMarker {
                source_offset: text.chars().count(),
                html,
                source_anchor_text: last_word_for_marker(text),
            });
        }
        return;
    }
    if let Some(value) = node.as_text() {
        let appended = append_normalized_text(text, &value.borrow(), pending_space);
        if let Some((start, end)) = appended.filter(|_| !active_tags.is_empty()) {
            spans.push(InlineFormattingSpan {
                start,
                end,
                tags: active_tags.clone(),
            });
        }
        return;
    }

    if node
        .as_element()
        .is_some_and(|element| matches!(element.name.local.as_ref(), "br" | "hr"))
    {
        *pending_space = true;
    }
    let pushed_tag = inline_formatting_tag_name(node);
    if let Some(tag) = pushed_tag.clone() {
        active_tags.push(tag);
    }
    for child in node.children() {
        collect_inline_model(&child, active_tags, text, spans, markers, pending_space);
    }
    if pushed_tag.is_some() {
        active_tags.pop();
    }
}

fn inline_phrase_probe(source_text: &str, span: InlineFormattingSpan) -> Option<InlinePhraseProbe> {
    let text = char_slice(source_text, span.start, span.end)?;
    let text = text.trim();
    if !phrase_is_specific_enough(text) || text.chars().count() > MAX_INLINE_PHRASE_CHARS {
        return None;
    }
    Some(InlinePhraseProbe {
        source_start: span.start,
        source_end: span.end,
        text: text.to_string(),
    })
}

/// Append text using the same whitespace contract as translation segments.
///
/// The returned range excludes any spacer inserted before the first word. That
/// detail matters when a text node is inside `<em>` after unstyled text: the
/// inserted normalizing space should not become emphasized.
///
/// `pending_space` carries whitespace state across sibling text nodes so
/// punctuation directly after a formatting element (`<strong>x</strong>.`)
/// stays attached instead of becoming a stray ` .` token in MT input.
fn append_normalized_text(
    output: &mut String,
    text: &str,
    pending_space: &mut bool,
) -> Option<(usize, usize)> {
    if text.chars().next().is_some_and(char::is_whitespace) {
        *pending_space = true;
    }
    let mut start = None;
    for word in text.split_whitespace() {
        if !output.is_empty() && (*pending_space || start.is_some()) {
            output.push(' ');
        }
        if start.is_none() {
            start = Some(output.chars().count());
        }
        output.push_str(word);
    }
    if start.is_some() {
        *pending_space = text.chars().next_back().is_some_and(char::is_whitespace);
    }
    start.map(|start| (start, output.chars().count()))
}

pub(crate) fn serialize_node(node: &NodeRef) -> Option<String> {
    let mut bytes = Vec::new();
    node.serialize(&mut bytes).ok()?;
    String::from_utf8(bytes).ok()
}

fn inline_formatting_tag_name(node: &NodeRef) -> Option<String> {
    let element = node.as_element()?;
    let tag = element.name.local.as_ref();
    if matches!(
        tag,
        "b" | "strong" | "i" | "em" | "u" | "s" | "sub" | "sup" | "code" | "mark"
    ) {
        Some(tag.to_string())
    } else {
        None
    }
}

fn coalesce_inline_spans(spans: Vec<InlineFormattingSpan>) -> Vec<InlineFormattingSpan> {
    let mut merged: Vec<InlineFormattingSpan> = Vec::new();
    for span in spans {
        if let Some(previous) = merged.last_mut() {
            if previous.end == span.start && previous.tags == span.tags {
                previous.end = span.end;
                continue;
            }
        }
        merged.push(span);
    }
    merged
}

fn phrase_is_specific_enough(phrase: &str) -> bool {
    phrase.chars().filter(|ch| ch.is_alphanumeric()).count() >= 4
}

/// Fold case and Latin diacritics for tolerant phrase comparison.
///
/// MT probe output routinely differs from in-context phrasing only by sentence
/// casing or accent normalization; byte-exact matching alone would discard
/// those hints and fall back to positional projection, styling wrong words.
///
/// Canonical (NFD) decomposition covers every combining-mark diacritic - the
/// previous hand-rolled table stopped at Spanish/French letters and missed
/// pairs like ř/r and ș/s. Letters without a canonical decomposition (such as
/// ł or ø) intentionally stay themselves rather than being approximated.
pub(crate) fn fold_phrase_char(ch: char) -> char {
    let mut base = ch;
    let mut seen_base = false;
    unicode_normalization::char::decompose_canonical(ch, |part| {
        if !seen_base && !unicode_normalization::char::is_combining_mark(part) {
            base = part;
            seen_base = true;
        }
    });
    base.to_lowercase().next().unwrap_or(base)
}

/// Whitespace-normalized, case/accent-folded key for hint source lookup.
fn folded_phrase_key(text: &str) -> String {
    normalize_fragment_text(text)
        .chars()
        .map(fold_phrase_char)
        .collect()
}

/// Strip wrapping quotes/punctuation an MT engine adds to a standalone phrase.
///
/// Probes are translated as isolated inputs, so the engine may sentence-close
/// them ("Scientific socialism.") or quote them. The prose occurrence carries
/// its own punctuation; matching should only cover the phrase words.
fn trim_phrase_noise(phrase: &str) -> &str {
    phrase.trim_matches(|ch: char| !ch.is_alphanumeric())
}

fn find_unique_phrase_byte_range(text: &str, phrase: &str) -> Option<(usize, usize)> {
    let mut search_start = 0usize;
    let mut found = None;
    while search_start <= text.len() {
        let Some(relative_start) = text[search_start..].find(phrase) else {
            break;
        };
        let start = search_start + relative_start;
        let end = start + phrase.len();
        if phrase_has_word_boundaries(text, start, end) {
            if found.is_some() {
                return None;
            }
            found = Some((start, end));
        }
        search_start = end;
    }
    found
}

/// Case/accent-insensitive variant of `find_unique_phrase_byte_range`.
///
/// Runs only after the exact search misses, and keeps the same uniqueness and
/// word-boundary requirements so tolerance never styles an ambiguous copy.
fn find_unique_phrase_byte_range_folded(text: &str, phrase: &str) -> Option<(usize, usize)> {
    let needle = phrase.chars().map(fold_phrase_char).collect::<Vec<_>>();
    if needle.is_empty() {
        return None;
    }
    let haystack = text
        .char_indices()
        .map(|(index, ch)| (index, fold_phrase_char(ch)))
        .collect::<Vec<_>>();
    let mut found = None;
    let mut index = 0usize;
    while index + needle.len() <= haystack.len() {
        let matches = haystack[index..index + needle.len()]
            .iter()
            .zip(&needle)
            .all(|((_, hay_ch), needle_ch)| hay_ch == needle_ch);
        if !matches {
            index += 1;
            continue;
        }
        let start = haystack[index].0;
        let end = haystack
            .get(index + needle.len())
            .map(|(byte, _)| *byte)
            .unwrap_or(text.len());
        if phrase_has_word_boundaries(text, start, end) {
            if found.is_some() {
                return None;
            }
            found = Some((start, end));
        }
        index += needle.len();
    }
    found
}

fn phrase_has_word_boundaries(text: &str, start: usize, end: usize) -> bool {
    let first = text[start..].chars().next();
    let last = text[..end].chars().next_back();
    let before = text[..start].chars().next_back();
    let after = text[end..].chars().next();

    let left_ok = !first.is_some_and(is_word_char) || !before.is_some_and(is_word_char);
    let right_ok = !last.is_some_and(is_word_char) || !after.is_some_and(is_word_char);
    left_ok && right_ok
}

/// Snap a projected character range to readable word boundaries.
///
/// Without this, proportional projection often bolds half a word after
/// translation expands/contracts a phrase. The rule is language-light: it uses
/// Unicode alphanumeric characters plus apostrophe/hyphen, which is good enough
/// for the first OPUS-MT MVP and easy to swap for a tokenizer-aware aligner.
pub(crate) fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '\'' || ch == '\u{2019}' || ch == '-'
}

fn last_word_for_marker(text: &str) -> Option<String> {
    text.split(|ch: char| !is_word_char(ch))
        .filter(|word| !word.trim().is_empty())
        .next_back()
        .map(str::to_string)
}

fn find_fragment_char_range(
    source_text: &str,
    fragment_source_text: &str,
    search_start: usize,
) -> Option<(usize, usize, usize)> {
    let fragment_text = normalize_fragment_text(fragment_source_text);
    if fragment_text.is_empty() || search_start > source_text.len() {
        return None;
    }
    let relative_start = source_text[search_start..].find(&fragment_text)?;
    let byte_start = search_start + relative_start;
    let byte_end = byte_start + fragment_text.len();
    Some((
        source_text[..byte_start].chars().count(),
        source_text[..byte_end].chars().count(),
        byte_end,
    ))
}

fn normalize_fragment_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn char_slice(text: &str, start: usize, end: usize) -> Option<String> {
    if start >= end || end > text.chars().count() {
        return None;
    }
    Some(text.chars().skip(start).take(end - start).collect())
}

pub(crate) fn byte_index_for_char(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map(|(index, _)| index)
        .unwrap_or(text.len())
}

#[cfg(test)]
mod tests {
    use super::{
        fragment_char_range, inline_phrase_probes_by_block, projected_translated_spans_best_effort,
        source_inline_model,
    };
    use crate::translation::html::parse_html_document;
    use crate::translation::storage::{PersistTranslationFragment, PersistTranslationInlinePhrase};

    #[test]
    fn collects_normalized_inline_spans() {
        let document = parse_html_document(
            "<!doctype html><html><body><p>La <em>puerta azul</em> es <strong>importante</strong>.</p></body></html>",
        );
        let node = document.select_first("p").expect("paragraph");

        let model = source_inline_model(node.as_node());
        let (text, spans) = (model.text, model.spans);

        assert_eq!(text, "La puerta azul es importante.");
        assert_eq!(spans.len(), 2);
        assert_eq!(&text[3..14], "puerta azul");
        assert_eq!(spans[0].start, 3);
        assert_eq!(spans[0].end, 14);
        assert_eq!(spans[0].tags, vec!["em"]);
        assert_eq!(spans[1].tags, vec!["strong"]);
    }

    #[test]
    fn prefers_verified_fragment_offsets() {
        let fragment = PersistTranslationFragment {
            source_start: 14,
            source_end: 19,
            source_text: "again".into(),
            text: "again".into(),
            inline_phrases: Vec::new(),
        };

        let range = fragment_char_range("first example again", &fragment, 0).expect("range");

        assert_eq!(range.0, 14);
        assert_eq!(range.1, 19);
    }

    #[test]
    fn refuses_partial_word_carryover_without_alignment() {
        let spans = vec![
            super::InlineFormattingSpan {
                start: 0,
                end: 8,
                tags: vec!["strong".into()],
            },
            super::InlineFormattingSpan {
                start: 4,
                end: 10,
                tags: vec!["em".into()],
            },
        ];

        let projected =
            projected_translated_spans_best_effort(&spans, "abcdefghij", "abcdefghij", &[]);

        assert!(projected.is_empty());
    }

    #[test]
    fn prefers_exact_carryover_for_unique_terms() {
        let spans = vec![super::InlineFormattingSpan {
            start: 12,
            end: 17,
            tags: vec!["em".into(), "strong".into()],
        }];

        let projected = projected_translated_spans_best_effort(
            &spans,
            "El proyecto Orion.",
            "The Orion project.",
            &[],
        );

        assert_eq!(projected.len(), 1);
        assert_eq!(
            &"The Orion project."[projected[0].start..projected[0].end],
            "Orion"
        );
        assert_eq!(projected[0].tags, vec!["em", "strong"]);
    }

    #[test]
    fn uses_translated_phrase_hint_for_exact_target_match() {
        let spans = vec![super::InlineFormattingSpan {
            start: 3,
            end: 14,
            tags: vec!["strong".into()],
        }];
        let hints = vec![PersistTranslationInlinePhrase {
            source_text: "puerta azul".into(),
            text: "blue door".into(),
        }];

        let projected = projected_translated_spans_best_effort(
            &spans,
            "La puerta azul abre.",
            "The blue door opens.",
            &hints,
        );

        assert_eq!(projected.len(), 1);
        assert_eq!(
            &"The blue door opens."[projected[0].start..projected[0].end],
            "blue door"
        );
    }

    #[test]
    fn uses_phrase_hint_despite_probe_casing_and_punctuation() {
        // "método práctico" is emphasized; the standalone probe translation
        // came back sentence-cased with a closing period, and the target prose
        // wraps the phrase in typographic quotes.
        let source_text = "Requiere del método práctico para avanzar.";
        let start = source_text
            .chars()
            .collect::<Vec<_>>()
            .windows("método práctico".chars().count())
            .position(|window| window.iter().collect::<String>() == "método práctico")
            .expect("phrase start");
        let spans = vec![super::InlineFormattingSpan {
            start,
            end: start + "método práctico".chars().count(),
            tags: vec!["strong".into()],
        }];
        let hints = vec![PersistTranslationInlinePhrase {
            source_text: "método práctico".into(),
            text: "Practical method.".into(),
        }];
        let translated_text = "It requires the “practical method” to advance.";

        let projected =
            projected_translated_spans_best_effort(&spans, source_text, translated_text, &hints);

        assert_eq!(projected.len(), 1);
        assert_eq!(
            &translated_text[projected[0].start..projected[0].end],
            "practical method"
        );
        assert_eq!(projected[0].tags, vec!["strong"]);
    }

    #[test]
    fn folds_extended_latin_diacritics_beyond_spanish_french() {
        assert_eq!(super::fold_phrase_char('É'), 'e');
        assert_eq!(super::fold_phrase_char('ñ'), 'n');
        assert_eq!(super::fold_phrase_char('Ř'), 'r');
        assert_eq!(super::fold_phrase_char('ș'), 's');
        assert_eq!(super::fold_phrase_char('ą'), 'a');
        // No canonical decomposition: must stay itself, never approximated.
        assert_eq!(super::fold_phrase_char('ł'), 'ł');
        assert_eq!(super::fold_phrase_char('x'), 'x');
    }

    #[test]
    fn folded_match_still_requires_unique_occurrence() {
        let spans = vec![super::InlineFormattingSpan {
            start: 0,
            end: 6,
            tags: vec!["em".into()],
        }];
        let hints = vec![PersistTranslationInlinePhrase {
            source_text: "método".into(),
            text: "Method".into(),
        }];

        // "method" appears twice in the target; tolerant matching must refuse
        // to pick one, and proportional fallback takes over instead.
        let projected = super::exact_translated_span(
            &spans[0],
            "método claro",
            "One method or another method works.",
            &hints,
        );

        assert!(projected.is_none());
    }

    #[test]
    fn collects_inline_phrase_probes_by_readable_block() {
        let probes = inline_phrase_probes_by_block(
            "<!doctype html><html><body><article><p>La <strong>puerta azul</strong> abre.</p></article></body></html>",
        );

        assert_eq!(probes.len(), 1);
        assert_eq!(probes[0][0].text, "puerta azul");
        assert_eq!(probes[0][0].source_start, 3);
    }

    #[test]
    fn excludes_footnote_markers_from_translation_source_text() {
        let html = "<!doctype html><html><body><article><p>Topic<a href=\"#fn1\" role=\"doc-noteref\"><sup>1</sup></a> changes.</p></article></body></html>";

        let blocks = super::DomPlan::new(html)
            .blocks
            .into_iter()
            .map(|block| block.inline.text)
            .collect::<Vec<_>>();
        let document = parse_html_document(html);
        let paragraph = document.select_first("p").expect("paragraph");
        let markers = source_inline_model(paragraph.as_node()).markers;

        assert_eq!(blocks, vec!["Topic changes."]);
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].source_offset, "Topic".chars().count());
        assert_eq!(markers[0].source_anchor_text.as_deref(), Some("Topic"));
        assert!(markers[0].html.contains("role=\"doc-noteref\""));
    }
}
