//! Editor presentation derives blockers without changing immutable evidence or publication admission.
use super::{
    publication_tests::{approve_all, mapped_snapshot},
    tests::{bold, edit, snapshot},
    *,
};

#[test]
fn comparison_fragments_follow_native_order_and_share_full_preview_namespace() {
    let saved = snapshot(
        "<h2 id='heading'>Titre</h2><blockquote><p id='first'>Une <strong>maison</strong>.</p></blockquote><p id='second'>Un <em>arbre</em>.</p>",
        &["Title", "A house.", "A tree."],
    );
    let frozen = saved.clone();
    let preview = saved.editor_preview().unwrap();
    assert_eq!(preview.blocks.len(), saved.document.blocks.len());
    let full = parse_html_document(&preview.source_html);
    for (index, expected) in ["Titre", "Une maison.", "Un arbre."]
        .into_iter()
        .enumerate()
    {
        let block = parse_html_document(&preview.blocks[index].source_html);
        let owner = block.select_first("[id]").unwrap();
        let attrs = owner.attributes.borrow();
        let id = attrs.get("id").unwrap();
        assert!(id.starts_with("papercut-review-source-"));
        assert_eq!(
            full.select_first(&format!("#{id}"))
                .unwrap()
                .text_contents(),
            expected
        );
        assert_eq!(owner.text_contents(), expected);
        assert!(preview.blocks[index].blockers.is_empty());
    }
    assert!(preview.blocks[1]
        .source_html
        .contains("<strong>maison</strong>"));
    assert!(preview.blocks[2].source_html.contains("<em>arbre</em>"));
    assert_eq!(preview.publication_error, None);
    assert_eq!(saved, frozen);
}

#[test]
fn source_only_structure_blocks_old_drafts_before_and_after_emphasis_approval() {
    // These structures were not stored as dedicated obligations in the original v1 evidence schema.
    for (source, blocker) in [
        (
            "<p>Une <b>maison</b><code>API</code>.</p>",
            "sourceOnlyLiteral",
        ),
        ("<pre><b>API</b> value</pre>", "sourceOnlyLiteral"),
        (
            "<p>Une <b>maison</b><span translate='no'>API</span>.</p>",
            "sourceOnlyLiteral",
        ),
        (
            "<p>Une <b>maison</b>.<br>Autre phrase.</p>",
            "sourceOnlyStructure",
        ),
        (
            "<p>Une <b id='nested'>maison</b>.</p>",
            "sourceOnlyStructure",
        ),
    ] {
        let mut saved = snapshot(source, &["A house."]);
        assert_eq!(
            saved.document.blocks[0].obligations,
            BTreeSet::from([Obligation::Emphasis])
        );
        let restored: Snapshot =
            serde_json::from_str(&serde_json::to_string(&saved).unwrap()).unwrap();
        assert_eq!(
            restored.editor_preview().unwrap().blocks[0].blockers,
            [blocker]
        );
        assert_eq!(restored.editor_preview().unwrap().publication_error, None);
        approve_all(&mut saved);
        let frozen = saved.clone();
        let preview = saved.editor_preview().unwrap();
        assert_eq!(preview.blocks[0].blockers, [blocker], "{source}");
        assert_eq!(
            preview.publication_error.as_deref(),
            Some("publication-blocked")
        );
        assert_eq!(
            saved.publication(&saved.document.expected),
            Err(Error::Pending)
        );
        assert_eq!(saved, frozen);
    }
}

#[test]
fn unresolved_nonemphasis_obligations_are_explicit_and_mapped_structure_is_not_blocked() {
    for (source, code) in [
        ("<p>Une <b>maison</b><a href='#n'>link</a>.</p>", "link"),
        ("<p>Une <b>maison</b><a href='#n' role='doc-noteref'>1</a>.</p>", "note"),
        ("<p>Une <b>maison</b><u>souligné</u>.</p>", "typography"),
        ("<p>Une <b>maison</b><img alt='image' data-papercut-asset='image-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.png'>.</p>", "attachment"),
    ] {
        let mut saved = snapshot(source, &["A house."]);
        approve_all(&mut saved);
        assert!(saved.editor_preview().unwrap().blocks[0].blockers.iter().any(|blocker| blocker == code), "{source}");
        assert_eq!(saved.publication(&saved.document.expected), Err(Error::Pending));
    }
    let mut warning = snapshot("<p>Une <b>maison</b>.</p>", &["A house."]);
    warning.mappings[0].source_formatting = true;
    warning.document.blocks[0]
        .obligations
        .insert(Obligation::SourceFormatting);
    approve_all(&mut warning);
    assert_eq!(
        warning.editor_preview().unwrap().blocks[0].blockers,
        ["sourceFormatting"]
    );
    assert_eq!(
        warning.publication(&warning.document.expected),
        Err(Error::Pending)
    );
    let saved = mapped_snapshot("<p id='p'>The <b>house</b> <code>API</code><a role='doc-noteref' href='#note'>1</a>.</p><p id='note'>Note.</p>");
    let preview = saved.editor_preview().unwrap();
    assert!(preview.blocks.iter().all(|block| block.blockers.is_empty()));
    assert_eq!(preview.publication_error, None);
    assert!(saved.publication(&saved.document.expected).is_ok());
}

#[test]
fn approved_global_preflight_error_does_not_turn_valid_draft_loading_into_failure() {
    let wrappers = MAX_DEPTH - 4;
    let mut saved = mapped_snapshot(&format!(
        "{}<p>English.</p>{}",
        "<div>".repeat(wrappers),
        "</div>".repeat(wrappers)
    ));
    saved.apply(&edit(&saved, 0, vec![bold(0, 7)])).unwrap();
    assert_eq!(saved.editor_preview().unwrap().publication_error, None);
    approve_all(&mut saved);
    let frozen = saved.clone();
    assert_eq!(
        saved.editor_preview().unwrap().publication_error.as_deref(),
        Some("publication-limit")
    );
    assert_eq!(
        saved.publication(&saved.document.expected),
        Err(Error::Limit)
    );
    assert_eq!(saved, frozen);
}

#[test]
fn valid_editor_preview_is_repeatable_and_cannot_mutate_future_publication() {
    let mut saved = snapshot(
        "<p id='p'>Une <strong>maison</strong>.</p>",
        &["A  house.\nSame English."],
    );
    saved.apply(&edit(&saved, 0, vec![bold(3, 8)])).unwrap();
    approve_all(&mut saved);
    let frozen = saved.clone();
    let publication = saved.publication(&saved.document.expected).unwrap();
    let preview = saved.editor_preview().unwrap();
    assert_eq!(preview.publication_error, None);
    assert!(preview.target_html.contains("<b>house</b>"));
    assert!(preview
        .target_html
        .contains("data-papercut-translation-review"));
    assert_eq!(
        saved.editor_preview().unwrap().target_html,
        preview.target_html
    );
    assert_eq!(
        saved.publication(&saved.document.expected).unwrap(),
        publication
    );
    assert_eq!(saved, frozen);
}

#[test]
fn inherited_emphasis_reaches_each_source_fragment_and_entire_english_block() {
    for (source, english, selector, kinds) in [
        (
            "<strong><p>Bonjour.</p><p>Salut.</p></strong>",
            vec!["Hello.", "Hi 🌅."],
            "strong > p",
            vec!["bold"],
        ),
        (
            "<em><blockquote><p>Bonjour.</p></blockquote></em>",
            vec!["Hello."],
            "em > p",
            vec!["italic"],
        ),
        (
            "<b><strong><em><blockquote><p>Bonjour.</p></blockquote></em></strong></b>",
            vec!["Hello."],
            "em > strong > p",
            vec!["bold", "italic"],
        ),
    ] {
        let saved = snapshot(source, &english);
        let preview = saved.editor_preview().unwrap();
        for (index, block) in preview.blocks.iter().enumerate() {
            let comparison = parse_html_document(&block.source_html);
            assert!(comparison.select_first(selector).is_ok(), "{source}");
            let encoded = serde_json::to_value(block).unwrap();
            let expected = kinds
                .iter()
                .map(|kind| {
                    serde_json::json!({
                        "kind": kind,
                        "range": { "start": 0, "end": english[index].chars().count() },
                    })
                })
                .collect::<Vec<_>>();
            assert_eq!(encoded["inheritedMarks"], serde_json::json!(expected));
            assert!(saved.document.blocks[index].fixed.is_empty());
        }
    }
}

#[test]
fn inherited_presentation_does_not_duplicate_owned_or_descendant_emphasis() {
    let saved = mapped_snapshot(
        "<strong><p>A <em>quiet</em> evening.</p></strong><p>A <b>bright</b> morning.</p>",
    );
    let preview = saved.editor_preview().unwrap();
    let first = parse_html_document(&preview.blocks[0].source_html);
    assert_eq!(first.select("strong").unwrap().count(), 1);
    assert_eq!(first.select("em").unwrap().count(), 1);
    assert_eq!(saved.document.blocks[0].fixed.len(), 1);
    assert_eq!(saved.document.blocks[0].fixed[0].kind, MarkKind::Italic);
    assert_eq!(
        serde_json::to_value(&preview.blocks[0]).unwrap()["inheritedMarks"],
        serde_json::json!([{ "kind": "bold", "range": { "start": 0, "end": 16 } }])
    );
    let second = parse_html_document(&preview.blocks[1].source_html);
    assert!(second.select_first("strong").is_err());
    assert_eq!(second.select("b").unwrap().count(), 1);
    assert_eq!(saved.document.blocks[1].fixed.len(), 1);
    assert_eq!(
        serde_json::to_value(&preview.blocks[1]).unwrap()["inheritedMarks"],
        serde_json::json!([])
    );
    // An emphasis owner is already captured by the existing immutable geometry.
    let owner = parse_html_document("<strong>Owned.</strong>")
        .select_first("strong")
        .unwrap()
        .as_node()
        .clone();
    let mut geometry = Geometry::default();
    collect(&owner, &mut geometry).unwrap();
    assert_eq!(geometry.fixed, vec![bold(0, 6)]);
    assert!(inherited_emphasis(&owner).is_empty());
}

#[test]
fn ancestor_presentation_does_not_migrate_saved_evidence_or_publication() {
    let mut saved = snapshot(
        "<strong><em><blockquote><p>Bonjour.</p><p>Salut.</p></blockquote></em></strong>",
        &["Hello.", "Hi."],
    );
    let before = serde_json::to_string(&saved).unwrap();
    assert!(!before.contains("inheritedMarks"));
    let mut resumed: Snapshot = serde_json::from_str(&before).unwrap();
    resumed.validate().unwrap();
    let preview = resumed.editor_preview().unwrap();
    assert_eq!(serde_json::to_string(&resumed).unwrap(), before);
    assert_eq!(serde_json::to_string(&saved).unwrap(), before);
    assert!(preview.blocks.iter().all(|block| {
        serde_json::to_value(block).unwrap()["inheritedMarks"]
            .as_array()
            .unwrap()
            .len()
            == 2
    }));
    approve_all(&mut saved);
    approve_all(&mut resumed);
    let output = resumed.publication(&resumed.document.expected).unwrap();
    assert_eq!(saved.publication(&saved.document.expected).unwrap(), output);
    let published = parse_html_document(&output);
    assert_eq!(published.select("strong").unwrap().count(), 1);
    assert_eq!(published.select("em").unwrap().count(), 1);
    assert_eq!(
        published
            .select("strong > em > blockquote > p")
            .unwrap()
            .count(),
        2
    );
    assert_eq!(
        published
            .select_first("blockquote")
            .unwrap()
            .text_contents(),
        "Hello.Hi."
    );
    assert!(resumed
        .document
        .blocks
        .iter()
        .all(|block| block.fixed.is_empty() && block.editable.is_empty()));
}
