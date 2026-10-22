use super::tests::{bold, edit, snapshot};
use super::*;

fn measured<T>(operation: impl FnOnce() -> T) -> (T, usize) {
    PATH_VISITS.with(|visits| visits.set(0));
    let result = operation();
    (result, PATH_VISITS.with(std::cell::Cell::get))
}

fn evidence_nodes(snapshot: &Snapshot) -> usize {
    [&snapshot.source_html, &snapshot.baseline_html]
        .into_iter()
        .map(|html| parse_html_document(html).inclusive_descendants().count())
        .sum()
}

#[test]
fn eight_thousand_block_capture_validate_and_preview_walk_each_tree_once_per_check() {
    const BLOCKS: usize = 8_000;
    let source = format!("<article>{}</article>", "<p>Repetido.</p>".repeat(BLOCKS));
    let (mut saved, capture_work) = measured(|| snapshot(&source, &vec!["Repeated."; BLOCKS]));
    let nodes = evidence_nodes(&saved);
    // Capture admits its native inputs and then its completed snapshot: two bounded checks.
    assert_eq!(capture_work, 2 * nodes);
    let (result, validation_work) = measured(|| saved.validate());
    result.unwrap();
    assert_eq!(validation_work, nodes);
    saved
        .apply(&edit(&saved, BLOCKS - 1, vec![bold(0, 8)]))
        .unwrap();
    let (result, preview_work) = measured(|| saved.previews());
    let (source_preview, preview) = result.unwrap();
    assert_eq!(preview_work, nodes);
    assert_eq!(DomPlan::new(&preview).blocks.len(), BLOCKS);
    assert_eq!(DomPlan::new(&source_preview).blocks.len(), BLOCKS);
    let dom = parse_html_document(&preview);
    let added: Vec<_> = dom.select("b").unwrap().collect();
    assert_eq!(added.len(), 1);
    assert_eq!(added[0].text_contents(), "Repeated");
    let (result, reconstruction_work) = measured(|| saved.reconstruct());
    assert_eq!(result.unwrap(), preview);
    assert_eq!(reconstruction_work, nodes);
    // Preview mutations belong only to its local parsed tree, never the next call or saved HTML.
    assert_eq!(saved.previews().unwrap(), (source_preview, preview));
    assert!(!saved.baseline_html.contains("<b>Repeated</b>"));
}

#[test]
fn linear_paths_keep_legacy_all_child_indices_with_nested_repeats_and_protected_content() {
    let html = "<article> \n<p>Repeat.</p><!-- keep child indices -->\n<blockquote><p>Repeat.</p></blockquote><section><p id='ref'>A <code>user_id</code><a role='doc-noteref' href='#note'>1</a>.</p></section><p id='note'>Note<a role='doc-backlink' href='#ref'>Back</a>.</p></article>";
    let plan = checked_plan(&crate::document_uploads::sanitize_html(html)).unwrap();
    let (paths, work) = block_paths(&plan).unwrap();
    assert_eq!(work, plan.document.inclusive_descendants().count());
    assert_eq!(paths.len(), 4);
    for (block, path) in plan.blocks.iter().zip(&paths) {
        let mut node = plan.document.clone();
        for index in path {
            node = node.children().nth(*index).unwrap();
        }
        assert_eq!(node, block.node);
        // Compatibility check against the old saved representation, used only on this tiny fixture.
        let mut legacy = Vec::new();
        while node != plan.document {
            let parent = node.parent().unwrap();
            legacy.push(parent.children().position(|child| child == node).unwrap());
            node = parent;
        }
        legacy.reverse();
        assert_eq!(*path, legacy);
    }
    assert_ne!(paths[0], paths[1]);
}

#[test]
fn mutated_paths_cannot_redirect_an_edit_to_an_identical_paragraph() {
    let saved = snapshot(
        "<section><p>Igual.</p><blockquote><p>Igual.</p></blockquote></section>",
        &["Same.", "Same."],
    );
    for source in [true, false] {
        let mut tampered = saved.clone();
        if source {
            tampered.mappings[0].source_path = saved.mappings[1].source_path.clone();
        } else {
            tampered.mappings[0].target_path = saved.mappings[1].target_path.clone();
        }
        assert_eq!(tampered.validate(), Err(Error::Evidence));
        assert_eq!(tampered.reconstruct(), Err(Error::Evidence));
        assert_eq!(tampered.previews(), Err(Error::Evidence));
    }
}
