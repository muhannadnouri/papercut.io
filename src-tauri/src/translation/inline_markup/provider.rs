//! Shared provider contracts and strict UTF-8 offset conversion.

use std::ops::Range;

// Internal strict evidence; public jobs/catalog do not opt in before semantic acceptance.
pub(crate) mod contract;
// Internal exact-piece planning, cache admission and source-owned reconstruction.
#[allow(dead_code)]
pub(crate) mod pieces;

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
        let target = " é🙂عَرَبِيّ ";
        let range = scalar_range_from_utf8(target, 1..target.len() - 1).unwrap();
        assert_eq!(range, 1..target.chars().count() - 1);
        assert_eq!(
            target
                .chars()
                .skip(range.start)
                .take(range.len())
                .collect::<String>(),
            "é🙂عَرَبِيّ"
        );
    }
}
