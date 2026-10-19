//! Validate provider emphasis mappings without accepting provider tags or attributes.
//! Source span indices are local to the retained inline model, never persistent DOM ids.

use std::ops::Range;

use super::{InlineSourceModel, ProjectedInlineSpan};

#[derive(Debug, Clone)]
pub(crate) struct InlineSpanMapping {
    pub(crate) source_span: usize,
    /// Half-open Unicode scalar range in the exact, untrimmed target text.
    pub(crate) target: Range<usize>,
}

/// This first representation maps each source formatting run exactly once.
/// Nested tags are one source-owned tag stack; overlapping target runs are invalid.
/// Callers must retain the existing preservation path for links, notes, code and assets.
pub(crate) fn validate_inline_mapping(
    source: &InlineSourceModel,
    source_echo: &str,
    target: &str,
    mappings: &[InlineSpanMapping],
) -> Result<Vec<ProjectedInlineSpan>, String> {
    if source.text != source_echo || source.text.trim().is_empty() || target.trim().is_empty() {
        return Err("Inline mapping does not match nonempty source and target text".into());
    }
    if mappings.len() != source.spans.len() {
        return Err("Inline mapping is missing or duplicates source formatting runs".into());
    }
    let boundaries = target
        .char_indices()
        .map(|(byte, _)| byte)
        .chain([target.len()])
        .collect::<Vec<_>>();
    let source_len = source.text.chars().count();
    let mut seen = vec![false; source.spans.len()];
    let mut spans = Vec::with_capacity(mappings.len());
    for mapping in mappings {
        let Some(span) = source.spans.get(mapping.source_span) else {
            return Err("Inline mapping references unknown source formatting".into());
        };
        if seen[mapping.source_span]
            || span.start >= span.end
            || span.end > source_len
            || span.tags.is_empty()
            || span.tags.iter().any(|tag| tag == "code")
        {
            return Err(
                "Inline mapping references duplicate or unsupported source formatting".into(),
            );
        }
        seen[mapping.source_span] = true;
        if mapping.target.start >= mapping.target.end || mapping.target.end >= boundaries.len() {
            return Err("Inline mapping has an invalid target scalar range".into());
        }
        spans.push(ProjectedInlineSpan {
            start: boundaries[mapping.target.start],
            end: boundaries[mapping.target.end],
            tags: span.tags.clone(),
        });
    }
    spans.sort_by_key(|span| (span.start, span.end));
    if spans.windows(2).any(|pair| pair[0].end > pair[1].start) {
        return Err("Inline mapping contains overlapping target formatting runs".into());
    }
    Ok(spans)
}

/// Reject provider byte offsets inside a scalar instead of rounding into another character.
pub(crate) fn scalar_range_from_utf8(
    text: &str,
    bytes: Range<usize>,
) -> Result<Range<usize>, String> {
    if bytes.start >= bytes.end || text.get(bytes.clone()).is_none() {
        return Err("Provider returned an invalid UTF-8 byte range".into());
    }
    Ok(text[..bytes.start].chars().count()..text[..bytes.end].chars().count())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::translation::{dom::DomPlan, inline_markup::source_inline_model};

    fn model(html: &str) -> InlineSourceModel {
        let safe = crate::document_uploads::sanitize_html(html);
        let plan = DomPlan::new(&safe);
        source_inline_model(&plan.blocks[0].node)
    }

    #[test]
    fn reorders_source_owned_nested_styles_and_converts_target_scalars_to_bytes() {
        let source = model("<p>Une bague en <strong><em>or</em></strong> et <i>argent</i>.</p>");
        let target = "An argent🙂 and gold ring.";
        // Reply order and target order both differ from source order.
        let spans = validate_inline_mapping(
            &source,
            &source.text,
            target,
            &[
                InlineSpanMapping {
                    source_span: 1,
                    target: 3..9,
                },
                InlineSpanMapping {
                    source_span: 0,
                    target: 15..19,
                },
            ],
        )
        .unwrap();
        assert_eq!(&target[spans[0].start..spans[0].end], "argent");
        assert_eq!(spans[0].tags, ["i"]);
        assert_eq!(&target[spans[1].start..spans[1].end], "gold");
        assert_eq!(spans[1].tags, ["strong", "em"]);
        assert_eq!(spans[1].start, 18); // Emoji is one scalar and four bytes.
    }

    #[test]
    fn repeated_words_use_explicit_source_identities_not_phrase_search() {
        let source = model("<p><em>Orion</em> puis <strong>Orion</strong>.</p>");
        let target = "Orion, then Orion.";
        let spans = validate_inline_mapping(
            &source,
            &source.text,
            target,
            &[
                InlineSpanMapping {
                    source_span: 0,
                    target: 12..17,
                },
                InlineSpanMapping {
                    source_span: 1,
                    target: 0..5,
                },
            ],
        )
        .unwrap();
        assert_eq!(spans[0].tags, ["strong"]);
        assert_eq!(spans[1].tags, ["em"]);
    }

    #[test]
    fn rejects_unknown_missing_duplicate_overlapping_and_invalid_scalar_ranges() {
        let source = model("<p><strong>un</strong> et <em>deux</em></p>");
        let target = "one and two";
        let first = InlineSpanMapping {
            source_span: 0,
            target: 0..3,
        };
        let second = InlineSpanMapping {
            source_span: 1,
            target: 8..11,
        };
        for mappings in [
            vec![],
            vec![first.clone()],
            vec![first.clone(), second.clone(), first.clone()],
            vec![first.clone(), first.clone()],
            vec![
                first.clone(),
                InlineSpanMapping {
                    source_span: 99,
                    ..second.clone()
                },
            ],
            vec![
                first.clone(),
                InlineSpanMapping {
                    target: 2..8,
                    ..second.clone()
                },
            ],
            vec![
                first.clone(),
                InlineSpanMapping {
                    target: 0..11,
                    ..second.clone()
                },
            ],
            vec![
                first.clone(),
                InlineSpanMapping {
                    target: 0..3,
                    ..second.clone()
                },
            ],
            vec![
                first.clone(),
                InlineSpanMapping {
                    target: 8..8,
                    ..second.clone()
                },
            ],
            vec![
                first.clone(),
                InlineSpanMapping {
                    target: 9..8,
                    ..second.clone()
                },
            ],
            vec![
                first.clone(),
                InlineSpanMapping {
                    target: 8..12,
                    ..second.clone()
                },
            ],
            vec![
                first.clone(),
                InlineSpanMapping {
                    target: 0..usize::MAX,
                    ..second.clone()
                },
            ],
        ] {
            assert!(
                validate_inline_mapping(&source, &source.text, target, &mappings).is_err(),
                "{mappings:?}"
            );
        }
        assert!(validate_inline_mapping(
            &source,
            "changed source",
            target,
            &[first.clone(), second.clone()]
        )
        .is_err());
        assert!(validate_inline_mapping(&source, &source.text, " \n", &[first, second]).is_err());
    }

    #[test]
    fn utf8_conversion_rejects_partial_accents_emoji_and_arabic_combining_marks() {
        let text = "é🙂عَرَبِيّ";
        assert_eq!(scalar_range_from_utf8(text, 0..2).unwrap(), 0..1);
        assert_eq!(scalar_range_from_utf8(text, 2..6).unwrap(), 1..2);
        assert_eq!(scalar_range_from_utf8(text, 8..10).unwrap(), 3..4);
        assert_eq!(
            scalar_range_from_utf8(text, 0..text.len()).unwrap().end,
            text.chars().count()
        );
        for range in [0..1, 3..6, 8..9, 0..usize::MAX, 2..2, 6..2] {
            assert!(
                scalar_range_from_utf8(text, range.clone()).is_err(),
                "{range:?}"
            );
        }
    }

    #[test]
    fn retains_exact_untrimmed_target_offsets_through_utf8_conversion() {
        let source = model("<p><em>Résumé</em></p>");
        let target = " é🙂عَرَبِيّ ";
        let range = scalar_range_from_utf8(target, 1..target.len() - 1).unwrap();
        let spans = validate_inline_mapping(
            &source,
            &source.text,
            target,
            &[InlineSpanMapping {
                source_span: 0,
                target: range,
            }],
        )
        .unwrap();
        assert_eq!(&target[spans[0].start..spans[0].end], "é🙂عَرَبِيّ");
        assert_eq!(spans[0].start, 1);
        assert_eq!(spans[0].end, target.len() - 1);
    }

    #[test]
    fn plain_text_needs_no_mapping_and_code_cannot_be_remapped_as_emphasis() {
        let source = model("<p>Bonjour</p>");
        assert!(validate_inline_mapping(&source, &source.text, "Hello", &[])
            .unwrap()
            .is_empty());
        assert!(validate_inline_mapping(
            &source,
            &source.text,
            "Hello",
            &[InlineSpanMapping {
                source_span: 0,
                target: 0..5
            },]
        )
        .is_err());
        let code = model("<p><code>user_id</code></p>");
        assert!(validate_inline_mapping(
            &code,
            &code.text,
            "translated",
            &[InlineSpanMapping {
                source_span: 0,
                target: 0..10
            },]
        )
        .is_err());
    }
}
