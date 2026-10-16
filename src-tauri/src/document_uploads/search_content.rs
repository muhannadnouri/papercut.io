//! Derived main-content ranges. Authored text and literal search remain intact.

use std::ops::Range;

/// Unknown legacy content remains readable; malformed derived offsets fail closed.
pub(super) fn ranges(text: &str, encoded: Option<&str>) -> Result<Vec<Range<usize>>, String> {
    let ranges: Vec<Range<usize>> = match encoded {
        Some(value) => serde_json::from_str(value).map_err(|err| err.to_string())?,
        None => {
            if text.is_empty() {
                Vec::new()
            } else {
                vec![0..text.len()]
            }
        }
    };
    let mut end = 0;
    for range in &ranges {
        if range.start < end || range.start >= range.end || text.get(range.clone()).is_none() {
            return Err("Invalid main-content source ranges".into());
        }
        end = range.end;
    }
    Ok(ranges)
}

/// Keep disjoint body spans separate so previews cannot bridge a removed note.
#[cfg(test)]
pub(super) fn body_text(text: &str, ranges: &[Range<usize>]) -> String {
    ranges
        .iter()
        .map(|range| &text[range.clone()])
        .collect::<Vec<_>>()
        .join("\n")
}

/// Recognize only structural EPUB/DPUB metadata, never a vocabulary of subject words.
pub(super) fn auxiliary(node: &kuchikiki::NodeRef) -> bool {
    node.ancestors()
        .chain(std::iter::once(node.clone()))
        .any(|node| {
            let Some(element) = node.as_element() else {
                return false;
            };
            let attrs = element.attributes.borrow();
            element.name.local.as_ref() == "nav"
                || ["role", "epub:type"].iter().any(|attribute| {
                    attrs.get(*attribute).is_some_and(|value| {
                        value
                            .split_whitespace()
                            .any(|token| auxiliary_role(attribute, token))
                    })
                })
        })
}

/// Preserve only inert role tokens through both EPUB and HTML security boundaries.
pub(super) fn safe_role(attribute: &str, value: &str) -> Option<String> {
    let tokens = value
        .split_whitespace()
        .filter(|token| auxiliary_role(attribute, token))
        .collect::<Vec<_>>()
        .join(" ");
    (!tokens.is_empty()).then_some(tokens)
}

fn auxiliary_role(attribute: &str, token: &str) -> bool {
    match attribute {
        "role" => matches!(
            token,
            "doc-footnote"
                | "doc-endnote"
                | "doc-endnotes"
                | "doc-bibliography"
                | "doc-toc"
                | "doc-index"
                | "navigation"
        ),
        "epub:type" => matches!(
            token,
            "footnote"
                | "endnote"
                | "footnotes"
                | "endnotes"
                | "rearnotes"
                | "bibliography"
                | "toc"
                | "index"
        ),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_ranges_reject_overlap_and_broken_utf8() {
        assert_eq!(ranges("港😀", None).unwrap(), vec![0..7]);
        assert!(ranges("港😀", Some(r#"[{"start":1,"end":7}]"#)).is_err());
        assert!(ranges("abcd", Some(r#"[{"start":0,"end":3},{"start":2,"end":4}]"#)).is_err());
        assert!(ranges("abcd", Some("[]")).unwrap().is_empty());
    }
}
