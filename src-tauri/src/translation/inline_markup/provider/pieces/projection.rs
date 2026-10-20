//! Combine checked piece scopes and anchored attachments against one retained source identity tree.
use super::*;
use crate::translation::inline_markup::{
    protected::SourceCode,
    provider::contract::{
        check_projected_nodes, NodeMapping, Rejection, ScopeIssue, WordLinks, MAX_MAPPED_RANGES,
    },
};

/// Borrow exact translated text and its source plan; consumers cannot manufacture a completed projection.
pub(crate) struct Projection<'a, 'p> {
    pub(crate) translation: &'a Translation<'p>,
    nodes: Vec<NodeMapping>,
}
impl Projection<'_, '_> {
    /// Use only the protected source renderer, bypassing every legacy formatting/note fallback.
    /// Cancellation discards a late reconstruction before it becomes eligible for later save admission.
    pub(crate) fn restore<E: From<Rejection>>(
        &self,
        check_cancelled: impl Fn() -> Result<(), E>,
    ) -> Result<crate::translation::inline_markup::protected::RestoredMarkup, E> {
        check_cancelled()?;
        let result = self
            .translation
            .plan
            .protected
            .restore_mapped(&self.translation.text, &self.nodes)?;
        check_cancelled()?;
        Ok(result)
    }
    /// Version scope semantics separately from the extraction model/rule; later cache admission needs both.
    pub(crate) fn policy(&self) -> &'static str {
        super::super::contract::PROJECTION_POLICY
    }
    pub(crate) fn nodes(&self) -> &[NodeMapping] {
        &self.nodes
    }
}

impl<'p> Translation<'p> {
    /// Align prose sequentially and validate all scopes atomically; copied units never enter a model.
    pub(crate) fn project<E: From<Rejection>>(
        &self,
        mut align: impl for<'b> FnMut(&'b str, &'b str) -> Result<WordLinks<'b>, E>,
        unresolved: Option<ScopeIssue>,
        check_cancelled: impl Fn() -> Result<(), E>,
    ) -> Result<Projection<'_, 'p>, E> {
        check_cancelled()?;
        if let Some(issue) = unresolved {
            return Err(Rejection::Unresolved(issue).into());
        }
        let codes = self.plan.protected.source_codes().collect::<Vec<_>>();
        if self.targets.len() != self.plan.units.len() || self.text.len() > MAX_HTML_BYTES {
            return Err(Rejection::Binding.into());
        }
        let source = self.plan.protected.source_text();
        let sb = scalar_bytes(source);
        let tb = scalar_bytes(&self.text);
        let mut nodes = codes
            .iter()
            .map(|c| NodeMapping {
                source_id: c.id,
                targets: vec![],
            })
            .collect::<Vec<_>>();
        let mut previous = 0;
        for (index, (unit, target)) in self.plan.units.iter().zip(&self.targets).enumerate() {
            check_cancelled()?;
            if target.start != previous || target.start > target.end || target.end >= tb.len() {
                return Err(Rejection::Binding.into());
            }
            previous = target.end;
            let st = &source[sb[unit.source.start]..sb[unit.source.end]];
            let tt = &self.text[tb[target.start]..tb[target.end]];
            // Clip coordinates only; the original source tree and authored identities remain unique.
            let local = codes
                .iter()
                .filter(|c| {
                    !c.range.is_empty()
                        && c.range.start < unit.source.end
                        && unit.source.start < c.range.end
                })
                .map(|c| SourceCode {
                    id: c.id,
                    parent: c.parent,
                    repeatable: c.repeatable,
                    literal: c.literal,
                    range: c.range.start.max(unit.source.start) - unit.source.start
                        ..c.range.end.min(unit.source.end) - unit.source.start,
                })
                .collect::<Vec<_>>();
            let attachments = self
                .plan
                .attachments
                .iter()
                .filter(|a| a.owner == index)
                .collect::<Vec<_>>();
            let mut anchored = Vec::new();
            let mut mapped = if unit.kind == Kind::Prose {
                let links = align(st, tt)?;
                if !links.matches(st, tt) {
                    return Err(Rejection::Binding.into());
                }
                for attachment in &attachments {
                    anchored.push((
                        attachment.source_id,
                        links.attachment(attachment.offset - unit.source.start)?,
                    ));
                }
                links.project(&local)?
            } else {
                if st != tt {
                    return Err(Rejection::Binding.into());
                }
                for attachment in &attachments {
                    anchored.push((attachment.source_id, attachment.offset - unit.source.start));
                }
                local
                    .iter()
                    .map(|c| NodeMapping {
                        source_id: c.id,
                        targets: vec![c.range.clone()],
                    })
                    .collect()
            };
            mapped.extend(anchored.into_iter().map(|(source_id, offset)| NodeMapping {
                source_id,
                targets: vec![offset..offset],
            }));
            for mapped in mapped {
                let node = nodes
                    .iter_mut()
                    .find(|n| n.source_id == mapped.source_id)
                    .ok_or(Rejection::Identity)?;
                for range in mapped.targets {
                    let range = target.start + range.start..target.start + range.end;
                    if let Some(last) = node
                        .targets
                        .last_mut()
                        .filter(|last| last.end == range.start)
                    {
                        last.end = range.end;
                    } else {
                        node.targets.push(range);
                    }
                }
            }
            // Bound the final quadratic hierarchy check even when every piece emits disjoint scopes.
            if nodes.iter().map(|n| n.targets.len()).sum::<usize>() > MAX_MAPPED_RANGES {
                return Err(Rejection::Limit.into());
            }
        }
        if previous != self.text.chars().count() {
            return Err(Rejection::Binding.into());
        }
        check_projected_nodes(&self.plan.protected, &self.text, &nodes)?;
        // Discard cancellation accepted during the last pair or final hierarchy assembly before returning any map.
        check_cancelled()?;
        Ok(Projection {
            translation: self,
            nodes,
        })
    }
}

#[cfg(test)]
mod tests;
