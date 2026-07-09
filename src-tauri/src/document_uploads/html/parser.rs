//! HTML structure extraction: title + ordered readable sections.
//!
//! Sanitizes first, then walks the parsed DOM to build searchable sections,
//! attaching each block to the most recent heading.

use kuchikiki::{parse_html, traits::TendrilSink, NodeRef};

use super::{decode_entities, normalize_text, sanitize_html, strip_tags};
use crate::document_uploads::parsed::{ParsedDocument, ParsedSection};

/// Parse raw HTML into a sanitized document: title, sanitized source, and the
/// ordered sections fed to the FTS index. Each block inherits the current heading.
pub(crate) fn parse_html_document(html: &str) -> ParsedDocument {
    let sanitized = sanitize_html(html);
    let title = extract_title(&sanitized).unwrap_or_else(|| "Imported HTML Document".into());
    parsed_html_document(title, "html", sanitized)
}

/// Convert already-sanitized HTML into the shared parsed document shape.
pub(crate) fn parsed_html_document(
    title: String,
    format: impl Into<String>,
    sanitized_html: String,
) -> ParsedDocument {
    let blocks = extract_text_blocks(&sanitized_html);
    let mut sections = Vec::new();
    let mut current_heading: Option<String> = None;

    for block in blocks {
        if block.is_heading {
            current_heading = Some(block.text.clone());
            sections.push(ParsedSection {
                heading: current_heading.clone(),
                text: block.text,
            });
        } else if !block.text.is_empty() {
            sections.push(ParsedSection {
                heading: current_heading.clone(),
                text: block.text,
            });
        }
    }

    ParsedDocument {
        title,
        format: format.into(),
        view_html: sanitized_html,
        sections,
    }
}

/// One extracted block of body text plus whether it came from a heading tag.
struct TextBlock {
    is_heading: bool,
    text: String,
}

/// Walk the sanitized DOM for readable blocks in document order.
///
/// EPUBs commonly use paragraph-like `<div>` nodes instead of `<p>`. A DOM walk
/// lets us index those leaf blocks while skipping wrapper containers that would
/// otherwise duplicate an entire chapter or body.
fn extract_text_blocks(html: &str) -> Vec<TextBlock> {
    let document = parse_html().one(html).document_node;
    let root = document_body(&document);
    let mut blocks = Vec::new();

    for node in root.inclusive_descendants() {
        let Some(tag_name) = node_tag_name(&node) else {
            continue;
        };
        if !is_readable_block(&tag_name) {
            continue;
        }
        if is_fallback_container(&tag_name) && has_readable_block_descendant(&node) {
            continue;
        }

        let text = normalize_text(&strip_tags(&serialize_node(&node)));
        if !text.is_empty() {
            blocks.push(TextBlock {
                is_heading: is_heading(&tag_name),
                text,
            });
        }
    }

    if blocks.is_empty() {
        let text = normalize_text(&strip_tags(&serialize_node(&root)));
        if !text.is_empty() {
            blocks.push(TextBlock {
                is_heading: false,
                text,
            });
        }
    }

    blocks
}

/// Return the parsed body node, falling back to the document for fragments.
///
/// `kuchikiki` normally creates a body even for partial HTML, but keeping the
/// fallback makes this safe for unusual parser output without reviving the old
/// string-based body scanner.
fn document_body(document: &NodeRef) -> NodeRef {
    document
        .select_first("body")
        .ok()
        .map(|body| body.as_node().clone())
        .unwrap_or_else(|| document.clone())
}

fn node_tag_name(node: &NodeRef) -> Option<String> {
    node.as_element()
        .map(|element| element.name.local.to_string().to_ascii_lowercase())
}

fn is_readable_block(tag_name: &str) -> bool {
    is_heading(tag_name)
        || matches!(
            tag_name,
            "p" | "li" | "blockquote" | "div" | "section" | "article" | "main"
        )
}

fn is_heading(tag_name: &str) -> bool {
    matches!(tag_name, "h1" | "h2" | "h3" | "h4" | "h5" | "h6")
}

fn is_fallback_container(tag_name: &str) -> bool {
    matches!(
        tag_name,
        "div" | "section" | "article" | "main" | "li" | "blockquote"
    )
}

/// Prevent wrapper containers from duplicating their child sections.
///
/// EPUB generators often emit `<section><div class="cct">paragraph</div>...`;
/// HTML authors may also nest paragraphs inside list items or blockquotes. The
/// leaf block is the searchable unit, while the surrounding container should not
/// become a second giant search hit.
fn has_readable_block_descendant(node: &NodeRef) -> bool {
    node.descendants()
        .filter_map(|descendant| node_tag_name(&descendant))
        .any(|tag_name| is_readable_block(&tag_name))
}

/// Serialize one chosen DOM block before flattening it with `strip_tags`.
///
/// `NodeRef::text_contents()` would drop semantic separators such as `<br>`.
/// Reusing the sanitizer's tag stripper keeps line breaks/block boundaries and
/// inline text behavior aligned between indexing and import validation.
fn serialize_node(node: &NodeRef) -> String {
    let mut bytes = Vec::new();
    if node.serialize(&mut bytes).is_err() {
        return String::new();
    }
    String::from_utf8(bytes).unwrap_or_default()
}

/// Extract and clean the document `<title>`, returning `None` when absent/empty.
fn extract_title(html: &str) -> Option<String> {
    extract_between_case_insensitive(html, "<title", "</title>")
        .and_then(|content| content.find('>').map(|idx| content[idx + 1..].to_string()))
        .map(|title| normalize_text(&decode_entities(&strip_tags(&title))))
        .filter(|title| !title.is_empty())
}

/// Return the slice between the first case-insensitive `open` and `close` markers,
/// indexing back into the original (case-preserving) string.
fn extract_between_case_insensitive<'a>(html: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let lower = html.to_ascii_lowercase();
    let start = lower.find(open)?;
    let end = lower[start..].find(close)? + start;
    Some(&html[start..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section_texts(parsed: &ParsedDocument) -> Vec<String> {
        parsed
            .sections
            .iter()
            .map(|section| section.text.clone())
            .collect()
    }

    #[test]
    fn parses_title_and_body_when_unicode_precedes_tags() {
        let html = "<html><head>İ<TITLE>Expected Title</TITLE></head><body>İ<P>Readable text</P></body></html>";
        let parsed = parse_html_document(html);

        assert_eq!(parsed.title, "Expected Title");
        assert!(parsed
            .sections
            .iter()
            .any(|section| section.text == "Readable text"));
    }

    #[test]
    fn indexes_paragraph_like_leaf_divs() {
        let html = r#"
            <html><body>
                <section>
                    <div class="cct">They walked around the splashing fountain in the center.</div>
                </section>
            </body></html>
        "#;
        let parsed = parse_html_document(html);

        assert!(parsed.sections.iter().any(|section| section
            .text
            .contains("around the splashing fountain in the center")));
    }

    #[test]
    fn skips_wrapper_divs_that_contain_readable_blocks() {
        let html =
            "<body><div><div>First paragraph.</div><div>Second paragraph.</div></div></body>";
        let parsed = parse_html_document(html);

        assert_eq!(
            section_texts(&parsed),
            vec![
                "First paragraph.".to_string(),
                "Second paragraph.".to_string()
            ]
        );
    }

    #[test]
    fn keeps_heading_context_for_leaf_div_sections() {
        let html = "<body><h2>Bathhouse</h2><div class=\"cct\">The fountain was loud.</div></body>";
        let parsed = parse_html_document(html);

        let section = parsed
            .sections
            .iter()
            .find(|section| section.text == "The fountain was loud.")
            .expect("leaf div section should be indexed");
        assert_eq!(section.heading.as_deref(), Some("Bathhouse"));
    }

    #[test]
    fn keeps_inline_split_words_searchable_in_extracted_blocks() {
        let html = "<body><div><b>C</b><b>ORNELIA</b> had always loved Saturnalia.</div></body>";
        let parsed = parse_html_document(html);

        assert!(parsed
            .sections
            .iter()
            .any(|section| section.text == "CORNELIA had always loved Saturnalia."));
    }
}
