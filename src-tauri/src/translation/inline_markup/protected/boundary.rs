//! Exact source-boundary reconstruction; no phrase matching or worker attachment repositioning.
use super::*;
use crate::translation::inline_markup::pieces::{Kind, Translation};

impl ProtectedPlan {
    /// Every shell endpoint must coincide with a translated/copied unit edge, never a projected word.
    pub(crate) fn boundary_output(&self, translation: &Translation<'_>) -> Result<String, String> {
        if self.alignment_identity() != translation.plan.protected.alignment_identity() {
            return Err("Boundary translation belongs to another source plan".into());
        }
        let mut mapped = self.clone();
        mapped.source = translation.text.clone();
        let endpoint = |offset| {
            translation
                .plan
                .units()
                .iter()
                .zip(&translation.targets)
                .find_map(|(unit, target)| {
                    if unit.source.start == offset {
                        Some(target.start)
                    } else if unit.source.end == offset {
                        Some(target.end)
                    } else if unit.kind != Kind::Prose && unit.source.contains(&offset) {
                        // Copied symbols/literals have exact scalar correspondence, unlike translated words.
                        Some(target.start + offset - unit.source.start)
                    } else {
                        None
                    }
                })
                .ok_or("Source formatting boundary is inside a translated unit")
        };
        for code in &mut mapped.codes {
            code.start = endpoint(code.start)?;
            code.end = endpoint(code.end)?;
        }
        // Copied content is never synthesized; literals still use opaque placeholders in transport.
        for (unit, target) in translation.plan.units().iter().zip(&translation.targets) {
            if matches!(
                unit.kind,
                Kind::Literal(_) | Kind::Verbatim | Kind::Whitespace
            ) {
                let source: String = self
                    .source
                    .chars()
                    .skip(unit.source.start)
                    .take(unit.source.len())
                    .collect();
                let copied: String = translation
                    .text
                    .chars()
                    .skip(target.start)
                    .take(target.len())
                    .collect();
                if source != copied {
                    return Err("Boundary translation changed copied source content".into());
                }
            }
        }
        let html = encode_children(&mapped, None, 0..mapped.source.chars().count())?;
        self.restore_boundaries(&html)?;
        Ok(html)
    }

    /// Revalidate cached source order and hierarchy; attachments already occupy their exact fragment boundary.
    pub(crate) fn restore_boundaries(&self, output: &str) -> Result<RestoredMarkup, String> {
        if output.len() > MAX_HTML_BYTES {
            return Err("Boundary markup exceeds its bound".into());
        }
        let wrapped = format!("<root>{output}</root>");
        let doc = roxmltree::Document::parse(&wrapped).map_err(|_| "Invalid boundary markup")?;
        let ids = doc
            .root_element()
            .descendants()
            .filter(|n| n.is_element())
            .skip(1)
            .map(|n| n.attribute("data-pc").unwrap_or("").to_string())
            .collect::<Vec<_>>();
        if ids
            != self
                .codes
                .iter()
                .map(|c| c.id.to_string())
                .collect::<Vec<_>>()
        {
            return Err("Boundary markup changed source identity order".into());
        }
        let html = self.render(doc.root_element())?;
        // HTML fragment parsing needs the original context for table/list children.
        use kuchikiki::traits::TendrilSink;
        let (ns, local) = self
            .context
            .rsplit_once(':')
            .ok_or("Missing source fragment context")?;
        let context_document = parse_html_document("");
        let context_root = context_document
            .select_first("html")
            .map_err(|_| "Missing HTML context")?;
        let mut context = context_root
            .as_node()
            .as_element()
            .ok_or("Missing HTML context")?
            .name
            .clone();
        context.ns = ns.into();
        context.local = local.into();
        let parsed = kuchikiki::parse_fragment(context, vec![]).one(html.clone());
        let root = parsed
            .document_node
            .select_first("html")
            .map_err(|_| "Invalid boundary fragment")?;
        let text = source_inline_model(root.as_node()).text;
        if text.trim().is_empty() {
            return Err("Boundary translation returned empty text".into());
        }
        Ok(RestoredMarkup { html, text })
    }
}
