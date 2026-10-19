//! Plain-text and Markdown adapters for the shared reader-document pipeline.

use encoding_rs::Encoding;
use pulldown_cmark::{html, Parser};

use super::html::{parsed_html_document, sanitize_html};
use super::parsed::ParsedDocument;

/// The user-facing source format retained in document metadata and transfers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TextDocumentFormat {
    PlainText,
    Markdown,
}

impl TextDocumentFormat {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::PlainText => "txt",
            Self::Markdown => "markdown",
        }
    }
}

/// Decode reflowable text without guessing an unmarked legacy code page.
/// UTF-8 covers normal cross-platform files; a BOM is required for UTF-16 so
/// Arabic and other non-Latin text cannot be silently decoded as the wrong script.
pub(crate) fn decode_text_bytes(bytes: &[u8]) -> Result<String, String> {
    if let Ok(text) = std::str::from_utf8(bytes) {
        return Ok(text.strip_prefix('\u{feff}').unwrap_or(text).to_owned());
    }
    let Some((encoding, bom_len)) = Encoding::for_bom(bytes) else {
        return Err("Text document must be UTF-8 or include a UTF-16 byte-order mark".into());
    };
    let (decoded, _, had_errors) = encoding.decode(&bytes[bom_len..]);
    if had_errors {
        return Err(format!(
            "Text document could not be decoded cleanly as {}",
            encoding.name()
        ));
    }
    Ok(decoded.into_owned())
}

/// Convert a decoded source into sanitized HTML plus the same searchable
/// sections used by HTML and EPUB. Plain text is escaped before markup is added;
/// Markdown output is sanitized because CommonMark permits embedded raw HTML.
/// That existing boundary also drops remote and sibling-file images; add an
/// explicit owned-asset contract before allowing Markdown to read local paths.
pub(crate) fn parse_text_document(
    text: &str,
    title: String,
    format: TextDocumentFormat,
) -> ParsedDocument {
    let body = match format {
        TextDocumentFormat::PlainText => plain_text_body(text),
        TextDocumentFormat::Markdown => {
            let mut output = String::with_capacity(text.len());
            html::push_html(&mut output, Parser::new(text));
            output
        }
    };
    let source = format!(
        "<!doctype html><html><head><title>{}</title></head><body dir=\"auto\">{body}</body></html>",
        escape_html_text(&title),
    );
    parsed_html_document(title, format.as_str(), sanitize_html(&source))
}

/// Preserve paragraph and explicit line-break structure while ensuring text
/// that merely looks like HTML remains literal reader content.
fn plain_text_body(text: &str) -> String {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut output = String::with_capacity(normalized.len());
    let mut paragraph: Vec<&str> = Vec::new();

    let push_paragraph = |output: &mut String, lines: &mut Vec<&str>| {
        if lines.is_empty() {
            return;
        }
        output.push_str("<p>");
        for (index, line) in lines.iter().enumerate() {
            if index > 0 {
                output.push_str("<br>");
            }
            output.push_str(&escape_html_text(line));
        }
        output.push_str("</p>");
        lines.clear();
    };

    for line in normalized.lines() {
        if line.trim().is_empty() {
            push_paragraph(&mut output, &mut paragraph);
        } else {
            paragraph.push(line);
        }
    }
    push_paragraph(&mut output, &mut paragraph);
    output
}

fn escape_html_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_keeps_markup_literal_and_paragraphs_searchable() {
        let parsed = parse_text_document(
            "<script>alert('no')</script>\ncontinued\n\nمرحبا بالعالم",
            "Notes & ideas".into(),
            TextDocumentFormat::PlainText,
        );

        assert_eq!(parsed.title, "Notes & ideas");
        assert_eq!(parsed.format, "txt");
        assert!(parsed.view_html.contains("&lt;script&gt;"));
        assert!(!parsed.view_html.contains("<script>"));
        assert_eq!(parsed.sections.len(), 2);
        assert!(parsed.sections[0].text.contains("continued"));
        assert_eq!(parsed.sections[1].text, "مرحبا بالعالم");
    }

    #[test]
    fn markdown_preserves_structure_but_sanitizes_raw_html() {
        let parsed = parse_text_document(
            "# Heading\n\nReadable **prose**.\n\n<script>alert(1)</script>\n\n```\ncode();\n```",
            "Notes".into(),
            TextDocumentFormat::Markdown,
        );

        assert_eq!(parsed.format, "markdown");
        assert!(parsed
            .view_html
            .contains("<h1 data-papercut-section=\"0\">Heading</h1>"));
        assert!(!parsed.view_html.contains("alert(1)"));
        assert!(parsed
            .sections
            .iter()
            .any(|section| section.text == "Heading"));
        assert!(parsed.view_html.contains("<pre><code>code();"));
    }

    #[test]
    fn text_decoder_requires_unambiguous_unicode() {
        assert_eq!(decode_text_bytes(b"UTF-8 text").unwrap(), "UTF-8 text");
        assert_eq!(
            decode_text_bytes(&[0xff, 0xfe, b'H', 0, b'i', 0]).unwrap(),
            "Hi"
        );
        assert!(decode_text_bytes(&[0x93, b'H', 0x94]).is_err());
    }
}
