//! Bounded local receipts retain exact decoder pieces, never tensors or provider HTML.
use super::*;
use crate::translation::inline_markup::provider::contract::{Binding, NodeMapping, Pair, Versions};
use sha2::{Digest, Sha256};
use std::ops::Range;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MappedSegment {
    pub(crate) binding: Binding,
    units: Vec<Unit>,
    targets: Vec<Range<usize>>,
    nodes: Vec<NodeMapping>,
    checksum: String,
}

impl MappedSegment {
    /// Only fully reconstructed output becomes reusable; hashes detect local corruption, not model correctness.
    pub(crate) fn new(
        translation: &Translation<'_>,
        versions: Versions,
        nodes: Vec<NodeMapping>,
    ) -> Result<Self, String> {
        let plan = translation.plan;
        let binding = pair(plan, &translation.text, &versions)
            .binding()
            .map_err(error)?;
        plan.protected
            .restore_mapped(&translation.text, &nodes)
            .map_err(error)?;
        let mut value = Self {
            binding,
            units: plan.units().to_vec(),
            targets: translation.targets.clone(),
            nodes,
            checksum: String::new(),
        };
        value.checksum = value.digest()?;
        Ok(value)
    }

    /// Reuse only the exact admitted decoder pairs. A policy/tokenizer change still requires new mapping.
    pub(crate) fn translation<'a>(
        &self,
        plan: &'a SourcePlan,
        versions: &Versions,
        fits: impl Fn(&str, Side) -> Result<bool, String>,
    ) -> Option<Translation<'a>> {
        if !self.intact()
            || self.binding.versions.translator != versions.translator
            || self.binding.versions.normalization != versions.normalization
            || self.units != plan.units()
            || self.binding
                != pair(plan, &self.binding.target, &self.binding.versions)
                    .binding()
                    .ok()?
            || self.targets.len() != plan.units().len()
        {
            return None;
        }
        let bytes = scalar_bytes(&self.binding.target);
        let source_bytes = scalar_bytes(plan.protected.source_text());
        let mut cursor = 0;
        for (unit, range) in plan.units().iter().zip(&self.targets) {
            if range.start != cursor || range.start > range.end || range.end >= bytes.len() {
                return None;
            }
            let target = &self.binding.target[bytes[range.start]..bytes[range.end]];
            if unit.kind == Kind::Prose {
                if target.trim().is_empty() || !fits(target, Side::Target).ok()? {
                    return None;
                }
            } else if target
                != &plan.protected.source_text()
                    [source_bytes[unit.source.start]..source_bytes[unit.source.end]]
            {
                return None;
            }
            cursor = range.end;
        }
        let graphemes = self
            .binding
            .target
            .grapheme_indices(true)
            .map(|(i, _)| i)
            .chain([self.binding.target.len()])
            .collect::<Vec<_>>();
        if cursor != bytes.len() - 1
            || self.targets.iter().any(|r| {
                graphemes.binary_search(&bytes[r.start]).is_err()
                    || graphemes.binary_search(&bytes[r.end]).is_err()
            })
        {
            return None;
        }
        Some(Translation {
            plan,
            text: self.binding.target.clone(),
            targets: self.targets.clone(),
        })
    }

    /// Renderer admission repeats source/location/text binding and never invokes worker note repositioning.
    pub(crate) fn restore(
        &self,
        plan: &ProtectedPlan,
        segment: &TranslationTextSegment,
    ) -> Result<super::super::super::protected::RestoredMarkup, String> {
        if !self.intact()
            || !self
                .binding
                .versions
                .policy
                .ends_with(super::super::contract::PROJECTION_POLICY)
        {
            return Err("Invalid aligned cache receipt".into());
        }
        let expected = Pair {
            plan,
            segment_id: &segment.id,
            source_ordinal: segment.source_block_index,
            source_start: segment.source_start,
            target_start: 0,
            target: &self.binding.target,
            versions: &self.binding.versions,
        }
        .binding()
        .map_err(error)?;
        if self.binding != expected
            || segment.text != plan.source_text()
            || segment.source_end.checked_sub(segment.source_start)
                != Some(segment.text.chars().count())
        {
            return Err("Aligned fragment no longer matches source".into());
        }
        plan.restore_mapped(&self.binding.target, &self.nodes)
            .map_err(error)
    }

    /// Check size/counts before hashing or reconstruction; no tensor/link matrix enters a cache record.
    fn intact(&self) -> bool {
        [
            &self.binding.segment_id,
            &self.binding.source_plan,
            &self.binding.versions.translator,
            &self.binding.versions.aligner,
            &self.binding.versions.tokenizer,
            &self.binding.versions.normalization,
            &self.binding.versions.policy,
        ]
        .iter()
        .all(|s| !s.trim().is_empty() && s.len() <= 256)
            && self.binding.source.len() <= MAX_HTML_BYTES
            && self.binding.target.len() <= MAX_HTML_BYTES
            && self.units.len() <= MAX_UNITS
            && self.targets.len() <= MAX_UNITS
            && self.nodes.len() <= super::super::super::protected::MAX_CODES
            && self.nodes.iter().map(|n| n.targets.len()).sum::<usize>()
                <= super::super::contract::MAX_MAPPED_RANGES
            && self.checksum.len() == 64
            && self.digest().ok().as_ref() == Some(&self.checksum)
    }

    /// This checksum is an accidental-corruption guard; it does not authenticate an attacker-controlled disk.
    fn digest(&self) -> Result<String, String> {
        let bytes = serde_json::to_vec(&(&self.binding, &self.units, &self.targets, &self.nodes))
            .map_err(|_| "Aligned receipt serialization failed")?;
        Ok(format!("{:x}", Sha256::digest(bytes)))
    }
}

/// Use the shared binding contract so cache, projection and renderer agree on scalar coordinates.
fn pair<'a>(plan: &'a SourcePlan, target: &'a str, versions: &'a Versions) -> Pair<'a> {
    Pair {
        plan: &plan.protected,
        segment_id: &plan.segment_id,
        source_ordinal: plan.source_ordinal,
        source_start: plan.source_start,
        target_start: 0,
        target,
        versions,
    }
}

fn error(error: super::super::contract::Rejection) -> String {
    format!("Aligned reconstruction rejected: {error:?}")
}
