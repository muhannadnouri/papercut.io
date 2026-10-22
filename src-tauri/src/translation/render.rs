//! Rendering translated documents.
//!
//! The preferred path clones the sanitized reader HTML and replaces readable
//! block text in document order. That preserves links, images, ids, and EPUB
//! asset rewrites around the text. Footnote/link anchors are preserved in
//! translated blocks; media/table-heavy blocks keep the original block intact
//! and insert translated text nearby instead of destroying assets.

use kuchikiki::NodeRef;

use super::dom::DomPlan;
use super::formatting::FormattingPolicy;
use super::html::parse_html_document;
use super::inline_markup::source_inline_model;
use super::storage::{PersistTranslationRequest, PersistTranslationSection};

mod legacy;

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
    // Old internal strict requests cannot silently downgrade to best-effort rendering.
    if request.formatting == FormattingPolicy::Strict {
        return None;
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
    if request.formatting == FormattingPolicy::SentenceReview
        && (!FormattingPolicy::supports_sentence_review(&request.model_id)
            || request.translated_sections.iter().any(|s| {
                s.text.trim().is_empty()
                    || s.fragments.is_empty()
                    || s.fragments.iter().any(|f| {
                        f.needs_review == f.markup.is_some() || !f.inline_phrases.is_empty()
                    })
            }))
    {
        return None;
    }
    if request.formatting != FormattingPolicy::SentenceReview
        && request
            .translated_sections
            .iter()
            .flat_map(|s| &s.fragments)
            .any(|f| f.needs_review)
    {
        return None;
    }
    let document = plan.document;
    update_title(&document, title);
    annotate_article(&document, request);

    let mut replaced_any = false;
    for (block, section) in plan.blocks.into_iter().zip(&request.translated_sections) {
        let node = block.node;
        annotate_block(&node, section);
        if section.text.trim().is_empty() {
            continue;
        }
        if section.fragments.iter().any(|f| f.needs_review) {
            retain_original_for_review(&node, section, request).ok()?;
        } else if section.fragments.iter().any(|f| f.markup.is_some()) {
            replace_protected_fragments(
                &node,
                section,
                request.formatting == FormattingPolicy::SentenceReview,
            )
            .ok()?;
        } else if request.formatting == FormattingPolicy::Provider {
            legacy::replace_block(&node, section);
        } else {
            // Missing validated sentence output never authorizes best-effort formatting.
            return None;
        }
        replaced_any = true;
    }

    if !replaced_any {
        return None;
    }
    serialize_document(&document)
}

/// Keep uncertain formatting on the source once; labels/source never become narration or search text.
fn retain_original_for_review(
    node: &NodeRef,
    section: &PersistTranslationSection,
    request: &PersistTranslationRequest,
) -> Result<(), String> {
    let [fragment] = section.fragments.as_slice() else {
        return Err("Formatting review requires one complete source block".into());
    };
    let source = source_inline_model(node).text;
    if !fragment.needs_review
        || fragment.markup.is_some()
        || !fragment.inline_phrases.is_empty()
        || fragment.source_start != 0
        || fragment.source_end != source.chars().count()
        || fragment.source_text != source
        || fragment.text != section.text
        || fragment.text.trim().is_empty()
    {
        return Err("Formatting review does not match the source/result block".into());
    }
    let element = node.as_element().ok_or("Missing formatting review block")?;
    let container = matches!(element.name.local.as_ref(), "body" | "li");
    let tag = if container {
        "p"
    } else {
        element.name.local.as_ref()
    };
    let additions = parse_html_document(format!(
        "<html><body><{tag} data-papercut-translation-review='label'><strong>Translation — formatting needs review</strong></{tag}><{tag}>{}</{tag}></body></html>",
        escape_html(&section.text)
    ));
    let additions = additions
        .select_first("body")
        .map_err(|_| "Invalid formatting review output")?;
    let original = if container {
        // Keep body ownership and list numbering: source/translation belong inside the same container.
        let wrapper = parse_html_document("<html><body><div></div></body></html>".to_string());
        let wrapper = wrapper
            .select_first("div")
            .map_err(|_| "Invalid original wrapper")?
            .as_node()
            .clone();
        for child in node.children().collect::<Vec<_>>() {
            wrapper.append(child);
        }
        node.append(wrapper.clone());
        wrapper
    } else {
        node.clone()
    };
    let original_element = original.as_element().ok_or("Missing original element")?;
    original_element
        .attributes
        .borrow_mut()
        .insert("data-papercut-translation-review", "original".into());
    original_element
        .attributes
        .borrow_mut()
        .insert("lang", request.source_language.clone());
    let label = parse_html_document("<html><body><span data-papercut-translation-review='label' lang='en'><strong>Original — formatting retained</strong><br></span></body></html>".to_string());
    original.prepend(
        label
            .select_first("span")
            .map_err(|_| "Invalid original label")?
            .as_node()
            .clone(),
    );
    let mut previous = original;
    for addition in additions.as_node().children().collect::<Vec<_>>() {
        if let Some(element) = addition.as_element() {
            let mut attributes = element.attributes.borrow_mut();
            attributes.insert("lang", request.target_language.clone());
            // Note bodies keep their search/narration role; anchors remain on the retained source only.
            if attributes.get("data-papercut-translation-review").is_none() {
                for attribute in ["role", "epub:type"] {
                    if let Some(value) = node.as_element().and_then(|source| {
                        source.attributes.borrow().get(attribute).map(str::to_owned)
                    }) {
                        attributes.insert(attribute, value);
                    }
                }
            }
        }
        previous.insert_after(addition.clone());
        previous = addition;
    }
    Ok(())
}

/// Rebind every rich fragment to the cloned source DOM; malformed rich output never falls back.
/// Sentence fragments retain their source parsing context and bypass worker note movement.
fn replace_protected_fragments(
    node: &NodeRef,
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
    for fragment in &section.fragments {
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
        let markup = fragment
            .markup
            .as_deref()
            .ok_or("Missing protected fragment")?;
        let restored = if boundary {
            plan.restore_boundaries(markup)?
        } else {
            plan.restore(markup)?
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
    let document = if boundary {
        use kuchikiki::traits::TendrilSink;
        let context = node
            .as_element()
            .ok_or("Missing sentence block context")?
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
        .select_first(if boundary { "html" } else { "body > div" })
        .map_err(|_| "Invalid protected render")?;
    for child in node.children().collect::<Vec<_>>() {
        child.detach();
    }
    for child in wrapper.as_node().children().collect::<Vec<_>>() {
        node.append(child);
    }
    if boundary && source_inline_model(node).text != section.text {
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

/// Insert literal text for titles and the legacy fallback; never parse translated prose as HTML.
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
    use super::{render_translated_html, FormattingPolicy};
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

    /// Review output keeps source ownership and never interprets decoder text as markup.
    #[test]
    fn sentence_review_retains_original_hierarchy_assets_notes_once_and_escapes_translation() {
        let html = "<article><p id='r'>Je <strong><em>ne</em></strong> sais <a href='#n'>pas</a><a role='doc-noteref' href='#n' id='ref'>1</a><img data-papercut-asset='image.png'>.</p><p id='n' role='doc-footnote'>Note<a role='doc-backlink' href='#r'>Back</a>.</p></article>";
        let req = review_request(html, &["I do not know <script>alert(1)</script>.", "Note."]);
        let output = render_translated_html("English", &req).unwrap();
        let dom = parse_html_document(&output);
        for id in ["r", "n", "ref"] {
            assert_eq!(dom.select(&format!("#{id}")).unwrap().count(), 1);
        }
        assert_eq!(
            dom.select("#r > strong > em")
                .unwrap()
                .next()
                .unwrap()
                .text_contents(),
            "ne"
        );
        assert_eq!(dom.select("#r > a[href='#n']").unwrap().count(), 2);
        assert_eq!(dom.select("#n > a[href='#r']").unwrap().count(), 1);
        assert_eq!(
            dom.select("p[role='doc-footnote']:not([data-papercut-translation-review])")
                .unwrap()
                .next()
                .unwrap()
                .text_contents(),
            "Note."
        );
        assert_eq!(
            dom.select("img[data-papercut-asset='image.png']")
                .unwrap()
                .count(),
            1
        );
        assert_eq!(
            dom.select("[data-papercut-translation-review='original']")
                .unwrap()
                .count(),
            2
        );
        assert_eq!(dom.select("script").unwrap().count(), 0);
        assert!(output.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(output.contains("Translation — formatting needs review"));
        crate::translation::quality::validate_translated_output(
            &output,
            &req.source.blocks,
            &req.translated_sections,
            &[],
            "en",
        )
        .unwrap();
    }

    #[test]
    fn sentence_review_rejects_mixed_or_unbound_fragment_claims() {
        for invalid in 0..8 {
            let mut req =
                review_request("<p>Una <strong>casa</strong> roja.</p>", &["A red house."]);
            match invalid {
                0 => req.formatting = FormattingPolicy::Provider,
                1 => req.model_id = "bergamot-es-en-worker".into(),
                2 => req.translated_sections[0].fragments[0].markup = Some("A red house.".into()),
                3 => req.translated_sections[0].fragments[0].source_end -= 1,
                4 => req.translated_sections[0].fragments[0].source_text = "Other source".into(),
                5 => req.translated_sections[0].fragments[0].text = "Other target".into(),
                6 => req.translated_sections[0].fragments[0].needs_review = false,
                _ => {
                    let f = req.translated_sections[0].fragments[0].clone();
                    req.translated_sections[0].fragments.push(f);
                }
            }
            assert!(
                render_translated_html("English", &req).is_err(),
                "invalid {invalid}"
            );
        }
    }

    #[test]
    fn sentence_review_preserves_body_only_source_without_nesting_translated_text_inside_it() {
        let req = review_request(
            "<html><body>Una <strong>casa</strong> roja.</body></html>",
            &["A red house."],
        );
        let output = render_translated_html("English", &req).unwrap();
        let dom = parse_html_document(&output);
        let original = dom
            .select_first("body > div[data-papercut-translation-review='original']")
            .unwrap();
        assert!(original.text_contents().contains("Una casa roja."));
        assert!(!original.text_contents().contains("A red house."));
        assert_eq!(
            dom.select_first("body > p:not([data-papercut-translation-review])")
                .unwrap()
                .text_contents(),
            "A red house."
        );
    }

    #[test]
    fn sentence_review_retains_ordered_list_items_without_extra_numbered_labels() {
        let req = review_request("<ol><li id='first'>Una <strong>casa</strong> roja.</li><li id='second'>Otra <em>casa</em>.</li></ol>", &["A red house.", "Another house."]);
        let output = render_translated_html("English", &req).unwrap();
        let dom = parse_html_document(&output);
        assert_eq!(dom.select("ol > li").unwrap().count(), 2);
        assert_eq!(
            dom.select("#first > div[data-papercut-translation-review='original'] > strong")
                .unwrap()
                .next()
                .unwrap()
                .text_contents(),
            "casa"
        );
        assert_eq!(
            dom.select("#first > p:not([data-papercut-translation-review])")
                .unwrap()
                .next()
                .unwrap()
                .text_contents(),
            "A red house."
        );
        let reparsed = super::DomPlan::new(&crate::document_uploads::sanitize_html(&output));
        assert_eq!(
            reparsed
                .blocks
                .iter()
                .map(|b| b.inline.text.as_str())
                .collect::<Vec<_>>(),
            ["A red house.", "Another house."]
        );
    }

    #[test]
    fn sentence_accepted_uniform_mapping_keeps_style_without_review_original() {
        use crate::translation::inline_markup::protected::ProtectedSource;
        let source = "<p><strong><em>Bonjour.</em></strong></p>";
        let mut req = review_request(source, &["Hello."]);
        let dom = super::DomPlan::new(source);
        let block = &dom.blocks[0];
        let plan = ProtectedSource::for_boundaries(&block.node)
            .unwrap()
            .fragment(0..block.inline.text.chars().count(), &block.inline.text)
            .unwrap();
        let f = &mut req.translated_sections[0].fragments[0];
        f.needs_review = false;
        f.markup = Some(plan.html.replace("Bonjour.", "Hello."));
        let output = render_translated_html("English", &req).unwrap();
        assert!(output.contains("<strong><em>Hello.</em></strong>"));
        assert!(!output.contains("data-papercut-translation-review"));
        assert!(!output.contains("Bonjour"));
    }

    #[test]
    fn retired_strict_policy_cannot_render_as_legacy_text() {
        let mut req = request("<p>Bonjour.</p>", vec![section(0, false, "Hello.")]);
        assert!(render_translated_html("Example", &req).is_ok());
        req.formatting = FormattingPolicy::Strict;
        assert!(render_translated_html("Example", &req).is_err());
    }

    /// The same text can render through legacy projection, but rejected rich claims must never use it.
    #[test]
    fn invalid_sentence_or_protected_output_cannot_fall_through_to_legacy_rendering() {
        let source = "<p><strong>Bonjour.</strong></p>";
        let mut f = fragment("Bonjour.", "Hello.");
        f.source_end = f.source_text.chars().count();
        let plain = || {
            request(
                source,
                vec![section_with_fragments(0, false, "Hello.", vec![f.clone()])],
            )
        };
        assert!(render_translated_html("English", &plain())
            .unwrap()
            .contains("<strong>Hello.</strong>"));

        for policy in [FormattingPolicy::Provider, FormattingPolicy::SentenceReview] {
            let rich = || {
                let mut request = plain();
                request.formatting = policy;
                request.translated_sections[0].fragments[0].markup =
                    Some("<span data-pc='0'>Hello.</span>".into());
                request
            };
            assert!(render_translated_html("English", &rich())
                .unwrap()
                .contains("<strong>Hello.</strong>"));
            for invalid in ["syntax", "missing-code", "source", "target", "mixed"] {
                let mut broken = rich();
                let section = &mut broken.translated_sections[0];
                let fragment = &mut section.fragments[0];
                match invalid {
                    "syntax" => fragment.markup = Some("<span data-pc='0'>Hello.".into()),
                    "missing-code" => fragment.markup = Some("Hello.".into()),
                    "source" => fragment.source_text = "Changed.".into(),
                    "target" => fragment.text = "Changed.".into(),
                    "mixed" => {
                        let mut plain_fragment = fragment.clone();
                        plain_fragment.markup = None;
                        section.fragments.push(plain_fragment);
                    }
                    _ => unreachable!(),
                }
                assert!(
                    render_translated_html("English", &broken).is_err(),
                    "{policy:?}: {invalid}"
                );
            }
        }

        let mut sentence = plain();
        sentence.formatting = FormattingPolicy::SentenceReview;
        // Missing validated markup is not the explicit retained-original review decision.
        assert!(render_translated_html("English", &sentence).is_err());
        sentence.translated_sections[0].fragments.clear();
        assert!(render_translated_html("English", &sentence).is_err());

        let mut review = review_request(source, &["Hello."]);
        assert!(render_translated_html("English", &review).is_ok());
        review.translated_sections[0].fragments[0].source_end -= 1;
        assert!(render_translated_html("English", &review).is_err());
    }

    fn review_request(html: &str, targets: &[&str]) -> PersistTranslationRequest {
        let dom = super::DomPlan::new(html);
        assert_eq!(dom.blocks.len(), targets.len());
        let sections = dom
            .blocks
            .iter()
            .zip(targets)
            .map(|(block, target)| {
                let mut f = fragment(&block.inline.text, target);
                f.source_end = block.inline.text.chars().count();
                f.needs_review = true;
                section_with_fragments(block.ordinal, block.is_heading, target, vec![f])
            })
            .collect();
        let mut req = request(html, sections);
        req.formatting = FormattingPolicy::SentenceReview;
        req
    }

    fn request(
        view_html: &str,
        translated_sections: Vec<PersistTranslationSection>,
    ) -> PersistTranslationRequest {
        PersistTranslationRequest {
            formatting: FormattingPolicy::Provider,

            source: TranslationSourceDocument {
                document_id: "source1".into(),
                document_url: "/uploads/source1.html".into(),
                title: "Source".into(),
                format: "html".into(),
                view_html: view_html.into(),
                source_formatting: None,
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
            needs_review: false,

            markup: None,
            source_start: 0,
            source_end: 0,
            source_text: source_text.into(),
            text: text.into(),
            inline_phrases: Vec::new(),
        }
    }
}
