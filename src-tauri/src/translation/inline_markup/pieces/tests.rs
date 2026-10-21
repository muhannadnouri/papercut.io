use super::*;
use crate::translation::dom::DomPlan;
use crate::translation::inline_markup::protected::ProtectedSource;
use std::cell::Cell;

struct Translator {
    words: usize,
    calls: Vec<Vec<String>>,
    fault: &'static str,
}
impl Translator {
    fn new(words: usize) -> Self {
        Self {
            words,
            calls: vec![],
            fault: "",
        }
    }
}
impl ExactPieceTranslator for Translator {
    fn source_fits(&self, source: &str) -> Result<bool, String> {
        Ok(source.split_whitespace().count() + 1 <= self.words)
    }
    fn translate_exact(&mut self, sources: &[&str]) -> Result<Vec<String>, String> {
        self.calls
            .push(sources.iter().map(|s| s.to_string()).collect());
        if self.fault == "count" {
            return Ok(vec![]);
        }
        Ok(sources
            .iter()
            .map(|s| match self.fault {
                "empty" => " ".into(),
                "large" => "x".repeat(MAX_HTML_BYTES + 1),
                "target" => "one two three four five six".into(),
                "padding" => format!("  {}🙂 ", s.to_uppercase()),
                "combining" => "\u{301}".into(),
                _ => s.to_uppercase(),
            })
            .collect())
    }
}

fn fixture(html: &str) -> (ProtectedSource, TranslationTextSegment) {
    let dom = DomPlan::new(html);
    let block = &dom.blocks[0];
    (
        ProtectedSource::new(&block.node).unwrap(),
        TranslationTextSegment {
            id: "b7:s2".into(),
            source_block_index: 7,
            source_start: 0,
            source_end: block.inline.text.chars().count(),
            text: block.inline.text.clone(),
        },
    )
}

/// Every source and target character has one unit, including joins that do not go through a model.
fn coverage(plan: &SourcePlan, translated: &Translation) {
    assert!(std::ptr::eq(plan, translated.plan));
    let source = plan.protected.source_text();
    let pieces = plan
        .units
        .iter()
        .map(|u| {
            source
                .chars()
                .skip(u.source.start)
                .take(u.source.len())
                .collect::<String>()
        })
        .collect::<String>();
    assert_eq!(pieces, source);
    let target = translated
        .targets
        .iter()
        .map(|r| {
            translated
                .text
                .chars()
                .skip(r.start)
                .take(r.len())
                .collect::<String>()
        })
        .collect::<String>();
    assert_eq!(target, translated.text);
    assert_eq!(plan.units.len(), translated.targets.len());
    assert!(translated
        .targets
        .windows(2)
        .all(|r| r[0].end == r[1].start));
}

/// Partial emphasis must not split a sentence; copied code and its receipt coordinates stay exact.
#[test]
fn sentence_entry_keeps_decoder_context_copied_literals_and_review_decisions() {
    for (sentence, literal, review) in [
        ("<em>Il ne vient pas.</em>", "X < 2", false),
        ("Il <em>ne vient</em> pas.", "X < 2", true),
        // A lowercase continuation is not a natural sentence edge; retain the conservative review decision.
        ("<em>Il ne vient pas.</em>", "x < 2", true),
    ] {
        let (source, segment) = fixture(&format!(
            "<p>{sentence} <code id='literal'>{}</code> Merci.</p>",
            literal.replace('<', "&lt;")
        ));
        let protected = source
            .fragment(0..segment.source_end, &segment.text)
            .unwrap();
        let mut translator = Translator::new(100);
        let plan = SourcePlan::from_sentences(&segment, protected, &translator).unwrap();
        assert_eq!(plan.formatting_needs_review(), review);
        assert_eq!(
            serde_json::to_value(plan.units()).unwrap(),
            serde_json::json!([
                {"kind":"Prose", "source":{"start":0,"end":16}},
                {"kind":"Whitespace", "source":{"start":16,"end":17}},
                {"kind":{"Literal":1}, "source":{"start":17,"end":22}},
                {"kind":"Whitespace", "source":{"start":22,"end":23}},
                {"kind":"Prose", "source":{"start":23,"end":29}}
            ])
        );
        let translated = plan.translate(&mut translator, || false).unwrap();
        assert_eq!(translator.calls, [vec!["Il ne vient pas.", "Merci."]]);
        assert_eq!(
            translated.text,
            format!("IL NE VIENT PAS. {literal} MERCI.")
        );
        coverage(&plan, &translated);
        if !review {
            let markup = plan.protected.boundary_output(&translated);
            let restored = plan.protected.restore_boundaries(&markup.unwrap()).unwrap();
            assert_eq!(restored.text, translated.text);
            assert!(restored.html.contains("<em>IL NE VIENT PAS.</em>"));
            assert!(restored.html.contains("id=\"literal\""));
        }
    }
}

/// Moving planning must retain decoder rejection and discard a native reply after cancellation.
#[test]
fn sentence_entry_rejects_bad_decoder_replies_and_late_cancellation() {
    let (source, segment) = fixture("<p>Un deux. <code>private_id</code></p>");
    let protected = source
        .fragment(0..segment.source_end, &segment.text)
        .unwrap();
    let mut translator = Translator::new(100);
    let plan = SourcePlan::from_sentences(&segment, protected, &translator).unwrap();
    for fault in ["count", "empty", "large"] {
        translator.fault = fault;
        assert!(plan.translate(&mut translator, || false).is_err());
    }
    translator.fault = "";
    let checks = Cell::new(0);
    let error = plan.translate(&mut translator, || {
        let n = checks.get();
        checks.set(n + 1);
        n > 0
    });
    assert!(error.err().unwrap().contains("cancelled"));
    assert_eq!(translator.calls, vec![vec!["Un deux."]; 4]);
}

/// Literal-only content, including its image attachment, never needs a model or tokenizer.
#[test]
fn copied_literals_and_images_need_no_runtime_and_keep_source_ownership() {
    struct NoRuntime;
    impl ExactPieceTranslator for NoRuntime {
        fn source_fits(&self, _: &str) -> Result<bool, String> {
            panic!("literal was tokenized")
        }
        fn translate_exact(&mut self, _: &[&str]) -> Result<Vec<String>, String> {
            panic!("literal was translated")
        }
    }
    let (source, segment) =
        fixture("<p><code id='literal'>private🙂</code><img src='owned.png'></p>");
    let protected = source
        .fragment(0..segment.source_end, &segment.text)
        .unwrap();
    let plan = SourcePlan::from_sentences(&segment, protected, &NoRuntime).unwrap();
    assert!(!plan.formatting_needs_review());
    let result = plan.translate(&mut NoRuntime, || false).unwrap();
    assert_eq!(result.text, segment.text);
    coverage(&plan, &result);
    let restored = plan
        .protected
        .restore_boundaries(&plan.protected.boundary_output(&result).unwrap())
        .unwrap();
    assert!(restored.html.contains("id=\"literal\""));
    assert!(restored.html.contains("src=\"owned.png\""));
}

/// A planner move/removal must not turn an indivisible word or combining mark into an unsafe split.
#[test]
fn decoder_limits_and_source_target_graphemes_still_reject_unsafe_units() {
    let mut translator = Translator::new(100);
    for html in [
        "<p><code>e</code>́ x</p>",
        "<p>A <code>x</code> ́b</p>",
        &format!("<p>{}</p>", "x".repeat(901)),
    ] {
        let (source, segment) = fixture(html);
        let protected = source
            .fragment(0..segment.source_end, &segment.text)
            .unwrap();
        assert!(SourcePlan::from_sentences(&segment, protected, &translator).is_err());
    }
    let (source, segment) = fixture("<p><code>e</code>b</p>");
    let protected = source
        .fragment(0..segment.source_end, &segment.text)
        .unwrap();
    let plan = SourcePlan::from_sentences(&segment, protected, &translator).unwrap();
    translator.fault = "combining";
    assert!(plan
        .translate(&mut translator, || false)
        .err()
        .unwrap()
        .contains("grapheme"));
}
