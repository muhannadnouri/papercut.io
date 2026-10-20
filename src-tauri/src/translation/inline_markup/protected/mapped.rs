//! Checked scalar maps become opaque transport for the existing source-owned renderer, never model HTML.
use super::{escape, ProtectedPlan, Reconstruction, RestoredMarkup, MAX_HTML_BYTES};
use crate::translation::inline_markup::provider::contract::{
    check_projected_nodes, NodeMapping, Rejection,
};

impl ProtectedPlan {
    /// Recheck the combined hierarchy and copied literals before constructing any reader markup.
    /// Strict maps bypass worker note repositioning; their attachment offsets already require lexical evidence.
    pub(in crate::translation::inline_markup) fn restore_mapped(
        &self,
        target: &str,
        nodes: &[NodeMapping],
    ) -> Result<RestoredMarkup, Rejection> {
        if target.len() > MAX_HTML_BYTES || !self.alignment_payload_fits(MAX_HTML_BYTES * 8) {
            return Err(Rejection::Limit);
        }
        check_projected_nodes(self, target, nodes)?;
        let rows = self
            .codes
            .iter()
            .map(|c| {
                nodes
                    .iter()
                    .find(|n| n.source_id == c.id)
                    .map(|n| (c, n))
                    .ok_or(Rejection::Identity)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let source = self.source.chars().collect::<Vec<_>>();
        let bytes = target
            .char_indices()
            .map(|(i, _)| i)
            .chain([target.len()])
            .collect::<Vec<_>>();
        for (code, node) in rows
            .iter()
            .filter(|(c, _)| c.literal.is_some() && c.start < c.end)
        {
            let [range] = node.targets.as_slice() else {
                return Err(Rejection::IdentitySplit);
            };
            if target[bytes[range.start]..bytes[range.end]]
                != source[code.start..code.end].iter().collect::<String>()
            {
                return Err(Rejection::ScopeMismatch);
            }
        }
        let mut points = vec![0, bytes.len() - 1];
        for node in nodes {
            for range in &node.targets {
                points.extend([range.start, range.end]);
            }
        }
        points.sort_unstable();
        points.dedup();
        let mut transport = Transport {
            plan: self,
            // Includes escaped target and bounded opaque spans; source attributes/literals are not repeated here.
            output: Reconstruction {
                html: String::new(),
                limit: MAX_HTML_BYTES * 8,
            },
            stack: vec![],
            seen: std::collections::HashSet::new(),
        };
        for (index, &position) in points.iter().enumerate() {
            // Preorder ids retain sibling order and zero-width ancestor chains at a shared boundary.
            for (code, _) in rows.iter().filter(|(_, n)| {
                n.targets
                    .iter()
                    .any(|r| r.is_empty() && r.start == position)
            }) {
                let mut path = vec![code.id];
                let mut parent = code.parent;
                while let Some(id) = parent {
                    path.push(id);
                    parent = self
                        .codes
                        .iter()
                        .find(|c| c.id == id)
                        .ok_or(Rejection::Identity)?
                        .parent;
                }
                path.reverse();
                transport.transition(&path)?;
            }
            let active = rows
                .iter()
                .filter(|(_, n)| {
                    n.targets
                        .iter()
                        .any(|r| r.start <= position && position < r.end)
                })
                .map(|(c, _)| c.id)
                .collect::<Vec<_>>();
            transport.transition(&active)?;
            if let Some(&end) = points.get(index + 1) {
                // A literal's exact target slice was checked above, and its source payload replaces it once.
                if !active.iter().any(|id| {
                    self.codes
                        .iter()
                        .any(|c| c.id == *id && c.literal.is_some())
                }) {
                    transport
                        .output
                        .push(&escape(&target[bytes[position]..bytes[end]]))
                        .map_err(|_| Rejection::Limit)?;
                }
            }
        }
        transport.transition(&[])?;
        let wrapped = format!("<root>{}</root>", transport.output.html);
        let doc = roxmltree::Document::parse(&wrapped).map_err(|_| Rejection::Reconstruction)?;
        let html = self
            .render(doc.root_element())
            .map_err(|_| Rejection::Reconstruction)?;
        let restored = Self::read_restored(html).map_err(|_| Rejection::Reconstruction)?;
        // Reader normalization must not hide lost text, duplicate literal content or invented separators.
        if restored.text != target {
            return Err(Rejection::Reconstruction);
        }
        Ok(restored)
    }
}

/// A boundary sweep splits only repeatable styles when an outside attachment interrupts their target range.
/// Reopening an ID/link/literal is an error, not an opportunity to clone or reparent source content.
struct Transport<'a> {
    plan: &'a ProtectedPlan,
    output: Reconstruction,
    stack: Vec<usize>,
    seen: std::collections::HashSet<usize>,
}

impl Transport<'_> {
    /// Close to the common source ancestry, then open only source identities in the desired target scope.
    fn transition(&mut self, desired: &[usize]) -> Result<(), Rejection> {
        let common = self
            .stack
            .iter()
            .zip(desired)
            .take_while(|(a, b)| a == b)
            .count();
        while self.stack.len() > common {
            self.output.push("</span>").map_err(|_| Rejection::Limit)?;
            self.stack.pop();
        }
        for &id in &desired[common..] {
            let code = self
                .plan
                .codes
                .iter()
                .find(|c| c.id == id)
                .ok_or(Rejection::Identity)?;
            if code.parent != self.stack.last().copied() {
                return Err(Rejection::Hierarchy);
            }
            if !self.seen.insert(id) && !code.repeatable {
                return Err(Rejection::IdentitySplit);
            }
            self.output
                .push(&format!(
                    "<span data-pc=\"{id}\"{}>",
                    if code.literal.is_some() {
                        " translate=\"no\""
                    } else {
                        ""
                    }
                ))
                .map_err(|_| Rejection::Limit)?;
            if code.literal.is_some() {
                self.output
                    .push(&code.literal_placeholder())
                    .map_err(|_| Rejection::Limit)?;
            }
            self.stack.push(id);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
