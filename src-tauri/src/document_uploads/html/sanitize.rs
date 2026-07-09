//! Conservative HTML sanitization and text normalization.
//!
//! A first-pass, dependency-free sanitizer: it strips active elements, drops
//! risky attributes, and provides the tag-stripping / entity-decoding / whitespace
//! helpers the parser reuses. Not a full standards-compliant sanitizer.

/// Strip active/risky elements and unsafe attributes, returning storable HTML.
pub(crate) fn sanitize_html(html: &str) -> String {
    let without_active = strip_element(html, "script");
    let without_active = strip_element(&without_active, "style");
    let without_active = strip_element(&without_active, "iframe");
    let without_active = strip_element(&without_active, "object");
    let without_active = strip_element(&without_active, "embed");
    sanitize_tag_attributes(&without_active)
}

/// Remove every `<tag>...</tag>` region (case-insensitive) for the named element;
/// drops to end-of-input if a closing tag is missing.
fn strip_element(html: &str, tag: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let lower = html.to_ascii_lowercase();
    let open_prefix = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut pos = 0usize;

    while let Some(start_rel) = lower[pos..].find(&open_prefix) {
        let start = pos + start_rel;
        out.push_str(&html[pos..start]);
        if let Some(close_rel) = lower[start..].find(&close) {
            pos = start + close_rel + close.len();
        } else {
            pos = html.len();
            break;
        }
    }
    out.push_str(&html[pos..]);
    out
}

/// Walk every tag and rewrite it through [`sanitize_single_tag`], passing through
/// the non-tag text between them unchanged.
fn sanitize_tag_attributes(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut pos = 0usize;

    while let Some(start_rel) = html[pos..].find('<') {
        let start = pos + start_rel;
        out.push_str(&html[pos..start]);
        let Some(end_rel) = html[start..].find('>') else {
            out.push_str(&html[start..]);
            return out;
        };
        let end = start + end_rel;
        let tag = &html[start + 1..end];
        out.push('<');
        out.push_str(&sanitize_single_tag(tag));
        out.push('>');
        pos = end + 1;
    }
    out.push_str(&html[pos..]);
    out
}

/// Sanitize one tag's inner text: keep closing/doctype/PI tags as-is, otherwise
/// drop `on*`, `style`, `src`, and `javascript:` href attributes.
fn sanitize_single_tag(tag: &str) -> String {
    let trimmed = tag.trim();
    if trimmed.starts_with('/') || trimmed.starts_with('!') || trimmed.starts_with('?') {
        return trimmed.to_string();
    }

    let self_closing = trimmed.ends_with('/');
    let inner = trimmed.trim_end_matches('/').trim();
    let mut parts = inner.split_whitespace();
    let Some(name) = parts.next() else {
        return String::new();
    };
    let mut safe = String::from(name);
    for attr in parts {
        let lower = attr.to_ascii_lowercase();
        if lower.starts_with("on") || lower.starts_with("style") || lower.starts_with("src=") {
            continue;
        }
        if lower.starts_with("href=") && lower.contains("javascript:") {
            continue;
        }
        safe.push(' ');
        safe.push_str(attr);
    }
    if self_closing {
        safe.push_str(" /");
    }
    safe
}

/// Strip all tags to plain text and decode entities.
///
/// Inline markup must not become a word boundary: many EPUBs style drop caps or
/// emphasis as adjacent inline tags, e.g. `<b>C</b><b>ORNELIA</b>`, and the FTS
/// index needs to see the same word a reader sees. Block-like tags still add a
/// separator so neighboring paragraphs do not collapse together.
pub(crate) fn strip_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut pos = 0usize;

    while let Some(start_rel) = html[pos..].find('<') {
        let start = pos + start_rel;
        out.push_str(&html[pos..start]);

        let Some(end_rel) = html[start..].find('>') else {
            pos = html.len();
            break;
        };
        let end = start + end_rel;
        if tag_separates_text(&html[start + 1..end]) {
            out.push(' ');
        }
        pos = end + 1;
    }

    out.push_str(&html[pos..]);
    decode_entities(&out)
}

/// Decide whether removing a tag should leave a word boundary in indexed text.
///
/// The list is intentionally a small block/row/line-break set. Inline tags like
/// `b`, `i`, `span`, and `a` are omitted so styled words remain searchable as
/// the continuous text users see in the reader.
fn tag_separates_text(tag: &str) -> bool {
    matches!(
        tag_name(tag).as_str(),
        "address"
            | "article"
            | "aside"
            | "blockquote"
            | "br"
            | "caption"
            | "dd"
            | "div"
            | "dl"
            | "dt"
            | "figcaption"
            | "figure"
            | "footer"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "header"
            | "hr"
            | "li"
            | "main"
            | "nav"
            | "ol"
            | "p"
            | "pre"
            | "section"
            | "td"
            | "th"
            | "tr"
            | "ul"
    )
}

/// Pull the element name out of a raw tag body from the lightweight scanner.
///
/// This intentionally handles only the shapes the sanitizer passes here, such as
/// `p class="x"`, `/p`, and `br/`. If we ever need full HTML tokenization, this
/// whole scanner should move to a DOM parser rather than grow custom parsing.
fn tag_name(tag: &str) -> String {
    tag.trim()
        .trim_start_matches('/')
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_end_matches('/')
        .to_ascii_lowercase()
}

/// Decode the small set of HTML entities that appear in extracted text.
pub(crate) fn decode_entities(text: &str) -> String {
    text.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

/// Collapse all runs of whitespace into single spaces and trim the result.
pub(crate) fn normalize_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_active_elements_when_unicode_precedes_tag() {
        let html = "<p>İ before script</p><SCRIPT>alert(1)</SCRIPT><p>Safe</p>";
        let sanitized = sanitize_html(html);

        assert!(sanitized.contains("İ before script"));
        assert!(sanitized.contains("Safe"));
        assert!(!sanitized.contains("alert(1)"));
        assert!(!sanitized.to_ascii_lowercase().contains("<script"));
    }

    #[test]
    fn strip_tags_keeps_adjacent_inline_text_together() {
        let text = normalize_text(&strip_tags(
            "<div><b>C</b><b>ORNELIA</b> had <i>always</i> loved Saturnalia.</div>",
        ));

        assert_eq!(text, "CORNELIA had always loved Saturnalia.");
    }

    #[test]
    fn strip_tags_separates_blocks_and_line_breaks() {
        let text = normalize_text(&strip_tags("<p>First</p><p>Second<br/>line</p>"));

        assert_eq!(text, "First Second line");
    }
}
