//! Small synthetic vocabularies exercise the real tokenizer library; no pretrained assets live in Git.
use super::*;
use serde_json::{json, Value};

/// A synthetic BERT profile with sparse, reserved IDs matching the candidate's framing contract.
fn profile() -> Value {
    let mut vocab = serde_json::Map::new();
    for (word, id) in [
        ("[PAD]", 0),
        ("[UNK]", 100),
        ("[CLS]", 101),
        ("[SEP]", 102),
        ("[MASK]", 103),
        ("Bonjour", 200),
        ("monde", 201),
        (".", 202),
        ("café", 203),
        ("cafe", 204),
        ("##́", 205),
        ("عَرَبِيّ", 206),
        ("book", 207),
        ("Un", 208),
        ("livre", 209),
        ("A", 210),
    ] {
        vocab.insert(word.into(), json!(id));
    }
    json!({"version":"1.0", "truncation":null,"padding":null,
        "added_tokens":[
            {"id":0,"content":"[PAD]","single_word":false,"lstrip":false,"rstrip":false,"normalized":false,"special":true},
            {"id":100,"content":"[UNK]","single_word":false,"lstrip":false,"rstrip":false,"normalized":false,"special":true},
            {"id":101,"content":"[CLS]","single_word":false,"lstrip":false,"rstrip":false,"normalized":false,"special":true},
            {"id":102,"content":"[SEP]","single_word":false,"lstrip":false,"rstrip":false,"normalized":false,"special":true},
            {"id":103,"content":"[MASK]","single_word":false,"lstrip":false,"rstrip":false,"normalized":false,"special":true}],
        "normalizer":{"type":"BertNormalizer","clean_text":true,"handle_chinese_chars":true,"strip_accents":null,"lowercase":false},
        "pre_tokenizer":{"type":"BertPreTokenizer"},
        "post_processor":{"type":"BertProcessing","sep":["[SEP]",102],"cls":["[CLS]",101]},
        "decoder":{"type":"WordPiece","prefix":"##","cleanup":true},
        "model":{"type":"WordPiece","unk_token":"[UNK]","continuing_subword_prefix":"##","max_input_chars_per_word":100,"vocab":vocab}})
}

pub(super) fn synthetic() -> CandidateTokenizer {
    CandidateTokenizer::from_snapshot(&serde_json::to_vec(&profile()).unwrap()).unwrap()
}

#[test]
fn planning_splits_only_size_overflow_and_does_not_hide_unknown_or_normalization_failures() {
    let tokenizer = synthetic();
    for side in [Side::Source, Side::Target] {
        assert!(tokenizer
            .text_fits(&vec!["book"; MAX_TOKENS - 2].join(" "), side)
            .unwrap());
        assert!(!tokenizer
            .text_fits(&vec!["book"; MAX_TOKENS - 1].join(" "), side)
            .unwrap());
        for text in ["🙂", "[UNK]", "ca\u{200b}fe", ""] {
            assert!(tokenizer.text_fits(text, side).is_err());
        }
    }
}

#[test]
fn exact_piece_planning_uses_real_tokenizer_framing_and_keeps_unknown_literals_outside_it() {
    use crate::translation::{
        dom::DomPlan,
        inline_markup::{
            protected::ProtectedSource,
            provider::pieces::{ExactPieceTranslator, SourcePlan},
        },
        segment::TranslationTextSegment,
    };
    struct Translator;
    impl ExactPieceTranslator for Translator {
        fn source_fits(&self, s: &str) -> Result<bool, String> {
            Ok(s.split_whitespace().count() + 1 <= 448)
        }
        fn translate_exact(&mut self, s: &[&str]) -> Result<Vec<String>, String> {
            assert!(s.iter().all(|s| !s.contains("secret")));
            Ok(s.iter().map(|_| "A book".into()).collect())
        }
    }
    let dom = DomPlan::new(&format!(
        "<p>{}<code>secret🙂</code> book</p>",
        "book ".repeat(260)
    ));
    let block = &dom.blocks[0];
    let source = ProtectedSource::new(&block.node).unwrap();
    let segment = TranslationTextSegment {
        id: "b0:s0".into(),
        source_block_index: 0,
        source_start: 0,
        source_end: block.inline.text.chars().count(),
        text: block.inline.text.clone(),
    };
    let tokenizer = synthetic();
    let fits = |s: &str, side| {
        tokenizer
            .text_fits(s, side)
            .map_err(|_| "Alignment token admission failed".into())
    };
    let plan = SourcePlan::new(&segment, &source, &Translator, fits).unwrap();
    let result = plan.translate(&mut Translator, fits, || false).unwrap();
    assert!(result.text.contains("secret🙂"));
    assert!(plan.units().len() > 3);
}

/// The 23 generic admission patterns are independent of any source article or translation sample.
fn policy_cases() -> Vec<(String, bool)> {
    let mut cases: Vec<_> = [
        ("Bonjour monde.", true),
        ("café", true),
        ("cafe\u{301}", true),
        ("عَرَبِيّ", true),
        ("Bonjour\tmonde", true),
        ("🙂", false),
        ("🚀", false),
        ("👩‍👩‍👧‍👦", false),
        ("\u{e000}", false),
        ("\u{200f}كتاب\u{200e}", false),
        ("می\u{200c}روم", false),
        ("ca\u{200b}fe", false),
        ("co\u{ad}operate", false),
        ("a\0b", false),
        ("a\u{fffd}b", false),
        ("x\u{2066}y\u{2069}", false),
        ("a\u{1c}b", false),
        ("[CLS]", false),
        ("[SEP]", false),
        ("[MASK]", false),
        ("[PAD]", false),
        ("[UNK]", false),
    ]
    .into_iter()
    .map(|(text, pass)| (text.into(), pass))
    .collect();
    cases.push(("a".repeat(101), false));
    cases
}

fn assert_policy(adapter: &CandidateTokenizer) {
    let cases = policy_cases();
    assert_eq!(cases.len(), 23);
    assert_eq!(cases.iter().filter(|(_, pass)| *pass).count(), 5);
    for (index, (text, pass)) in cases.iter().enumerate() {
        for side in [Side::Source, Side::Target] {
            // Report only fixture index, not retained raw text, on unexpected tokenization.
            assert_eq!(
                adapter.encode(text, side).is_ok(),
                *pass,
                "case {index}, {side:?}"
            );
        }
    }
}

#[test]
fn actual_raw_tokenization_enforces_all_23_admission_patterns_on_both_sides() {
    assert_policy(&synthetic());
}

#[test]
fn original_utf8_word_ids_and_combining_subwords_survive_without_resplitting() {
    let adapter = synthetic();
    let text = "cafe\u{301}\tbook";
    let encoded = adapter.encode(text, Side::Source).unwrap();
    assert_eq!(encoded.text(), text);
    assert_eq!(encoded.normalized, "cafe\u{301} book");
    assert_eq!(
        encoded.tokens.iter().map(|t| t.id).collect::<Vec<_>>(),
        [101, 204, 205, 207, 102]
    );
    assert_eq!(
        encoded.tokens.iter().map(|t| t.word).collect::<Vec<_>>(),
        [None, Some(0), Some(0), Some(1), None]
    );
    assert_eq!(encoded.tokens[2].bytes, 4..6);
    assert_eq!(encoded.tokens[3].bytes, 7..11);
    let authored = adapter.encode("[CLS]", Side::Source).err().unwrap();
    assert!(matches!(
        authored,
        AdmissionError::Evidence(Rejection::ReservedToken(_) | Rejection::Framing(_))
    ));
    // A removed NUL can hide inside an otherwise full token envelope; normalization must still reject it.
    assert_eq!(
        adapter.encode("bo\0ok", Side::Target).err(),
        Some(AdmissionError::Evidence(Rejection::Normalization(
            Side::Target
        )))
    );
}

#[test]
fn exact_token_and_text_limits_reject_instead_of_silent_truncation() {
    let adapter = synthetic();
    let at_limit = vec!["book"; MAX_TOKENS - 2].join(" ");
    assert_eq!(
        adapter
            .encode(&at_limit, Side::Source)
            .unwrap()
            .tokens
            .len(),
        MAX_TOKENS
    );
    let too_many = vec!["book"; MAX_TOKENS - 1].join(" ");
    assert_eq!(
        adapter.encode(&too_many, Side::Target).err(),
        Some(AdmissionError::Evidence(Rejection::Limit))
    );
    for value in ["book ".repeat(MAX_TEXT_BYTES), " ".into()] {
        assert!(adapter.encode(&value, Side::Source).is_err());
    }
}

#[test]
fn changed_profiles_framing_padding_truncation_and_errors_do_not_bypass_admission() {
    for change in 0..4 {
        let mut p = profile();
        match change {
            0 => {
                p["padding"] = json!({"strategy":"BatchLongest","direction":"Right","pad_to_multiple_of":null,"pad_id":0,"pad_type_id":0,"pad_token":"[PAD]"})
            }
            1 => {
                p["truncation"] =
                    json!({"direction":"Right","max_length":8,"strategy":"LongestFirst","stride":0})
            }
            2 => p["normalizer"] = Value::Null,
            _ => p["model"]["vocab"]["[UNK]"] = json!(99),
        }
        assert!(CandidateTokenizer::from_snapshot(&serde_json::to_vec(&p).unwrap()).is_err());
    }
    let mut p = profile();
    p["post_processor"] = Value::Null;
    let adapter = CandidateTokenizer::from_snapshot(&serde_json::to_vec(&p).unwrap()).unwrap();
    assert!(adapter.encode("book", Side::Source).is_err());
    assert!(CandidateTokenizer::from_snapshot(b"private source content").is_err());
    assert_eq!(format!("{:?}", AdmissionError::Profile), "Profile");
}

#[test]
fn real_tokenizer_evidence_binds_to_pair_strings_and_policy_versions() {
    use crate::translation::{dom::DomPlan, inline_markup::protected::ProtectedSource};
    use contract::{NodeMapping, Versions};
    let adapter = synthetic();
    let safe = crate::document_uploads::sanitize_html("<p>Un <em>livre</em></p>");
    let dom = DomPlan::new(&safe);
    let block = &dom.blocks[0];
    let plan = ProtectedSource::new(&block.node)
        .unwrap()
        .fragment(0..block.inline.text.chars().count(), &block.inline.text)
        .unwrap();
    let versions = Versions {
        translator: "mt@one".into(),
        aligner: format!("{}@{}", assets::REPOSITORY, assets::REVISION),
        tokenizer: adapter.identity().into(),
        normalization: "raw-v1".into(),
        policy: "structural-test-only".into(),
    };
    let pair = Pair {
        plan: &plan,
        segment_id: "unit-1",
        source_ordinal: 0,
        source_start: 0,
        target_start: 0,
        target: "A book",
        versions: &versions,
    };
    let mut reply = Reply {
        binding: pair.binding().unwrap(),
        links: vec![(0, 0), (1, 1)],
        nodes: vec![NodeMapping {
            source_id: 0,
            targets: vec![2..6],
        }],
        unresolved: None,
    };
    assert_eq!(adapter.check_pair(&pair, &reply).unwrap().nodes().len(), 1);
    reply.binding.target.push(' ');
    assert_eq!(
        adapter.check_pair(&pair, &reply).err(),
        Some(AdmissionError::Evidence(Rejection::Binding))
    );
    let mut wrong = versions.clone();
    wrong.tokenizer = "unverified-tokenizer".into();
    let wrong_pair = Pair {
        versions: &wrong,
        ..pair
    };
    assert_eq!(
        adapter.check_pair(&wrong_pair, &reply).err(),
        Some(AdmissionError::Evidence(Rejection::TokenProfile))
    );
}

#[test]
#[ignore = "requires the exact external pinned candidate pack; never downloads assets"]
fn pinned_external_pack_passes_real_tokenizer_admission_without_inference() {
    let dir = std::env::var_os("PAPERCUT_ALIGNMENT_ASSETS_DIR")
        .expect("Set PAPERCUT_ALIGNMENT_ASSETS_DIR to an external pinned pack");
    let adapter = CandidateTokenizer::load(Path::new(&dir)).unwrap();
    assert_policy(&adapter);
    // Independent immutable IDs recorded during feasibility; these are not supplied token fixtures.
    for (text, expected) in [
        (
            "Bonjour monde.",
            vec![101, 30120, 98214, 10129, 13111, 119, 102],
        ),
        ("café", vec![101, 34551, 102]),
        ("cafe\u{301}", vec![101, 11135, 14601, 82091, 102]),
        (
            "عَرَبِيّ",
            vec![101, 781, 23112, 10673, 23112, 11086, 93510, 18519, 102],
        ),
    ] {
        let encoded = adapter.encode(text, Side::Source).unwrap();
        assert_eq!(
            encoded.tokens.iter().map(|t| t.id).collect::<Vec<_>>(),
            expected
        );
    }
    let text = vec!["book"; MAX_TOKENS - 2].join(" ");
    assert_eq!(
        adapter.encode(&text, Side::Source).unwrap().tokens.len(),
        MAX_TOKENS
    );
    assert!(adapter
        .encode(&format!("{text} book"), Side::Source)
        .is_err());
}
