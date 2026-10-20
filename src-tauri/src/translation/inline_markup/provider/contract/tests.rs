//! Synthetic contract fixtures only: supplied gold links are not model-quality evidence.
use super::*;
use crate::translation::{dom::DomPlan, inline_markup::protected::ProtectedSource};

fn versions() -> Versions {
    Versions {
        translator: "mt@one".into(),
        aligner: "aligner@one".into(),
        tokenizer: "tokens@one".into(),
        normalization: "raw-reader-v1".into(),
        policy: "softmax-.001/project-v1".into(),
    }
}

fn rules() -> TokenRules<'static> {
    TokenRules {
        identity: "tokens@one",
        unknown: 100,
        reserved: &[0, 100, 101, 102, 103],
        prefix: 101,
        suffix: 102,
    }
}

/// Reuse the real sanitized reader/identity plan; tests never invent an independent DOM tree.
fn plan(html: &str) -> ProtectedPlan {
    let safe = crate::document_uploads::sanitize_html(html);
    let dom = DomPlan::new(&safe);
    let block = &dom.blocks[0];
    ProtectedSource::new(&block.node)
        .unwrap()
        .fragment(0..block.inline.text.chars().count(), &block.inline.text)
        .unwrap()
}

fn pair<'a>(plan: &'a ProtectedPlan, target: &'a str, versions: &'a Versions) -> Pair<'a> {
    Pair {
        plan,
        target,
        versions,
        segment_id: "unit-1",
        source_ordinal: 3,
        source_start: 7,
        target_start: 11,
    }
}

/// A deterministic word-token fixture, not an imitation of the candidate's neural tokenizer.
fn tokens(text: &str) -> Vec<Token> {
    let mut result = vec![Token {
        id: 101,
        word: None,
        bytes: 0..0,
        special: true,
    }];
    for (word, (start, value)) in text
        .split_word_bound_indices()
        .filter(|(_, s)| !s.trim().is_empty())
        .enumerate()
    {
        result.push(Token {
            id: 1000 + word as u32,
            word: Some(word),
            bytes: start..start + value.len(),
            special: false,
        });
    }
    result.push(Token {
        id: 102,
        word: None,
        bytes: 0..0,
        special: true,
    });
    result
}

fn mapping(id: usize, targets: Vec<Range<usize>>) -> NodeMapping {
    NodeMapping {
        source_id: id,
        targets,
    }
}

fn reply(pair: &Pair<'_>, nodes: Vec<NodeMapping>, links: &[(usize, usize)]) -> Reply {
    Reply {
        binding: pair.binding().unwrap(),
        nodes,
        links: links.into(),
        unresolved: None,
    }
}

/// Exercise the full pure boundary with retained source text on both tokenizer evidence sides.
fn check<'a>(p: &Pair<'_>, reply: &'a Reply) -> Result<CheckedMapping<'a>, Rejection> {
    let source = tokens(p.plan.source_text());
    let target = tokens(p.target);
    p.check(
        &rules(),
        &TokenizedText {
            normalized: p.plan.source_text(),
            tokens: &source,
        },
        &TokenizedText {
            normalized: p.target,
            tokens: &target,
        },
        reply,
    )
}

#[test]
fn binds_exact_text_offsets_dom_and_all_runtime_policy_revisions() {
    let plan = plan("<p>Un <em>livre</em></p>");
    let versions = versions();
    let p = pair(&plan, "A book", &versions);
    let good = reply(&p, vec![mapping(0, vec![2..6])], &[(0, 0), (1, 1)]);
    assert!(check(&p, &good).is_ok());
    for change in 0..13 {
        let mut bad = good.clone();
        match change {
            0 => bad.binding.source.push(' '),
            1 => bad.binding.target.push(' '),
            2 => bad.binding.source_plan.push('x'),
            3 => bad.binding.source_ordinal += 1,
            4 => bad.binding.source_range.start += 1,
            5 => bad.binding.target_range.end += 1,
            6 => bad.binding.segment_id.push('x'),
            7 => bad.binding.version += 1,
            8 => bad.binding.versions.translator.push('x'),
            9 => bad.binding.versions.aligner.push('x'),
            10 => bad.binding.versions.tokenizer.push('x'),
            11 => bad.binding.versions.normalization.push('x'),
            _ => bad.binding.versions.policy.push('x'),
        }
        assert_eq!(
            check(&p, &bad).unwrap_err(),
            Rejection::Binding,
            "change {change}"
        );
    }
    let moved = Pair {
        target_start: usize::MAX,
        ..p
    };
    assert_eq!(moved.binding().unwrap_err(), Rejection::Binding);
}

#[test]
fn identity_binds_attributes_and_literals_even_with_identical_readable_text() {
    let a = plan("<p><a href='#a'>Mot</a></p>");
    let b = plan("<p><a href='#b'>Mot</a></p>");
    let v = versions();
    let old = pair(&a, "Word", &v);
    let new = pair(&b, "Word", &v);
    let output = reply(&old, vec![mapping(0, vec![0..4])], &[]);
    assert_eq!(check(&new, &output).unwrap_err(), Rejection::Binding);
}

#[test]
fn supports_reordered_nested_nodes_and_disjoint_many_to_many_gold_links() {
    let source = plan("<p><strong><em>rouge</em> livre</strong> ici</p>");
    let v = versions();
    let p = pair(&source, "book red here", &v);
    let output = reply(
        &p,
        vec![mapping(1, vec![5..8]), mapping(0, vec![0..8])],
        &[(0, 1), (1, 0), (2, 2)],
    );
    assert_eq!(check(&p, &output).unwrap().nodes, output.nodes);

    let source = plan("<p>Il <strong>travaille</strong> encore</p>");
    let p = pair(&source, "He is still working", &v);
    let output = reply(
        &p,
        vec![mapping(0, vec![3..5, 12..19])],
        &[(0, 0), (1, 1), (1, 3), (2, 2)],
    );
    assert!(check(&p, &output).is_ok());
    let mut hull = output.clone();
    hull.nodes[0].targets = vec![3..19];
    assert_eq!(check(&p, &hull).unwrap_err(), Rejection::ScopeMismatch);
}

#[test]
fn whole_unit_ownership_includes_auxiliaries_without_any_word_guessing() {
    let source = plan("<p><strong><em>Il reviendra demain.</em></strong></p>");
    let v = versions();
    let p = pair(&source, "He will return tomorrow.", &v);
    let full = 0..p.target.chars().count();
    let output = reply(
        &p,
        vec![mapping(0, vec![full.clone()]), mapping(1, vec![full])],
        &[],
    );
    assert!(check(&p, &output).is_ok());
    let mut clipped = output.clone();
    clipped.nodes[1].targets[0].start = 8;
    assert_eq!(check(&p, &clipped).unwrap_err(), Rejection::ScopeMismatch);
}

#[test]
fn rejects_missing_auxiliary_scope_and_never_turns_rejection_into_positive_acceptance() {
    let source = plan("<p>Il <em>reviendra</em> demain.</p>");
    let v = versions();
    let p = pair(&source, "He will return tomorrow.", &v);
    let output = reply(
        &p,
        vec![mapping(0, vec![8..14])],
        &[(0, 0), (1, 2), (2, 3), (3, 4)],
    );
    assert_eq!(check(&p, &output).unwrap_err(), Rejection::UnresolvedTarget);
    let mut guessed = output.clone();
    guessed.nodes[0].targets = vec![3..14];
    assert_eq!(
        check(&p, &guessed).unwrap_err(),
        Rejection::UnresolvedTarget
    );
}

#[test]
fn keeps_explicit_auxiliaries_outside_main_verb_and_negation_only_scopes() {
    let source = plan("<p>Elle a <em>terminé</em></p>");
    let v = versions();
    let p = pair(&source, "She has finished", &v);
    let output = reply(&p, vec![mapping(0, vec![8..16])], &[(0, 0), (1, 1), (2, 2)]);
    assert!(check(&p, &output).is_ok());
    let mut expanded = output.clone();
    expanded.nodes[0].targets = vec![4..16];
    assert_eq!(check(&p, &expanded).unwrap_err(), Rejection::ScopeMismatch);

    let source = plan("<p>Elle <em>ne</em> vient <em>pas</em></p>");
    let p = pair(&source, "She is not coming", &v);
    let output = reply(
        &p,
        vec![mapping(0, vec![7..10]), mapping(1, vec![7..10])],
        &[(0, 0), (1, 2), (2, 1), (2, 3), (3, 2)],
    );
    assert!(matches!(
        check(&p, &output),
        Err(Rejection::Hierarchy | Rejection::MixedOwnership)
    ));
    // Distinct inline identities cannot both be duplicated onto a fused target word.
    let source = plan("<p>Elle <em>ne vient pas</em></p>");
    let p = pair(&source, "She is not coming", &v);
    let output = reply(
        &p,
        vec![mapping(0, vec![4..17])],
        &[(0, 0), (1, 2), (2, 1), (2, 3), (3, 2)],
    );
    assert!(check(&p, &output).is_ok());
}

#[test]
fn rejects_fused_contractions_mixed_ownership_and_unaligned_required_source() {
    let source = plan("<p>Il ne <em>reviendra</em> pas.</p>");
    let v = versions();
    let p = pair(&source, "He won't return.", &v);
    let mut output = reply(
        &p,
        vec![mapping(0, vec![9..15])],
        &[(0, 0), (1, 1), (2, 2), (3, 1)],
    );
    // A graph alone cannot reveal fused morphology; a reported blocker must not be erased by coverage.
    assert!(check(&p, &output).is_ok());
    output.unresolved = Some(ScopeIssue::FusedMeaning);
    assert_eq!(
        check(&p, &output).unwrap_err(),
        Rejection::Unresolved(ScopeIssue::FusedMeaning)
    );
    output.unresolved = None;
    output.nodes[0].targets = vec![3..15];
    output.links.push((2, 1));
    assert_eq!(check(&p, &output).unwrap_err(), Rejection::MixedOwnership);
    let source = plan("<p>Un <em>livre</em></p>");
    let p = pair(&source, "A book", &v);
    let output = reply(&p, vec![mapping(0, vec![2..6])], &[(0, 0)]);
    assert_eq!(check(&p, &output).unwrap_err(), Rejection::UnresolvedSource);
    let mixed = reply(&p, vec![mapping(0, vec![2..6])], &[(0, 0), (0, 1), (1, 1)]);
    assert_eq!(check(&p, &mixed).unwrap_err(), Rejection::MixedOwnership);
}

#[test]
fn complete_wrong_graph_is_only_structural_evidence_not_semantic_approval() {
    let source = plan("<p>La <em>maison</em> est grande.</p>");
    let v = versions();
    let p = pair(&source, "The cat is hungry.", &v);
    let output = reply(
        &p,
        vec![mapping(0, vec![4..7])],
        &[(0, 0), (1, 1), (2, 2), (3, 3), (4, 4)],
    );
    // This deliberately wrong correspondence must not be described as model acceptance.
    assert!(check(&p, &output).is_ok());
}

#[test]
fn rejects_unknown_missing_duplicate_identity_and_invalid_link_indices() {
    let source = plan("<p>Un <em>livre</em></p>");
    let v = versions();
    let p = pair(&source, "A book", &v);
    let good = reply(&p, vec![mapping(0, vec![2..6])], &[(0, 0), (1, 1)]);
    for nodes in [
        vec![],
        vec![mapping(99, vec![2..6])],
        vec![mapping(0, vec![2..6]), mapping(0, vec![2..6])],
    ] {
        let bad = Reply {
            nodes,
            ..good.clone()
        };
        assert_eq!(check(&p, &bad).unwrap_err(), Rejection::Identity);
    }
    for links in [
        vec![(usize::MAX, 0)],
        vec![(0, usize::MAX)],
        vec![(0, 0), (0, 0)],
    ] {
        let bad = Reply {
            links,
            ..good.clone()
        };
        assert_eq!(check(&p, &bad).unwrap_err(), Rejection::Link);
    }
}

#[test]
fn rejects_crossing_hierarchy_reparenting_sibling_overlap_and_split_identity() {
    let source = plan("<p><strong><em>rouge</em> livre</strong> ici</p>");
    let v = versions();
    let p = pair(&source, "book red here", &v);
    let bad = reply(
        &p,
        vec![mapping(0, vec![0..4]), mapping(1, vec![5..8])],
        &[(0, 1), (1, 0), (2, 2)],
    );
    assert_eq!(check(&p, &bad).unwrap_err(), Rejection::Hierarchy);
    let source = plan("<p><em>un</em> <strong>deux</strong></p>");
    let p = pair(&source, "one two", &v);
    let bad = reply(
        &p,
        vec![mapping(0, vec![0..5]), mapping(1, vec![4..7])],
        &[(0, 0), (1, 1)],
    );
    assert_eq!(check(&p, &bad).unwrap_err(), Rejection::Hierarchy);
    let source = plan("<p><a id='x' href='#x'>un</a> deux</p>");
    let p = pair(&source, "one and two", &v);
    let bad = reply(&p, vec![mapping(0, vec![0..3, 4..7])], &[(0, 0), (1, 2)]);
    assert_eq!(check(&p, &bad).unwrap_err(), Rejection::IdentitySplit);
}

#[test]
fn source_owned_literals_and_zero_width_notes_or_images_require_explicit_later_evidence() {
    let v = versions();
    for html in [
        "<p>Mot<a role='doc-noteref' href='#n'>1</a></p>",
        "<p>Mot<img src='x.png'></p>",
    ] {
        let source = plan(html);
        let p = pair(&source, "Word", &v);
        let output = reply(&p, vec![mapping(0, vec![4..4])], &[(0, 0)]);
        assert_eq!(
            check(&p, &output).unwrap_err(),
            Rejection::AttachmentRequiresEvidence
        );
    }
    let source = plan("<p>Mot <code>x</code></p>");
    let p = pair(&source, "Word x", &v);
    let output = reply(&p, vec![mapping(0, vec![5..6])], &[(0, 0), (1, 1)]);
    assert_eq!(
        check(&p, &output).unwrap_err(),
        Rejection::LiteralRequiresIsolation
    );
}

#[test]
fn unicode_scalar_ranges_are_not_bytes_and_cannot_split_graphemes_or_words() {
    let v = versions();
    let source = plan("<p>Un <em>livre</em></p>");
    let p = pair(&source, "🙂 book", &v);
    let output = reply(&p, vec![mapping(0, vec![2..6])], &[(0, 0), (1, 1)]);
    assert!(check(&p, &output).is_ok());
    let bad = reply(&p, vec![mapping(0, vec![5..9])], &[(0, 0), (1, 1)]);
    assert_eq!(check(&p, &bad).unwrap_err(), Rejection::MappingRange);
    let source = plan("<p>Un <em>mot</em></p>");
    let p = pair(&source, "a cafe\u{301}", &v);
    let bad = reply(&p, vec![mapping(0, vec![2..6])], &[(0, 0), (1, 1)]);
    assert_eq!(
        check(&p, &bad).unwrap_err(),
        Rejection::Grapheme(Side::Target)
    );
    let source = plan("<p>Un <em>li</em>vre</p>");
    let p = pair(&source, "A book", &v);
    let bad = reply(&p, vec![mapping(0, vec![2..6])], &[(0, 0), (1, 1)]);
    assert_eq!(check(&p, &bad).unwrap_err(), Rejection::PartialWord);
}

#[test]
fn checked_mapping_carries_only_source_ids_and_ranges_never_provider_html() {
    let v = versions();
    let source = plan("<p><em>mot</em></p>");
    let p = pair(&source, "<script>x</script>", &v);
    let output = reply(&p, vec![mapping(0, vec![0..p.target.chars().count()])], &[]);
    assert!(check(&p, &output).is_ok());
    // The checked object contains ranges only; hostile-looking target text remains plain text, not HTML.
    assert_eq!(check(&p, &output).unwrap().nodes[0].source_id, 0);
}

#[test]
fn bounds_work_and_does_not_expose_source_content_in_rejection_diagnostics() {
    let v = versions();
    let source = plan("<p>secret <em>word</em></p>");
    let p = pair(&source, "a book", &v);
    let mut output = reply(&p, vec![mapping(0, vec![2..6])], &[(0, 0), (1, 1)]);
    output.links = vec![(0, 0); MAX_LINKS + 1];
    assert_eq!(check(&p, &output).unwrap_err(), Rejection::Limit);
    assert!(!format!("{:?}", check(&p, &output).unwrap_err()).contains("secret"));
    let target = "x".repeat(MAX_HTML_BYTES + 1);
    let p = pair(&source, &target, &v);
    assert_eq!(p.binding().unwrap_err(), Rejection::Limit);
    let huge = plan(&format!(
        "<p><span title='{}'>mot</span></p>",
        "x".repeat(MAX_HTML_BYTES * 9)
    ));
    assert_eq!(
        pair(&huge, "word", &v).binding().unwrap_err(),
        Rejection::Limit
    );
}

#[test]
fn tokenizer_admission_covers_unknown_reserved_lossy_and_supported_unicode_cases() {
    let cases = [
        ("Bonjour monde.", "Bonjour monde.", None),
        ("café", "café", None),
        ("cafe\u{301}", "cafe\u{301}", None),
        ("عَرَبِيّ", "عَرَبِيّ", None),
        ("Bonjour\tmonde", "Bonjour monde", None),
        ("🙂", "🙂", Some(Rejection::UnknownToken(Side::Source))),
        ("🚀", "🚀", Some(Rejection::UnknownToken(Side::Source))),
        (
            "👩‍👩‍👧‍👦",
            "👩👩👧👦",
            Some(Rejection::Normalization(Side::Source)),
        ),
        ("\u{e000}", "", Some(Rejection::Normalization(Side::Source))),
        (
            "\u{200f}كتاب\u{200e}",
            "كتاب",
            Some(Rejection::Normalization(Side::Source)),
        ),
        (
            "می\u{200c}روم",
            "میروم",
            Some(Rejection::Normalization(Side::Source)),
        ),
        (
            "ca\u{200b}fe",
            "cafe",
            Some(Rejection::Normalization(Side::Source)),
        ),
        (
            "co\u{ad}operate",
            "cooperate",
            Some(Rejection::Normalization(Side::Source)),
        ),
        ("a\0b", "ab", Some(Rejection::Normalization(Side::Source))),
        (
            "a\u{fffd}b",
            "ab",
            Some(Rejection::Normalization(Side::Source)),
        ),
        (
            "x\u{2066}y\u{2069}",
            "xy",
            Some(Rejection::Normalization(Side::Source)),
        ),
        (
            "a\u{1c}b",
            "ab",
            Some(Rejection::Normalization(Side::Source)),
        ),
    ];
    for (text, normalized, expected) in cases {
        let mut ts = tokens(text);
        if text == "🙂" || text == "🚀" {
            ts[1].id = 100;
        }
        let result = tokens::check(
            text,
            &rules(),
            &TokenizedText {
                normalized,
                tokens: &ts,
            },
            Side::Source,
        );
        match expected {
            Some(reason) => assert_eq!(result.err(), Some(reason), "{text:?}"),
            None => assert!(result.is_ok(), "{text:?}"),
        }
    }
    for (text, id) in [
        ("[CLS]", 101),
        ("[SEP]", 102),
        ("[MASK]", 103),
        ("[PAD]", 0),
        ("[UNK]", 100),
    ] {
        let ts = vec![
            tokens("a")[0].clone(),
            Token {
                id,
                word: Some(0),
                bytes: 0..text.len(),
                special: false,
            },
            tokens("a")[2].clone(),
        ];
        let expected = if id == 100 {
            Rejection::UnknownToken(Side::Target)
        } else {
            Rejection::ReservedToken(Side::Target)
        };
        assert_eq!(
            tokens::check(
                text,
                &rules(),
                &TokenizedText {
                    normalized: text,
                    tokens: &ts
                },
                Side::Target
            )
            .err(),
            Some(expected)
        );
    }
    let long = "a".repeat(101);
    let mut ts = tokens(&long);
    ts[1].id = 100;
    assert_eq!(
        tokens::check(
            &long,
            &rules(),
            &TokenizedText {
                normalized: &long,
                tokens: &ts
            },
            Side::Source
        )
        .err(),
        Some(Rejection::UnknownToken(Side::Source))
    );
}

#[test]
fn rejects_fake_framing_holes_bad_word_ids_and_partial_utf8_with_no_panics() {
    for change in 0..6 {
        let mut ts = tokens("café book");
        match change {
            0 => ts[0].special = false,
            1 => ts[0].word = Some(0),
            2 => ts[1].word = Some(usize::MAX),
            3 => ts[1].bytes = 0..4,
            4 => ts[1].special = true,
            _ => {
                ts.remove(1);
                ts[1].word = Some(0);
            }
        }
        assert!(tokens::check(
            "café book",
            &rules(),
            &TokenizedText {
                normalized: "café book",
                tokens: &ts
            },
            Side::Source
        )
        .is_err());
    }
    let text = "cafe\u{301}";
    let mut ts = tokens(text);
    ts[1].bytes = 0..4;
    ts.insert(
        2,
        Token {
            id: 2001,
            word: Some(0),
            bytes: 4..text.len(),
            special: false,
        },
    );
    assert!(tokens::check(
        text,
        &rules(),
        &TokenizedText {
            normalized: text,
            tokens: &ts
        },
        Side::Source
    )
    .is_ok());
    ts[2].word = Some(1);
    assert_eq!(
        tokens::check(
            text,
            &rules(),
            &TokenizedText {
                normalized: text,
                tokens: &ts
            },
            Side::Source
        )
        .err(),
        Some(Rejection::Grapheme(Side::Source))
    );
}

#[test]
fn candidate_has_no_catalog_or_installer_admission_in_this_contract_only_stage() {
    for id in ["cstr/awesome-align-onnx-int8", "cstr/awesome-align-onnx"] {
        assert!(crate::translation::models::find_planned_model(id).is_none());
        assert!(crate::translation::model_store::manifest_by_id(id).is_none());
    }
}

#[test]
fn unknown_context_is_rejected_on_either_side_even_outside_the_styled_word() {
    let source = plan("<p>🙂 <em>livre</em></p>");
    let v = versions();
    let p = pair(&source, "A book", &v);
    let output = reply(&p, vec![mapping(0, vec![2..6])], &[(0, 0), (1, 1)]);
    let mut st = tokens(source.source_text());
    st[1].id = 100;
    let tt = tokens(p.target);
    assert_eq!(
        p.check(
            &rules(),
            &TokenizedText {
                normalized: source.source_text(),
                tokens: &st
            },
            &TokenizedText {
                normalized: p.target,
                tokens: &tt
            },
            &output
        )
        .unwrap_err(),
        Rejection::UnknownToken(Side::Source)
    );
    let source = plan("<p>Un <em>livre</em></p>");
    let p = pair(&source, "🚀 book", &v);
    let output = reply(&p, vec![mapping(0, vec![2..6])], &[(0, 0), (1, 1)]);
    let st = tokens(source.source_text());
    let mut tt = tokens(p.target);
    tt[1].id = 100;
    assert_eq!(
        p.check(
            &rules(),
            &TokenizedText {
                normalized: source.source_text(),
                tokens: &st
            },
            &TokenizedText {
                normalized: p.target,
                tokens: &tt
            },
            &output
        )
        .unwrap_err(),
        Rejection::UnknownToken(Side::Target)
    );
}

#[test]
fn ranges_are_bounded_sorted_nonempty_and_do_not_split_source_graphemes() {
    let source = plan("<p>Un <em>livre</em></p>");
    let v = versions();
    let p = pair(&source, "A book", &v);
    for ranges in [
        vec![],
        vec![2..2],
        vec![6..2],
        vec![2..usize::MAX],
        vec![4..6, 2..4],
        vec![2..6, 2..6],
    ] {
        let output = reply(&p, vec![mapping(0, ranges)], &[(0, 0), (1, 1)]);
        assert!(check(&p, &output).is_err());
    }
    let output = reply(&p, vec![mapping(0, vec![0..1; MAX_TOKENS + 1])], &[]);
    assert_eq!(check(&p, &output).unwrap_err(), Rejection::Limit);
    let source = plan("<p>Un e<em>\u{301}</em></p>");
    let p = pair(&source, "A book", &v);
    let output = reply(&p, vec![mapping(0, vec![2..6])], &[(0, 0), (1, 1)]);
    assert_eq!(
        check(&p, &output).unwrap_err(),
        Rejection::Grapheme(Side::Source)
    );
}

#[test]
fn framing_profiles_and_token_limits_cannot_be_overridden_by_a_reply() {
    let source = plan("<p>Un <em>livre</em></p>");
    let v = versions();
    let p = pair(&source, "A book", &v);
    let output = reply(&p, vec![mapping(0, vec![2..6])], &[(0, 0), (1, 1)]);
    let st = tokens(source.source_text());
    let tt = tokens(p.target);
    let wrong = TokenRules {
        identity: "another-tokenizer",
        ..rules()
    };
    assert_eq!(
        p.check(
            &wrong,
            &TokenizedText {
                normalized: source.source_text(),
                tokens: &st
            },
            &TokenizedText {
                normalized: p.target,
                tokens: &tt
            },
            &output
        )
        .unwrap_err(),
        Rejection::TokenProfile
    );
    let text = vec!["book"; 510].join(" ");
    let ts = tokens(&text);
    assert_eq!(ts.len(), MAX_TOKENS);
    assert!(tokens::check(
        &text,
        &rules(),
        &TokenizedText {
            normalized: &text,
            tokens: &ts
        },
        Side::Source
    )
    .is_ok());
    let mut too_many = ts.clone();
    too_many.insert(1, ts[1].clone());
    assert_eq!(
        tokens::check(
            &text,
            &rules(),
            &TokenizedText {
                normalized: &text,
                tokens: &too_many
            },
            Side::Source
        )
        .err(),
        Some(Rejection::Limit)
    );
    let invalid = TokenRules {
        unknown: 999,
        ..rules()
    };
    assert_eq!(
        tokens::check(
            &text,
            &invalid,
            &TokenizedText {
                normalized: &text,
                tokens: &ts
            },
            Side::Source
        )
        .err(),
        Some(Rejection::TokenProfile)
    );
}

#[test]
fn explicit_scope_blockers_survive_complete_graphs_without_granting_confidence() {
    let source = plan("<p><em>Texte</em></p>");
    let v = versions();
    let p = pair(&source, "Text", &v);
    let mut output = reply(&p, vec![mapping(0, vec![0..4])], &[(0, 0)]);
    assert_eq!(check(&p, &output).unwrap().nodes().len(), 1);
    for issue in [
        ScopeIssue::Auxiliary,
        ScopeIssue::FusedMeaning,
        ScopeIssue::Attachment,
    ] {
        output.unresolved = Some(issue);
        assert_eq!(
            check(&p, &output).unwrap_err(),
            Rejection::Unresolved(issue)
        );
    }
}
