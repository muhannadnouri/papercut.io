//! Engine-independent reader mapping. Import rules choose blocks; this plan
//! gives extraction, inline probes and rendering the same identities and text.

use kuchikiki::NodeRef;

use super::html::parse_html_document;
use super::inline_markup::{source_inline_model, InlineSourceModel};
use crate::document_uploads::reader_block_nodes;

pub(super) struct DomPlan {
    pub(super) document: NodeRef,
    pub(super) blocks: Vec<DomBlock>,
}

pub(super) struct DomBlock {
    pub(super) ordinal: usize,
    pub(super) node: NodeRef,
    // Source/segment offsets are Unicode scalar indices; rendered spans are UTF-8 bytes.
    pub(super) inline: InlineSourceModel,
    pub(super) heading: Option<String>,
    pub(super) is_heading: bool,
}

impl DomPlan {
    pub(super) fn new(html: &str) -> Self {
        let document = parse_html_document(html);
        let mut heading = None;
        let blocks = reader_block_nodes(&document)
            .into_iter()
            .enumerate()
            .filter_map(|(ordinal, node)| {
                if node.inclusive_ancestors().any(|ancestor| {
                    ancestor.as_element().is_some_and(|element| {
                        element
                            .attributes
                            .borrow()
                            .get("translate")
                            .is_some_and(|value| value.eq_ignore_ascii_case("no"))
                    })
                }) {
                    return None;
                }
                let inline = source_inline_model(&node);
                let is_heading = node.as_element().is_some_and(|element| {
                    matches!(
                        element.name.local.as_ref(),
                        "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
                    )
                });
                if is_heading {
                    heading = Some(inline.text.clone());
                }
                if inline.text.is_empty() {
                    return None;
                }
                Some(DomBlock {
                    ordinal,
                    node,
                    inline,
                    heading: heading.clone(),
                    is_heading,
                })
            })
            .collect();
        Self { document, blocks }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::translation::inline_markup::inline_phrase_probes_by_block;

    #[test]
    fn extraction_and_probes_share_importer_leaf_order_and_unicode_offsets() {
        let html = "<article><h1>Titre</h1><div>Début<br>fin</div><ul><li><p>Répété</p><p>Répété</p></li></ul><blockquote><p>نص <strong>عَرَبِيّ🙂</strong></p></blockquote></article>";
        let plan = DomPlan::new(html);
        let texts: Vec<_> = plan
            .blocks
            .iter()
            .map(|block| block.inline.text.as_str())
            .collect();
        assert_eq!(
            texts,
            ["Titre", "Début fin", "Répété", "Répété", "نص عَرَبِيّ🙂"]
        );
        assert_eq!(
            plan.blocks
                .iter()
                .map(|block| block.ordinal)
                .collect::<Vec<_>>(),
            [0, 1, 2, 3, 4]
        );
        assert_eq!(plan.blocks[4].heading.as_deref(), Some("Titre"));
        let model = &plan.blocks[4].inline;
        let span = &model.spans[0];
        assert_eq!(
            model
                .text
                .chars()
                .skip(span.start)
                .take(span.end - span.start)
                .collect::<String>(),
            "عَرَبِيّ🙂"
        );
        let probes = inline_phrase_probes_by_block(html);
        assert_eq!(probes.len(), plan.blocks.len());
        assert_eq!(probes[4][0].text, "عَرَبِيّ🙂");
    }

    #[test]
    fn protected_blocks_and_marker_only_blocks_keep_their_original_ordinals() {
        let plan = DomPlan::new(
            "<p translate=no>user_id</p><p><a role=doc-noteref href='#n'>1</a></p><p>Bonjour</p>",
        );
        assert_eq!(plan.blocks.len(), 1);
        assert_eq!(plan.blocks[0].ordinal, 2);
        assert_eq!(plan.blocks[0].inline.text, "Bonjour");
    }

    #[test]
    fn heading_identity_comes_from_structure_not_repeated_text() {
        let plan = DomPlan::new("<h1>Titre</h1><p>Titre</p>");
        assert!(plan.blocks[0].is_heading);
        assert!(!plan.blocks[1].is_heading);
        assert_eq!(plan.blocks[1].heading.as_deref(), Some("Titre"));
    }
}
