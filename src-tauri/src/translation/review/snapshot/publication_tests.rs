use super::tests::{bold, edit, snapshot};
use super::*;

pub(super) fn approve_all(snapshot: &mut Snapshot) {
    for block in snapshot.document.blocks.clone() {
        snapshot
            .apply(&EditRequest {
                expected: snapshot.document.expected.clone(),
                block: block.key,
                edit: Edit::Approve {},
            })
            .unwrap();
    }
}

/// Build a source-owned rich mapping without invoking a provider or its retained-original fallback.
pub(super) fn mapped_snapshot(html: &str) -> Snapshot {
    let html = crate::document_uploads::sanitize_html(html);
    let plan = checked_plan(&html).unwrap();
    let (paths, _) = block_paths(&plan).unwrap();
    let mappings = plan
        .blocks
        .iter()
        .zip(paths)
        .map(|(block, path)| Mapping {
            ordinal: block.ordinal as u32,
            source_path: path.clone(),
            target_path: path,
            needs_review: false,
            source_formatting: false,
        })
        .collect::<Vec<_>>();
    let blocks = plan
        .blocks
        .iter()
        .zip(&mappings)
        .map(|(block, mapping)| capture_block(&block.node, &block.node, mapping).unwrap())
        .collect();
    let document = Document::new(
        "opus-mt-es-en-ctranslate2",
        "abc2".into(),
        "12".repeat(16),
        Evidence {
            normalization_policy: "source-emphasis-v1",
            source_snapshot: &html,
            published_html: &html,
        },
        blocks,
    )
    .unwrap();
    Snapshot {
        source_html: html.clone(),
        baseline_html: html,
        document,
        normalization_policy: "source-emphasis-v1".into(),
        mappings,
    }
}

#[test]
fn publication_requires_current_approval_and_never_changes_the_saved_draft() {
    let mut saved = snapshot("<p>Una <strong>casa</strong>.</p>", &["A house."]);
    assert_eq!(
        saved.publication(&saved.document.expected),
        Err(Error::Pending)
    );
    saved.apply(&edit(&saved, 0, vec![bold(2, 7)])).unwrap();
    approve_all(&mut saved);
    let expected = saved.document.expected.clone();
    let frozen = saved.clone();
    let html = saved.publication(&expected).unwrap();
    assert_eq!(saved.publication(&expected).unwrap(), html);
    assert_eq!(saved, frozen);
    assert!(saved
        .previews()
        .unwrap()
        .1
        .contains("data-papercut-translation-review"));
    assert!(!html.contains("data-papercut-translation-review"));
    assert_eq!(
        parse_html_document(&html)
            .select_first("b")
            .unwrap()
            .text_contents(),
        "house"
    );
    for wrong_generation in [false, true] {
        let mut stale = expected.clone();
        if wrong_generation {
            stale.generation = "34".repeat(16);
        } else {
            stale.revision -= 1;
        }
        assert_eq!(saved.publication(&stale), Err(Error::Stale));
    }
    saved.apply(&edit(&saved, 0, vec![])).unwrap();
    assert_eq!(
        saved.publication(&saved.document.expected),
        Err(Error::Pending)
    );
    assert_eq!(saved.publication(&expected), Err(Error::Stale));
}

#[test]
fn publication_preserves_nested_hierarchy_owner_ids_and_exact_english() {
    for source in [
        "<article><h2 id='title'>Título</h2><blockquote><p id='p' name='named'>Una <strong>casa</strong>.</p></blockquote></article>",
        "<article><h2 id='title'>Título</h2><ol start='3'><li id='p' name='named'>Una <strong>casa</strong>.</li></ol></article>",
    ] {
        let mut saved = snapshot(source, &["Title", "A  house.\nStill\tthere\u{a0}🙂."]);
        saved.apply(&edit(&saved, 1, vec![bold(3, 8)])).unwrap();
        approve_all(&mut saved);
        let baseline = checked_plan(&saved.baseline_html).unwrap();
        let html = saved.publication(&saved.document.expected).unwrap();
        let published = checked_plan(&html).unwrap();
        assert_eq!(consumer_signature(&baseline.document), consumer_signature(&published.document));
        assert_eq!(document_identifiers(&baseline.document), document_identifiers(&published.document));
        assert_eq!(published.blocks[1].node.text_contents(), "A  house.\nStill\tthere\u{a0}🙂.");
        let dom = &published.document;
        assert!(dom.select_first("#title").is_ok());
        assert!(dom.select_first("#p[name='named']").is_ok());
        assert_eq!(dom.select_first("b").unwrap().text_contents(), "house");
        assert_eq!(dom.select("article").unwrap().count(), 1);
        if source.contains("<ol") {
            assert_eq!(dom.select_first("ol[start='3'] > li#p > p").unwrap().text_contents(), "A  house.\nStill\tthere\u{a0}🙂.");
            assert_eq!(dom.select("li").unwrap().count(), 1);
        } else {
            assert!(dom.select_first("blockquote > p#p").is_ok());
        }
    }
    let mut body = snapshot("<body id='body'>Una <em>casa</em>.</body>", &["A house."]);
    approve_all(&mut body);
    let html = body.publication(&body.document.expected).unwrap();
    assert_eq!(DomPlan::new(&html).blocks[0].inline.text, "A house.");
    assert_eq!(
        parse_html_document(&html).select("body").unwrap().count(),
        1
    );
}

#[test]
fn publication_cannot_hide_nonemphasis_losses_behind_paragraph_approval() {
    for source in [
        "<p>Una <b>casa</b><a href='#x'>enlace</a>.</p>",
        "<p>Una <b>casa</b><img data-papercut-asset='image-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.png'>.</p>",
        "<p>Una <b>casa</b><u>subrayado</u>.</p>",
        "<p>Una <b>casa</b><code>API</code>.</p>",
        "<pre><b>API</b> value</pre>",
        "<p>Una <b id='nested'>casa</b>.</p>",
        "<p>Una <b>casa</b><span translate='no'>API</span>.</p>",
        "<p>Una <b>casa</b><a href='#n' role='doc-noteref'>1</a>.</p>",
    ] {
        let mut saved = snapshot(source, &["A house."]);
        approve_all(&mut saved);
        let frozen = saved.clone();
        assert_eq!(saved.publication(&saved.document.expected), Err(Error::Pending), "{source}");
        assert_eq!(saved, frozen);
    }
}

#[test]
fn already_mapped_literal_attachment_and_note_structure_survives_publication() {
    let mut saved = mapped_snapshot("<article><p id='p'>The <strong>house</strong> <code>API</code><a role='doc-noteref' href='#note'>1</a><img alt='diagram' data-papercut-asset='image-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.png'> end.</p><p role='doc-footnote' id='note'>Note<a href='#p' role='doc-backlink'>Back</a>.</p></article>");
    saved.apply(&edit(&saved, 0, vec![bold(0, 3)])).unwrap();
    approve_all(&mut saved);
    let published = saved.publication(&saved.document.expected).unwrap();
    let dom = parse_html_document(&published);
    assert_eq!(dom.select_first("code").unwrap().text_contents(), "API");
    assert_eq!(
        dom.select_first("a[role='doc-noteref']")
            .unwrap()
            .attributes
            .borrow()
            .get("href"),
        Some("#note")
    );
    assert!(dom.select_first("#note > a[href='#p']").is_ok());
    assert_eq!(dom.select("img[data-papercut-asset]").unwrap().count(), 1);
    assert!(dom.select_first("p#p > strong").is_ok());
    assert_eq!(
        consumer_signature(&parse_html_document(&published)),
        consumer_signature(&parse_html_document(&saved.baseline_html))
    );
}

#[test]
fn consumer_guard_ignores_emphasis_splits_but_rejects_other_chunk_input_changes() {
    let baseline = parse_html_document("<p>A red house.</p><ul><li>Second.</li></ul>");
    let styled = parse_html_document("<p>A <b>red <i>house</i></b>.</p><ul><li>Second.</li></ul>");
    assert_eq!(consumer_signature(&baseline), consumer_signature(&styled));
    for changed in [
        "<p>A  red house.</p><ul><li>Second.</li></ul>",
        "<div>A red house.</div><ul><li>Second.</li></ul>",
        "<p>A red house.<br></p><ul><li>Second.</li></ul>",
        "<p>A red house.</p><ul><li role='doc-footnote'>Second.</li></ul>",
        "<ul><li>Second.</li></ul><p>A red house.</p>",
        "<p>A red house.</p><ul><li><a href='#source'>Second.</a></li></ul>",
        "<p>A red house.</p><ul><li>Second.<img alt='new'></li></ul>",
    ] {
        assert_ne!(
            consumer_signature(&baseline),
            consumer_signature(&parse_html_document(changed)),
            "{changed}"
        );
    }
}

#[test]
fn publication_cannot_exceed_native_tree_depth_by_adding_emphasis() {
    let wrappers = MAX_DEPTH - 4;
    let mut saved = mapped_snapshot(&format!(
        "{}<p>English.</p>{}",
        "<div>".repeat(wrappers),
        "</div>".repeat(wrappers)
    ));
    saved.apply(&edit(&saved, 0, vec![bold(0, 7)])).unwrap();
    approve_all(&mut saved);
    let frozen = saved.clone();
    assert_eq!(
        saved.publication(&saved.document.expected),
        Err(Error::Limit)
    );
    assert_eq!(saved, frozen);
}

#[test]
fn source_only_breaks_remain_pending_but_generated_label_breaks_do_not_block() {
    let mut prose = snapshot("<p>Una <b>casa</b>.</p>", &["A house."]);
    approve_all(&mut prose);
    assert!(prose.baseline_html.contains("<br>"));
    assert!(prose.publication(&prose.document.expected).is_ok());

    for source in [
        "<p>Una <b>casa</b>.<br>Otra frase.</p>",
        "<p>Una <b>casa</b>.<span><br>Otra frase.</span></p>",
    ] {
        let mut authored = snapshot(source, &["A house. Another sentence."]);
        approve_all(&mut authored);
        let before = authored.clone();
        assert_eq!(
            authored.publication(&authored.document.expected),
            Err(Error::Pending)
        );
        assert_eq!(authored, before);
    }
}
