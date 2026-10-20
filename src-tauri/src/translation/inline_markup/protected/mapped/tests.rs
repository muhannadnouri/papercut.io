//! Gold mappings isolate reconstruction from model quality; private sample documents never enter fixtures.
use super::*;
use crate::translation::{dom::DomPlan, html::parse_html_document};

/// Extract the actual sanitized block tree; gold maps never invent their own source identity inventory.
fn plan(html: &str) -> ProtectedPlan {
    let dom = DomPlan::new(&crate::document_uploads::sanitize_html(html));
    let block = &dom.blocks[0];
    super::super::ProtectedSource::new(&block.node)
        .unwrap()
        .fragment(0..block.inline.text.chars().count(), &block.inline.text)
        .unwrap()
}

fn node(source_id: usize, targets: Vec<std::ops::Range<usize>>) -> NodeMapping {
    NodeMapping { source_id, targets }
}

#[test]
fn disjoint_nested_styles_and_reordered_links_keep_exact_source_ancestry() {
    let p = plan("<p><strong><em>Le chien</em></strong> et <a id='link' href='#n' title='retained'>chat</a></p>");
    let r = p
        .restore_mapped(
            "cat and the blue dog",
            &[
                node(0, vec![8..11, 17..20]),
                node(1, vec![8..11, 17..20]),
                node(2, vec![0..3]),
            ],
        )
        .unwrap();
    assert_eq!(r.text, "cat and the blue dog");
    let dom = parse_html_document(r.html);
    assert_eq!(
        dom.select("strong > em")
            .unwrap()
            .map(|n| n.text_contents())
            .collect::<Vec<_>>(),
        ["the", "dog"]
    );
    let link = dom
        .select_first("a#link[href='#n'][title='retained']")
        .unwrap();
    assert_eq!(link.text_contents(), "cat");
    assert_eq!(dom.select("#link").unwrap().count(), 1);
    assert!(!dom
        .select_first("body")
        .unwrap()
        .text_contents()
        .contains("chien"));
}

#[test]
fn scalar_text_is_escaped_and_cannot_create_model_tags_or_attributes() {
    let p = plan("<p><em id='one'>Texte</em></p>");
    let target = "عَرَبِيّ🙂 <img src='evil' onerror='evil()'> & café";
    let r = p
        .restore_mapped(target, &[node(0, vec![0..target.chars().count()])])
        .unwrap();
    assert_eq!(r.text, target);
    let dom = parse_html_document(r.html);
    assert_eq!(dom.select("img,script").unwrap().count(), 0);
    assert_eq!(dom.select("em#one").unwrap().count(), 1);
    assert_eq!(
        dom.select_first("em")
            .unwrap()
            .attributes
            .borrow()
            .map
            .len(),
        1
    );
}

#[test]
fn literals_are_restored_once_without_echoes_and_changed_text_rejects() {
    let p = plan("<p>Gardez <code title='source'>a &amp; b</code> fin.</p>");
    let r = p
        .restore_mapped("Keep a & b end.", &[node(0, vec![5..10])])
        .unwrap();
    assert_eq!(r.text, "Keep a & b end.");
    let dom = parse_html_document(r.html);
    assert_eq!(dom.select("code[title='source']").unwrap().count(), 1);
    assert_eq!(dom.select_first("code").unwrap().text_contents(), "a & b");
    assert_eq!(
        p.restore_mapped("Keep a + b end.", &[node(0, vec![5..10])])
            .err(),
        Some(Rejection::ScopeMismatch)
    );
}

#[test]
fn attachments_split_only_repeatable_siblings_and_never_an_authored_link() {
    for shell in ["em", "a id='unique' href='#n'"] {
        let closing = shell.split_whitespace().next().unwrap();
        let p = plan(&format!(
            "<p><{shell}>Un deux</{closing}><a role='doc-noteref' href='#n' id='ref'>1</a></p>"
        ));
        let result = p.restore_mapped("ONE TWO", &[node(0, vec![0..7]), node(1, vec![3..3])]);
        if closing == "em" {
            let r = result.unwrap();
            assert_eq!(r.text, "ONE TWO");
            assert!(r.html.contains("<em>ONE</em><a"), "{}", r.html);
            assert_eq!(parse_html_document(r.html).select("em").unwrap().count(), 2);
        } else {
            assert_eq!(result.err(), Some(Rejection::IdentitySplit));
        }
    }
}

#[test]
fn invalid_identity_crossing_parent_and_grapheme_maps_have_no_output() {
    let p = plan("<p><strong><em>Un</em> deux</strong> <a id='ref' href='#n'>fin</a></p>");
    for bad in [
        vec![node(0, vec![0..3]), node(1, vec![0..3])],
        vec![
            node(0, vec![0..3]),
            node(1, vec![4..7]),
            node(2, vec![8..11]),
        ],
        vec![
            node(0, vec![0..7]),
            node(1, vec![0..3]),
            node(2, vec![4..11]),
        ],
        vec![
            node(0, vec![0..7]),
            node(1, vec![0..3]),
            node(2, vec![8..11]),
            node(2, vec![8..11]),
        ],
        vec![
            node(0, vec![0..7]),
            node(1, vec![0..3]),
            node(2, vec![8..9, 10..11]),
        ],
    ] {
        assert!(p.restore_mapped("ONE TWO END", &bad).is_err());
    }
    let p = plan("<p><em>Texte</em></p>");
    assert!(p
        .restore_mapped("e\u{301} word", &[node(0, vec![0..1])])
        .is_err());
    assert_eq!(
        p.restore_mapped("word", &[node(0, vec![0..usize::MAX])])
            .err(),
        Some(Rejection::MappingRange)
    );
    // Exact offsets cannot be silently normalized into a different reader string after mapping.
    assert_eq!(
        p.restore_mapped("word  word", &[node(0, vec![0..10])])
            .err(),
        Some(Rejection::Reconstruction)
    );
}

#[test]
fn repeated_large_source_attributes_cannot_amplify_reconstruction() {
    let p = plan(&format!(
        "<p><em title='{}'>mot</em> fin</p>",
        "x".repeat(8192)
    ));
    let target = format!("{}end", "X ".repeat(64));
    let ranges = (0..64).map(|i| i * 2..i * 2 + 1).collect();
    assert!(p.restore_mapped(&target, &[node(0, ranges)]).is_err());
    assert_eq!(
        p.restore_mapped(&"x".repeat(MAX_HTML_BYTES + 1), &[node(0, vec![0..1])])
            .err(),
        Some(Rejection::Limit)
    );
}
