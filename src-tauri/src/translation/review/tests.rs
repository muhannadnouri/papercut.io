//! Synthetic rejection tests: no app data, private document, model or writer participates.
use super::*;
use serde_json::{json, Value};

fn evidence() -> Evidence<'static> {
    Evidence {
        normalization_policy: "source-emphasis-v1",
        source_snapshot: "<p>Source.</p><p id='note'>Note.</p>",
        published_html: "<p>English.</p>",
    }
}

fn range(start: u32, end: u32) -> ScalarRange {
    ScalarRange { start, end }
}

fn mark(start: u32, end: u32, kind: MarkKind) -> Mark {
    Mark {
        range: range(start, end),
        kind,
    }
}

fn block(ordinal: u32, text: &str) -> Block {
    Block::new(
        BlockKey {
            ordinal,
            source_identity: "source-plan-1".into(),
        },
        "structure-1".into(),
        text.into(),
        vec![],
        vec![],
        BTreeSet::from([Obligation::Emphasis]),
    )
    .unwrap()
}

fn document(blocks: Vec<Block>) -> Document {
    Document::new(
        "opus-mt-es-en-ctranslate2",
        "a1b2".into(),
        "12".repeat(16),
        evidence(),
        blocks,
    )
    .unwrap()
}

fn request(document: &Document, block: usize, edit: Edit) -> EditRequest {
    EditRequest {
        expected: document.expected.clone(),
        block: document.blocks[block].key.clone(),
        edit,
    }
}

fn replace(document: &mut Document, marks: Vec<Mark>) -> Result<(), Error> {
    let edit = request(document, 0, Edit::ReplaceMarks { marks });
    apply(Some(document), &edit, evidence())
}

fn approve(document: &mut Document) {
    let edit = request(document, 0, Edit::Approve {});
    apply(Some(document), &edit, evidence()).unwrap();
}

#[test]
fn identical_paragraphs_keep_source_ordinal_and_generation_identity() {
    let mut doc = document(vec![block(0, "Same text."), block(1, "Same text.")]);
    let edit = request(
        &doc,
        1,
        Edit::ReplaceMarks {
            marks: vec![mark(0, 4, MarkKind::Bold)],
        },
    );
    apply(Some(&mut doc), &edit, evidence()).unwrap();
    assert!(doc.blocks[0].editable.is_empty());
    assert_eq!(doc.blocks[1].runs()[0].range, range(0, 4));
    let before = doc.clone();
    for key in [
        BlockKey {
            ordinal: 2,
            source_identity: "source-plan-1".into(),
        },
        BlockKey {
            ordinal: 1,
            source_identity: "another-source".into(),
        },
    ] {
        let mut bad = request(&doc, 1, Edit::Approve {});
        bad.block = key;
        assert_eq!(
            apply(Some(&mut doc), &bad, evidence()),
            Err(Error::Identity)
        );
        assert_eq!(doc, before);
    }
    let mut bad = request(&doc, 1, Edit::Approve {});
    bad.expected.generation = "34".repeat(16);
    assert_eq!(apply(Some(&mut doc), &bad, evidence()), Err(Error::Stale));
    assert_eq!(doc, before);
}

#[test]
fn changed_snapshots_target_text_and_published_document_reject_without_mutation() {
    let mut doc = document(vec![block(0, "Original English.")]);
    let edit = request(&doc, 0, Edit::Approve {});
    for changed in [
        Evidence {
            normalization_policy: "source-emphasis-v2",
            ..evidence()
        },
        Evidence {
            source_snapshot: "Changed source.",
            ..evidence()
        },
        Evidence {
            published_html: "Changed English.",
            ..evidence()
        },
    ] {
        let before = doc.clone();
        assert_eq!(apply(Some(&mut doc), &edit, changed), Err(Error::Evidence));
        assert_eq!(doc, before);
    }
    // A future storage reader must retain/verify the original hash instead of blessing changed text.
    doc.blocks[0].text.push('!');
    let before = doc.clone();
    assert_eq!(
        apply(Some(&mut doc), &edit, evidence()),
        Err(Error::Evidence)
    );
    assert_eq!(doc, before);
}

#[test]
fn stale_concurrent_requests_deleted_bases_and_revision_exhaustion_cannot_apply() {
    let mut doc = document(vec![block(0, "English.")]);
    let first = request(&doc, 0, Edit::Approve {});
    apply(Some(&mut doc), &first, evidence()).unwrap();
    let before = doc.clone();
    assert_eq!(apply(Some(&mut doc), &first, evidence()), Err(Error::Stale));
    assert_eq!(doc, before);
    let mut deleted: Option<Document> = None;
    assert_eq!(
        apply(deleted.as_mut(), &first, evidence()),
        Err(Error::Missing)
    );
    assert!(deleted.is_none());
    assert_eq!(
        finish(None, &first.expected, evidence()),
        Err(Error::Missing)
    );
    doc.expected.revision = MAX_REVISION;
    let exhausted = request(&doc, 0, Edit::Reset {});
    let before = doc.clone();
    assert_eq!(
        apply(Some(&mut doc), &exhausted, evidence()),
        Err(Error::Revision)
    );
    assert_eq!(doc, before);
}

#[test]
fn unsupported_versions_and_variant_identities_never_mutate_the_base() {
    let mut doc = document(vec![block(0, "English.")]);
    for change in 0..4 {
        let mut edit = request(&doc, 0, Edit::Approve {});
        match change {
            0 => edit.expected.schema_version += 1,
            1 => edit.expected.policy = "other-policy".into(),
            2 => edit.expected.variant_id = "ff".into(),
            _ => edit.expected.revision = MAX_REVISION + 1,
        }
        let before = doc.clone();
        assert!(apply(Some(&mut doc), &edit, evidence()).is_err());
        assert_eq!(doc, before);
    }
}

#[test]
fn one_invalid_mark_rejects_the_entire_edit_and_preserves_approval() {
    let mut doc = document(vec![block(0, "abcdefghij")]);
    approve(&mut doc);
    let before = doc.clone();
    for bad in [
        mark(8, 99, MarkKind::Italic),
        mark(5, 4, MarkKind::Bold),
        mark(4, 4, MarkKind::Bold),
    ] {
        assert_eq!(
            replace(&mut doc, vec![mark(0, 3, MarkKind::Bold), bad]),
            Err(Error::Range)
        );
        assert_eq!(doc, before);
    }
}

#[test]
fn utf16_surrogates_combining_marks_arabic_diacritics_and_zwj_cuts_reject() {
    let text = "A🙂e\u{301}عَرَبِيّ";
    assert_eq!(selection_from_utf16(text, 1, 3), Ok(range(1, 2)));
    assert_eq!(selection_from_utf16(text, 3, 5), Ok(range(2, 4)));
    for (start, end) in [(1, 2), (2, 3), (3, 4), (5, 6), (0, 99), (3, 1), (0, 0)] {
        assert_eq!(selection_from_utf16(text, start, end), Err(Error::Range));
    }
    let family = "👩‍👩‍👧‍👦";
    assert_eq!(selection_from_utf16(family, 0, 2), Err(Error::Range));
    assert_eq!(selection_from_utf16(family, 0, 11), Ok(range(0, 7)));
    let mut doc = document(vec![block(0, text)]);
    let before = doc.blocks[0].text.as_bytes().to_vec();
    replace(
        &mut doc,
        vec![mark(2, 4, MarkKind::Bold), mark(4, 6, MarkKind::Italic)],
    )
    .unwrap();
    assert_eq!(doc.blocks[0].text.as_bytes(), before);
    assert_eq!(
        replace(&mut doc, vec![mark(4, 5, MarkKind::Bold)]),
        Err(Error::Range)
    );
}

#[test]
fn every_whole_grapheme_can_roundtrip_utf16_without_character_normalization() {
    for text in ["e\u{301} and é", "🙂 👩‍👩‍👧‍👦", "English عَرَبِيّ text", "a\r\nb"]
    {
        for (byte, cluster) in text.grapheme_indices(true) {
            let start = text[..byte].encode_utf16().count() as u32;
            let end = start + cluster.encode_utf16().count() as u32;
            let selection = selection_from_utf16(text, start, end).unwrap();
            assert_eq!(
                text.chars()
                    .skip(selection.start as usize)
                    .take((selection.end - selection.start) as usize)
                    .collect::<String>(),
                cluster
            );
        }
    }
}

#[test]
fn overlapping_duplicate_and_adjacent_marks_have_one_deterministic_geometry() {
    let mut doc = document(vec![block(0, "abcdefghij")]);
    let marks = vec![
        mark(0, 5, MarkKind::Bold),
        mark(3, 8, MarkKind::Italic),
        mark(0, 3, MarkKind::Bold),
        mark(5, 6, MarkKind::Bold),
    ];
    replace(&mut doc, marks.clone()).unwrap();
    let expected = vec![
        Run {
            range: range(0, 3),
            bold: true,
            italic: false,
        },
        Run {
            range: range(3, 6),
            bold: true,
            italic: true,
        },
        Run {
            range: range(6, 8),
            bold: false,
            italic: true,
        },
    ];
    assert_eq!(doc.blocks[0].runs(), expected);
    replace(&mut doc, marks.into_iter().rev().collect()).unwrap();
    assert_eq!(doc.blocks[0].runs(), expected);
    replace(&mut doc, vec![mark(3, 8, MarkKind::Italic)]).unwrap();
    assert_eq!(
        doc.blocks[0].runs(),
        vec![Run {
            range: range(3, 8),
            bold: false,
            italic: true
        }]
    );
}

#[test]
fn protected_islands_and_zero_width_anchors_cannot_be_enclosed_or_crossed() {
    let mut b = block(0, "abcdefghij");
    b.locked = vec![range(3, 5), range(7, 7)];
    let mut doc = document(vec![b]);
    for (start, end) in [(0, 10), (3, 5), (2, 4), (4, 6), (6, 8)] {
        let before = doc.clone();
        assert_eq!(
            replace(&mut doc, vec![mark(start, end, MarkKind::Bold)]),
            Err(Error::Locked)
        );
        assert_eq!(doc, before);
    }
    // Boundary-touching marks are geometrically allowed; the later renderer must keep anchors outside them.
    for (start, end) in [(0, 3), (5, 7), (7, 10)] {
        replace(&mut doc, vec![mark(start, end, MarkKind::Bold)]).unwrap();
    }
}

#[test]
fn source_validated_marks_over_locked_content_are_never_edited_or_reset() {
    let b = Block::new(
        BlockKey {
            ordinal: 0,
            source_identity: "source".into(),
        },
        "structure".into(),
        "abcdefghij".into(),
        vec![range(3, 5)],
        vec![mark(3, 5, MarkKind::Bold)],
        BTreeSet::from([Obligation::Emphasis]),
    )
    .unwrap();
    let mut doc = document(vec![b]);
    replace(&mut doc, vec![mark(0, 3, MarkKind::Italic)]).unwrap();
    let reset = request(&doc, 0, Edit::Reset {});
    apply(Some(&mut doc), &reset, evidence()).unwrap();
    assert_eq!(
        doc.blocks[0].runs(),
        vec![Run {
            range: range(3, 5),
            bold: true,
            italic: false
        }]
    );
    assert_eq!(
        replace(&mut doc, vec![mark(3, 5, MarkKind::Italic)]),
        Err(Error::Locked)
    );
}

#[test]
fn adjacent_marks_cannot_merge_across_a_zero_width_protected_anchor() {
    for fixed in [vec![], vec![mark(0, 5, MarkKind::Bold)]] {
        let b = Block::new(
            BlockKey {
                ordinal: 0,
                source_identity: "source".into(),
            },
            "tree".into(),
            "abcdefghij".into(),
            vec![range(5, 5)],
            fixed.clone(),
            BTreeSet::from([Obligation::Emphasis]),
        )
        .unwrap();
        let mut doc = document(vec![b]);
        let mut marks = vec![mark(5, 10, MarkKind::Bold)];
        if fixed.is_empty() {
            marks.push(mark(0, 5, MarkKind::Bold));
        }
        replace(&mut doc, marks).unwrap();
        assert_eq!(
            doc.blocks[0].runs(),
            vec![
                Run {
                    range: range(0, 5),
                    bold: true,
                    italic: false
                },
                Run {
                    range: range(5, 10),
                    bold: true,
                    italic: false
                }
            ]
        );
        assert_eq!(
            replace(&mut doc, vec![mark(0, 10, MarkKind::Bold)]),
            Err(Error::Locked)
        );
    }
}

#[test]
fn approval_never_clears_link_note_typography_attachment_or_import_obligations() {
    for obligation in [
        Obligation::Link,
        Obligation::Note,
        Obligation::Typography,
        Obligation::Attachment,
        Obligation::SourceFormatting,
    ] {
        let mut b = block(0, "English.");
        b.obligations.insert(obligation);
        let mut doc = document(vec![b]);
        approve(&mut doc);
        assert_eq!(doc.blocks[0].remaining(), BTreeSet::from([obligation]));
        assert_eq!(
            finish(Some(&doc), &doc.expected, evidence()),
            Err(Error::Pending)
        );
    }
}

#[test]
fn explicit_empty_approval_can_finish_but_edit_or_reset_requires_reapproval() {
    let mut doc = document(vec![block(0, "English.")]);
    assert_eq!(
        finish(Some(&doc), &doc.expected, evidence()),
        Err(Error::Pending)
    );
    approve(&mut doc);
    assert!(doc.blocks[0].editable.is_empty());
    assert!(finish(Some(&doc), &doc.expected, evidence()).is_ok());
    for edit in [Edit::ReplaceMarks { marks: vec![] }, Edit::Reset {}] {
        let edit = request(&doc, 0, edit);
        apply(Some(&mut doc), &edit, evidence()).unwrap();
        assert_eq!(
            finish(Some(&doc), &doc.expected, evidence()),
            Err(Error::Pending)
        );
        approve(&mut doc);
        assert!(finish(Some(&doc), &doc.expected, evidence()).is_ok());
    }
}

fn json_request() -> Value {
    let doc = document(vec![block(0, "English.")]);
    json!({"expected":doc.expected,"block":doc.blocks[0].key,"edit":{"action":"replaceMarks","marks":[{"range":{"start":0,"end":7},"kind":"bold"}]}})
}

#[test]
fn caller_cannot_smuggle_english_html_paths_obligations_or_approval_into_the_command() {
    assert!(serde_json::from_value::<EditRequest>(json_request()).is_ok());
    for (container, field, value) in [
        ("", "html", json!("<script>bad</script>")),
        ("", "targetText", json!("Changed English")),
        ("", "sourceUrl", json!("/private/path")),
        ("", "obligations", json!([])),
        ("edit", "approved", json!(true)),
        ("expected", "publishedHash", json!("forged")),
        ("block", "text", json!("identical")),
    ] {
        let mut forged = json_request();
        let target = if container.is_empty() {
            &mut forged
        } else {
            &mut forged[container]
        };
        target[field] = value;
        assert!(
            serde_json::from_value::<EditRequest>(forged).is_err(),
            "{field}"
        );
    }
    let mut forged = json_request();
    forged["edit"] = json!({"action":"approve","ready":true});
    assert!(serde_json::from_value::<EditRequest>(forged).is_err());
}

#[test]
fn malformed_nested_ranges_marks_and_numeric_offsets_reject_at_deserialization() {
    for value in [json!(-1), json!(1.5), json!(u64::MAX), json!("1")] {
        let mut forged = json_request();
        forged["edit"]["marks"][0]["range"]["start"] = value;
        assert!(serde_json::from_value::<EditRequest>(forged).is_err());
    }
    let mut forged = json_request();
    forged["edit"]["marks"][0]["kind"] = json!("underline");
    assert!(serde_json::from_value::<EditRequest>(forged).is_err());
    let mut forged = json_request();
    forged["edit"]["marks"][0]["range"]["unit"] = json!("bytes");
    assert!(serde_json::from_value::<EditRequest>(forged).is_err());
}

#[test]
fn raw_decoder_has_inclusive_byte_and_precoalescing_shape_limits() {
    let mut bytes = serde_json::to_vec(&json_request()).unwrap();
    bytes.resize(MAX_REQUEST_BYTES, b' ');
    assert!(decode_request(&bytes).is_ok());
    bytes.push(b' ');
    assert_eq!(decode_request(&bytes), Err(Error::Limit));
    let mut excessive = json_request();
    excessive["edit"]["marks"] = json!(vec![
        json!({"range":{"start":0,"end":1},"kind":"bold"});
        MAX_MARKS + 1
    ]);
    let bytes = serde_json::to_vec(&excessive).unwrap();
    assert!(bytes.len() < MAX_REQUEST_BYTES);
    assert_eq!(decode_request(&bytes), Err(Error::Limit));
    let mut unknown = json_request();
    unknown["edit"] = json!({"action":"reset","ready":true});
    assert_eq!(
        decode_request(&serde_json::to_vec(&unknown).unwrap()),
        Err(Error::Request)
    );
    let mut oversized_id = json_request();
    oversized_id["block"]["sourceIdentity"] = json!("x".repeat(129));
    assert_eq!(
        decode_request(&serde_json::to_vec(&oversized_id).unwrap()),
        Err(Error::Identity)
    );
}

#[test]
fn count_limits_apply_before_duplicates_can_be_coalesced() {
    let mut doc = document(vec![block(0, "English.")]);
    replace(&mut doc, vec![mark(0, 7, MarkKind::Bold); MAX_MARKS]).unwrap();
    let before = doc.clone();
    assert_eq!(
        replace(&mut doc, vec![mark(0, 7, MarkKind::Bold); MAX_MARKS + 1]),
        Err(Error::Limit)
    );
    assert_eq!(doc, before);
}

#[test]
fn text_byte_identity_and_document_count_budgets_have_at_limit_and_one_over_checks() {
    assert!(Block::new(
        BlockKey {
            ordinal: 0,
            source_identity: "source".into()
        },
        "tree".into(),
        "é".repeat(MAX_BLOCK_BYTES / 2),
        vec![],
        vec![],
        BTreeSet::new()
    )
    .is_ok());
    assert_eq!(
        Block::new(
            BlockKey {
                ordinal: 0,
                source_identity: "source".into()
            },
            "tree".into(),
            "é".repeat(MAX_BLOCK_BYTES / 2 + 1),
            vec![],
            vec![],
            BTreeSet::new()
        ),
        Err(Error::Limit)
    );
    assert_eq!(
        Block::new(
            BlockKey {
                ordinal: 0,
                source_identity: "../path".into()
            },
            "tree".into(),
            "text".into(),
            vec![],
            vec![],
            BTreeSet::new()
        ),
        Err(Error::Identity)
    );
    let blocks = (0..MAX_BLOCKS).map(|i| block(i as u32, "a")).collect();
    assert!(Document::new(
        "opus-mt-fr-en-ctranslate2",
        "ff".into(),
        "00".repeat(16),
        evidence(),
        blocks
    )
    .is_ok());
    let blocks = vec![block(0, "a"); MAX_BLOCKS + 1];
    assert_eq!(
        Document::new(
            "opus-mt-fr-en-ctranslate2",
            "ff".into(),
            "00".repeat(16),
            evidence(),
            blocks
        ),
        Err(Error::Limit)
    );
    let duplicate = vec![block(0, "a"), block(0, "a")];
    assert_eq!(
        Document::new(
            "opus-mt-fr-en-ctranslate2",
            "ff".into(),
            "00".repeat(16),
            evidence(),
            duplicate
        ),
        Err(Error::Identity)
    );
}

#[test]
fn aggregate_text_bound_rejects_before_any_editable_document_is_admitted() {
    let text = "a".repeat(MAX_BLOCK_BYTES);
    let count = MAX_DOCUMENT_TEXT_BYTES / MAX_BLOCK_BYTES;
    let blocks = (0..count).map(|i| block(i as u32, &text)).collect();
    assert!(Document::new(
        "opus-mt-fr-en-ctranslate2",
        "ff".into(),
        "00".repeat(16),
        evidence(),
        blocks
    )
    .is_ok());
    let blocks = (0..=count).map(|i| block(i as u32, &text)).collect();
    assert_eq!(
        Document::new(
            "opus-mt-fr-en-ctranslate2",
            "ff".into(),
            "00".repeat(16),
            evidence(),
            blocks
        ),
        Err(Error::Limit)
    );
}

#[test]
fn stage_one_contract_does_not_admit_other_providers_or_legacy_identity_shapes() {
    for model in ["bergamot-es-en-worker", "hy-mt2-1_8b", "retired-aligner"] {
        assert_eq!(
            Document::new(
                model,
                "ff".into(),
                "00".repeat(16),
                evidence(),
                vec![block(0, "English.")]
            ),
            Err(Error::Model)
        );
    }
    assert_eq!(
        Document::new(
            "opus-mt-fr-en-ctranslate2",
            "../source".into(),
            "00".repeat(16),
            evidence(),
            vec![block(0, "English.")]
        ),
        Err(Error::Identity)
    );
    assert_eq!(
        Document::new(
            "opus-mt-fr-en-ctranslate2",
            "ff".into(),
            "old-timestamp".into(),
            evidence(),
            vec![block(0, "English.")]
        ),
        Err(Error::Identity)
    );
}
