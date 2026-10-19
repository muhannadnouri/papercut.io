//! Byte-to-text decoding for imported HTML files.
//! Browser-style legacy charset decoding before parsing, sanitization, storage, and indexing.

use encoding_rs::Encoding;

const HTML_ENCODING_SNIFF_BYTES: usize = 64 * 1024;
const EQUALS: u8 = b"="[0];
const DOUBLE_QUOTE: u8 = b"\""[0];
const SINGLE_QUOTE: u8 = b"'"[0];
const SEMICOLON: u8 = b";"[0];
const TAG_END: u8 = b">"[0];

/// Decode imported HTML bytes using browser-compatible encoding labels.
/// Saved web pages are often Windows-1252 or another legacy encoding even when
/// modern app storage expects UTF-8. This keeps UTF-8 as the fast path, then
/// honors BOM/meta charset declarations before the parser/sanitizer sees text.
pub(crate) fn decode_html_bytes(bytes: &[u8]) -> Result<String, String> {
    if let Ok(html) = std::str::from_utf8(bytes) {
        return Ok(html.strip_prefix('\u{feff}').unwrap_or(html).to_owned());
    }
    let Some(encoding) = sniff_html_encoding(bytes) else {
        return Err(
            "HTML document is not valid UTF-8 and does not declare a supported legacy charset"
                .to_string(),
        );
    };
    let (decoded, _, had_errors) = encoding.decode(bytes);
    if had_errors {
        return Err(format!(
            "HTML document could not be decoded cleanly as {}",
            encoding.name()
        ));
    }
    Ok(decoded.into_owned())
}

/// Choose the decoder for a non-UTF-8 HTML upload. BOM wins because it is
/// unambiguous; otherwise we honor an early declared `<meta charset=...>` label
/// and let `encoding_rs` map browser aliases like `latin1` to Windows-1252.
fn sniff_html_encoding(bytes: &[u8]) -> Option<&'static Encoding> {
    if let Some((encoding, _)) = Encoding::for_bom(bytes) {
        return Some(encoding);
    }
    let label = find_declared_charset_label(bytes)?;
    Encoding::for_label(label.as_bytes())
}

/// Search only the first few KB for `<meta ...>` declarations, matching where
/// browsers expect charset metadata to appear while avoiding a full HTML parse
/// before we know how to decode the file.
fn find_declared_charset_label(bytes: &[u8]) -> Option<String> {
    let len = bytes.len().min(HTML_ENCODING_SNIFF_BYTES);
    let lower: Vec<u8> = bytes[..len]
        .iter()
        .map(|byte| byte.to_ascii_lowercase())
        .collect();
    let mut pos = 0usize;

    while let Some(found) = find_ascii(&lower[pos..], b"<meta") {
        let tag_start = pos + found;
        let Some(tag_end_rel) = find_ascii(&lower[tag_start..], &[TAG_END]) else {
            break;
        };
        let tag_end = tag_start + tag_end_rel;
        let tag = &lower[tag_start..tag_end];
        if let Some(label) = extract_charset_label(tag) {
            return Some(label);
        }
        pos = tag_end.saturating_add(1);
    }

    None
}

/// Pull a `charset=value` token out of one already-lowercased meta tag. This
/// intentionally handles the common saved-page forms, including quoted values
/// and `Content-Type` content attributes, without trying to implement HTML
/// tokenization here.
fn extract_charset_label(tag: &[u8]) -> Option<String> {
    let mut pos = 0usize;
    while let Some(found) = find_ascii(&tag[pos..], b"charset") {
        let mut cursor = pos + found + b"charset".len();
        while cursor < tag.len() && tag[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= tag.len() || tag[cursor] != EQUALS {
            pos = cursor.saturating_add(1);
            continue;
        }
        cursor += 1;
        while cursor < tag.len() && tag[cursor].is_ascii_whitespace() {
            cursor += 1;
        }

        let quote = tag
            .get(cursor)
            .copied()
            .filter(|byte| matches!(*byte, DOUBLE_QUOTE | SINGLE_QUOTE));
        if quote.is_some() {
            cursor += 1;
        }

        let start = cursor;
        while cursor < tag.len()
            && !tag[cursor].is_ascii_whitespace()
            && !matches!(
                tag[cursor],
                DOUBLE_QUOTE | SINGLE_QUOTE | SEMICOLON | TAG_END
            )
        {
            cursor += 1;
        }

        if cursor > start {
            return std::str::from_utf8(&tag[start..cursor])
                .ok()
                .map(|label| label.trim().to_string())
                .filter(|label| !label.is_empty());
        }

        pos = cursor.saturating_add(1);
    }

    None
}

/// Small byte-slice search helper used before decoding, where normal string
/// search is not available because the file may not be valid UTF-8 yet.
fn find_ascii(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod encoding_tests {
    use super::*;
    #[test]
    fn decodes_charset_after_injected_head_styles() {
        let mut bytes = b"<html><head><style>".to_vec();
        bytes.extend(std::iter::repeat(b"a"[0]).take(5 * 1024));
        bytes.extend_from_slice(br##"</style><meta http-equiv="Content-Type" content="text/html; charset=windows-1252"><body><p>"##);
        bytes.extend_from_slice(&[0x93, 72, 0x94]);
        bytes.extend_from_slice(br##"</p></body></html>"##);

        let decoded = decode_html_bytes(&bytes).expect("decode delayed charset declaration");

        assert!(decoded.contains("\u{201c}H\u{201d}"));
    }

    #[test]
    fn decodes_declared_windows_1252_html() {
        let mut bytes = br##"<html><head><meta http-equiv="Content-Type" content="text/html; charset=windows-1252"></head><body><p>"##.to_vec();
        bytes.extend_from_slice(&[0x93, 65, 110, 110, 101, 0x94, 32, 99, 97, 102, 0xe9]);
        bytes.extend_from_slice(br##"</p></body></html>"##);
        let decoded = decode_html_bytes(&bytes).expect("decode windows-1252 html");
        assert!(decoded.contains("\u{201c}Anne\u{201d} caf\u{e9}"));
    }
    #[test]
    fn decodes_short_meta_charset_form_case_insensitively() {
        let mut bytes = br##"<HTML><HEAD><META CHARSET=WINDOWS-1252></HEAD><BODY><p>"##.to_vec();
        bytes.extend_from_slice(&[0x93, 72, 0x94]);
        bytes.extend_from_slice(br##"</p></BODY></HTML>"##);

        let decoded = decode_html_bytes(&bytes).expect("decode uppercase meta charset");

        assert!(decoded.contains("\u{201c}H\u{201d}"));
    }

    #[test]
    fn ignores_charset_outside_meta_tags() {
        let bytes = b"<!-- charset=windows-1252 --><p>\x93</p>";

        let err = decode_html_bytes(bytes).expect_err("charset outside meta is ignored");

        assert!(err.contains("does not declare"));
    }

    #[test]
    fn rejects_unknown_declared_charset() {
        let bytes = b"<meta charset=definitely-not-real><p>\x93</p>";

        let err = decode_html_bytes(bytes).expect_err("unknown charset is rejected");

        assert!(err.contains("does not declare"));
    }

    #[test]
    fn keeps_valid_utf8_on_fast_path() {
        let html = "<html><body><p>Plain UTF-8</p></body></html>";
        assert_eq!(decode_html_bytes(html.as_bytes()).unwrap(), html);
    }

    #[test]
    fn strips_utf8_bom_on_fast_path() {
        let html = "\u{feff}<html><body><p>Plain UTF-8</p></body></html>";
        let decoded = decode_html_bytes(html.as_bytes()).expect("decode UTF-8 BOM html");

        assert_eq!(decoded, "<html><body><p>Plain UTF-8</p></body></html>");
    }
}
