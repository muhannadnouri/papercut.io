use super::*;
use crate::translation::{
    render::render_translated_html,
    source::{TranslationSourceBlock, TranslationSourceDocument},
    storage::{PersistTranslationFragment, PersistTranslationSection},
    types::TranslationRepairMode,
};

fn snapshot(source: &str, english: &[&str]) -> Snapshot {
    let source = crate::document_uploads::sanitize_html(source);
    let plan = DomPlan::new(&source);
    assert_eq!(plan.blocks.len(), english.len());
    let request = PersistTranslationRequest {
        formatting: FormattingPolicy::SentenceReview,
        source: TranslationSourceDocument {
            document_id: "abc1".into(),
            document_url: "/uploads/abc1.html".into(),
            title: "Synthetic".into(),
            format: "html".into(),
            view_html: source,
            source_formatting: Some(crate::document_uploads::SourceFormatting {
                policy: "source-emphasis-v1".into(),
                warnings: Vec::new(),
            }),
            blocks: plan
                .blocks
                .iter()
                .map(|b| TranslationSourceBlock {
                    ordinal: b.ordinal,
                    is_heading: b.is_heading,
                    heading: b.heading.clone(),
                    text: b.inline.text.clone(),
                })
                .collect(),
            assets: Default::default(),
        },
        source_language: "es".into(),
        target_language: "en".into(),
        model_id: "opus-mt-es-en-ctranslate2".into(),
        quality_mode: "balanced".into(),
        repair_mode: TranslationRepairMode::default(),
        job_id: "job1".into(),
        glossary: vec![],
        translated_sections: plan
            .blocks
            .iter()
            .zip(english)
            .map(|(b, text)| PersistTranslationSection {
                source_heading: b.heading.clone(),
                source_ordinal: b.ordinal,
                is_heading: b.is_heading,
                text: (*text).into(),
                fragments: vec![PersistTranslationFragment {
                    needs_review: true,
                    source_start: 0,
                    source_end: b.inline.text.chars().count(),
                    source_text: b.inline.text.clone(),
                    text: (*text).into(),
                    inline_phrases: vec![],
                    markup: None,
                }],
            })
            .collect(),
    };
    let html = crate::document_uploads::sanitize_html(
        &render_translated_html("English", &request).unwrap(),
    );
    Snapshot::capture(&request, "abc2".into(), "12".repeat(16), &html).unwrap()
}

fn edit(snapshot: &Snapshot, ordinal: usize, marks: Vec<Mark>) -> EditRequest {
    EditRequest {
        expected: snapshot.document.expected.clone(),
        block: snapshot.document.blocks[ordinal].key.clone(),
        edit: Edit::ReplaceMarks { marks },
    }
}

fn bold(start: u32, end: u32) -> Mark {
    Mark {
        range: ScalarRange { start, end },
        kind: MarkKind::Bold,
    }
}

#[test]
fn captures_exact_evidence_and_reloads_a_draft_without_models_or_originals() {
    let mut snapshot = snapshot("<p>Una <b>casa</b>.</p>", &["A house."]);
    let source = snapshot.source_html.clone();
    let baseline = snapshot.baseline_html.clone();
    snapshot
        .apply(&edit(&snapshot, 0, vec![bold(2, 7)]))
        .unwrap();
    let saved = serde_json::to_string(&snapshot).unwrap();
    let loaded: Snapshot = serde_json::from_str(&saved).unwrap();
    loaded.validate().unwrap();
    assert_eq!(loaded.source_html, source);
    assert_eq!(loaded.baseline_html, baseline);
    assert_eq!(loaded.document.expected.revision, 1);
    let output = parse_html_document(loaded.reconstruct().unwrap());
    let plan = DomPlan::new(&loaded.reconstruct().unwrap());
    assert_eq!(plan.blocks[0].inline.text, "A house.");
    assert_eq!(
        output
            .select("b")
            .unwrap()
            .filter(|n| n.text_contents() == "house")
            .count(),
        1
    );
    assert_eq!(loaded.document.blocks[0].text, "A house.");
    assert!(loaded.document.blocks[0]
        .remaining()
        .contains(&Obligation::Emphasis));
}

#[test]
fn emphasis_preview_keeps_reader_search_and_narration_coordinates_unchanged() {
    let mut snapshot = snapshot(
        "<p id='ref'>Una <b>casa</b><a href='#note' role='doc-noteref'>1</a>.</p><p id='note' role='doc-footnote'>Nota<a href='#ref' role='doc-backlink'>Back</a>.</p>",
        &["A house.", "Note."],
    );
    let baseline = crate::document_uploads::prepare_html_document(&snapshot.baseline_html);
    let before_dom = parse_html_document(&snapshot.baseline_html);
    snapshot
        .apply(&edit(&snapshot, 0, vec![bold(2, 7)]))
        .unwrap();
    let output = snapshot.reconstruct().unwrap();
    let reconstructed = crate::document_uploads::prepare_html_document(&output);
    // Shared parser coordinates feed Find, search and TTS; styling never changes their exact evidence.
    let baseline_coordinates: Vec<_> = baseline
        .sections
        .iter()
        .map(|s| (s.text.clone(), s.body_ranges.clone()))
        .collect();
    let reconstructed_coordinates: Vec<_> = reconstructed
        .sections
        .iter()
        .map(|s| (s.text.clone(), s.body_ranges.clone()))
        .collect();
    assert_eq!(baseline_coordinates, reconstructed_coordinates);
    assert!(baseline.sections.iter().any(|s| s.text == "A house."));
    assert!(baseline.sections.iter().all(|s| !s.text.contains("Una")
        && !s.text.contains("Translation")
        && !s.text.contains("Original")));
    let after_dom = parse_html_document(&output);
    assert_eq!(before_dom.text_contents(), after_dom.text_contents());
    assert_eq!(
        before_dom
            .select_first("[data-papercut-translation-review='original']")
            .unwrap()
            .text_contents(),
        after_dom
            .select_first("[data-papercut-translation-review='original']")
            .unwrap()
            .text_contents()
    );
}

#[test]
fn repeated_blocks_use_paths_and_ordinals_without_restyling_the_first_copy() {
    let mut snapshot = snapshot(
        "<p><b>Igual</b>.</p><p><b>Igual</b>.</p>",
        &["Same.", "Same."],
    );
    snapshot
        .apply(&edit(&snapshot, 1, vec![bold(0, 4)]))
        .unwrap();
    let reconstructed = snapshot.reconstruct().unwrap();
    let plan = DomPlan::new(&reconstructed);
    assert!(plan.blocks[0].node.select("b").unwrap().next().is_none());
    assert_eq!(
        plan.blocks[1]
            .node
            .select_first("b")
            .unwrap()
            .text_contents(),
        "Same"
    );
    assert_ne!(
        snapshot.document.blocks[0].key,
        snapshot.document.blocks[1].key
    );
    assert_ne!(
        snapshot.mappings[0].target_path,
        snapshot.mappings[1].target_path
    );
}

#[test]
fn source_preview_namespaces_notes_backlinks_named_anchors_and_keeps_snapshot_untouched() {
    let snapshot = snapshot("<p id='ref'>Una <b>casa</b><a href='#note' role='doc-noteref'>1</a>.</p><p id='note'><a name='named'></a>Nota<a role='doc-backlink' href='#ref'>Back</a>.</p>", &["A house.", "Note."]);
    let before = snapshot.source_html.clone();
    let preview = parse_html_document(snapshot.source_preview().unwrap());
    let prefix = format!(
        "papercut-review-source-{}-",
        snapshot.document.expected.generation
    );
    assert!(preview.select_first(&format!("#{prefix}note")).is_ok());
    let hrefs: Vec<_> = preview
        .select("a[href]")
        .unwrap()
        .map(|n| n.attributes.borrow().get("href").unwrap().to_owned())
        .collect();
    assert_eq!(hrefs, [format!("#{prefix}note"), format!("#{prefix}ref")]);
    assert_eq!(
        preview
            .select_first("a[name]")
            .unwrap()
            .attributes
            .borrow()
            .get("name"),
        Some(format!("{prefix}named").as_str())
    );
    assert_eq!(snapshot.source_html, before);
    assert!(snapshot.document.blocks[0]
        .remaining()
        .contains(&Obligation::Note));
}

#[test]
fn authored_prefix_collisions_and_percent_encoded_fragments_keep_distinct_targets() {
    let generation = "12".repeat(16);
    let collision = format!("papercut-review-source-{generation}-note x");
    let source = format!("<p id='note x'><b>Uno</b><a href='#note%20x'>nota</a>.</p><p id='{collision}'><i>Dos</i>.</p>");
    let snapshot = snapshot(&source, &["One note.", "Two."]);
    let preview = parse_html_document(snapshot.source_preview().unwrap());
    let prefix = format!("papercut-review-source-{generation}-1-");
    let ids: BTreeSet<_> = preview
        .select("[id]")
        .unwrap()
        .map(|n| n.attributes.borrow().get("id").unwrap().to_owned())
        .collect();
    assert!(ids.contains(&format!("{prefix}note x")));
    assert!(!ids.contains(&collision));
    assert_eq!(
        preview
            .select_first("a")
            .unwrap()
            .attributes
            .borrow()
            .get("href"),
        Some(format!("#{prefix}note%20x").as_str())
    );
    // Prefixing the encoded spelling preserves browser decoding to the namespaced id with a space.
    assert!(ids.contains(&format!("{prefix}note%20x").replace("%20", " ")));
}

#[test]
fn native_draft_replacement_cannot_change_generation_variant_or_policy() {
    let snapshot = snapshot("<p><b>Casa</b>.</p>", &["House."]);
    for index in 0..4 {
        let mut document = snapshot.document.clone();
        match index {
            0 => document.expected.generation = "34".repeat(16),
            1 => document.expected.variant_id = "abc3".into(),
            2 => document.expected.policy = "future-policy".into(),
            _ => document.expected.schema_version += 1,
        }
        assert_eq!(snapshot.with_document(document), Err(Error::Stale));
    }
    assert_eq!(snapshot.reconstruct().unwrap(), snapshot.baseline_html);
}

#[test]
fn persisted_native_html_must_be_sanitized_and_within_structural_limits() {
    assert!(matches!(
        checked_plan("<p onclick='evil()'>Safe</p>"),
        Err(Error::Evidence)
    ));
    assert!(matches!(
        checked_plan("<p><script>bad()</script>Safe</p>"),
        Err(Error::Evidence)
    ));
    let deep = format!(
        "{}text{}",
        "<div>".repeat(MAX_DEPTH + 1),
        "</div>".repeat(MAX_DEPTH + 1)
    );
    assert!(matches!(checked_plan(&deep), Err(Error::Limit)));
    let text = "x".repeat(MAX_BLOCK_BYTES + 1);
    let node = parse_html_document(format!("<p>{text}</p>"))
        .select_first("p")
        .unwrap()
        .as_node()
        .clone();
    assert!(matches!(
        collect(&node, &mut Geometry::default()),
        Err(Error::Limit)
    ));
}

#[test]
fn draft_reconstruction_preserves_unicode_exactly_and_rejects_grapheme_splits() {
    let mut snapshot = snapshot("<p><b>Texto</b>.</p>", &["Cafe\u{301} 🙂 <safe> & exact."]);
    let before = snapshot.clone();
    assert_eq!(
        snapshot.apply(&edit(&snapshot, 0, vec![bold(0, 4)])),
        Err(Error::Range)
    );
    assert_eq!(snapshot, before);
    snapshot
        .apply(&edit(
            &snapshot,
            0,
            vec![
                bold(0, 5),
                Mark {
                    range: ScalarRange { start: 6, end: 7 },
                    kind: MarkKind::Italic,
                },
            ],
        ))
        .unwrap();
    let html = snapshot.reconstruct().unwrap();
    assert!(html.contains("&lt;safe&gt; &amp; exact."));
    assert_eq!(
        DomPlan::new(&html).blocks[0].inline.text,
        "Cafe\u{301} 🙂 <safe> & exact."
    );
    assert_eq!(
        snapshot.document.blocks[0].text,
        "Cafe\u{301} 🙂 <safe> & exact."
    );
}

#[test]
fn corrupt_deserialized_evidence_and_draft_metadata_are_rejected() {
    let snapshot = snapshot("<p>Una <b>casa</b>.</p>", &["A house."]);
    let value = serde_json::to_value(&snapshot).unwrap();
    for pointer in [
        "/document/blocks/0/text",
        "/document/blocks/0/structureIdentity",
        "/document/blocks/0/key/sourceIdentity",
        "/sourceHtml",
        "/baselineHtml",
        "/normalizationPolicy",
    ] {
        let mut changed = value.clone();
        *changed.pointer_mut(pointer).unwrap() = "changed".into();
        let corrupt: Snapshot = serde_json::from_value(changed).unwrap();
        assert!(corrupt.validate().is_err(), "{pointer}");
    }
    for (pointer, replacement) in [
        (
            "/document/blocks/0/locked",
            serde_json::json!([{"start":0,"end":1}]),
        ),
        (
            "/document/blocks/0/fixed",
            serde_json::json!([{"range":{"start":0,"end":1},"kind":"bold"}]),
        ),
        ("/document/blocks/0/obligations", serde_json::json!([])),
        ("/mappings/0/targetPath", serde_json::json!([99])),
    ] {
        let mut changed = value.clone();
        *changed.pointer_mut(pointer).unwrap() = replacement;
        let corrupt: Snapshot = serde_json::from_value(changed).unwrap();
        assert!(corrupt.validate().is_err(), "{pointer}");
    }
    let mut unknown = value;
    unknown["clientHtml"] = "<script>bad</script>".into();
    assert!(serde_json::from_value::<Snapshot>(unknown).is_err());
}

#[test]
fn structural_english_keeps_fixed_marks_links_literals_notes_and_attachments() {
    // Exercise a source-owned mapped shell, independent of the plain fallback renderer.
    let html = crate::document_uploads::sanitize_html("<p id='p'>The <strong>house <em>is</em></strong> <code>API</code><a href='#n'>link</a><a role='doc-noteref' href='#n'>1</a><img alt='image' data-papercut-asset='image-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.png'> end.</p>");
    let plan = checked_plan(&html).unwrap();
    let node = &plan.blocks[0].node;
    let mapping = Mapping {
        ordinal: 0,
        source_path: path(&plan.document, node).unwrap(),
        target_path: path(&plan.document, node).unwrap(),
        needs_review: false,
        source_formatting: false,
    };
    let block = capture_block(node, node, &mapping).unwrap();
    let document = Document::new(
        "opus-mt-es-en-ctranslate2",
        "abc2".into(),
        "12".repeat(16),
        Evidence {
            normalization_policy: "source-emphasis-v1",
            source_snapshot: &html,
            published_html: &html,
        },
        vec![block],
    )
    .unwrap();
    let mut snapshot = Snapshot {
        source_html: html.clone(),
        baseline_html: html.clone(),
        document,
        normalization_policy: "source-emphasis-v1".into(),
        mappings: vec![mapping],
    };
    let target = snapshot.document.blocks[0].text.clone();
    assert_eq!(target, "The house is APIlink1 end.");
    for marks in [
        vec![bold(0, 17)],
        vec![bold(13, 16)],
        vec![bold(16, 20)],
        vec![bold(19, 24)],
    ] {
        let before = snapshot.clone();
        assert_eq!(
            snapshot.apply(&edit(&snapshot, 0, marks)),
            Err(Error::Locked)
        );
        assert_eq!(snapshot, before);
    }
    snapshot
        .apply(&edit(&snapshot, 0, vec![bold(4, 9)]))
        .unwrap();
    let output = snapshot.reconstruct().unwrap();
    let doc = parse_html_document(&output);
    assert_eq!(
        doc.select_first("strong > b").unwrap().text_contents(),
        "house"
    );
    assert_eq!(
        doc.select_first("strong > em").unwrap().text_contents(),
        "is"
    );
    assert_eq!(doc.select_first("code").unwrap().text_contents(), "API");
    assert_eq!(
        doc.select_first("a[role='doc-noteref']")
            .unwrap()
            .text_contents(),
        "1"
    );
    assert_eq!(doc.select("img").unwrap().count(), 1);
    assert_eq!(doc.select_first("p").unwrap().text_contents(), target);
    assert_eq!(
        doc.select_first("p").unwrap().attributes.borrow().get("id"),
        Some("p")
    );
    assert_eq!(snapshot.baseline_html, html);
}

#[test]
fn source_only_attachments_and_import_gaps_stay_pending_after_emphasis_approval() {
    let mut snapshot = snapshot(
        "<p>Una <b>casa</b><img data-papercut-asset='image-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.png'> y <u>texto</u>.</p>",
        &["A house and text."],
    );
    snapshot.mappings[0].source_formatting = true;
    snapshot.document.blocks[0]
        .obligations
        .insert(Obligation::SourceFormatting);
    snapshot.validate().unwrap();
    let request = EditRequest {
        expected: snapshot.document.expected.clone(),
        block: snapshot.document.blocks[0].key.clone(),
        edit: Edit::Approve {},
    };
    snapshot.apply(&request).unwrap();
    let remaining = snapshot.document.blocks[0].remaining();
    assert!(!remaining.contains(&Obligation::Emphasis));
    for obligation in [
        Obligation::Attachment,
        Obligation::Typography,
        Obligation::SourceFormatting,
    ] {
        assert!(remaining.contains(&obligation));
    }
    assert!(snapshot
        .source_preview()
        .unwrap()
        .contains("data-papercut-asset=\"image-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.png\""));
}
