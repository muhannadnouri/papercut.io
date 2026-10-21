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

fn admit(_: &str, _: Side) -> Result<bool, String> {
    Ok(true)
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
        let translated = plan.translate(&mut translator, admit, || false).unwrap();
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
        assert!(plan.translate(&mut translator, admit, || false).is_err());
    }
    translator.fault = "";
    let checks = Cell::new(0);
    let error = plan.translate(&mut translator, admit, || {
        let n = checks.get();
        checks.set(n + 1);
        n > 0
    });
    assert!(error.err().unwrap().contains("cancelled"));
    assert_eq!(translator.calls, vec![vec!["Un deux."]; 4]);
}

#[test]
fn emphasis_and_authored_identity_do_not_split_a_fitting_sentence() {
    let (source, segment) = fixture("<p><a id='identity' href='#chapter'>La <strong><em>voiture</em> rouge</strong></a> est ici.</p>");
    let mut translator = Translator::new(100);
    let plan = SourcePlan::new(&segment, &source, &translator, admit).unwrap();
    assert_eq!(plan.units.len(), 1);
    assert_eq!(plan.segment_id, "b7:s2");
    assert_eq!(plan.source_ordinal, 7);
    let translated = plan.translate(&mut translator, admit, || false).unwrap();
    assert_eq!(translator.calls, [vec![segment.text]]);
    coverage(&plan, &translated);
}

#[test]
fn literal_payloads_never_enter_inference_and_boundary_attachments_have_one_owner() {
    let (source, segment) = fixture("<p><a role='doc-noteref' href='#start'>0</a>Gardez <code id='code'>secret_user_id</code> puis <span translate='no'>42</span><img src='asset.png'>.</p>");
    let mut translator = Translator::new(100);
    let plan = SourcePlan::new(&segment, &source, &translator, admit).unwrap();
    let identity = plan.protected.identity();
    let translated = plan.translate(&mut translator, admit, || false).unwrap();
    assert_eq!(translated.text, "GARDEZ secret_user_id PUIS 42.");
    let requests = translator.calls.concat().join("|");
    for hidden in ["secret_user_id", "42", "asset.png", "#start", "PC"] {
        assert!(!requests.contains(hidden));
    }
    assert_eq!(plan.attachments.len(), 2);
    assert_eq!(plan.attachments[0].owner, 0);
    let image = &plan.attachments[1];
    assert_eq!(plan.units[image.owner].source.end, image.offset);
    assert_eq!(plan.protected.identity(), identity);
    coverage(&plan, &translated);
}

#[test]
fn tokenizer_subdivisions_keep_identity_tree_and_cut_notes_once() {
    let (source, segment) = fixture("<p><span id='one'>Un deux;<a role='doc-noteref' href='#n'>1</a> Trois quatre; Cinq six.</span></p>");
    let mut translator = Translator::new(3); // Two lexical tokens plus framing.
    let plan = SourcePlan::new(&segment, &source, &translator, admit).unwrap();
    let codes = plan.protected.source_codes().collect::<Vec<_>>();
    assert_eq!(codes[0].range, 0..segment.source_end); // The authored wrapper is never clipped into piece identities.
    let note = &plan.attachments[0];
    assert_eq!(plan.units[note.owner].source.end, note.offset);
    assert_eq!(note.parent, Some(codes[0].id));
    assert_eq!(
        plan.units.iter().filter(|u| u.kind == Kind::Prose).count(),
        3
    );
    let translated = plan.translate(&mut translator, admit, || false).unwrap();
    assert_eq!(
        translator.calls[0],
        ["Un deux;", "Trois quatre;", "Cinq six."]
    );
    coverage(&plan, &translated);
}

#[test]
fn leading_trailing_whitespace_and_multiscalar_unicode_use_exact_parent_offsets() {
    let (source, mut segment) = fixture("<p>A café é عَرَبِيّ B</p>");
    segment.source_start = 1;
    segment.source_end -= 1;
    segment.text = segment
        .text
        .chars()
        .skip(1)
        .take(segment.source_end - 1)
        .collect();
    let mut translator = Translator::new(100);
    translator.fault = "padding";
    let plan = SourcePlan::new(&segment, &source, &translator, admit).unwrap();
    assert_eq!(plan.source_start, 1);
    assert_eq!(plan.units[0].kind, Kind::Whitespace);
    assert_eq!(plan.units.last().unwrap().kind, Kind::Whitespace);
    let translated = plan.translate(&mut translator, admit, || false).unwrap();
    assert!(translated.text.starts_with("   CAFÉ"));
    assert!(translated.text.ends_with("🙂  "));
    coverage(&plan, &translated);
}

#[test]
fn both_source_budgets_and_900_scalar_cap_apply_without_word_or_grapheme_chopping() {
    let (source, segment) = fixture(&format!("<p>{}</p>", "été ".repeat(240)));
    let mut translator = Translator::new(1000);
    let aligner = |s: &str, _: Side| Ok(s.split_whitespace().count() + 2 <= 82);
    let plan = SourcePlan::new(&segment, &source, &translator, aligner).unwrap();
    assert!(plan.units.iter().filter(|u| u.kind == Kind::Prose).count() >= 3);
    assert!(plan.units.iter().all(|u| u.source.len() <= 900));
    let translated = plan.translate(&mut translator, aligner, || false).unwrap();
    coverage(&plan, &translated);
    for text in ["x".repeat(901), "é".repeat(500)] {
        let (source, segment) = fixture(&format!("<p>{text}</p>"));
        assert!(SourcePlan::new(&segment, &source, &translator, admit).is_err());
    }
    let (source, segment) = fixture(&format!("<p>{}</p>", "x".repeat(900)));
    assert_eq!(
        SourcePlan::new(&segment, &source, &translator, admit)
            .unwrap()
            .units
            .len(),
        1
    );
}

#[test]
fn over_limit_targets_bad_reply_counts_and_cancellation_reject_without_independent_target_splitting(
) {
    let (source, segment) = fixture("<p>Un deux.</p>");
    for fault in ["count", "empty", "large", "target"] {
        let mut translator = Translator::new(100);
        translator.fault = fault;
        let aligner = |s: &str, _: Side| Ok(s.split_whitespace().count() + 2 <= 6);
        let plan = SourcePlan::new(&segment, &source, &translator, aligner).unwrap();
        assert!(plan.translate(&mut translator, aligner, || false).is_err());
        assert_eq!(translator.calls.len(), 1);
    }
    let mut translator = Translator::new(100);
    let plan = SourcePlan::new(&segment, &source, &translator, admit).unwrap();
    assert!(plan
        .translate(&mut translator, admit, || true)
        .err()
        .unwrap()
        .contains("cancelled"));
    assert!(translator.calls.is_empty());
    let checkpoints = Cell::new(0);
    assert!(plan
        .translate(&mut translator, admit, || {
            let n = checkpoints.get();
            checkpoints.set(n + 1);
            n >= 1
        })
        .err()
        .unwrap()
        .contains("cancelled"));
    assert_eq!(translator.calls.len(), 1);
}

#[test]
fn batching_and_piece_count_are_bounded_and_previous_public_identity_cuts_reject() {
    let (source, segment) = fixture(&format!("<p>{}</p>", "mot ".repeat(40)));
    let mut translator = Translator::new(2);
    let plan = SourcePlan::new(&segment, &source, &translator, admit).unwrap();
    let translated = plan.translate(&mut translator, admit, || false).unwrap();
    assert!(translator.calls.len() > 1);
    assert!(translator
        .calls
        .iter()
        .all(|b| b.len() <= DEFAULT_BATCH_SEGMENT_LIMIT));
    coverage(&plan, &translated);
    let (source, segment) = fixture(&format!("<p>{}</p>", "mot ".repeat(200)));
    assert!(SourcePlan::new(&segment, &source, &translator, admit).is_err());
    let (source, mut segment) = fixture("<p><span id='owned'>Un deux trois</span></p>");
    segment.source_end = 2;
    segment.text = "Un".into();
    assert!(SourcePlan::new(&segment, &source, &translator, admit)
        .err()
        .unwrap()
        .contains("boundary"));
}

/// DOM joins consume metadata, not decoder work; admission must still stop before a 129th MT piece.
#[test]
fn boundary_budgets_separate_translated_pieces_from_joins_and_leave_strict_limits_unchanged() {
    let mut translator = Translator::new(100);
    for count in [64, 65] {
        let (source, segment) =
            fixture(&format!("<p>{}</p>", "<em>Mot.</em> Suite. ".repeat(count)));
        let protected = source
            .fragment(0..segment.source_end, &segment.text)
            .unwrap();
        let plan = SourcePlan::from_sentences(&segment, protected, &translator);
        if count == 65 {
            let error = plan.err().unwrap();
            assert!(
                error.contains("translation piece bound")
                    || error.contains("planning exceeds its work bound"),
                "{error}"
            );
            assert_eq!(translator.calls.iter().map(Vec::len).sum::<usize>(), 128);
            continue;
        }
        let plan = plan.unwrap();
        assert!(plan.units.len() > 128);
        assert_eq!(
            plan.units.iter().filter(|u| u.kind == Kind::Prose).count(),
            128
        );
        let translated = plan.translate(&mut translator, admit, || false).unwrap();
        coverage(&plan, &translated);
        assert_eq!(translator.calls.iter().map(Vec::len).sum::<usize>(), 128);
        assert!(translator
            .calls
            .iter()
            .all(|b| b.len() <= DEFAULT_BATCH_SEGMENT_LIMIT));
    }
    let (source, segment) = fixture(&format!("<p>{}</p>", "mot ".repeat(70)));
    let translator = Translator::new(2);
    assert!(SourcePlan::new(&segment, &source, &translator, admit).is_err());
    let protected = source
        .fragment(0..segment.source_end, &segment.text)
        .unwrap();
    let plan = SourcePlan::from_sentences(&segment, protected, &translator).unwrap();
    assert!(plan.units.len() > 128);
    assert_eq!(
        plan.units.iter().filter(|u| u.kind == Kind::Prose).count(),
        70
    );
}

#[test]
fn literal_only_segments_need_no_inference_and_unknown_prose_never_becomes_a_literal() {
    let (source, segment) = fixture("<p><code>unrecognized🙂</code><img src='owned.png'></p>");
    let mut translator = Translator::new(2);
    let plan = SourcePlan::new(&segment, &source, &translator, |_, _| {
        Err("should not tokenize a literal".into())
    })
    .unwrap();
    let translated = plan
        .translate(
            &mut translator,
            |_, _| Err("should not tokenize a literal".into()),
            || false,
        )
        .unwrap();
    assert_eq!(translated.text, segment.text);
    assert!(translator.calls.is_empty());
    coverage(&plan, &translated);
    let (source, segment) = fixture("<p>Unknown name inside ordinary prose.</p>");
    assert!(SourcePlan::new(&segment, &source, &translator, |_, _| Err(
        "unknown token".into()
    ))
    .is_err());
}

#[test]
fn literal_whitespace_and_target_joins_never_cut_inside_a_grapheme() {
    let mut translator = Translator::new(100);
    for html in ["<p><code>e</code>́ x</p>", "<p>A <code>x</code> ́b</p>"] {
        let (source, segment) = fixture(html);
        assert!(SourcePlan::new(&segment, &source, &translator, admit).is_err());
    }
    let (source, segment) = fixture("<p><code>e</code>b</p>");
    let plan = SourcePlan::new(&segment, &source, &translator, admit).unwrap();
    translator.fault = "combining";
    assert!(plan
        .translate(&mut translator, admit, || false)
        .err()
        .unwrap()
        .contains("grapheme"));
}

#[test]
fn note_at_exact_900_scalar_cut_keeps_one_owner_and_one_enclosing_authored_identity() {
    let (source, segment) = fixture(&format!(
        "<p><span id='whole'>{}.<a role='doc-noteref' href='#n'>1</a> Fin.</span></p>",
        "a".repeat(899)
    ));
    let mut translator = Translator::new(1000);
    let plan = SourcePlan::new(&segment, &source, &translator, admit).unwrap();
    assert_eq!(plan.attachments.len(), 1);
    let note = &plan.attachments[0];
    assert_eq!(note.offset, 900);
    assert_eq!(plan.units[note.owner].source.end, 900);
    assert_eq!(note.parent, Some(0));
    assert_eq!(
        plan.protected.source_codes().filter(|c| c.id == 0).count(),
        1
    );
    assert_eq!(plan.protected.source_codes().next().unwrap().range, 0..905);
    let translated = plan.translate(&mut translator, admit, || false).unwrap();
    assert_eq!(translator.calls[0].len(), 2);
    assert_eq!(plan.segment_id, segment.id);
    coverage(&plan, &translated);
}
