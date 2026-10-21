//! Rendering translated documents.
//!
//! The preferred path clones the sanitized reader HTML and replaces readable
//! block text in document order. That preserves links, images, ids, and EPUB
//! asset rewrites around the text. Footnote/link anchors are preserved in
//! translated blocks; media/table-heavy blocks keep the original block intact
//! and insert translated text nearby instead of destroying assets.

use crate::document_uploads::is_note_marker;
use kuchikiki::NodeRef;

use super::dom::DomPlan;
use super::html::parse_html_document;
use super::inline_markup::{
    byte_index_for_char, fold_phrase_char, fragment_char_range, is_inline_formatting_element,
    is_word_char, local_spans_for_fragment, projected_translated_spans_best_effort, serialize_node,
    source_inline_model, InlinePreservedMarker, ProjectedInlineSpan,
};
use super::storage::{
    PersistTranslationFragment, PersistTranslationInlinePhrase, PersistTranslationRequest,
    PersistTranslationSection,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProjectedInlineMarker {
    byte_offset: usize,
    html: String,
}

/// Render a persisted translated document.
///
/// Reject a stale/incomplete mapping instead of silently dropping document structure.
pub(crate) fn render_translated_html(
    title: &str,
    request: &PersistTranslationRequest,
) -> Result<String, String> {
    if request.source.view_html.trim().is_empty() {
        return Err("Translation source reader HTML is missing".into());
    }
    render_translated_dom(title, request)
        .ok_or_else(|| "Translation source/result DOM mapping does not match".into())
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
    let plan = DomPlan::new(&request.source.view_html);
    // Strict jobs are all-or-nothing; one missing receipt must never enter a legacy best-effort branch.
    let alignment = request.alignment.as_ref();
    if alignment.is_none()
        && request
            .translated_sections
            .iter()
            .flat_map(|s| &s.fragments)
            .any(|f| f.aligned.is_some())
    {
        return None;
    }
    if let Some(versions) = alignment {
        if request.translated_sections.iter().any(|s| {
            s.fragments.is_empty()
                || s.fragments.iter().any(|f| {
                    f.markup.is_some()
                        || !f.inline_phrases.is_empty()
                        || f.aligned
                            .as_ref()
                            .is_none_or(|m| &m.binding.versions != versions)
                })
        }) {
            return None;
        }
    }
    if plan.blocks.len() != request.source.blocks.len()
        || plan.blocks.len() != request.translated_sections.len()
    {
        return None;
    }
    for ((block, source), section) in plan
        .blocks
        .iter()
        .zip(&request.source.blocks)
        .zip(&request.translated_sections)
    {
        if block.ordinal != source.ordinal
            || source.ordinal != section.source_ordinal
            || block.inline.text != source.text
            || block.is_heading != source.is_heading
            || source.is_heading != section.is_heading
        {
            return None;
        }
    }
    if request.boundary
        && (!super::boundary::applies(&request.model_id)
            || alignment.is_some()
            || request.translated_sections.iter().any(|s| {
                s.text.trim().is_empty()
                    || s.fragments.is_empty()
                    || s.fragments.iter().any(|f| {
                        f.markup.is_none() || f.aligned.is_some() || !f.inline_phrases.is_empty()
                    })
            }))
    {
        return None;
    }
    let document = plan.document;
    update_title(&document, title);
    annotate_article(&document, request);

    let mut replaced_any = false;
    for (block_index, (block, section)) in plan
        .blocks
        .into_iter()
        .zip(&request.translated_sections)
        .enumerate()
    {
        let node = block.node;
        annotate_block(&node, section);
        if section.text.trim().is_empty() {
            continue;
        }
        if alignment.is_some() || section.fragments.iter().any(|f| f.markup.is_some()) {
            replace_protected_fragments(&node, block_index, section, request.boundary).ok()?;
        } else if requires_preserved_block(&node) {
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
            // Only unique phrase matches are styled; uncertain spans stay plain text.
        } else {
            replace_children_with_text(&node, section.text.trim());
        }
        replaced_any = true;
    }

    if !replaced_any {
        return None;
    }
    serialize_document(&document)
}

/// Rebind every rich fragment to the cloned source DOM; malformed rich output never falls back.
/// Strict receipts bypass worker note movement and retain their source block's HTML parsing context.
fn replace_protected_fragments(
    node: &NodeRef,
    block_index: usize,
    section: &PersistTranslationSection,
    boundary: bool,
) -> Result<(), String> {
    let source = source_inline_model(node).text.chars().collect::<Vec<_>>();
    let rich_source = if boundary {
        super::inline_markup::protected::ProtectedSource::for_boundaries(node)?
    } else {
        super::inline_markup::protected::ProtectedSource::new(node)?
    };
    let mut html = Vec::new();
    let mut texts = Vec::new();
    let mut cursor = 0;
    for (part_index, fragment) in section.fragments.iter().enumerate() {
        if source
            .get(cursor..fragment.source_start)
            .is_none_or(|gap| gap.iter().any(|c| !c.is_whitespace()))
        {
            return Err("Protected fragments overlap or omit source prose".into());
        }
        let plan = rich_source.fragment(
            fragment.source_start..fragment.source_end,
            &fragment.source_text,
        )?;
        let restored = if let Some(receipt) = &fragment.aligned {
            let segment = super::segment::TranslationTextSegment {
                id: format!("b{block_index}:s{part_index}"),
                source_block_index: block_index,
                source_start: fragment.source_start,
                source_end: fragment.source_end,
                text: fragment.source_text.clone(),
            };
            receipt.restore(&plan, &segment)?
        } else {
            let markup = fragment
                .markup
                .as_deref()
                .ok_or("Missing protected fragment")?;
            if boundary {
                plan.restore_boundaries(markup)?
            } else {
                plan.restore(markup)?
            }
        };
        if restored.text != fragment.text {
            return Err("Protected fragment text changed after validation".into());
        }
        html.push(restored.html);
        texts.push(restored.text);
        cursor = fragment.source_end;
    }
    if source
        .get(cursor..)
        .is_none_or(|gap| gap.iter().any(|c| !c.is_whitespace()))
        || texts.join(" ") != section.text
    {
        return Err("Protected fragments do not cover the source/result".into());
    }
    let strict = section.fragments.iter().any(|f| f.aligned.is_some()) || boundary;
    let document = if strict {
        use kuchikiki::traits::TendrilSink;
        let context = node
            .as_element()
            .ok_or("Missing aligned block context")?
            .name
            .clone();
        kuchikiki::parse_fragment(context, vec![])
            .one(html.join(" "))
            .document_node
    } else {
        parse_html_document(format!(
            "<html><body><div>{}</div></body></html>",
            html.join(" ")
        ))
    };
    let wrapper = document
        .select_first(if strict { "html" } else { "body > div" })
        .map_err(|_| "Invalid protected render")?;
    for child in node.children().collect::<Vec<_>>() {
        child.detach();
    }
    for child in wrapper.as_node().children().collect::<Vec<_>>() {
        node.append(child);
    }
    if strict && source_inline_model(node).text != section.text {
        return Err("Aligned block context changed readable text".into());
    }
    Ok(())
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

        let html = render_translated_html("Translated", &request).expect("mapped render");

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

        let html = render_translated_html("Translated", &request).expect("mapped render");

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

        let html = render_translated_html("Translated", &request).expect("mapped render");

        assert!(html.contains("<strong>Important.</strong>"));
        assert!(html.contains("<em><strong>Very urgent.</strong></em>"));
        assert!(!html.contains("Importante."));
    }

    #[test]
    fn real_segment_path_preserves_whole_block_nested_formatting() {
        let request = request(
            "<p><strong><em>Importante</em></strong></p>",
            vec![section_with_fragments(
                0,
                false,
                "Important",
                vec![fragment("Importante", "Important")],
            )],
        );
        let html = render_translated_html("English", &request).unwrap();
        assert!(html.contains("<strong><em>Important</em></strong>"));
    }

    #[test]
    fn omits_unaligned_partial_emphasis() {
        let request = request(
            "<!doctype html><html><body><article><p>Esto es <strong>importante</strong>.</p></article></body></html>",
            vec![section(0, false, "This is important.")],
        );

        let html = render_translated_html("Translated", &request).expect("mapped render");

        assert!(html.contains(">This is important.</p>"));
        assert!(!html.contains("<strong>"));
        assert!(!html.contains("importante"));
    }

    #[test]
    fn does_not_guess_multiple_cross_language_spans() {
        let request = request(
            "<!doctype html><html><body><article><p>Esto es <strong>importante</strong> y <em>urgente</em>.</p></article></body></html>",
            vec![section(0, false, "This is important and urgent.")],
        );

        let html = render_translated_html("Translated", &request).expect("mapped render");

        assert!(html.contains(">This is important and urgent.</p>"));
        assert!(!html.contains("<strong>"));
        assert!(!html.contains("<em>"));
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

        let html = render_translated_html("Translated", &request).expect("mapped render");

        assert!(html.contains("The blue door is important."));
        assert!(!html.contains("<em>blue door</em>"));
        assert!(!html.contains("<strong>important</strong>"));
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

        let html = render_translated_html("Translated", &request).expect("mapped render");

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

        let html = render_translated_html("Translated", &request).expect("mapped render");

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

        let html = render_translated_html("Translated", &request).expect("mapped render");
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

        let html = render_translated_html("Translated", &request).expect("mapped render");

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

        let html = render_translated_html("Translated", &request).expect("mapped render");

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

        let html = render_translated_html("Translated", &request).expect("mapped render");

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

        let html = render_translated_html("Translated", &request).expect("mapped render");

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

        let html = render_translated_html("Translated", &request).expect("mapped render");

        assert!(html.contains("dir=\"rtl\""));
        assert!(html.contains("href=\"#fn1\""));
        assert!(html.contains("id=\"fn1\""));
        assert!(html.contains("See note 1."));
        assert!(html.contains("Footnote body."));
    }

    #[test]
    fn rejects_incomplete_mapping_without_a_destructive_fallback() {
        let request = request(
            "<!doctype html><html><body><article><h1>Chapitre</h1></article></body></html>",
            vec![section(0, true, "Chapter"), section(1, false, "Hello")],
        );

        assert!(render_translated_html("Translated", &request).is_err());
    }

    #[test]
    fn rejects_same_length_stale_source_and_reordered_results() {
        let mut request = request(
            "<div>Bonjour</div><blockquote><p>Salut</p></blockquote>",
            vec![section(0, false, "Hello"), section(1, false, "Hi")],
        );
        request.translated_sections.swap(0, 1);
        assert!(render_translated_html("English", &request).is_err());
        request.translated_sections.swap(0, 1);
        request.source.view_html = "<div>Salut</div><blockquote><p>Bonjour</p></blockquote>".into();
        assert!(render_translated_html("English", &request).is_err());
    }

    #[test]
    fn real_french_and_spanish_reordering_never_guesses_wrong_emphasis() {
        for (source, translation) in [
            (
                "<p>La bague en <strong>or</strong> est perdue.</p>",
                "The gold ring is lost.",
            ),
            (
                "<p>La casa del <strong>rey</strong> es grande.</p>",
                "The king's house is big.",
            ),
        ] {
            let request = request(source, vec![section(0, false, translation)]);
            let html = render_translated_html("English", &request).unwrap();
            assert!(html.contains(translation));
            assert!(!html.contains("<strong>"));
        }
    }

    #[test]
    fn leaf_blocks_repeated_text_protected_content_and_untrusted_output_stay_separate() {
        let source = "<article><div id='start'>Même texte</div><ul><li><p>Même texte</p></li></ul><p translate=no>user_id</p><p>Utiliser <code translate=no>user_id</code> et <span id='answer'>42</span>.</p></article>";
        let request = request(
            source,
            vec![
                section(0, false, "First translation"),
                section(1, false, "Second translation"),
                section(3, false, "Use <script>alert(1)</script>"),
            ],
        );
        let html = render_translated_html("English", &request).unwrap();
        let dom = parse_html_document(html.clone());
        assert_eq!(
            dom.select_first("#start").unwrap().text_contents(),
            "First translation"
        );
        assert_eq!(
            dom.select_first("li p").unwrap().text_contents(),
            "Second translation"
        );
        assert_eq!(
            dom.select_first("code[translate=no]")
                .unwrap()
                .text_contents(),
            "user_id"
        );
        assert_eq!(dom.select_first("#answer").unwrap().text_contents(), "42");
        assert_eq!(dom.select("#answer").unwrap().count(), 1);
        assert_eq!(dom.select("p[translate=no]").unwrap().count(), 1);
        assert!(html.contains("&lt;script&gt;"));
        assert!(dom.select_first("script").is_err());
    }

    #[test]
    fn bare_table_fallback_keeps_assets_and_translation_inside_body() {
        let request = request(
            "<table><tr><td>Nom</td><td>Valeur</td></tr></table>",
            vec![section(0, false, "Name Value")],
        );
        let html = render_translated_html("English", &request).unwrap();
        let document = parse_html_document(&html);
        assert_eq!(
            document.select_first("body table").unwrap().text_contents(),
            "NomValeur"
        );
        assert_eq!(
            document
                .select_first("body > p.papercut-translation-inline")
                .unwrap()
                .text_contents(),
            "Name Value"
        );
    }

    #[test]
    fn validated_provider_spans_use_the_shared_text_renderer_without_trusting_html() {
        use crate::translation::inline_markup::provider::{
            validate_inline_mapping, InlineSpanMapping,
        };
        let source = crate::document_uploads::sanitize_html(
            r#"<p id="r">Une bague en <strong>or</strong><a role="doc-noteref" href='#n'>1</a>.</p><p id="n">Note<a role="doc-backlink" href='#r'>Back</a></p>"#,
        );
        let plan = super::DomPlan::new(&source);
        let block = &plan.blocks[0];
        let target = "A gold ring. <a href='#forged'>Fake</a><script>bad()</script>";
        let spans = validate_inline_mapping(
            &block.inline,
            &block.inline.text,
            target,
            &[InlineSpanMapping {
                source_span: 0,
                target: 2..6,
            }],
        )
        .unwrap();
        let markers = super::project_inline_markers(
            &block.inline.markers,
            0,
            block.inline.text.chars().count(),
            target,
        );
        super::replace_children_with_projected_formatting(&block.node, target, &spans, &markers);
        let saved = crate::document_uploads::sanitize_html(
            &super::serialize_document(&plan.document).unwrap(),
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

    #[test]
    fn sanitized_note_references_and_protected_text_survive_render_and_save() {
        let source = crate::document_uploads::sanitize_html(
            r#"<p translate="no">user_id</p>
            <p id="r"><strong>Le sujet<a role="doc-noteref" href='#n'><sup>1</sup></a> change.</strong></p>
            <p id="n" role="doc-endnote">Une note.<a role="doc-backlink" href='#r'>Back</a></p>"#,
        );
        let mut request = request(
            &source,
            vec![
                section_with_fragments(
                    1,
                    false,
                    "The topic changes.",
                    vec![fragment("Le sujet change.", "The topic changes.")],
                ),
                section_with_fragments(2, false, "A note.", vec![fragment("Une note.", "A note.")]),
            ],
        );
        for (source, section) in request
            .source
            .blocks
            .iter()
            .zip(&mut request.translated_sections)
        {
            section.fragments[0].source_end = source.text.chars().count();
        }
        assert_eq!(
            request
                .source
                .blocks
                .iter()
                .map(|block| block.text.as_str())
                .collect::<Vec<_>>(),
            ["Le sujet change.", "Une note."]
        );
        let rendered = render_translated_html("English", &request).unwrap();
        let saved = crate::document_uploads::sanitize_html(&rendered);
        let document = parse_html_document(&saved);
        assert_eq!(
            document
                .select_first("p[translate=no]")
                .unwrap()
                .text_contents(),
            "user_id"
        );
        assert_eq!(
            document
                .select("#r strong a[role=doc-noteref]")
                .unwrap()
                .count(),
            1
        );
        assert_eq!(
            document
                .select_first("a[role=doc-noteref]")
                .unwrap()
                .text_contents(),
            "1"
        );
        assert_eq!(document.select("a[role=doc-backlink]").unwrap().count(), 1);
        assert!(document
            .select_first("#r strong")
            .unwrap()
            .text_contents()
            .contains("topic"));
        let output_plan = super::DomPlan::new(&saved);
        assert_eq!(
            output_plan
                .blocks
                .iter()
                .map(|block| block.inline.text.as_str())
                .collect::<Vec<_>>(),
            ["The topic changes.", "A note."]
        );
    }

    #[test]
    fn mixed_whole_block_formatting_survives_real_one_and_multiple_fragment_paths() {
        for (source, fragments, target) in [
            (
                "<p><strong>Bonjour <em>monde</em> !</strong></p>",
                vec![fragment("Bonjour monde !", "Hello world!")],
                "Hello world!",
            ),
            (
                "<p><strong>Bonjour <em>Orion</em>. Salut <i>monde</i>.</strong></p>",
                vec![
                    fragment("Bonjour Orion.", "Hello Orion."),
                    fragment("Salut monde.", "Hi world."),
                ],
                "Hello Orion. Hi world.",
            ),
        ] {
            let source = crate::document_uploads::sanitize_html(source);
            let mut request = request(
                &source,
                vec![section_with_fragments(0, false, target, fragments)],
            );
            let limit = if request.translated_sections[0].fragments.len() == 1 {
                900
            } else {
                14
            };
            let planned = crate::translation::segment::segment_text_blocks(
                [request.source.blocks[0].text.as_str()],
                limit,
            )
            .unwrap();
            let fragments = &mut request.translated_sections[0].fragments;
            assert_eq!(planned.len(), fragments.len());
            for (planned, fragment) in planned.iter().zip(fragments) {
                assert_eq!(planned.text, fragment.source_text);
                fragment.source_start = planned.source_start;
                fragment.source_end = planned.source_end;
            }
            let saved = crate::document_uploads::sanitize_html(
                &render_translated_html("English", &request).unwrap(),
            );
            let document = parse_html_document(&saved);
            assert_eq!(document.select("strong").unwrap().count(), 1);
            assert_eq!(
                document.select_first("p > strong").unwrap().text_contents(),
                target
            );
            assert!(document.select_first("i").is_err());
            if target.contains("Orion") {
                assert_eq!(
                    document
                        .select_first("strong > em")
                        .unwrap()
                        .text_contents(),
                    "Orion"
                );
            } else {
                assert!(document.select_first("em").is_err());
            }
        }
    }

    #[test]
    fn rich_fragment_render_preserves_nested_styles_links_and_refuses_partial_or_stale_markup() {
        let source = "<p><strong>Bonjour. Merci.</strong> <a href='#n'>Note</a><a role='doc-noteref' href='#n'><sup>1</sup></a></p>";
        let text = super::DomPlan::new(source).blocks[0].inline.text.clone();
        let mut f = fragment(&text, "Hello. Thanks. Note");
        f.source_end = text.chars().count();
        f.markup = Some("<span data-pc='0'>Hello. Thanks.</span> <span data-pc='1'>Note</span><span data-pc='2' translate='no'></span>".into());
        let mut req = request(
            source,
            vec![section_with_fragments(0, false, &f.text, vec![f.clone()])],
        );
        let rendered = render_translated_html("English", &req).unwrap();
        assert!(rendered.contains("<strong>Hello. Thanks.</strong>"));
        assert!(rendered.contains("href=\"#n\""));
        for invalid in [None, Some("invented without codes".into())] {
            // A mixed rich/plain block must not silently fall back to approximate formatting.
            let mut broken = f.clone();
            broken.markup = invalid;
            req.translated_sections[0].fragments = vec![f.clone(), broken];
            assert!(render_translated_html("English", &req).is_err());
        }
        req.translated_sections[0].fragments = vec![f.clone()];
        req.translated_sections[0].fragments[0].text = "changed text".into();
        assert!(render_translated_html("English", &req).is_err());
        req.translated_sections[0].fragments = vec![f];
        req.translated_sections[0].fragments[0].source_start = 1;
        assert!(render_translated_html("English", &req).is_err());
    }

    #[test]
    fn rich_multiple_segments_keep_each_style_and_do_not_duplicate_boundary_note_ids() {
        let source = "<p><strong>Bonjour.<a role='doc-noteref' href='#n' id='middle'><sup>1</sup></a> Merci.</strong><a role='doc-noteref' href='#n' id='ref'><sup>2</sup></a></p>";
        let mut first = fragment("Bonjour.", "Hello.");
        first.source_end = 8;
        first.markup =
            Some("<span data-pc='0'>Hello.<span data-pc='1' translate='no'></span></span>".into());
        let mut second = fragment("Merci.", "Thanks.");
        second.source_start = 9;
        second.source_end = 15;
        second.markup =
            Some("<span data-pc='0'>Thanks.</span><span data-pc='2' translate='no'></span>".into());
        let req = request(
            source,
            vec![section_with_fragments(
                0,
                false,
                "Hello. Thanks.",
                vec![first, second],
            )],
        );
        let html = render_translated_html("English", &req).unwrap();
        let dom = parse_html_document(html);
        assert_eq!(dom.select("strong").unwrap().count(), 2);
        assert_eq!(dom.select("#ref").unwrap().count(), 1);
        assert_eq!(dom.select("#middle").unwrap().count(), 1);
        assert_eq!(dom.select_first("#ref").unwrap().text_contents(), "2");
    }

    /// Exercise the real scalar cut and renderer together, including non-ASCII source byte offsets.
    #[test]
    fn real_900_scalar_segments_keep_boundary_children_with_their_formatting_ancestors() {
        use crate::translation::inline_markup::protected::ProtectedSource;
        use crate::translation::segment::segment_text_blocks;
        let asset = crate::document_uploads::ParsedDocumentAsset::new(
            "image/png",
            b"owned boundary image".to_vec(),
        )
        .unwrap();
        for prefix in [
            format!("{}fin!", "Bonjour ".repeat(112)),
            format!("{}fin!!", "Été🙂 ".repeat(179)),
        ] {
            assert_eq!(prefix.chars().count(), 900);
            for (atom, selector) in [
                (
                    "<a role='doc-noteref' href='#n' id='ref'><sup>1</sup></a>".to_string(),
                    "a#ref",
                ),
                (
                    format!("<img data-papercut-asset='{}'>", asset.file_name),
                    "img",
                ),
                ("<br>".into(), "br"),
            ] {
                let safe = crate::document_uploads::sanitize_html(&format!(
                    "<p>{prefix}<strong><em>{atom}Suite.</em></strong></p>"
                ));
                let dom = super::DomPlan::new(&safe);
                let block = &dom.blocks[0];
                let extracted = ProtectedSource::new(&block.node).unwrap();
                let full = extracted
                    .fragment(0..block.inline.text.chars().count(), &block.inline.text)
                    .unwrap();
                assert_eq!(full.restore(&full.html).unwrap().text, block.inline.text);
                let segments = segment_text_blocks([&block.inline.text], 900).unwrap();
                assert_eq!(segments.len(), 2);
                assert_eq!(
                    (
                        segments[0].source_start,
                        segments[0].source_end,
                        segments[1].source_start
                    ),
                    (0, 900, if selector == "br" { 901 } else { 900 })
                );
                let mut fragments = Vec::new();
                for (index, segment) in segments.iter().enumerate() {
                    let plan = extracted
                        .fragment(segment.source_start..segment.source_end, &segment.text)
                        .unwrap();
                    assert!(!plan.html.contains("href"));
                    assert!(!plan.html.contains(&asset.file_name));
                    let roundtrip = plan.restore(&plan.html).unwrap();
                    assert_eq!(roundtrip.text, segment.text);
                    let restored_dom = parse_html_document(&roundtrip.html);
                    assert_eq!(
                        restored_dom
                            .select(&format!("strong > em > {selector}"))
                            .unwrap()
                            .count(),
                        usize::from(index == 0)
                    );
                    // Providers still cannot move the boundary child outside its retained parent.
                    if index == 0 {
                        assert!(plan
                            .restore(&plan.html.replace("data-pc=\"2\"", "data-pc=\"99\""))
                            .is_err());
                        assert!(plan
                            .restore(&plan.html.replace("<span data-pc=\"1\">", "").replacen(
                                "</span></span>",
                                "</span>",
                                1
                            ))
                            .is_err());
                    }
                    let target = plan
                        .html
                        .replace("Bonjour", "Hello")
                        .replace("Été🙂", "Summer🙂")
                        .replace("fin!", "end!")
                        .replace("Suite.", "Next.");
                    let restored = plan.restore(&target).unwrap();
                    let mut result = fragment(&segment.text, &restored.text);
                    result.source_start = segment.source_start;
                    result.source_end = segment.source_end;
                    result.markup = Some(target);
                    fragments.push(result);
                }
                let text = fragments
                    .iter()
                    .map(|f| f.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                let req = request(
                    &safe,
                    vec![section_with_fragments(0, false, &text, fragments)],
                );
                let html = render_translated_html("English", &req).unwrap();
                let output = parse_html_document(&html);
                assert_eq!(output.select(selector).unwrap().count(), 1);
                assert_eq!(
                    output
                        .select(&format!("strong > em > {selector}"))
                        .unwrap()
                        .count(),
                    1
                );
                assert_eq!(output.select("p > strong > em").unwrap().count(), 2);
                assert!(html.contains("Next."));
                assert!(!html.contains("data-pc"));
            }
        }
    }

    fn request(
        view_html: &str,
        translated_sections: Vec<PersistTranslationSection>,
    ) -> PersistTranslationRequest {
        PersistTranslationRequest {
            boundary: false,
            alignment: None,
            source: TranslationSourceDocument {
                document_id: "source1".into(),
                document_url: "/uploads/source1.html".into(),
                title: "Source".into(),
                format: "html".into(),
                view_html: view_html.into(),
                blocks: super::DomPlan::new(view_html)
                    .blocks
                    .into_iter()
                    .map(|block| TranslationSourceBlock {
                        ordinal: block.ordinal,
                        is_heading: block.is_heading,
                        heading: block.heading,
                        text: block.inline.text,
                    })
                    .collect(),
                assets: Default::default(),
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
            source_heading: Some("Chapitre".into()),
            source_ordinal,
            is_heading,
            text: text.into(),
            fragments,
        }
    }

    fn fragment(source_text: &str, text: &str) -> PersistTranslationFragment {
        PersistTranslationFragment {
            aligned: None,
            markup: None,
            source_start: 0,
            source_end: 0,
            source_text: source_text.into(),
            text: text.into(),
            inline_phrases: Vec::new(),
        }
    }
}
