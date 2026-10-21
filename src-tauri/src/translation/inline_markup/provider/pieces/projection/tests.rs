use super::*;
use crate::translation::{
    dom::DomPlan,
    inline_markup::{
        protected::ProtectedSource,
        provider::contract::{
            tests::{rules, tokens},
            TokenizedText,
        },
    },
};
use std::cell::Cell;

struct Translator {
    limit: usize,
}
impl ExactPieceTranslator for Translator {
    fn source_fits(&self, source: &str) -> Result<bool, String> {
        Ok(source.split_whitespace().count() <= self.limit)
    }
    fn translate_exact(&mut self, sources: &[&str]) -> Result<Vec<String>, String> {
        Ok(sources.iter().map(|s| s.to_uppercase()).collect())
    }
}
fn plan(html: &str, limit: usize) -> SourcePlan {
    let dom = DomPlan::new(html);
    let block = &dom.blocks[0];
    let source = ProtectedSource::new(&block.node).unwrap();
    let segment = TranslationTextSegment {
        id: "b0:s0".into(),
        source_block_index: 0,
        source_start: 0,
        source_end: block.inline.text.chars().count(),
        text: block.inline.text.clone(),
    };
    SourcePlan::new(&segment, &source, &Translator { limit }, |_, _| Ok(true)).unwrap()
}
fn align<'a>(source: &'a str, target: &'a str) -> Result<WordLinks<'a>, Rejection> {
    let st = tokens(source);
    let tt = tokens(target);
    let links = (0..st.len() - 2).map(|i| (i, i)).collect();
    WordLinks::new(
        source,
        target,
        &rules(),
        &TokenizedText {
            normalized: source,
            tokens: &st,
        },
        &TokenizedText {
            normalized: target,
            tokens: &tt,
        },
        links,
    )
}

#[test]
fn both_test_providers_produce_the_same_mapping_without_engine_specific_hints() {
    struct ProviderB;
    impl ExactPieceTranslator for ProviderB {
        fn source_fits(&self, _: &str) -> Result<bool, String> {
            Ok(true)
        }
        fn translate_exact(&mut self, source: &[&str]) -> Result<Vec<String>, String> {
            assert_eq!(source, ["Un livre ici"]);
            Ok(vec!["UN LIVRE ICI".into()])
        }
    }
    let plan = plan("<p>Un <strong><em>livre</em></strong> ici</p>", 10);
    let translated = plan
        .translate(&mut Translator { limit: 10 }, |_, _| Ok(true), || false)
        .unwrap();
    let a = translated.project(align, None, || Ok(())).unwrap();
    let translated_b = plan
        .translate(&mut ProviderB, |_, _| Ok(true), || false)
        .unwrap();
    let b = translated_b.project(align, None, || Ok(())).unwrap();
    assert_eq!(a.nodes(), b.nodes());
    assert!(std::ptr::eq(a.translation, &translated));
    assert_eq!(a.nodes()[0].targets, [3..8]);
    assert_eq!(a.policy(), super::super::super::contract::PROJECTION_POLICY);
}

#[test]
fn actual_subdivisions_and_copied_literal_whitespace_keep_one_authored_identity() {
    let plan = plan(
        "<p><span id='one'>Un deux; Trois <code>secret🙂</code> quatre; Cinq six.</span></p>",
        2,
    );
    let identity = plan.protected.identity();
    let translated = plan
        .translate(&mut Translator { limit: 2 }, |_, _| Ok(true), || false)
        .unwrap();
    let mut calls = 0;
    let mapped = translated
        .project(
            |s, t| {
                calls += 1;
                assert!(!s.contains("secret"));
                align(s, t)
            },
            None,
            || Ok(()),
        )
        .unwrap();
    assert!(calls > 1);
    assert_eq!(
        mapped.nodes()[0].targets,
        [0..translated.text.chars().count()]
    );
    assert_eq!(
        mapped.nodes().iter().filter(|n| n.source_id == 0).count(),
        1
    );
    assert_eq!(plan.protected.identity(), identity);
    assert!(translated.text.contains("secret🙂"));
    let restored = mapped.restore::<Rejection>(|| Ok(())).unwrap();
    assert_eq!(restored.text, translated.text);
    let dom = crate::translation::html::parse_html_document(restored.html);
    assert_eq!(dom.select("#one").unwrap().count(), 1);
    assert_eq!(dom.select("#one > code").unwrap().count(), 1);
    assert_eq!(
        dom.select_first("code").unwrap().text_contents(),
        "secret🙂"
    );
}

#[test]
fn known_scope_blockers_fail_without_inference() {
    let plan = plan("<p><em>Un livre</em></p>", 10);
    let translated = plan
        .translate(&mut Translator { limit: 10 }, |_, _| Ok(true), || false)
        .unwrap();
    for issue in [
        ScopeIssue::Auxiliary,
        ScopeIssue::FusedMeaning,
        ScopeIssue::Attachment,
    ] {
        let error = translated
            .project::<Rejection>(
                |_, _| panic!("known unresolved must not infer"),
                Some(issue),
                || Ok(()),
            )
            .err();
        assert_eq!(error, Some(Rejection::Unresolved(issue)));
    }
}

#[test]
fn stale_pairs_changed_literal_and_partial_failures_never_return_a_completed_map() {
    let plan = plan("<p>Un <em>livre</em></p>", 1);
    let mut translated = plan
        .translate(&mut Translator { limit: 1 }, |_, _| Ok(true), || false)
        .unwrap();
    let mut calls = 0;
    assert_eq!(
        translated
            .project(
                |s, t| {
                    calls += 1;
                    if calls == 2 {
                        return Err(Rejection::UnresolvedSource);
                    }
                    align(s, t)
                },
                None,
                || Ok(()),
            )
            .err(),
        Some(Rejection::UnresolvedSource)
    );
    assert_eq!(calls, 2);
    assert_eq!(
        translated
            .project(|_, _| align("stale", "STALE"), None, || Ok(()))
            .err(),
        Some(Rejection::Binding)
    );
    translated.targets[0].end = usize::MAX;
    assert_eq!(
        translated.project(align, None, || Ok(())).err(),
        Some(Rejection::Binding)
    );

    let plan = self::plan("<p><code>secret</code></p>", 1);
    let mut translated = plan
        .translate(&mut Translator { limit: 1 }, |_, _| Ok(true), || false)
        .unwrap();
    translated.text = "public".into();
    assert_eq!(
        translated.project(align, None, || Ok(())).err(),
        Some(Rejection::Binding)
    );
}

#[derive(Debug, PartialEq, Eq)]
enum CancelError {
    Mapping(Rejection),
    Cancelled,
}
impl From<Rejection> for CancelError {
    fn from(error: Rejection) -> Self {
        Self::Mapping(error)
    }
}

#[test]
fn late_cancellation_between_pairs_stops_before_aligning_the_next_pair() {
    let plan = plan("<p><span id='one'>Un deux; Trois quatre.</span></p>", 2);
    let translated = plan
        .translate(&mut Translator { limit: 2 }, |_, _| Ok(true), || false)
        .unwrap();
    let cancelled = Cell::new(false);
    let calls = Cell::new(0);
    let result = translated.project(
        |s, t| {
            calls.set(calls.get() + 1);
            let links = align(s, t)?;
            cancelled.set(true); // The first successful pair completes before cancellation arrives.
            Ok::<_, CancelError>(links)
        },
        None,
        || {
            if cancelled.get() {
                Err(CancelError::Cancelled)
            } else {
                Ok(())
            }
        },
    );
    assert_eq!(result.err(), Some(CancelError::Cancelled));
    assert_eq!(calls.get(), 1);
    assert!(
        plan.units()
            .iter()
            .filter(|u| u.kind == Kind::Prose)
            .count()
            > 1
    );
}

#[test]
fn late_cancellation_after_final_pair_discards_the_assembled_projection() {
    let plan = plan("<p><span id='one'>Un deux; Trois quatre.</span></p>", 2);
    let translated = plan
        .translate(&mut Translator { limit: 2 }, |_, _| Ok(true), || false)
        .unwrap();
    let pairs = plan
        .units()
        .iter()
        .filter(|u| u.kind == Kind::Prose)
        .count();
    assert_eq!(plan.units().last().unwrap().kind, Kind::Prose);
    // Prove this exact input, all pairs and the final authored-identity assembly succeed without cancellation.
    let control = translated.project(align, None, || Ok(())).unwrap();
    assert_eq!(
        control.nodes()[0].targets,
        [0..translated.text.chars().count()]
    );
    let cancelled = Cell::new(false);
    let calls = Cell::new(0);
    let result = translated.project(
        |s, t| {
            let links = align(s, t)?;
            calls.set(calls.get() + 1);
            if calls.get() == pairs {
                cancelled.set(true);
            }
            Ok::<_, CancelError>(links)
        },
        None,
        || {
            if cancelled.get() {
                Err(CancelError::Cancelled)
            } else {
                Ok(())
            }
        },
    );
    assert_eq!(calls.get(), pairs); // All pairs returned valid links; there is no next piece to catch cancellation.
    assert_eq!(result.err(), Some(CancelError::Cancelled));
}

/// Supply one exact decoded target through the real planning/admission path, not a forged Translation.
struct Fixed<'a>(&'a str);
impl ExactPieceTranslator for Fixed<'_> {
    fn source_fits(&self, _: &str) -> Result<bool, String> {
        Ok(true)
    }
    fn translate_exact(&mut self, sources: &[&str]) -> Result<Vec<String>, String> {
        assert_eq!(sources.len(), 1);
        Ok(vec![self.0.into()])
    }
}

/// Gold lexical anchors ignore punctuation links so quote placement cannot accidentally rely on MT punctuation.
fn lexical_links<'a>(
    source: &'a str,
    target: &'a str,
    pairs: Vec<(usize, usize)>,
) -> Result<WordLinks<'a>, Rejection> {
    let st = tokens(source);
    let tt = tokens(target);
    WordLinks::new(
        source,
        target,
        &rules(),
        &TokenizedText {
            normalized: source,
            tokens: &st,
        },
        &TokenizedText {
            normalized: target,
            tokens: &tt,
        },
        pairs,
    )
}

#[test]
fn forward_reverse_and_nested_quotes_anchor_notes_without_sentence_zipping() {
    let note = "<a role='doc-noteref' id='ref' href='#n'><sup>1</sup></a>";
    for (source, target, expected) in [
        (format!("<p>«Bonjour»{note}.</p>"), "“Hello.”", "“Hello.”<a"),
        (format!("<p>«Bonjour{note}.»</p>"), "“Hello”.", "“Hello<a"),
        (format!("<p>«Bonjour.{note}»</p>"), "“Hello”.", "“Hello<a"),
        (
            format!("<p>«‘Bonjour.{note}’»</p>"),
            "“‘Hello’”.",
            "“‘Hello<a",
        ),
        (
            format!("<p><strong><em>«‘Bonjour.’{note}»</em></strong></p>"),
            "“‘Hello’”.",
            "“‘Hello’<a",
        ),
    ] {
        let p = plan(&source, 20);
        let translated = p
            .translate(&mut Fixed(target), |_, _| Ok(true), || false)
            .unwrap();
        let mapped = translated
            .project(
                |s, t| {
                    let lexical = |text: &str| {
                        tokens(text)
                            .into_iter()
                            .skip(1)
                            .take_while(|t| !t.special)
                            .enumerate()
                            .filter(|(_, token)| {
                                text[token.bytes.clone()].chars().any(char::is_alphanumeric)
                            })
                            .map(|(i, _)| i)
                            .next()
                            .unwrap()
                    };
                    lexical_links(s, t, vec![(lexical(s), lexical(t))])
                },
                None,
                || Ok(()),
            )
            .unwrap();
        let r = mapped.restore::<Rejection>(|| Ok(())).unwrap();
        assert_eq!(r.text, target);
        assert!(r.html.contains(expected), "{}", r.html);
        let dom = crate::translation::html::parse_html_document(r.html);
        assert_eq!(dom.select("a#ref[href='#n'] > sup").unwrap().count(), 1);
        if source.contains("<strong>") {
            assert_eq!(dom.select("strong > em > a#ref").unwrap().count(), 1);
        }
    }
}

#[test]
fn multiple_identical_note_labels_follow_lexical_owners_and_source_order_at_piece_cuts() {
    let p = plan("<p><span id='one'>Un deux;<a role='doc-noteref' id='r1' href='#n'>1</a><a role='doc-noteref' id='r2' href='#n'>1</a> Trois quatre.</span></p>", 2);
    let translated = p
        .translate(&mut Translator { limit: 2 }, |_, _| Ok(true), || false)
        .unwrap();
    assert!(p.units().len() > 1);
    let mapped = translated.project(align, None, || Ok(())).unwrap();
    let r = mapped.restore::<Rejection>(|| Ok(())).unwrap();
    assert_eq!(r.text, "UN DEUX; TROIS QUATRE.");
    let dom = crate::translation::html::parse_html_document(r.html);
    assert_eq!(dom.select("#one").unwrap().count(), 1);
    assert_eq!(dom.select("#one > #r1 + #r2").unwrap().count(), 1);
    assert_eq!(dom.select("#r1,#r2").unwrap().count(), 2);
}

#[test]
fn empty_styled_link_image_and_note_keep_source_hierarchy_and_asset_reference() {
    let asset =
        crate::document_uploads::ParsedDocumentAsset::new("image/png", b"owned pixels".to_vec())
            .unwrap();
    let p = plan(&format!("<p><a id='image-link' href='../notes.xhtml#n'><strong><img data-papercut-asset='{}'></strong></a> Bonjour<a role='doc-noteref' id='ref' href='../notes.xhtml#n'>1</a>.</p>", asset.file_name), 20);
    let translated = p
        .translate(&mut Translator { limit: 20 }, |_, _| Ok(true), || false)
        .unwrap();
    let r = translated
        .project(align, None, || Ok(()))
        .unwrap()
        .restore::<Rejection>(|| Ok(()))
        .unwrap();
    assert_eq!(r.text, "BONJOUR.");
    let dom = crate::translation::html::parse_html_document(r.html);
    let image = dom.select_first("a#image-link > strong > img").unwrap();
    assert_eq!(
        image.attributes.borrow().get("data-papercut-asset"),
        Some(asset.file_name.as_str())
    );
    assert_eq!(
        dom.select_first("a#image-link")
            .unwrap()
            .attributes
            .borrow()
            .get("href"),
        Some("../notes.xhtml#n")
    );
    assert_eq!(dom.select("#image-link,#ref").unwrap().count(), 2);
    assert_eq!(asset.bytes, b"owned pixels");
}

#[test]
fn opaque_mixed_content_table_preserves_cells_literals_and_owned_images_once() {
    let asset = crate::document_uploads::ParsedDocumentAsset::new(
        "image/png",
        b"table image bytes".to_vec(),
    )
    .unwrap();
    let p = plan(&format!("<table id='grid'><tr><th><b>Nom</b></th><td>Oui <code>id42</code><img data-papercut-asset='{}'></td></tr></table>", asset.file_name), 20);
    let identity = p.protected.identity();
    let translated = p
        .translate(&mut Translator { limit: 20 }, |_, _| Ok(true), || false)
        .unwrap();
    let mapped = translated.project(align, None, || Ok(())).unwrap();
    let restored = mapped.restore::<Rejection>(|| Ok(())).unwrap();
    // The existing source planner treats this whole table as an immutable island, not new translatable cells.
    assert_eq!(restored.text, "Nom Oui id42");
    let dom = crate::translation::html::parse_html_document(restored.html);
    assert_eq!(
        dom.select("table#grid > tbody > tr > th > b")
            .unwrap()
            .count(),
        1
    );
    assert_eq!(dom.select_first("th").unwrap().text_contents(), "Nom");
    assert_eq!(dom.select("table#grid").unwrap().count(), 1);
    assert_eq!(
        dom.select_first("td > code").unwrap().text_contents(),
        "id42"
    );
    assert_eq!(
        dom.select_first("td > img")
            .unwrap()
            .attributes
            .borrow()
            .get("data-papercut-asset"),
        Some(asset.file_name.as_str())
    );
    assert_eq!(p.protected.identity(), identity);
}

#[test]
fn attachments_in_copied_units_do_not_enter_either_model() {
    let p = plan(
        "<p><code>abc</code><a role='doc-noteref' id='ref' href='#n'>1</a></p>",
        20,
    );
    let translated = p
        .translate(
            &mut Fixed("must not infer"),
            |_, _| panic!("literal must not enter tokenizer"),
            || false,
        )
        .unwrap();
    let mapped = translated
        .project::<Rejection>(
            |_, _| panic!("literal must not enter aligner"),
            None,
            || Ok(()),
        )
        .unwrap();
    let restored = mapped.restore::<Rejection>(|| Ok(())).unwrap();
    assert_eq!(restored.text, "abc");
    let dom = crate::translation::html::parse_html_document(restored.html);
    assert_eq!(dom.select("code + a#ref[href='#n']").unwrap().count(), 1);
}

#[test]
fn standalone_image_line_break_and_backreference_are_each_restored_once() {
    let p = plan("<p><img id='cover' src='../images/cover.png'>Bonjour<br> monde<a role='doc-backlink' id='back' href='#ref'>↩</a>.</p>", 20);
    let translated = p
        .translate(&mut Translator { limit: 20 }, |_, _| Ok(true), || false)
        .unwrap();
    let r = translated
        .project(align, None, || Ok(()))
        .unwrap()
        .restore::<Rejection>(|| Ok(()))
        .unwrap();
    assert_eq!(r.text, "BONJOUR MONDE.");
    let dom = crate::translation::html::parse_html_document(r.html);
    assert_eq!(
        dom.select("img#cover[src='../images/cover.png']")
            .unwrap()
            .count(),
        1
    );
    assert_eq!(dom.select("br").unwrap().count(), 1);
    assert_eq!(dom.select("a#back[href='#ref']").unwrap().count(), 1);
    assert_eq!(dom.select_first("a#back").unwrap().text_contents(), "↩");
}

#[test]
fn missing_fused_reversed_or_changed_punctuation_attachment_evidence_rejects() {
    let p = plan("<p>Un<a role='doc-noteref' href='#n'>1</a> deux</p>", 20);
    for (target, pairs) in [
        ("ONE TWO", vec![(0, 0)]),
        ("ONE TWO", vec![(0, 0), (0, 1), (1, 1)]),
        ("TWO ONE", vec![(0, 1), (1, 0)]),
        ("ONE extra TWO", vec![(0, 0), (1, 2)]),
    ] {
        let translated = p
            .translate(&mut Fixed(target), |_, _| Ok(true), || false)
            .unwrap();
        assert_eq!(
            translated
                .project(|s, t| lexical_links(s, t, pairs.clone()), None, || Ok(()))
                .err(),
            Some(Rejection::AttachmentRequiresEvidence)
        );
    }
    let p = plan("<p>Bonjour<a role='doc-noteref' href='#n'>1</a>.</p>", 20);
    let translated = p
        .translate(&mut Fixed("HELLO!"), |_, _| Ok(true), || false)
        .unwrap();
    assert_eq!(
        translated
            .project(|s, t| lexical_links(s, t, vec![(0, 0)]), None, || Ok(()))
            .err(),
        Some(Rejection::AttachmentRequiresEvidence)
    );
}

#[test]
fn leading_notes_stay_inside_nonempty_authored_parents_across_whitespace_and_pieces() {
    let note = "<a role='doc-noteref' id='ref' href='#n'><sup>1</sup></a>";
    for prefix in ["Un", "Été🙂"] {
        for limit in [20, 1] {
            for shell in [
                format!("<span id='phrase'>{note}deux</span>"),
                format!("<span id='phrase'><strong id='style'>{note}deux</strong></span>"),
            ] {
                let p = plan(&format!("<p>{prefix} {shell}</p>"), limit);
                let t = p
                    .translate(&mut Translator { limit }, |_, _| Ok(true), || false)
                    .unwrap();
                let mapped = t.project(align, None, || Ok(())).unwrap();
                let restored = mapped.restore::<Rejection>(|| Ok(())).unwrap();
                assert_eq!(restored.text, format!("{} DEUX", prefix.to_uppercase()));
                let dom = crate::translation::html::parse_html_document(restored.html);
                assert_eq!(dom.select("#phrase").unwrap().count(), 1);
                assert_eq!(dom.select("#ref[href='#n'] > sup").unwrap().count(), 1);
                let parent = if shell.contains("strong") {
                    "#phrase > strong#style"
                } else {
                    "#phrase"
                };
                assert_eq!(dom.select(&format!("{parent} > a#ref")).unwrap().count(), 1);
                assert_eq!(
                    dom.select_first(parent)
                        .unwrap()
                        .as_node()
                        .first_child()
                        .unwrap()
                        .as_element()
                        .unwrap()
                        .attributes
                        .borrow()
                        .get("id"),
                    Some("ref")
                );
                assert_eq!(
                    crate::translation::inline_markup::source_inline_model(
                        dom.select_first(parent).unwrap().as_node()
                    )
                    .text,
                    "DEUX"
                );
                assert_eq!(dom.select_first("#ref").unwrap().text_contents(), "1");
            }
        }
    }
}

#[test]
fn leading_images_stay_inside_nonempty_styled_links_without_cloning_ids_or_assets() {
    let asset = crate::document_uploads::ParsedDocumentAsset::new(
        "image/png",
        b"leading image bytes".to_vec(),
    )
    .unwrap();
    for prefix in ["Un", "Été🙂"] {
        for limit in [20, 1] {
            for caption_in_style in [true, false] {
                let image = format!(
                    "<img id='picture' data-papercut-asset='{}'>",
                    asset.file_name
                );
                // An empty style must resolve before its image, using the completed nonempty link's range.
                let content = if caption_in_style {
                    format!("<strong id='style'>{image}deux</strong>")
                } else {
                    format!("<strong id='style'>{image}</strong>deux")
                };
                let p = plan(
                    &format!(
                        "<p>{prefix} <a id='phrase' href='../notes.xhtml#n'>{content}</a></p>"
                    ),
                    limit,
                );
                let t = p
                    .translate(&mut Translator { limit }, |_, _| Ok(true), || false)
                    .unwrap();
                let mapped = t.project(align, None, || Ok(())).unwrap();
                let restored = mapped.restore::<Rejection>(|| Ok(())).unwrap();
                assert_eq!(restored.text, format!("{} DEUX", prefix.to_uppercase()));
                let dom = crate::translation::html::parse_html_document(restored.html);
                let image = dom
                    .select_first("a#phrase[href='../notes.xhtml#n'] > strong#style > img#picture")
                    .unwrap();
                assert_eq!(
                    image.attributes.borrow().get("data-papercut-asset"),
                    Some(asset.file_name.as_str())
                );
                for id in ["phrase", "style", "picture"] {
                    assert_eq!(dom.select(&format!("#{id}")).unwrap().count(), 1);
                }
                let style = dom.select_first("strong#style").unwrap();
                assert_eq!(
                    style
                        .as_node()
                        .first_child()
                        .unwrap()
                        .as_element()
                        .unwrap()
                        .attributes
                        .borrow()
                        .get("id"),
                    Some("picture")
                );
                assert_eq!(
                    crate::translation::inline_markup::source_inline_model(style.as_node()).text,
                    if caption_in_style { "DEUX" } else { "" }
                );
                assert_eq!(
                    crate::translation::inline_markup::source_inline_model(
                        dom.select_first("#phrase").unwrap().as_node()
                    )
                    .text,
                    "DEUX"
                );
            }
        }
    }
}

#[test]
fn attachment_parent_conflict_cannot_move_a_marker_outside_its_checked_gap() {
    for marker in [
        "<a role='doc-noteref' id='ref' href='#n'>1</a>",
        "<img id='picture' src='../images/picture.png'>",
    ] {
        let p = plan(
            &format!("<p>Un <em id='phrase'>{marker};</em> deux;</p>"),
            20,
        );
        let t = p
            .translate(&mut Translator { limit: 20 }, |_, _| Ok(true), || false)
            .unwrap();
        // Complete gold links map the styled semicolon to the far end, outside the attachment's lexical gap.
        assert_eq!(
            t.project(
                |s, t| lexical_links(s, t, vec![(0, 0), (1, 3), (2, 2), (3, 1)]),
                None,
                || Ok(())
            )
            .err(),
            Some(Rejection::Hierarchy)
        );
    }
}

#[test]
fn reconstruction_checks_cancellation_before_work_and_before_admitting_output() {
    let p = plan("<p><em>Un livre</em></p>", 20);
    let translated = p
        .translate(&mut Translator { limit: 20 }, |_, _| Ok(true), || false)
        .unwrap();
    let mapped = translated.project(align, None, || Ok(())).unwrap();
    let calls = Cell::new(0);
    let result = mapped.restore(|| {
        calls.set(calls.get() + 1);
        if calls.get() == 2 {
            Err(CancelError::Cancelled)
        } else {
            Ok(())
        }
    });
    assert_eq!(calls.get(), 2);
    assert_eq!(result.err(), Some(CancelError::Cancelled));
    assert!(mapped
        .restore(|| Err::<(), _>(CancelError::Cancelled))
        .is_err());
    assert_eq!(
        mapped.restore::<Rejection>(|| Ok(())).unwrap().text,
        translated.text
    );
}
