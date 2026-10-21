//! Best-effort text/phrase projection for the legacy Provider path only.
//! The parent owns policy dispatch and rejects invalid sentence/protected output before this module.
use kuchikiki::NodeRef;

use super::{escape_html, replace_children_with_text};
use crate::document_uploads::is_note_marker;
use crate::translation::{
    html::parse_html_document,
    inline_markup::{
        byte_index_for_char, fold_phrase_char, fragment_char_range, is_inline_formatting_element,
        is_word_char, local_spans_for_fragment, projected_translated_spans_best_effort,
        serialize_node, source_inline_model, InlinePreservedMarker, ProjectedInlineSpan,
    },
    storage::{
        PersistTranslationFragment, PersistTranslationInlinePhrase, PersistTranslationSection,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProjectedInlineMarker {
    byte_offset: usize,
    html: String,
}

/// Preserve the existing fallback order; these guesses cannot repair rejected protected mappings.
pub(super) fn replace_block(node: &NodeRef, section: &PersistTranslationSection) {
    if requires_preserved_block(node) {
        insert_translation_after(node, section);
    } else if replace_text_with_fragment_formatting(node, &section.fragments) {
        // Segment-level source/target pairs take precedence over block-wide projection.
    } else if replace_text_preserving_full_inline_formatting(node, section.text.trim()) {
        // The enclosing source wrapper supplies unambiguous whole-block formatting.
    } else if replace_text_projecting_inline_formatting(
        node,
        section.text.trim(),
        &section_inline_phrase_hints(section),
    ) {
        // Only unique phrase matches are styled; uncertain spans remain plain text.
    } else {
        replace_children_with_text(node, section.text.trim());
    }
}

/// Preserve media/table-heavy source blocks by inserting translation nearby.
///
/// Replacing table/image-heavy content can destroy layout or asset references.
/// For those blocks, the source block remains intact and the translated text is
/// inserted immediately after it.
fn insert_translation_after(node: &NodeRef, section: &PersistTranslationSection) {
    let tag = if section.is_heading { "h2" } else { "p" };
    let wrapper = parse_html_document(format!(
        "<!doctype html><html><body><{tag} class=\"papercut-translation-inline\" data-source-ordinal=\"{}\">{}</{tag}></body></html>",
        section.source_ordinal,
        escape_html(section.text.trim())
    ));
    let Ok(inserted) = wrapper.select_first(&format!("{tag}.papercut-translation-inline")) else {
        return;
    };
    if node
        .as_element()
        .is_some_and(|element| element.name.local.as_ref() == "body")
    {
        node.append(inserted.as_node().clone());
    } else {
        node.insert_after(inserted.as_node().clone());
    }
}

/// Keep source nodes whose content/identity cannot be reconstructed from plain MT text.
fn requires_preserved_block(node: &NodeRef) -> bool {
    node.descendants()
        .filter(|child| child != node)
        .any(|child| {
            let Some(element) = child.as_element() else {
                return false;
            };
            let attrs = element.attributes.borrow();
            matches!(
                element.name.local.as_ref(),
                "img" | "table" | "figure" | "audio" | "video" | "code"
            ) || attrs
                .get("translate")
                .is_some_and(|value| value.eq_ignore_ascii_case("no"))
                || (element.name.local.as_ref() != "a" && attrs.get("id").is_some())
        })
}

/// Preserve whole-block inline emphasis when it is structurally unambiguous.
///
/// MT works on plain text, so we cannot safely place word-level markup after
/// translation without alignment. This narrow path handles the reliable case:
/// one formatting wrapper owns the entire block, for example
/// `<p><em>...</em></p>` or `<p><strong><em>...</em></strong></p>`.
fn replace_text_preserving_full_inline_formatting(node: &NodeRef, text: &str) -> bool {
    let Some(leaf) = full_block_inline_formatting_leaf(node) else {
        return false;
    };
    if !subtree_is_inline_formatting_only(&leaf) {
        return false;
    }
    replace_children_with_text(&leaf, text);
    true
}

fn full_block_inline_formatting_leaf(node: &NodeRef) -> Option<NodeRef> {
    let significant = significant_children(node);
    if significant.len() != 1 || !is_inline_formatting_element(&significant[0]) {
        return None;
    }

    let mut leaf = significant[0].clone();
    loop {
        let children = significant_children(&leaf);
        if children.len() != 1 || !is_inline_formatting_element(&children[0]) {
            break;
        }
        leaf = children[0].clone();
    }

    Some(leaf)
}

fn significant_children(node: &NodeRef) -> Vec<NodeRef> {
    node.children()
        .filter(|child| !is_whitespace_text(child))
        .collect()
}

fn is_whitespace_text(node: &NodeRef) -> bool {
    node.as_text()
        .map(|text| text.borrow().trim().is_empty())
        .unwrap_or(false)
}

fn subtree_is_inline_formatting_only(node: &NodeRef) -> bool {
    if node.as_text().is_some() {
        return true;
    }
    if !is_inline_formatting_element(node) {
        return false;
    }
    node.children()
        .filter(|child| !is_whitespace_text(child))
        .all(|child| subtree_is_inline_formatting_only(&child))
}

/// Project mixed inline emphasis spans onto translated text.
///
/// This is still conservative: source DOM text nodes give us inline span
/// positions/tag stacks, then we keep only exact, unique phrase matches. If projected spans overlap or cannot
/// land cleanly, we keep the plain-text fallback rather than drawing misleading
/// emphasis.
fn replace_text_projecting_inline_formatting(
    node: &NodeRef,
    translated_text: &str,
    translation_hints: &[PersistTranslationInlinePhrase],
) -> bool {
    let source_model = source_inline_model(node);
    let preserved_anchors = serialized_anchor_nodes(node);
    if source_model.text.is_empty()
        || translated_text.trim().is_empty()
        || (source_model.spans.is_empty()
            && source_model.markers.is_empty()
            && preserved_anchors.is_empty())
    {
        return false;
    }

    let source_len = source_model.text.chars().count();
    if source_len == 0 {
        return false;
    }

    let projected_spans = if source_model.spans.is_empty() {
        Vec::new()
    } else if source_model
        .spans
        .iter()
        .any(|span| span.start == 0 && span.end >= source_len)
    {
        Vec::new()
    } else {
        projected_translated_spans_best_effort(
            &source_model.spans,
            &source_model.text,
            translated_text,
            translation_hints,
        )
    };
    let projected_markers =
        project_inline_markers(&source_model.markers, 0, source_len, translated_text);

    replace_children_with_projected_formatting(
        node,
        translated_text,
        &projected_spans,
        &projected_markers,
    );
    append_preserved_anchor_nodes(node, &preserved_anchors);
    true
}

/// All phrase hints from a section's fragments, for block-level projection.
///
/// When fragment-to-source pairing fails, the probe-translated emphasized
/// phrases are still valid for the whole section; passing them along keeps
/// exact phrase placement available instead of losing every hint with the
/// fragment path.
fn section_inline_phrase_hints(
    section: &PersistTranslationSection,
) -> Vec<PersistTranslationInlinePhrase> {
    section
        .fragments
        .iter()
        .flat_map(|fragment| fragment.inline_phrases.iter().cloned())
        .collect()
}

/// Render translated segment fragments while preserving source inline marks.
///
/// Each fragment is one planned engine segment with its original source text.
/// Matching those fragments back into the source DOM gives a much smaller
/// alignment window than paragraph-wide projection, which is crucial for long
/// academic paragraphs with many independent bold/italic phrases.
fn replace_text_with_fragment_formatting(
    node: &NodeRef,
    fragments: &[PersistTranslationFragment],
) -> bool {
    let fragments = fragments
        .iter()
        .filter(|fragment| !fragment.text.trim().is_empty())
        .collect::<Vec<_>>();
    if fragments.is_empty() {
        return false;
    }

    let content = full_block_inline_formatting_leaf(node).unwrap_or_else(|| node.clone());
    let mut source_model = source_inline_model(&content);
    if content != *node {
        // The enclosing DOM already supplies this tag; match only its inner emphasis.
        for span in &mut source_model.spans {
            span.tags.remove(0);
        }
        source_model.spans.retain(|span| !span.tags.is_empty());
    }
    if source_model.text.is_empty() {
        return false;
    }

    let mut search_start = 0usize;
    let mut rendered_fragments = Vec::new();
    for fragment in fragments {
        let Some((source_start, source_end, next_search_start)) =
            fragment_char_range(&source_model.text, fragment, search_start)
        else {
            return false;
        };
        search_start = next_search_start;
        let local_source_text = source_model
            .text
            .chars()
            .skip(source_start)
            .take(source_end.saturating_sub(source_start))
            .collect::<String>();
        let local_spans = local_spans_for_fragment(&source_model.spans, source_start, source_end);
        let projected_spans = if local_spans.is_empty() || local_source_text.is_empty() {
            Vec::new()
        } else {
            projected_translated_spans_best_effort(
                &local_spans,
                &local_source_text,
                fragment.text.trim(),
                &fragment.inline_phrases,
            )
        };
        let projected_markers = project_inline_markers(
            &source_model.markers,
            source_start,
            source_end,
            fragment.text.trim(),
        );
        rendered_fragments.push((
            fragment.text.trim().to_string(),
            projected_spans,
            projected_markers,
        ));
    }

    let preserved_anchors = serialized_anchor_nodes(&content);
    let children = content.children().collect::<Vec<_>>();
    for child in children {
        child.detach();
    }
    for (index, (text, spans, markers)) in rendered_fragments.iter().enumerate() {
        if index > 0 {
            content.append(NodeRef::new_text(" "));
        }
        append_projected_formatting(&content, text, spans, markers);
    }
    append_preserved_anchor_nodes(&content, &preserved_anchors);
    true
}

fn replace_children_with_projected_formatting(
    node: &NodeRef,
    translated_text: &str,
    spans: &[ProjectedInlineSpan],
    markers: &[ProjectedInlineMarker],
) {
    let children = node.children().collect::<Vec<_>>();
    for child in children {
        child.detach();
    }

    append_projected_formatting(node, translated_text, spans, markers);
}

/// Append translated text with already-projected inline spans.
///
/// This function only materializes DOM nodes. It deliberately does not decide
/// whether a span is trustworthy; that belongs in `inline_markup`, where future
/// phrase alignment or placeholder repair can replace today's projection.
fn append_projected_formatting(
    node: &NodeRef,
    translated_text: &str,
    spans: &[ProjectedInlineSpan],
    markers: &[ProjectedInlineMarker],
) {
    let mut cursor = 0usize;
    let mut marker_index = 0usize;
    for span in spans {
        if cursor < span.start {
            append_text_range_with_markers(
                node,
                translated_text,
                cursor,
                span.start,
                markers,
                &mut marker_index,
                &[],
            );
        }
        append_text_range_with_markers(
            node,
            translated_text,
            span.start,
            span.end,
            markers,
            &mut marker_index,
            &span.tags,
        );
        cursor = span.end;
    }
    if cursor < translated_text.len() {
        append_text_range_with_markers(
            node,
            translated_text,
            cursor,
            translated_text.len(),
            markers,
            &mut marker_index,
            &[],
        );
    }
    while marker_index < markers.len() {
        append_projected_marker_node(node, &markers[marker_index]);
        marker_index += 1;
    }
}

/// Target ranges are UTF-8 byte offsets; clip ordered markers to this styled slice without duplicating them.
fn append_text_range_with_markers(
    node: &NodeRef,
    translated_text: &str,
    start: usize,
    end: usize,
    markers: &[ProjectedInlineMarker],
    marker_index: &mut usize,
    tags: &[String],
) {
    let mut cursor = start;
    while *marker_index < markers.len() && markers[*marker_index].byte_offset <= end {
        let marker_offset = markers[*marker_index].byte_offset.max(start).min(end);
        append_text_piece(node, &translated_text[cursor..marker_offset], tags);
        append_projected_marker_node(node, &markers[*marker_index]);
        *marker_index += 1;
        cursor = marker_offset;
    }
    append_text_piece(node, &translated_text[cursor..end], tags);
}

fn append_text_piece(node: &NodeRef, text: &str, tags: &[String]) {
    if text.is_empty() {
        return;
    }
    if tags.is_empty() {
        node.append(NodeRef::new_text(text));
    } else if let Some(formatted) = formatted_inline_node(tags, text) {
        node.append(formatted);
    } else {
        node.append(NodeRef::new_text(text));
    }
}

/// Legacy-only marker guesses convert source scalar offsets into target byte positions, not semantic links.
fn project_inline_markers(
    markers: &[InlinePreservedMarker],
    source_start: usize,
    source_end: usize,
    translated_text: &str,
) -> Vec<ProjectedInlineMarker> {
    let source_len = source_end.saturating_sub(source_start);
    markers
        .iter()
        .filter(|marker| marker.source_offset >= source_start && marker.source_offset <= source_end)
        .map(|marker| ProjectedInlineMarker {
            byte_offset: {
                let local_offset = marker.source_offset.saturating_sub(source_start);
                // A marker after the last source word belongs after the whole
                // translated text (including any punctuation MT added there);
                // anchor-word lookup would drag it back before that punctuation.
                if local_offset >= source_len {
                    translated_text.len()
                } else {
                    let fallback =
                        projected_marker_byte_offset(local_offset, source_len, translated_text);
                    marker
                        .source_anchor_text
                        .as_deref()
                        .and_then(|anchor| {
                            marker_anchor_byte_offset(translated_text, anchor, fallback)
                        })
                        .unwrap_or(fallback)
                }
            },
            html: marker.html.clone(),
        })
        .collect()
}

/// Prefer a carried-over anchor word nearest the legacy estimate when the source term survives translation.
fn marker_anchor_byte_offset(
    translated_text: &str,
    source_anchor: &str,
    projected_offset: usize,
) -> Option<usize> {
    let normalized_anchor = normalize_marker_lookup_word(source_anchor);
    if normalized_anchor.is_empty() {
        return None;
    }
    translated_word_ranges(translated_text)
        .into_iter()
        .filter(|range| {
            normalize_marker_lookup_word(&translated_text[range.0..range.1]) == normalized_anchor
        })
        .min_by_key(|range| range.1.abs_diff(projected_offset))
        .map(|range| range.1)
}

/// Proportional placement is retained only for legacy output; sentence/protected mappings never use it.
fn projected_marker_byte_offset(
    local_source_offset: usize,
    local_source_len: usize,
    translated_text: &str,
) -> usize {
    let translated_len = translated_text.chars().count();
    if local_source_len == 0 || translated_len == 0 {
        return translated_text.len();
    }
    let char_offset = local_source_offset.saturating_mul(translated_len) / local_source_len;
    snap_marker_byte_offset(
        translated_text,
        byte_index_for_char(translated_text, char_offset.min(translated_len)),
    )
}

/// Keep the estimated UTF-8 boundary outside a target word rather than inserting a note in its middle.
fn snap_marker_byte_offset(text: &str, offset: usize) -> usize {
    for (start, end) in translated_word_ranges(text) {
        if offset > start && offset < end {
            return end;
        }
    }
    offset.min(text.len())
}

fn translated_word_ranges(text: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut start = None;
    for (index, ch) in text.char_indices() {
        if is_word_char(ch) {
            if start.is_none() {
                start = Some(index);
            }
        } else if let Some(word_start) = start.take() {
            ranges.push((word_start, index));
        }
    }
    if let Some(word_start) = start {
        ranges.push((word_start, text.len()));
    }
    ranges
}

fn normalize_marker_lookup_word(value: &str) -> String {
    value
        .chars()
        .filter_map(|ch| {
            let folded = fold_phrase_char(ch);
            is_word_char(folded).then_some(folded)
        })
        .collect()
}

fn serialized_anchor_nodes(node: &NodeRef) -> Vec<String> {
    let Ok(anchors) = node.select("a") else {
        return Vec::new();
    };
    anchors
        .filter(|anchor| !is_note_marker(anchor.as_node()))
        .filter_map(|anchor| serialize_node(anchor.as_node()))
        .collect()
}

fn append_projected_marker_node(node: &NodeRef, marker: &ProjectedInlineMarker) {
    if let Some(cloned) = parsed_first_element(&marker.html, "a") {
        node.append(cloned);
    }
}

fn append_preserved_anchor_nodes(node: &NodeRef, anchors: &[String]) {
    for anchor in anchors {
        node.append(NodeRef::new_text(" "));
        if let Some(cloned) = parsed_first_element(anchor, "a") {
            node.append(cloned);
        }
    }
}

fn formatted_inline_node(tags: &[String], text: &str) -> Option<NodeRef> {
    let outer = tags.first()?;
    let mut html = String::from("<!doctype html><html><body>");
    for tag in tags {
        html.push('<');
        html.push_str(tag);
        html.push('>');
    }
    html.push_str(&escape_html(text));
    for tag in tags.iter().rev() {
        html.push_str("</");
        html.push_str(tag);
        html.push('>');
    }
    html.push_str("</body></html>");
    let wrapper = parse_html_document(html);
    parsed_first_element_from_document(&wrapper, outer)
}

fn parsed_first_element(html: &str, selector: &str) -> Option<NodeRef> {
    let wrapper = parse_html_document(format!("<!doctype html><html><body>{html}</body></html>"));
    parsed_first_element_from_document(&wrapper, selector)
}

fn parsed_first_element_from_document(document: &NodeRef, selector: &str) -> Option<NodeRef> {
    document
        .select_first(selector)
        .ok()
        .map(|node| node.as_node().clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::translation::dom::DomPlan;

    #[test]
    fn projected_source_styles_use_the_shared_text_renderer_without_trusting_html() {
        let source = crate::document_uploads::sanitize_html(
            r#"<p id="r">Une bague en <strong>or</strong><a role="doc-noteref" href='#n'>1</a>.</p><p id="n">Note<a role="doc-backlink" href='#r'>Back</a></p>"#,
        );
        let plan = DomPlan::new(&source);
        let block = &plan.blocks[0];
        let target = "A gold ring. <a href='#forged'>Fake</a><script>bad()</script>";
        let spans = vec![crate::translation::inline_markup::ProjectedInlineSpan {
            start: 2,
            end: 6,
            tags: block.inline.spans[0].tags.clone(),
        }];
        let markers = super::project_inline_markers(
            &block.inline.markers,
            0,
            block.inline.text.chars().count(),
            target,
        );
        super::replace_children_with_projected_formatting(&block.node, target, &spans, &markers);
        let saved = crate::document_uploads::sanitize_html(
            &super::super::serialize_document(&plan.document).unwrap(),
        );
        let document = parse_html_document(&saved);
        assert_eq!(
            document.select_first("#r strong").unwrap().text_contents(),
            "gold"
        );
        assert_eq!(document.select("a").unwrap().count(), 2);
        assert_eq!(
            document
                .select_first("a[role=doc-noteref]")
                .unwrap()
                .attributes
                .borrow()
                .get("href"),
            Some("#n")
        );
        assert_eq!(
            document
                .select_first("a[role=doc-backlink]")
                .unwrap()
                .attributes
                .borrow()
                .get("href"),
            Some("#r")
        );
        assert!(document.select_first("script").is_err());
        document
            .select_first("a[role=doc-noteref]")
            .unwrap()
            .as_node()
            .detach();
        assert!(document
            .select_first("#r")
            .unwrap()
            .text_contents()
            .contains("<a href='#forged'>Fake</a><script>bad()</script>"));
    }
}
