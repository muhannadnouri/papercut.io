//! Version 1 of the derived, forgiving search form. Source sections stay intact.

pub(super) const VERSION: &str = "1";

pub(super) fn has_arabic(text: &str) -> bool {
    text.chars().any(|ch| matches!(ch, '\u{0600}'..='\u{06ff}'))
}

pub(super) fn normalize(text: &str) -> String {
    text.chars().filter_map(normalize_char).collect()
}

fn normalize_char(ch: char) -> Option<char> {
    match ch {
        // Tashkeel and tatweel are optional decoration in the search form.
        '\u{0640}' | '\u{064b}'..='\u{065f}' | '\u{0670}' => None,
        // Merge common alef/hamza spellings only; ya, alef-maqsura,
        // ta-marbuta and ha remain distinct.
        '\u{0622}' | '\u{0623}' | '\u{0625}' => Some('\u{0627}'),
        _ => Some(ch),
    }
}

/// Character-index mapping for source excerpts. A projected offset is never
/// treated as an authored-text offset after marks have been removed.
pub(super) fn source_span(source: &str, term: &str) -> Option<(usize, usize)> {
    let mut projected = Vec::new();
    let mut source_chars = Vec::new();
    for (index, ch) in source.chars().enumerate() {
        if let Some(ch) = normalize_char(ch) {
            for folded in ch.to_lowercase() {
                projected.push(folded);
                source_chars.push(index);
            }
        }
    }
    let needle = term.to_lowercase().chars().collect::<Vec<_>>();
    if needle.is_empty() {
        return None;
    }
    let word = |ch: char| ch.is_alphanumeric() || ch == '_';
    let start = projected
        .windows(needle.len())
        .enumerate()
        .find_map(|(start, window)| {
            let end = start + needle.len();
            (window == needle
                && (start == 0 || !word(projected[start - 1]))
                && (end == projected.len() || !word(projected[end])))
            .then_some(start)
        })?;
    let end = start + needle.len();
    Some((
        source_chars[start],
        source_chars
            .get(end)
            .copied()
            .unwrap_or_else(|| source.chars().count()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arabic_forms_and_source_offsets() {
        assert_eq!(normalize("إِنْتَاجُ الطـاقةِ"), "انتاج الطاقة");
        assert_eq!(normalize("ى ي ة ه"), "ى ي ة ه");
        let source = "🙂 إِنْتَاجُ الطـاقةِ";
        let (start, end) = source_span(source, "انتاج").unwrap();
        assert_eq!(
            source
                .chars()
                .skip(start)
                .take(end - start)
                .collect::<String>(),
            "إِنْتَاجُ"
        );
        assert_eq!(source_span("🙂 إنتاجية ثم إنتاج", "انتاج"), Some((13, 18)));
        assert_eq!(
            source_span("İ 🙂 إِنْتَاجُ", "انتاج").map(|(start, end)| "İ 🙂 إِنْتَاجُ"
                .chars()
                .skip(start)
                .take(end - start)
                .collect::<String>()),
            Some("إِنْتَاجُ".into())
        );
    }
}
