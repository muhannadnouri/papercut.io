//! Provider-neutral protected codes: only prose and opaque identities leave Rust.
//! Returned markup is a transport format, never trusted reader HTML.
use std::ops::Range;

use kuchikiki::NodeRef;
use serde::Serialize;

use super::{
    append_normalized_text, is_inline_formatting_element, serialize_node, source_inline_model,
};
use crate::document_uploads::{is_note_marker, tag_separates_text};
use crate::translation::html::parse_html_document;

/// Source normalization and hierarchy semantics participate in worker cache compatibility.
pub(crate) const MAPPING_VERSION: &str = "protected-codes-v3";
const MAX_CODES: usize = 128;
pub(crate) const MAX_HTML_BYTES: usize = 32 * 1024;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ProtectedPlan {
    pub(crate) html: String,
    source: String,
    codes: Vec<Code>,
}

#[derive(Debug, Clone, Serialize)]
struct Code {
    id: usize,
    parent: Option<usize>,
    start: usize,
    end: usize,
    open: String,
    close: String,
    literal: Option<String>,
    indivisible: bool,
    repeatable: bool,
}

impl Code {
    /// Notes/images have no source text: an invented word can be translated or aligned outside its code.
    fn literal_placeholder(&self) -> String {
        if self.start == self.end {
            String::new()
        } else {
            format!("PC{}", self.id)
        }
    }
}

pub(crate) struct RestoredMarkup {
    pub(crate) html: String,
    pub(crate) text: String,
}

/// Retain only the current block's scalar table; fragment planning does not rescan a long paragraph.
pub(crate) struct ProtectedSource {
    chars: Vec<char>,
    codes: Vec<Code>,
}

impl ProtectedSource {
    /// Extract once using the shared DOM text contract, keeping normalized wrappers around leading children.
    pub(crate) fn new(node: &NodeRef) -> Result<Self, String> {
        let mut text = String::new();
        let mut codes = Vec::new();
        let mut pending = false;
        for child in node.children() {
            collect(&child, None, &mut text, &mut pending, &mut codes)?;
        }
        if text != source_inline_model(node).text {
            return Err("Protected markup does not match the source DOM".into());
        }
        // A normalized leading spacer belongs outside the inline code.
        let chars = text.chars().collect::<Vec<_>>();
        for code in &mut codes {
            while code.start < code.end && chars[code.start] == ' ' {
                code.start += 1;
            }
        }
        // Preorder ids put parents first; walking backward keeps every wrapper around its children.
        for index in (0..codes.len()).rev() {
            if let Some(parent) = codes[index].parent {
                codes[parent].start = codes[parent].start.min(codes[index].start);
            }
        }
        Ok(Self { chars, codes })
    }

    /// Clip formatting with all selected ancestors; boundary children may need an empty wrapper.
    /// Protected literals and authored anchor identities remain indivisible even when selected as ancestors.
    pub(crate) fn fragment(
        &self,
        range: Range<usize>,
        source: &str,
    ) -> Result<ProtectedPlan, String> {
        if range.start >= range.end
            || self
                .chars
                .get(range.clone())
                .map(|s| s.iter().collect::<String>())
                .as_deref()
                != Some(source)
        {
            return Err("Protected markup does not match the source fragment".into());
        }
        let mut included = self
            .codes
            .iter()
            .map(|code| {
                // Zero-width notes belong to the preceding fragment, including its final boundary.
                if code.start == code.end {
                    (range.start < code.start && code.start <= range.end)
                        || (range.start == 0 && code.start == 0)
                } else {
                    code.start < range.end && code.end > range.start
                }
            })
            .collect::<Vec<_>>();
        // A child assigned to this fragment still needs its source-owned parent chain.
        for index in (0..self.codes.len()).rev() {
            if included[index] {
                if let Some(parent) = self.codes[index].parent {
                    included[parent] = true;
                }
            }
        }
        let mut retained = Vec::new();
        for (code, included) in self.codes.iter().zip(included) {
            if !included {
                continue;
            }
            if code.indivisible && (code.start < range.start || code.end > range.end) {
                return Err("A protected literal or anchor crosses a segment boundary".into());
            }
            let mut code = code.clone();
            code.start = code.start.max(range.start) - range.start;
            code.end = code.end.min(range.end) - range.start;
            retained.push(code);
        }
        let mut plan = ProtectedPlan {
            html: String::new(),
            source: source.into(),
            codes: retained,
        };
        plan.html = encode_children(&plan, None, 0..source.chars().count())?;
        if plan.html.len() > MAX_HTML_BYTES {
            return Err("Protected markup input exceeds its bound".into());
        }
        Ok(plan)
    }
}

/// Fixture convenience; production callers retain one extracted block across its fragments.
#[cfg(test)]
pub(crate) fn plan_fragment(
    node: &NodeRef,
    range: Range<usize>,
    source: &str,
) -> Result<ProtectedPlan, String> {
    ProtectedSource::new(node)?.fragment(range, source)
}

/// Share whitespace rules with the DOM plan; opaque subtree text advances offsets but is never sent.
fn collect(
    node: &NodeRef,
    parent: Option<usize>,
    text: &mut String,
    pending: &mut bool,
    codes: &mut Vec<Code>,
) -> Result<(), String> {
    if let Some(value) = node.as_text() {
        append_normalized_text(text, &value.borrow(), pending);
        return Ok(());
    }
    let Some(element) = node.as_element() else {
        return Ok(());
    };
    let tag = element.name.local.as_ref();
    let attrs = element.attributes.borrow();
    let opaque = is_note_marker(node)
        || matches!(
            tag,
            "code" | "img" | "svg" | "math" | "audio" | "video" | "table" | "figure"
        )
        || attrs
            .get("translate")
            .is_some_and(|v| v.eq_ignore_ascii_case("no"));
    if codes.len() >= MAX_CODES || node.ancestors().count() > 64 {
        return Err("Protected markup structure exceeds its bound".into());
    }
    let id = codes.len();
    let empty = NodeRef::new_element(element.name.clone(), attrs.map.clone());
    let shell = serialize_node(&empty).ok_or("Could not serialize source markup")?;
    let closing = format!("</{tag}>");
    let (open, close) = shell
        .strip_suffix(&closing)
        .map(|open| (open.to_string(), closing))
        .unwrap_or((shell, String::new()));
    let start = text.chars().count();
    codes.push(Code {
        id,
        parent,
        start,
        end: start,
        open,
        close,
        literal: None,
        indivisible: opaque || attrs.get("id").is_some() || attrs.get("name").is_some(),
        // HTML alignment can split emphasis across target words; never duplicate authored identities or literals.
        repeatable: is_inline_formatting_element(node)
            && !opaque
            && attrs.get("id").is_none()
            && attrs.get("name").is_none(),
    });
    if opaque {
        // Notes are excluded by the shared model; all other literals retain their normalized offsets.
        if !is_note_marker(node) {
            collect_plain(node, text, pending);
        }
        codes[id].literal =
            Some(serialize_node(node).ok_or("Could not serialize protected source")?);
    } else {
        if tag_separates_text(tag) {
            *pending = true;
        }
        for child in node.children() {
            collect(&child, Some(id), text, pending, codes)?;
        }
        if tag_separates_text(tag) {
            *pending = true;
        }
    }
    codes[id].end = text.chars().count();
    Ok(())
}

/// Advance source coordinates inside an opaque subtree using exactly the extraction whitespace rules.
fn collect_plain(node: &NodeRef, text: &mut String, pending: &mut bool) {
    if is_note_marker(node) {
        return;
    }
    if let Some(value) = node.as_text() {
        append_normalized_text(text, &value.borrow(), pending);
        return;
    }
    let separates = node
        .as_element()
        .is_some_and(|e| tag_separates_text(e.name.local.as_ref()));
    if separates {
        *pending = true;
    }
    for child in node.children() {
        collect_plain(&child, text, pending);
    }
    if separates {
        *pending = true;
    }
}

/// Encode hierarchy without source attributes or literals; siblings may reorder, parents may not change.
fn encode_children(
    plan: &ProtectedPlan,
    parent: Option<usize>,
    range: Range<usize>,
) -> Result<String, String> {
    let chars = plan.source.chars().collect::<Vec<_>>();
    let mut cursor = range.start;
    let mut html = String::new();
    for code in plan.codes.iter().filter(|code| code.parent == parent) {
        if code.start < cursor || code.end > range.end {
            return Err("Protected markup has overlapping source ranges".into());
        }
        html.push_str(&escape(
            &chars[cursor..code.start].iter().collect::<String>(),
        ));
        html.push_str(&format!("<span data-pc=\"{}\"", code.id));
        if code.literal.is_some() {
            html.push_str(&format!(
                " translate=\"no\">{}</span>",
                code.literal_placeholder()
            ));
        } else {
            html.push('>');
            html.push_str(&encode_children(plan, Some(code.id), code.start..code.end)?);
            html.push_str("</span>");
        }
        cursor = code.end;
    }
    html.push_str(&escape(
        &chars[cursor..range.end].iter().collect::<String>(),
    ));
    Ok(html)
}

impl ProtectedPlan {
    /// Bind cached output to source structure/literals as well as prose, without leaking them to the provider.
    pub(crate) fn identity(&self) -> String {
        crate::translation::hash::stable_text_hash(
            &serde_json::to_string(&(MAPPING_VERSION, self)).expect("serializable protected plan"),
        )
    }

    /// Strict XML rejects repaired/crossing tags; rebuild only source-owned shells after identity checks.
    pub(crate) fn restore(&self, output: &str) -> Result<RestoredMarkup, String> {
        if output.len() > MAX_HTML_BYTES {
            return Err("Protected markup output exceeds its bound".into());
        }
        let wrapped = format!("<root>{output}</root>");
        let doc =
            roxmltree::Document::parse(&wrapped).map_err(|_| "Invalid protected markup syntax")?;
        let mut seen = std::collections::HashSet::new();
        let html = self.restore_children(doc.root_element(), None, &mut seen)?;
        if seen.len() != self.codes.len() {
            return Err("Protected markup is missing source identities".into());
        }
        let parsed = parse_html_document(format!("<html><body><div>{html}</div></body></html>"));
        let node = parsed
            .select_first("body > div")
            .map_err(|_| "Invalid restored markup")?;
        let text = source_inline_model(node.as_node()).text;
        if text.trim().is_empty() {
            return Err("Protected markup returned empty text".into());
        }
        Ok(RestoredMarkup { html, text })
    }

    /// Allow disjoint copies of formatting only, under the same parent; all other identities remain single-use.
    /// Provider tags/attributes never reach the reader, even on a repeated formatting code.
    fn restore_children(
        &self,
        node: roxmltree::Node<'_, '_>,
        parent: Option<usize>,
        seen: &mut std::collections::HashSet<usize>,
    ) -> Result<String, String> {
        let mut html = String::new();
        for child in node.children() {
            if child.is_text() {
                html.push_str(&escape(child.text().unwrap_or_default()));
                continue;
            }
            if !child.is_element()
                || child.tag_name().name() != "span"
                || child.tag_name().namespace().is_some()
                || child.namespaces().len() != 0
            {
                return Err("Unexpected protected markup node".into());
            }
            let raw = child
                .attribute("data-pc")
                .ok_or("Missing protected identity")?;
            let code = self
                .codes
                .iter()
                .find(|c| c.id.to_string() == raw)
                .ok_or("Unknown protected identity")?;
            let opaque = code.literal.is_some();
            if code.parent != parent
                || (!seen.insert(code.id) && !code.repeatable)
                || child.attributes().len() != if opaque { 2 } else { 1 }
                || child.attributes().any(|a| {
                    a.namespace().is_some()
                        || (a.name() != "data-pc"
                            && !(opaque && a.name() == "translate" && a.value() == "no"))
                })
            {
                return Err("Changed, duplicated or reparented protected identity".into());
            }
            if let Some(literal) = &code.literal {
                if child.children().any(|c| !c.is_text())
                    || child
                        .children()
                        .filter_map(|c| c.text())
                        .collect::<String>()
                        != code.literal_placeholder()
                {
                    return Err("Changed protected literal code".into());
                }
                // Keep source whitespace at opaque boundaries even if HTML alignment swallowed it.
                if code.start > 0 && self.source.chars().nth(code.start - 1) == Some(' ') {
                    html.push(' ');
                }
                html.push_str(literal);
                if self.source.chars().nth(code.end) == Some(' ') {
                    html.push(' ');
                }
            } else {
                html.push_str(&code.open);
                html.push_str(&self.restore_children(child, Some(code.id), seen)?);
                html.push_str(&code.close);
            }
        }
        Ok(html)
    }
}

/// Escape text at both provider and renderer boundaries; only retained source shells are executable markup.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::translation::dom::DomPlan;

    /// Use the same sanitized DOM source as real jobs, not a hand-written code inventory.
    fn plan(html: &str) -> ProtectedPlan {
        let safe = crate::document_uploads::sanitize_html(html);
        let dom = DomPlan::new(&safe);
        let block = &dom.blocks[0];
        plan_fragment(
            &block.node,
            0..block.inline.text.chars().count(),
            &block.inline.text,
        )
        .unwrap()
    }

    #[test]
    fn leading_zero_width_children_stay_inside_all_normalized_formatting_parents() {
        let asset = crate::document_uploads::ParsedDocumentAsset::new(
            "image/png",
            b"owned hierarchy image".to_vec(),
        )
        .unwrap();
        let atoms = [
            (
                "<a role='doc-noteref' href='#n' id='ref'><sup>1</sup></a>".to_string(),
                "a#ref",
            ),
            (
                format!("<img data-papercut-asset='{}'>", asset.file_name),
                "img",
            ),
            ("<br>".into(), "br"),
        ];
        for (atom, selector) in atoms {
            for (prefix, content) in [
                ("Bonjour.", format!("{atom} Merci.")),
                ("Bonjour. ", format!("{atom}Merci.")),
                ("Bonjour.", format!(" {atom}Merci.")),
            ] {
                let safe = crate::document_uploads::sanitize_html(&format!(
                    "<p>{prefix}<strong><em>{content}</em></strong></p>"
                ));
                let dom = DomPlan::new(&safe);
                let block = &dom.blocks[0];
                let extracted = ProtectedSource::new(&block.node).unwrap();
                for code in &extracted.codes {
                    if let Some(parent) = code.parent {
                        let parent = &extracted.codes[parent];
                        assert!(
                            parent.start <= code.start && parent.end >= code.end,
                            "parent excludes {selector}: {safe}"
                        );
                    }
                }
                let plan = extracted
                    .fragment(0..block.inline.text.chars().count(), &block.inline.text)
                    .unwrap();
                let restored = plan.restore(&plan.html).unwrap();
                assert_eq!(restored.text, block.inline.text);
                let doc = parse_html_document(&restored.html);
                assert_eq!(
                    doc.select(&format!("strong > em > {selector}"))
                        .unwrap()
                        .count(),
                    1
                );
                assert!(!plan.html.contains("href"));
                assert!(!plan.html.contains(&asset.file_name));
            }
        }
    }

    #[test]
    fn boundary_ancestor_retention_cannot_split_authored_identities_or_literals() {
        let prefix = format!("{}fin!", "Bonjour ".repeat(112));
        for identity in ["id='keep'", "name='keep'"] {
            let safe = crate::document_uploads::sanitize_html(&format!("<p>{prefix}<strong {identity}><em><a role='doc-noteref' href='#n'>1</a>Suite.</em></strong></p>"));
            let dom = DomPlan::new(&safe);
            let extracted = ProtectedSource::new(&dom.blocks[0].node).unwrap();
            assert!(extracted
                .fragment(0..900, &prefix)
                .unwrap_err()
                .contains("crosses a segment boundary"));
        }
        let prefix = "Bonjour ".repeat(112);
        let dom = DomPlan::new(&format!(
            "<p>{prefix}<strong><code>user id value</code></strong></p>"
        ));
        let block = &dom.blocks[0];
        let segment = crate::translation::segment::segment_text_blocks([&block.inline.text], 900)
            .unwrap()
            .remove(0);
        assert_eq!(segment.source_end, 900);
        assert!(ProtectedSource::new(&block.node)
            .unwrap()
            .fragment(segment.source_start..segment.source_end, &segment.text)
            .unwrap_err()
            .contains("crosses a segment boundary"));
    }

    #[test]
    fn zero_width_markers_send_no_synthetic_prose_and_remain_single_use() {
        let p = plan("<p>Bonjour<a role='doc-noteref' id='ref' href='#note'><sup>1</sup></a>.</p>");
        assert_eq!(
            p.html,
            "Bonjour<span data-pc=\"0\" translate=\"no\"></span>."
        );
        let valid = "Hello<span data-pc='0' translate='no'></span>.";
        let restored = p.restore(valid).unwrap();
        assert_eq!(restored.text, "Hello.");
        assert_eq!(
            parse_html_document(restored.html)
                .select("a#ref[href='#note'] > sup")
                .unwrap()
                .count(),
            1
        );
        for invalid in [
            valid.replace("></span>", ">PC0</span>"),
            valid.replace("></span>", ">translated words</span>"),
            format!("{valid}{valid}"),
            "Hello.".into(),
        ] {
            assert!(p.restore(&invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn restores_discontinuous_formatting_without_styling_intervening_words() {
        let p = plan("<p><strong><em>El perro</em></strong> azul.</p>");
        let restored = p.restore("<span data-pc='0'><span data-pc='1'>The</span></span> blue <span data-pc='0'><span data-pc='1'>dog🙂</span></span>.").unwrap();
        assert_eq!(restored.text, "The blue dog🙂.");
        assert_eq!(
            restored.html,
            "<strong><em>The</em></strong> blue <strong><em>dog🙂</em></strong>."
        );

        // Stock es→en splits one emphasized source phrase around target disarmament.
        let p =
            plan("<p>aceptar un desarme <em>parcial,</em> contraviniendo las líneas rojas.</p>");
        let restored = p.restore("accepting <span data-pc='0'>partial</span> disarmament<span data-pc='0'>,</span> contravening the red lines.").unwrap();
        assert_eq!(
            restored.html,
            "accepting <em>partial</em> disarmament<em>,</em> contravening the red lines."
        );
    }

    #[test]
    fn discontinuous_formatting_does_not_allow_duplicated_identities_or_reparenting() {
        for shell in [
            "<strong id='unique'>Bonjour</strong>",
            "<em name='unique'>Bonjour</em>",
            "<strong translate='no'>Bonjour</strong>",
            "<strong><a id='unique' href='#note'>Bonjour</a></strong>",
            "<a href='#note'>Bonjour</a>",
            "<span>Bonjour</span>",
            "<code>Bonjour</code>",
            "<span translate='no'>Bonjour</span>",
            "<a role='doc-noteref' href='#note'>1</a>",
        ] {
            let p = plan(&format!("<p>Texte {shell} fin.</p>"));
            assert!(
                p.restore(&format!("{}{}", p.html, p.html)).is_err(),
                "{shell}"
            );
        }
        let p = plan("<p><strong><em>Bonjour</em></strong></p>");
        for invalid in [
            "<span data-pc='0'>Hello</span><span data-pc='1'>world</span>",
            "<span data-pc='0'><span data-pc='1'>Hello</span></span><span data-pc='0' onclick='bad()'>world</span>",
            "<span data-pc='0'><span data-pc='1'>Hello</span><span data-pc='0'>world</span></span>",
        ] { assert!(p.restore(invalid).is_err(), "{invalid}"); }
    }

    #[test]
    fn reorders_repeated_nested_emphasis_and_keeps_source_link_attributes() {
        let p = plan(
            "<p>Une <strong><em>bague</em></strong> et <a href='#note' id='link'>bague</a>.</p>",
        );
        assert_eq!(p.html, "Une <span data-pc=\"0\"><span data-pc=\"1\">bague</span></span> et <span data-pc=\"2\">bague</span>.");
        let restored = p.restore("A <span data-pc=\"2\">ring</span> and <span data-pc=\"0\"><span data-pc=\"1\">ring🙂</span></span>.").unwrap();
        assert_eq!(restored.text, "A ring and ring🙂.");
        assert!(restored.html.contains("<strong><em>ring🙂</em></strong>"));
        assert!(restored.html.contains("href=\"#note\""));
        assert!(restored.html.contains("id=\"link\""));
        assert!(!p.html.contains("#note"));
    }

    #[test]
    fn literals_notes_assets_and_boundary_spaces_are_source_owned() {
        let asset = crate::document_uploads::ParsedDocumentAsset::new(
            "image/png",
            b"owned image bytes".to_vec(),
        )
        .unwrap();
        let p = plan(&format!("<p>Gardez <code>private_user_id</code> et <span translate='no'>42</span><a href='#n' role='doc-noteref'><sup>1</sup></a><img data-papercut-asset='{}'>.</p>", asset.file_name));
        for secret in ["private_user_id", "42", "#n", asset.file_name.as_str()] {
            assert!(!p.html.contains(secret));
        }
        let restored = p.restore("Keep<span data-pc=\"0\" translate=\"no\">PC0</span>and<span data-pc=\"1\" translate=\"no\">PC1</span><span data-pc=\"2\" translate=\"no\"></span><span data-pc=\"3\" translate=\"no\"></span>.").unwrap();
        assert_eq!(restored.text, "Keep private_user_id and 42.");
        for retained in [
            "<code>private_user_id</code>",
            "translate=\"no\"",
            "href=\"#n\"",
            &format!("data-papercut-asset=\"{}\"", asset.file_name),
        ] {
            assert!(
                restored.html.contains(retained),
                "missing {retained} in {}",
                restored.html
            );
        }
        assert!(!restored.html.contains("PC0"));
    }

    #[test]
    fn rejects_missing_duplicate_changed_reparented_crossing_or_injected_codes() {
        let p = plan("<p><strong>Une <em>bague</em></strong> <code>user_id</code></p>");
        let valid = p.html.clone();
        assert!(p.restore(&valid).is_ok());
        for invalid in [
            valid.replace("data-pc=\"0\"", "data-pc=\"99\""),
            valid.replace("PC2", "changed"),
            valid.replace("translate=\"no\"", "translate=\"yes\""),
            valid.replace("data-pc=\"1\"", "data-pc=\"0\""),
            valid.replace("data-pc=\"1\"", "data-pc=\"1\" onclick=\"evil()\""),
            valid.replace("data-pc=\"1\"", "data-pc=\"1\" xmlns:x=\"urn:untrusted\""),
            valid.replace("data-pc=\"1\"", "data-pc=\"1\" xmlns=\"urn:untrusted\""),
            format!("{valid}{valid}"),
            "translated with no identities".into(),
            "<span data-pc='0'>one</span><span data-pc='1'>two</span><span data-pc='2' translate='no'>PC2</span>".into(),
            "<span data-pc='0'><span data-pc='1'>one</span></bad>".into(),
            format!("<!--ignored-->{valid}"),
            format!("<script>evil()</script>{valid}"),
            "x".repeat(MAX_HTML_BYTES + 1),
        ] { assert!(p.restore(&invalid).is_err(), "{invalid}"); }
    }

    #[test]
    fn rejects_oversize_deep_and_code_dense_source_plans_before_transport() {
        for html in [
            format!("<p>{}</p>", "x".repeat(MAX_HTML_BYTES + 1)),
            format!("<p>{}</p>", "<em>x</em>".repeat(MAX_CODES + 1)),
            format!("<p>{}x{}</p>", "<span>".repeat(65), "</span>".repeat(65)),
        ] {
            let dom = DomPlan::new(&html);
            let block = &dom.blocks[0];
            assert!(plan_fragment(
                &block.node,
                0..block.inline.text.chars().count(),
                &block.inline.text
            )
            .is_err());
        }
    }

    #[test]
    fn escaping_unicode_and_literal_looking_prose_never_invent_reader_tags() {
        let p = plan("<p><em>عَرَبِيّ🙂 &amp; Résumé</em></p>");
        let restored = p
            .restore("<span data-pc='0'>عَرَبِيّ🙂 &amp; &lt;img src='x'&gt;</span>")
            .unwrap();
        assert_eq!(restored.text, "عَرَبِيّ🙂 & <img src='x'>");
        assert!(restored.html.contains("&lt;img"));
        assert!(!restored.html.contains("<img"));
        let other = plan("<p><strong>عَرَبِيّ🙂 &amp; Résumé</strong></p>");
        assert_eq!(p.html, other.html);
        assert_ne!(p.identity(), other.identity());
    }

    #[test]
    fn fragments_clip_formatting_but_never_split_literals_or_authored_ids() {
        let dom = DomPlan::new("<p><strong>Bonjour. Merci.</strong></p>");
        let node = &dom.blocks[0].node;
        let first = plan_fragment(node, 0..8, "Bonjour.").unwrap();
        let second = plan_fragment(node, 9..15, "Merci.").unwrap();
        assert_eq!(
            first
                .restore("<span data-pc='0'>Hello.</span>")
                .unwrap()
                .html,
            "<strong>Hello.</strong>"
        );
        assert_eq!(
            second
                .restore("<span data-pc='0'>Thanks.</span>")
                .unwrap()
                .html,
            "<strong>Thanks.</strong>"
        );
        let dom = DomPlan::new("<p>A <code>user_id</code> B</p>");
        assert!(plan_fragment(&dom.blocks[0].node, 0..5, "A use").is_err());
        let dom = DomPlan::new("<p><em id='keep'>Bonjour. Merci.</em></p>");
        assert!(plan_fragment(&dom.blocks[0].node, 0..8, "Bonjour.").is_err());
        let dom = DomPlan::new("<p><a name='keep'>Bonjour. Merci.</a></p>");
        assert!(plan_fragment(&dom.blocks[0].node, 0..8, "Bonjour.").is_err());
    }
}
