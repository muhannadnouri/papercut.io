//! First-pass quality gates for completed translation output.
//!
//! These checks are intentionally cheap and deterministic. They do not judge
//! translation quality yet; they only prevent storing obviously broken output
//! such as empty translations or generated HTML with internal links that no
//! longer resolve.

use std::collections::{BTreeMap, HashSet};

use kuchikiki::NodeRef;

use super::html::parse_html_document;
use super::inline_markup::is_word_char;
use super::source::TranslationSourceBlock;
use super::storage::PersistTranslationSection;
use super::types::TranslationGlossaryEntry;

const MIN_RATIO_SOURCE_CHARS: usize = 120;
const MIN_LANGUAGE_DETECT_CHARS: usize = 120;
const MIN_WRONG_LANGUAGE_SECTIONS: usize = 3;
const MIN_TRANSLATED_REPEAT_CHARS: usize = 24;
const MAX_REPEAT_COUNT: usize = 5;
const MIN_LENGTH_RATIO: f32 = 0.05;
const MAX_LENGTH_RATIO: f32 = 8.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TranslationQualityIssue {
    pub(crate) kind: TranslationQualityIssueKind,
    pub(crate) source_ordinal: Option<usize>,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TranslationQualityIssueKind {
    EmptyOutput,
    SectionCoverage,
    LengthRatio,
    RepeatedOutput,
    WrongLanguage,
    GlossaryTarget,
    BrokenInternalLink,
}

impl TranslationQualityIssue {
    fn new(
        kind: TranslationQualityIssueKind,
        source_ordinal: Option<usize>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            source_ordinal,
            message: message.into(),
        }
    }

    fn to_user_message(&self) -> String {
        match self.source_ordinal {
            Some(ordinal) => format!("source section {}: {}", ordinal + 1, self.message),
            None => self.message.clone(),
        }
    }
}

/// Validate generated translated HTML before it is promoted into reader/search storage.
///
/// This runs after rendering because some problems only exist in final HTML.
/// Keeping it before durable writes avoids indexing variants that users can
/// open but whose footnotes/internal navigation are already known broken.
pub(crate) fn validate_translated_output(
    html: &str,
    source_blocks: &[TranslationSourceBlock],
    sections: &[PersistTranslationSection],
    glossary: &[TranslationGlossaryEntry],
    target_language: &str,
) -> Result<(), String> {
    validate_non_empty_sections(sections).map_err(format_quality_issue)?;
    validate_section_coverage(source_blocks, sections).map_err(format_quality_issue)?;
    validate_each_source_section_has_output(source_blocks, sections)
        .map_err(format_quality_issue)?;
    validate_length_ratios(source_blocks, sections).map_err(format_quality_issue)?;
    validate_repeated_outputs(sections).map_err(format_quality_issue)?;
    validate_target_language(sections, target_language).map_err(format_quality_issue)?;
    validate_glossary_terms(source_blocks, sections, glossary).map_err(format_quality_issue)?;
    let document = parse_html_document(html);
    validate_internal_links(&document).map_err(format_quality_issue)?;
    Ok(())
}

/// Reject jobs where every translated section is blank.
///
/// Native engines can fail softly by returning empty strings for all segments.
/// That should be reported as a translation failure, not stored as a valid
/// translated document with empty search rows.
fn validate_non_empty_sections(
    sections: &[PersistTranslationSection],
) -> Result<(), TranslationQualityIssue> {
    if sections
        .iter()
        .any(|section| !section.text.trim().is_empty())
    {
        Ok(())
    } else {
        Err(TranslationQualityIssue::new(
            TranslationQualityIssueKind::EmptyOutput,
            None,
            "Translation output was empty",
        ))
    }
}

/// Reject partial jobs before storage promotes them into normal documents.
///
/// Segment caches can prove the engine translated many individual chunks, but
/// the durable reader/search document must have one output section for every
/// source block. Otherwise failures like "only the title translated" look like
/// successful jobs and become harder to diagnose after import.
fn validate_section_coverage(
    source_blocks: &[TranslationSourceBlock],
    sections: &[PersistTranslationSection],
) -> Result<(), TranslationQualityIssue> {
    if sections.len() == source_blocks.len() {
        return Ok(());
    }
    Err(TranslationQualityIssue::new(
        TranslationQualityIssueKind::SectionCoverage,
        None,
        format!(
            "Translation output is incomplete: {} source section(s), {} translated section(s)",
            source_blocks.len(),
            sections.len()
        ),
    ))
}

/// Reject blank output for a specific source section once coverage is preserved.
///
/// Section count alone can pass even when one native segment returns empty text.
/// Reporting the source ordinal gives the UI/user a concrete failed section
/// instead of a confusing "100% translated, then incomplete" storage error.
fn validate_each_source_section_has_output(
    source_blocks: &[TranslationSourceBlock],
    sections: &[PersistTranslationSection],
) -> Result<(), TranslationQualityIssue> {
    let by_ordinal = source_blocks_by_ordinal(source_blocks);
    for (index, section) in sections.iter().enumerate() {
        if !section.text.trim().is_empty() {
            continue;
        }
        let source_ordinal = source_block_for_section(&by_ordinal, source_blocks, section, index)
            .map(|source| source.ordinal)
            .or(Some(section.source_ordinal));
        return Err(TranslationQualityIssue::new(
            TranslationQualityIssueKind::EmptyOutput,
            source_ordinal,
            "Translation output was empty for this source section",
        ));
    }
    Ok(())
}

/// Catch source/translation length mismatches that are almost certainly engine failures.
///
/// This is intentionally loose. Literary and academic translation can expand or
/// compress, especially across scripts. The thresholds only reject output that
/// looks like truncation, decoder loops, or a prompt echo gone very wrong.
fn validate_length_ratios(
    source_blocks: &[TranslationSourceBlock],
    sections: &[PersistTranslationSection],
) -> Result<(), TranslationQualityIssue> {
    let by_ordinal = source_blocks_by_ordinal(source_blocks);
    for (index, section) in sections.iter().enumerate() {
        let Some(source) = source_block_for_section(&by_ordinal, source_blocks, section, index)
        else {
            continue;
        };
        let source_chars = source.text.trim().chars().count();
        let translated_chars = section.text.trim().chars().count();
        if source_chars < MIN_RATIO_SOURCE_CHARS || translated_chars == 0 {
            continue;
        }
        let ratio = translated_chars as f32 / source_chars as f32;
        if !(MIN_LENGTH_RATIO..=MAX_LENGTH_RATIO).contains(&ratio) {
            return Err(TranslationQualityIssue::new(
                TranslationQualityIssueKind::LengthRatio,
                Some(source.ordinal),
                format!(
                    "Translation output length looks unsafe: source {} chars, translated {} chars",
                    source_chars, translated_chars
                ),
            ));
        }
    }
    Ok(())
}

/// Reject repeated long outputs across many sections.
///
/// A common local-model failure mode is returning the same generic sentence for
/// every segment. This does not block repeated short labels/headings; it only
/// catches long repeated bodies that would make a book unusable.
fn validate_repeated_outputs(
    sections: &[PersistTranslationSection],
) -> Result<(), TranslationQualityIssue> {
    let mut counts = std::collections::BTreeMap::<String, (usize, usize)>::new();
    for section in sections.iter().filter(|section| !section.is_heading) {
        let normalized = normalize_quality_text(&section.text);
        if normalized.chars().count() < MIN_TRANSLATED_REPEAT_CHARS {
            continue;
        }
        let (count, first_ordinal) = counts
            .entry(normalized.clone())
            .or_insert((0, section.source_ordinal));
        *count += 1;
        if *count >= MAX_REPEAT_COUNT {
            return Err(TranslationQualityIssue::new(
                TranslationQualityIssueKind::RepeatedOutput,
                Some(*first_ordinal),
                format!(
                    "Translation output repeats the same long text across {MAX_REPEAT_COUNT} sections: {}",
                    truncate_quality_preview(&normalized, 80)
                ),
            ));
        }
    }
    Ok(())
}

/// Reject output whose dominant language is not the requested target.
///
/// A common local-engine failure is echoing source text (or emitting another
/// language) while reporting success. Detection is statistical, so this gate
/// is deliberately conservative: only long sections count, only reliable
/// detections count, and rejection needs either a wrong-language majority
/// across several sections or a unanimously wrong multi-section document.
/// Foreign quotations inside an otherwise correct translation must never fail
/// a job, and unmapped target codes skip the gate entirely.
fn validate_target_language(
    sections: &[PersistTranslationSection],
    target_language: &str,
) -> Result<(), TranslationQualityIssue> {
    let Some(expected) = whatlang_target(target_language) else {
        return Ok(());
    };
    let mut checked = 0usize;
    let mut wrong = 0usize;
    let mut first_wrong_ordinal = None;
    let mut detected_example: Option<whatlang::Lang> = None;
    for section in sections {
        let text = section.text.trim();
        if text.chars().count() < MIN_LANGUAGE_DETECT_CHARS {
            continue;
        }
        let Some(info) = whatlang::detect(text) else {
            continue;
        };
        if !info.is_reliable() {
            continue;
        }
        checked += 1;
        if info.lang() != expected {
            wrong += 1;
            if first_wrong_ordinal.is_none() {
                first_wrong_ordinal = Some(section.source_ordinal);
                detected_example = Some(info.lang());
            }
        }
    }

    let majority_wrong = wrong >= MIN_WRONG_LANGUAGE_SECTIONS && wrong * 2 > checked;
    let unanimous_wrong = checked >= 2 && wrong == checked;
    if majority_wrong || unanimous_wrong {
        return Err(TranslationQualityIssue::new(
            TranslationQualityIssueKind::WrongLanguage,
            first_wrong_ordinal,
            format!(
                "Translation output looks like {} instead of the requested target language {:?} in {wrong} of {checked} checked section(s)",
                detected_example
                    .map(|lang| lang.eng_name())
                    .unwrap_or("another language"),
                target_language
            ),
        ));
    }
    Ok(())
}

/// Map the request's ISO 639-1 target code onto whatlang's 639-3 languages.
///
/// Unknown codes return None so the gate skips instead of misfiring on
/// language pairs whatlang cannot judge.
fn whatlang_target(code: &str) -> Option<whatlang::Lang> {
    let code = code.trim().to_ascii_lowercase();
    let iso639_3 = match code.as_str() {
        "en" => "eng",
        "es" => "spa",
        "fr" => "fra",
        "de" => "deu",
        "ru" => "rus",
        "zh" => "cmn",
        "ar" => "ara",
        "pt" => "por",
        "it" => "ita",
        "nl" => "nld",
        "pl" => "pol",
        "tr" => "tur",
        "ja" => "jpn",
        "ko" => "kor",
        other => other,
    };
    whatlang::Lang::from_code(iso639_3)
}

/// Ensure protected glossary terms survive in translated output.
///
/// Glossary entries are user instructions, not soft hints. If the source section
/// contains the source term and the translated section does not contain the
/// requested target term, storing the variant would make the glossary look
/// successful while silently ignoring it.
///
/// Source-term detection requires word boundaries: substring hits such as
/// "cat" inside "category" must not demand the target term in sections that
/// never used the real word. Target detection stays substring-tolerant because
/// inflection can legitimately extend the requested target form.
fn validate_glossary_terms(
    source_blocks: &[TranslationSourceBlock],
    sections: &[PersistTranslationSection],
    glossary: &[TranslationGlossaryEntry],
) -> Result<(), TranslationQualityIssue> {
    if glossary.is_empty() {
        return Ok(());
    }
    let lowered_entries = glossary
        .iter()
        .map(|entry| {
            (
                entry.source.trim().to_lowercase(),
                entry.target.trim().to_lowercase(),
                entry,
            )
        })
        .filter(|(source, target, _)| !source.is_empty() && !target.is_empty())
        .collect::<Vec<_>>();
    let by_ordinal = source_blocks_by_ordinal(source_blocks);
    for (index, section) in sections.iter().enumerate() {
        let Some(source) = source_block_for_section(&by_ordinal, source_blocks, section, index)
        else {
            continue;
        };
        let source_lower = source.text.to_lowercase();
        let section_lower = section.text.to_lowercase();
        for (source_term, target_term, entry) in &lowered_entries {
            if lowered_contains_word_bounded(&source_lower, source_term)
                && !section_lower.contains(target_term.as_str())
            {
                return Err(TranslationQualityIssue::new(
                    TranslationQualityIssueKind::GlossaryTarget,
                    Some(source.ordinal),
                    format!(
                        "Translation output missed glossary target {:?} for source term {:?}",
                        entry.target.trim(),
                        entry.source.trim()
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// Word-bounded term search over already-lowercased text.
///
/// A boundary is only demanded on term edges that are themselves word
/// characters, so punctuation-edged terms ("C++") still match naturally.
pub(crate) fn lowered_contains_word_bounded(text: &str, term: &str) -> bool {
    if term.is_empty() {
        return false;
    }
    let mut search_start = 0usize;
    while let Some(relative) = text[search_start..].find(term) {
        let start = search_start + relative;
        let end = start + term.len();
        let first = term.chars().next();
        let last = term.chars().next_back();
        let before = text[..start].chars().next_back();
        let after = text[end..].chars().next();
        let left_ok = !first.is_some_and(is_word_char) || !before.is_some_and(is_word_char);
        let right_ok = !last.is_some_and(is_word_char) || !after.is_some_and(is_word_char);
        if left_ok && right_ok {
            return true;
        }
        search_start = end;
    }
    false
}

/// Ensure local `#anchor` links still point at an id in generated HTML.
///
/// This catches the common preservation regression: footnote/table-of-contents
/// links survive as `href="#note"` but the target id is lost during rendering.
/// External links and empty page-top anchors are ignored here.
fn validate_internal_links(document: &NodeRef) -> Result<(), TranslationQualityIssue> {
    let ids = document
        .select("[id]")
        .map_err(|_| {
            TranslationQualityIssue::new(
                TranslationQualityIssueKind::BrokenInternalLink,
                None,
                "Failed to inspect translated HTML ids",
            )
        })?
        .filter_map(|node| {
            node.attributes
                .borrow()
                .get("id")
                .map(|value| value.trim().to_string())
        })
        .filter(|value| !value.is_empty())
        .collect::<HashSet<_>>();

    let mut missing = Vec::new();
    for node in document.select("a[href]").map_err(|_| {
        TranslationQualityIssue::new(
            TranslationQualityIssueKind::BrokenInternalLink,
            None,
            "Failed to inspect translated HTML links",
        )
    })? {
        let href = node
            .attributes
            .borrow()
            .get("href")
            .map(|value| value.trim().to_string());
        let Some(href) = href else {
            continue;
        };
        let Some(target) = href.strip_prefix('#') else {
            continue;
        };
        if target.is_empty() {
            continue;
        }
        if !ids.contains(target) {
            missing.push(href);
        }
    }

    if missing.is_empty() {
        Ok(())
    } else {
        Err(TranslationQualityIssue::new(
            TranslationQualityIssueKind::BrokenInternalLink,
            None,
            format!(
                "Translation output has broken internal link target(s): {}",
                missing.join(", ")
            ),
        ))
    }
}

fn format_quality_issue(issue: TranslationQualityIssue) -> String {
    issue.to_user_message()
}

/// Index source blocks by ordinal once so per-section lookups stay cheap on
/// book-sized documents instead of re-scanning the block list per section.
fn source_blocks_by_ordinal(
    source_blocks: &[TranslationSourceBlock],
) -> BTreeMap<usize, &TranslationSourceBlock> {
    source_blocks
        .iter()
        .map(|block| (block.ordinal, block))
        .collect()
}

fn source_block_for_section<'a>(
    by_ordinal: &BTreeMap<usize, &'a TranslationSourceBlock>,
    source_blocks: &'a [TranslationSourceBlock],
    section: &PersistTranslationSection,
    fallback_index: usize,
) -> Option<&'a TranslationSourceBlock> {
    by_ordinal
        .get(&section.source_ordinal)
        .copied()
        .or_else(|| source_blocks.get(fallback_index))
}

fn normalize_quality_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_quality_preview(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let preview = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{preview}...")
    } else {
        preview
    }
}

#[cfg(test)]
mod tests {
    use super::validate_translated_output;
    use crate::translation::source::TranslationSourceBlock;
    use crate::translation::storage::PersistTranslationSection;
    use crate::translation::types::TranslationGlossaryEntry;

    #[test]
    fn accepts_internal_links_with_targets() {
        validate_translated_output(
            "<article><p><a href=\"#note-1\">note</a></p><p id=\"note-1\">body</p></article>",
            &[source_block(0, "Source text")],
            &[section("Translated text")],
            &[],
            "en",
        )
        .expect("valid links");
    }

    #[test]
    fn rejects_missing_internal_link_targets() {
        let error = validate_translated_output(
            "<article><p><a href=\"#missing\">note</a></p></article>",
            &[source_block(0, "Source text")],
            &[section("Translated text")],
            &[],
            "en",
        )
        .expect_err("broken target");

        assert!(error.contains("#missing"));
    }

    #[test]
    fn rejects_empty_translated_sections() {
        let error = validate_translated_output(
            "<article></article>",
            &[source_block(0, "Source text")],
            &[section("   ")],
            &[],
            "en",
        )
        .expect_err("empty translation");

        assert!(error.contains("empty"));
    }

    #[test]
    fn rejects_specific_empty_translated_section() {
        let error = validate_translated_output(
            "<article><h1>Title</h1><p></p></article>",
            &[source_block(0, "Title"), source_block(1, "Body text")],
            &[
                section_with_ordinal(0, "Title"),
                section_with_ordinal(1, "   "),
            ],
            &[],
            "en",
        )
        .expect_err("empty translated body");

        assert!(error.contains("source section 2"));
        assert!(error.contains("empty for this source section"));
    }

    #[test]
    fn rejects_incomplete_section_coverage() {
        let error = validate_translated_output(
            "<article><h1>Title</h1></article>",
            &[source_block(0, "Title"), source_block(1, "Body text")],
            &[section("Title")],
            &[],
            "en",
        )
        .expect_err("missing translated body");

        assert!(error.contains("incomplete"));
        assert!(error.contains("2 source section"));
    }

    #[test]
    fn rejects_extreme_length_ratio() {
        let source = "Long source text. ".repeat(20);
        let error = validate_translated_output(
            "<article><p>tiny</p></article>",
            &[source_block(0, &source)],
            &[section("tiny")],
            &[],
            "en",
        )
        .expect_err("unsafe length");

        assert!(error.contains("length"));
    }

    #[test]
    fn rejects_repeated_long_outputs() {
        let repeated = "This is the same suspicious translation body.";
        let sources = (0..5)
            .map(|index| source_block(index, "Different source paragraph with enough text."))
            .collect::<Vec<_>>();
        let sections = (0..5).map(|_| section(repeated)).collect::<Vec<_>>();

        let error =
            validate_translated_output("<article></article>", &sources, &sections, &[], "en")
                .expect_err("repeated output");

        assert!(error.contains("repeats"));
    }

    #[test]
    fn rejects_output_in_wrong_target_language() {
        // Long, clearly Spanish sections while English was requested - the
        // signature of an engine echoing its source instead of translating.
        let spanish = [
            "La puerta azul se abre lentamente cada mañana mientras los vecinos del barrio conversan sobre el clima y esperan la llegada del mercado semanal con sus frutas frescas.",
            "El bibliotecario ordena los estantes durante toda la tarde y explica a los estudiantes cómo encontrar los manuales de historia entre las colecciones antiguas del archivo.",
            "Los trenes regionales salen temprano desde la estación central y cruzan los campos de trigo antes de llegar a los pueblos pequeños donde terminan su recorrido diario.",
        ];
        let sources = (0..3)
            .map(|index| source_block(index, spanish[index]))
            .collect::<Vec<_>>();
        let sections = spanish
            .iter()
            .enumerate()
            .map(|(index, text)| section_with_ordinal(index, text))
            .collect::<Vec<_>>();

        let error =
            validate_translated_output("<article></article>", &sources, &sections, &[], "en")
                .expect_err("wrong language output");

        assert!(error.contains("target language"));
    }

    #[test]
    fn accepts_foreign_quotes_inside_correct_target_language() {
        let english = [
            "The blue door opens slowly every morning while the neighbours discuss the weather and wait for the weekly market to arrive with its fresh fruit and vegetables.",
            "The librarian arranges the shelves all afternoon and shows the students how to find the history manuals among the oldest collections of the archive downstairs.",
            "The regional trains leave early from the central station and cross the wheat fields before reaching the small towns where they finish their daily route.",
        ];
        let quoted_spanish = "La puerta azul se abre lentamente cada mañana mientras los vecinos del barrio conversan sobre el clima y esperan la llegada del mercado semanal con sus frutas.";
        let mut texts = english.to_vec();
        texts.push(quoted_spanish);
        let sources = (0..texts.len())
            .map(|index| {
                source_block(
                    index,
                    "Source paragraph with plenty of length for ratio checks and coverage.",
                )
            })
            .collect::<Vec<_>>();
        let sections = texts
            .iter()
            .enumerate()
            .map(|(index, text)| section_with_ordinal(index, text))
            .collect::<Vec<_>>();

        validate_translated_output("<article></article>", &sources, &sections, &[], "en")
            .expect("one foreign-quote section must not fail the job");
    }

    #[test]
    fn unmapped_target_language_skips_detection_gate() {
        let spanish = "La puerta azul se abre lentamente cada mañana mientras los vecinos del barrio conversan sobre el clima y esperan la llegada del mercado semanal con sus frutas.";
        let sources = (0..2)
            .map(|index| source_block(index, spanish))
            .collect::<Vec<_>>();
        let sections = (0..2)
            .map(|index| section_with_ordinal(index, spanish))
            .collect::<Vec<_>>();

        validate_translated_output("<article></article>", &sources, &sections, &[], "xx")
            .expect("unknown target codes must skip language detection");
    }

    #[test]
    fn accepts_glossary_target_when_source_term_is_present() {
        validate_translated_output(
            "<article><p>State and Revolution</p></article>",
            &[source_block(0, "Estado y revolucion")],
            &[section("State and Revolution")],
            &[glossary("Estado", "State")],
            "en",
        )
        .expect("glossary target present");
    }

    #[test]
    fn substring_source_term_does_not_demand_glossary_target() {
        // "mar" appears only inside "marca"; the glossary term was never used
        // as a word, so the missing target must not fail the job.
        validate_translated_output(
            "<article><p>The brand grows.</p></article>",
            &[source_block(0, "La marca crece.")],
            &[section("The brand grows.")],
            &[glossary("mar", "sea")],
            "en",
        )
        .expect("substring must not trigger glossary gate");
    }

    #[test]
    fn word_bounded_lookup_matches_only_whole_words() {
        assert!(super::lowered_contains_word_bounded(
            "la marca del mar crece",
            "mar"
        ));
        assert!(!super::lowered_contains_word_bounded(
            "la marca crece",
            "mar"
        ));
        assert!(super::lowered_contains_word_bounded("usa c++ hoy", "c++"));
    }

    #[test]
    fn rejects_missing_glossary_target() {
        let error = validate_translated_output(
            "<article><p>Government and Revolution</p></article>",
            &[source_block(0, "Estado y revolucion")],
            &[section("Government and Revolution")],
            &[glossary("Estado", "State")],
            "en",
        )
        .expect_err("missing glossary target");

        assert!(error.contains("glossary"));
        assert!(error.contains("State"));
    }

    fn section(text: &str) -> PersistTranslationSection {
        section_with_ordinal(0, text)
    }

    fn section_with_ordinal(source_ordinal: usize, text: &str) -> PersistTranslationSection {
        PersistTranslationSection {
            source_heading: None,
            source_ordinal,
            is_heading: false,
            text: text.into(),
            fragments: Vec::new(),
        }
    }

    fn source_block(ordinal: usize, text: &str) -> TranslationSourceBlock {
        TranslationSourceBlock {
            ordinal,
            is_heading: false,
            heading: None,
            text: text.into(),
        }
    }

    fn glossary(source: &str, target: &str) -> TranslationGlossaryEntry {
        TranslationGlossaryEntry {
            source: source.into(),
            target: target.into(),
            note: None,
        }
    }
}
