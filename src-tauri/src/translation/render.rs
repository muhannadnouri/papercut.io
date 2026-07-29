//! Rendering translated documents.
//!
//! The preferred path clones the sanitized reader HTML and replaces readable
//! block text in document order. That preserves links, images, ids, and EPUB
//! asset rewrites around the text. Footnote/link anchors are preserved in
//! translated blocks; media/table-heavy blocks keep the original block intact
//! and insert translated text nearby instead of destroying assets.

use kuchikiki::NodeRef;

use super::html::parse_html_document;
use super::inline_markup::{
    byte_index_for_char, fold_phrase_char, fragment_char_range, is_inline_formatting_element,
    is_nontranslatable_inline_marker, is_word_char, local_spans_for_fragment,
    projected_translated_spans_best_effort, serialize_node, source_inline_model,
    InlinePreservedMarker, ProjectedInlineSpan,
};
use super::storage::{
    PersistTranslationFragment, PersistTranslationInlinePhrase, PersistTranslationRequest,
    PersistTranslationSection,
};

const MEDIA_DESCENDANT_SELECTOR: &str = "img,table,figure,audio,video";

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProjectedInlineMarker {
    byte_offset: usize,
    html: String,
}

/// Render a persisted translated document.
///
/// The DOM-preserving path is best-effort by design: if the stored reader HTML
/// cannot be parsed/mapped safely, we fall back to section-only HTML so a
/// completed translation is still viewable, searchable, and durable.
pub(crate) fn render_translated_html(title: &str, request: &PersistTranslationRequest) -> String {
    render_translated_dom(title, request).unwrap_or_else(|| {
        // Losing DOM preservation drops links/images/anchors down to plain
        // sections; that trade-off should be visible when triaging output.
        log::warn!(
            "translation: DOM-preserving render failed for '{title}'; using section-only fallback document"
        );
        render_section_document(title, request)
    })
}

/// Clone the safe reader HTML and replace readable blocks in source order.
///
/// This keeps EPUB-rewritten image links, footnote anchors, and existing ids in
/// the output. Render block discovery mirrors upload section extraction so
/// nested footnote paragraphs do not shift translated sections out of place.
fn render_translated_dom(title: &str, request: &PersistTranslationRequest) -> Option<String> {
    if request.source.view_html.trim().is_empty() {
        return None;
    }
    let document = parse_html_document(request.source.view_html.clone());
    update_title(&document, title);
    annotate_article(&document, request);

    let mut sections = request.translated_sections.iter();
    let mut replaced_any = false;
    let mut mapped_sections = 0usize;
    for node in collect_render_blocks(&document) {
        let Some(section) = sections.next() else {
            break;
        };
        mapped_sections += 1;
        annotate_block(&node, section);
        if section.text.trim().is_empty() {
            continue;
        }
        if has_media_descendant(&node) {
            insert_translation_after(&node, section);
        } else if replace_text_with_fragment_formatting(&node, &section.fragments) {
            // Segment-level source/target pairs give safer inline placement than
            // projecting formatting across a whole translated paragraph.
        } else if replace_text_preserving_full_inline_formatting(&node, section.text.trim()) {
            // The helper already replaced the deepest safe inline wrapper text.
        } else if replace_text_projecting_inline_formatting(
            &node,
            section.text.trim(),
            &section_inline_phrase_hints(section),
        ) {
            // The helper projected safe source emphasis spans onto translated text.
        } else {
            replace_children_with_text(&node, section.text.trim());
        }
        replaced_any = true;
    }

    if !replaced_any || mapped_sections != request.translated_sections.len() {
        return None;
    }
    serialize_document(&document)
}

/// Collect the same block units the upload parser extracts.
///
/// CSS selection returns both `<li>` and its nested `<p>`, which shifts
/// translated sections out of alignment and scrambles endnotes. This traversal
/// takes the first readable block and does not recurse into it, matching the
/// importer scanner's "consume block and skip its descendants" behavior.
fn collect_render_blocks(document: &NodeRef) -> Vec<NodeRef> {
    let root = document
        .select_first("body")
        .ok()
        .map(|body| body.as_node().clone())
        .unwrap_or_else(|| document.clone());
    let mut blocks = Vec::new();
    collect_render_blocks_from(&root, &mut blocks);
    blocks
}

fn collect_render_blocks_from(node: &NodeRef, blocks: &mut Vec<NodeRef>) {
    for child in node.children() {
        if is_render_block(&child) {
            blocks.push(child);
        } else {
            collect_render_blocks_from(&child, blocks);
        }
    }
}

fn is_render_block(node: &NodeRef) -> bool {
    let Some(element) = node.as_element() else {
        return false;
    };
    matches!(
        element.name.local.as_ref(),
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p" | "li" | "blockquote"
    )
}

/// Build a simple translated document when source DOM preservation is not safe.
///
/// This fallback is deliberately boring HTML: escaped text, stable section ids,
/// source ordinals, and heading metadata. It is less rich visually, but it keeps
/// search and future audiobook generation on the same generated-document path.
fn render_section_document(title: &str, request: &PersistTranslationRequest) -> String {
    let mut body = String::new();
    for section in &request.translated_sections {
        append_section_html(&mut body, section);
    }

    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{}</title></head>\
         <body><article data-papercut-translation=\"true\" data-source-document=\"{}\" data-target-language=\"{}\">\
         <h1>{}</h1>{}</article></body></html>",
        escape_html(title),
        escape_html(&request.source.document_url),
        escape_html(&request.target_language),
        escape_html(title),
        body
    )
}

/// Append one translated section to the fallback HTML renderer.
///
/// Heading sections become real heading nodes so the generated document keeps a
/// useful outline even if the original DOM could not be reused.
fn append_section_html(body: &mut String, section: &PersistTranslationSection) {
    let section_id = section_anchor_id(section);
    body.push_str("<section id=\"");
    body.push_str(&section_id);
    body.push_str("\" data-source-ordinal=\"");
    body.push_str(&section.source_ordinal.to_string());
    body.push('"');
    if let Some(heading) = section
        .source_heading
        .as_deref()
        .filter(|heading| !heading.trim().is_empty())
    {
        body.push_str(" data-source-heading=\"");
        body.push_str(&escape_html(heading));
        body.push('"');
    }
    body.push('>');
    if section.is_heading {
        body.push_str("<h2 id=\"");
        body.push_str(&section_id);
        body.push_str("-heading\">");
        body.push_str(&escape_html(section.text.trim()));
        body.push_str("</h2>");
        body.push_str("</section>");
        return;
    }
    for paragraph in section
        .text
        .split('\n')
        .map(str::trim)
        .filter(|paragraph| !paragraph.is_empty())
    {
        body.push_str("<p>");
        body.push_str(&escape_html(paragraph));
        body.push_str("</p>");
    }
    body.push_str("</section>");
}

/// Update the document title when the cloned reader HTML has one.
fn update_title(document: &NodeRef, title: &str) {
    if let Ok(title_node) = document.select_first("title") {
        replace_children_with_text(title_node.as_node(), title);
    }
}

/// Mark the cloned reader root as a generated translation.
///
/// The attributes are useful for future styling/debugging and for distinguishing
/// translated variants from original uploads without needing another DB lookup.
fn annotate_article(document: &NodeRef, request: &PersistTranslationRequest) {
    let article = document
        .select_first("article")
        .ok()
        .or_else(|| document.select_first("body").ok());
    let Some(article) = article else {
        return;
    };
    let mut attrs = article.attributes.borrow_mut();
    attrs.insert("data-papercut-translation", "true".to_string());
    attrs.insert("data-source-document", request.source.document_url.clone());
    attrs.insert("data-target-language", request.target_language.clone());
}

/// Attach source metadata to a translated block.
///
/// Existing ids win because EPUB/HTML links may target them. Missing ids get a
/// stable translation-section id so search results and future readers have an
/// anchor to land on.
fn annotate_block(node: &NodeRef, section: &PersistTranslationSection) {
    let Some(element) = node.as_element() else {
        return;
    };
    let mut attrs = element.attributes.borrow_mut();
    if attrs.get("id").is_none() {
        attrs.insert("id", section_anchor_id(section));
    }
    attrs.insert("data-source-ordinal", section.source_ordinal.to_string());
    if let Some(heading) = section
        .source_heading
        .as_deref()
        .filter(|heading| !heading.trim().is_empty())
    {
        attrs.insert("data-source-heading", heading.to_string());
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
    node.insert_after(inserted.as_node().clone());
}

/// Detect media/table descendants whose behavior should survive rendering.
fn has_media_descendant(node: &NodeRef) -> bool {
    node.select(MEDIA_DESCENDANT_SELECTOR)
        .ok()
        .and_then(|mut nodes| nodes.next())
        .is_some()
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
    replace_children_with_text(&leaf, text);
    true
}

fn full_block_inline_formatting_leaf(node: &NodeRef) -> Option<NodeRef> {
    let significant = significant_children(node);
    if significant.len() != 1 || !is_inline_formatting_element(&significant[0]) {
        return None;
    }

    let mut leaf = significant[0].clone();
    if !subtree_is_inline_formatting_only(&leaf) {
        return None;
    }

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
/// positions/tag stacks, then we map each span by relative character position
/// snapped to translated word boundaries. If projected spans overlap or cannot
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

    let source_model = source_inline_model(node);
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

    let preserved_anchors = serialized_anchor_nodes(node);
    let children = node.children().collect::<Vec<_>>();
    for child in children {
        child.detach();
    }
    for (index, (text, spans, markers)) in rendered_fragments.iter().enumerate() {
        if index > 0 {
            node.append(NodeRef::new_text(" "));
        }
        append_projected_formatting(node, text, spans, markers);
    }
    append_preserved_anchor_nodes(node, &preserved_anchors);
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
        .filter(|anchor| !is_nontranslatable_inline_marker(anchor.as_node()))
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

/// Replace all child nodes with one text node.
///
/// This is the plain-text fallback for blocks whose inline formatting is too
/// ambiguous for the current span projection. Richer replacement should
/// wait for phrase alignment that can survive translated word reordering.
fn replace_children_with_text(node: &NodeRef, text: &str) {
    let children = node.children().collect::<Vec<_>>();
    for child in children {
        child.detach();
    }
    node.append(NodeRef::new_text(text));
}

/// Serialize the Kuchikiki DOM back into UTF-8 HTML.
fn serialize_document(document: &NodeRef) -> Option<String> {
    let mut bytes = Vec::new();
    document.serialize(&mut bytes).ok()?;
    String::from_utf8(bytes).ok()
}

/// Derive a stable anchor from the original source ordinal.
fn section_anchor_id(section: &PersistTranslationSection) -> String {
    format!("translation-section-{}", section.source_ordinal + 1)
}

/// Escape text for the fallback renderer and generated inline translation nodes.
fn escape_html(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::render_translated_html;
    use crate::translation::html::parse_html_document;
    use crate::translation::source::{TranslationSourceBlock, TranslationSourceDocument};
    use crate::translation::storage::{
        PersistTranslationFragment, PersistTranslationRequest, PersistTranslationSection,
    };

    #[test]
    fn preserves_links_when_block_contains_anchor() {
        let request = request(
            "<!doctype html><html><head><title>Source</title></head><body><article><p>See <a href=\"#note\">note</a>.</p><p id=\"note\">Note body.</p></article></body></html>",
            vec![
                section(0, false, "Voir la note."),
                section(1, false, "Corps de la note."),
            ],
        );

        let html = render_translated_html("Translated", &request);

        assert!(html.contains("href=\"#note\""));
        assert!(html.contains("Voir la note."));
        assert!(html.contains("Corps de la note."));
        assert!(html.contains("data-papercut-translation=\"true\""));
    }

    #[test]
    fn replaces_simple_heading_text_in_place() {
        let request = request(
            "<!doctype html><html><head><title>Source</title></head><body><article><h1>Chapitre</h1><p>Bonjour</p></article></body></html>",
            vec![section(0, true, "Chapter"), section(1, false, "Hello")],
        );

        let html = render_translated_html("Translated", &request);

        assert!(html.contains("<h1"));
        assert!(html.contains(">Chapter</h1>"));
        assert!(html.contains("<p"));
        assert!(html.contains(">Hello</p>"));
        assert!(!html.contains(">Chapitre</h1>"));
    }

    #[test]
    fn preserves_whole_block_inline_emphasis() {
        let request = request(
            "<!doctype html><html><body><article><p><strong>Importante.</strong></p><p><em><strong>Muy urgente.</strong></em></p></article></body></html>",
            vec![
                section(0, false, "Important."),
                section(1, false, "Very urgent."),
            ],
        );

        let html = render_translated_html("Translated", &request);

        assert!(html.contains("<strong>Important.</strong>"));
        assert!(html.contains("<em><strong>Very urgent.</strong></em>"));
        assert!(!html.contains("Importante."));
    }

    #[test]
    fn projects_single_partial_inline_emphasis_to_translated_word_boundary() {
        let request = request(
            "<!doctype html><html><body><article><p>Esto es <strong>importante</strong>.</p></article></body></html>",
            vec![section(0, false, "This is important.")],
        );

        let html = render_translated_html("Translated", &request);

        assert!(html.contains(">This is <strong>important</strong>.</p>"));
        assert!(!html.contains("importante"));
    }

    #[test]
    fn projects_multiple_partial_inline_spans_when_ranges_do_not_overlap() {
        let request = request(
            "<!doctype html><html><body><article><p>Esto es <strong>importante</strong> y <em>urgente</em>.</p></article></body></html>",
            vec![section(0, false, "This is important and urgent.")],
        );

        let html = render_translated_html("Translated", &request);

        assert!(html.contains(">This is <strong>important</strong> and <em>urgent</em>.</p>"));
        assert!(!html.contains("importante"));
        assert!(!html.contains("urgente"));
    }

    #[test]
    fn projects_inline_spans_inside_translated_fragments() {
        let request = request(
            "<!doctype html><html><body><article><p>La <em>puerta azul</em> es <strong>importante</strong>. El codigo es <em><strong>Orion</strong></em>.</p></article></body></html>",
            vec![section_with_fragments(
                0,
                false,
                "The blue door is important. The code is Orion.",
                vec![
                    fragment("La puerta azul es importante.", "The blue door is important."),
                    fragment("El codigo es Orion.", "The code is Orion."),
                ],
            )],
        );

        let html = render_translated_html("Translated", &request);

        assert!(html.contains("<em>blue door</em>"));
        assert!(html.contains("<strong>important</strong>"));
        assert!(html.contains("<em><strong>Orion</strong></em>"));
        assert!(!html.contains("puerta azul"));
        assert!(!html.contains("importante."));
    }

    #[test]
    fn block_projection_still_uses_phrase_hints_when_fragment_pairing_fails() {
        // Fragment source text does not exist in the DOM block, so the
        // fragment path bails; the block-level path must still exact-place
        // emphasis using the fragment's translated phrase hint.
        let mut fragment = fragment("Texto que no aparece.", "The blue door opens.");
        fragment.inline_phrases.push(
            crate::translation::storage::PersistTranslationInlinePhrase {
                source_text: "puerta azul".into(),
                text: "Blue door.".into(),
            },
        );
        let request = request(
            "<!doctype html><html><body><article><p>La <strong>puerta azul</strong> abre bien.</p></article></body></html>",
            vec![section_with_fragments(
                0,
                false,
                "The blue door opens well.",
                vec![fragment],
            )],
        );

        let html = render_translated_html("Translated", &request);

        assert!(html.contains("<strong>blue door</strong>"));
        assert!(!html.contains("puerta azul"));
    }

    #[test]
    fn places_reordered_inline_phrase_using_translated_hint() {
        let mut translated_fragment = fragment(
            "Para avanzar se necesita el método práctico.",
            "The practical method is needed to advance.",
        );
        translated_fragment.inline_phrases.push(
            crate::translation::storage::PersistTranslationInlinePhrase {
                source_text: "método práctico".into(),
                text: "Practical method.".into(),
            },
        );
        let request = request(
            "<!doctype html><html><body><article><p>Para avanzar se necesita el <strong>método práctico</strong>.</p></article></body></html>",
            vec![section_with_fragments(
                0,
                false,
                "The practical method is needed to advance.",
                vec![translated_fragment],
            )],
        );

        let html = render_translated_html("Translated", &request);

        assert!(html.contains("The <strong>practical method</strong> is needed to advance."));
        assert!(!html.contains("método práctico"));
    }

    #[test]
    fn preserves_nested_internal_link_markup_and_target() {
        let request = request(
            "<!doctype html><html><body><article><p>Voir les <a class=\"cross-reference\" href=\"#details\"><em>détails importants</em></a>.</p><p id=\"details\">Texte détaillé.</p></article></body></html>",
            vec![
                section(0, false, "See the important details."),
                section(1, false, "Detailed text."),
            ],
        );

        let html = render_translated_html("Translated", &request);
        let document = parse_html_document(html.clone());

        assert_eq!(html.matches("href=\"#details\"").count(), 1);
        assert!(html.contains("class=\"cross-reference\""));
        assert!(document
            .select_first("a.cross-reference[href=\"#details\"] em")
            .is_ok());
        assert!(html.contains("id=\"details\""));
        assert!(html.contains("Detailed text."));
    }

    #[test]
    fn preserves_ordered_footnote_items_and_backlinks() {
        let request = request(
            "<!doctype html><html><body><article><p>Texto<a href=\"#sdfootnote1sym\" id=\"sdfootnote1anc\" role=\"doc-noteref\"><sup>1</sup></a>.</p><section role=\"doc-endnotes\"><ol><li id=\"sdfootnote1sym\"><p>Nota uno<a href=\"#sdfootnote1anc\" role=\"doc-backlink\">↩︎</a></p></li><li id=\"sdfootnote2sym\"><p>Nota dos<a href=\"#sdfootnote2anc\" role=\"doc-backlink\">↩︎</a></p></li></ol></section></article></body></html>",
            vec![
                section(0, false, "Body text."),
                section(1, false, "First note."),
                section(2, false, "Second note."),
            ],
        );

        let html = render_translated_html("Translated", &request);

        assert!(html.contains("<ol>"));
        assert!(html.contains("<li id=\"sdfootnote1sym\""));
        assert!(html.contains("First note."));
        assert!(html.contains("Second note."));
        assert!(html.contains("href=\"#sdfootnote1anc\""));
        assert!(html.contains("role=\"doc-backlink\""));
        assert!(!html.contains("Nota uno"));
        assert!(!html.contains("Nota dos"));
    }

    #[test]
    fn renders_footnote_marker_without_translating_marker_label_as_text() {
        let request = request(
            "<!doctype html><html><body><article><p>Tópico<a href=\"#fn1\" id=\"ref1\" role=\"doc-noteref\"><sup>1</sup></a> cambia.</p><p id=\"fn1\">Note body<a href=\"#ref1\" role=\"doc-backlink\">↩︎</a></p></article></body></html>",
            vec![
                section_with_fragments(
                    0,
                    false,
                    "The topic changes.",
                    vec![fragment("Tópico cambia.", "The topic changes.")],
                ),
                section(1, false, "Translated note body."),
            ],
        );

        let html = render_translated_html("Translated", &request);

        assert!(html.contains(
            "The topic<a href=\"#fn1\" id=\"ref1\" role=\"doc-noteref\"><sup>1</sup></a> changes."
        ));
        assert!(html.contains("changes."));
        assert!(html.contains("href=\"#fn1\""));
        assert!(html.contains("id=\"ref1\""));
        assert!(html.contains("role=\"doc-noteref\""));
        assert!(html.contains("<sup>1</sup>"));
        assert!(html.contains("Translated note body."));
        assert!(html.contains("role=\"doc-backlink\""));
        assert!(!html.contains("topic 1"));
    }

    #[test]
    fn preserves_image_and_table_blocks_by_inserting_translation_nearby() {
        let request = request(
            "<!doctype html><html><body><article><p><img src=\"asset://cover.png\" alt=\"Cover\"> Couverture</p><blockquote><table><tr><td>Nom</td></tr></table></blockquote></article></body></html>",
            vec![
                section(0, false, "Cover image."),
                section(1, false, "Name table."),
            ],
        );

        let html = render_translated_html("Translated", &request);

        assert!(html.contains("src=\"asset://cover.png\""));
        assert!(html.contains("<table>"));
        assert!(html.contains("Cover image."));
        assert!(html.contains("Name table."));
        assert!(html.contains("papercut-translation-inline"));
    }

    #[test]
    fn preserves_links_inside_table_blocks_and_their_targets() {
        let request = request(
            "<!doctype html><html><body><article><blockquote><table><tr><td><a href=\"#table-note\">Nom</a></td></tr></table></blockquote><p id=\"table-note\">Note du tableau.</p></article></body></html>",
            vec![
                section(0, false, "Name table."),
                section(1, false, "Table note."),
            ],
        );

        let html = render_translated_html("Translated", &request);

        assert_eq!(html.matches("href=\"#table-note\"").count(), 1);
        assert!(html.contains("<table>"));
        assert!(html.contains("id=\"table-note\""));
        assert!(html.contains("Name table."));
        assert!(html.contains("Table note."));
    }

    #[test]
    fn preserves_rtl_source_links_while_rendering_ltr_translation() {
        let request = request(
            "<!doctype html><html><body><article dir=\"rtl\"><p>انظر <a href=\"#fn1\">١</a></p><p id=\"fn1\">حاشية</p></article></body></html>",
            vec![
                section(0, false, "See note 1."),
                section(1, false, "Footnote body."),
            ],
        );

        let html = render_translated_html("Translated", &request);

        assert!(html.contains("dir=\"rtl\""));
        assert!(html.contains("href=\"#fn1\""));
        assert!(html.contains("id=\"fn1\""));
        assert!(html.contains("See note 1."));
        assert!(html.contains("Footnote body."));
    }

    #[test]
    fn falls_back_when_dom_cannot_map_every_translated_section() {
        let request = request(
            "<!doctype html><html><body><article><h1>Chapitre</h1></article></body></html>",
            vec![section(0, true, "Chapter"), section(1, false, "Hello")],
        );

        let html = render_translated_html("Translated", &request);

        assert!(html.contains("data-papercut-translation=\"true\""));
        assert!(html.contains("Chapter"));
        assert!(html.contains("Hello"));
        assert!(html.contains("translation-section-2"));
    }

    fn request(
        view_html: &str,
        translated_sections: Vec<PersistTranslationSection>,
    ) -> PersistTranslationRequest {
        PersistTranslationRequest {
            source: TranslationSourceDocument {
                document_id: "source1".into(),
                document_url: "/uploads/source1.html".into(),
                title: "Source".into(),
                format: "html".into(),
                view_html: view_html.into(),
                blocks: vec![
                    TranslationSourceBlock {
                        ordinal: 0,
                        heading: Some("Chapitre".into()),
                        text: "Chapitre".into(),
                    },
                    TranslationSourceBlock {
                        ordinal: 1,
                        heading: Some("Chapitre".into()),
                        text: "Bonjour".into(),
                    },
                ],
            },
            source_language: "fr".into(),
            target_language: "en".into(),
            model_id: "opus-mt-fr-en-ctranslate2".into(),
            quality_mode: "balanced".into(),
            repair_mode: Default::default(),
            job_id: "job1".into(),
            glossary: Vec::new(),
            translated_sections,
        }
    }

    fn section(source_ordinal: usize, is_heading: bool, text: &str) -> PersistTranslationSection {
        section_with_fragments(source_ordinal, is_heading, text, Vec::new())
    }

    fn section_with_fragments(
        source_ordinal: usize,
        is_heading: bool,
        text: &str,
        fragments: Vec<PersistTranslationFragment>,
    ) -> PersistTranslationSection {
        PersistTranslationSection {
            heading: Some("Chapter".into()),
            source_heading: Some("Chapitre".into()),
            source_ordinal,
            is_heading,
            text: text.into(),
            fragments,
        }
    }

    fn fragment(source_text: &str, text: &str) -> PersistTranslationFragment {
        PersistTranslationFragment {
            source_start: 0,
            source_end: 0,
            source_text: source_text.into(),
            text: text.into(),
            inline_phrases: Vec::new(),
        }
    }
}
