//! Assisted-emphasis contracts and pure source-owned reconstruction. Storage owns durable
//! identity and compare-and-swap; this module never performs inference or publishes a draft.
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use unicode_segmentation::UnicodeSegmentation;

pub(super) const SCHEMA_VERSION: u32 = 1;
pub(super) const POLICY: &str = "opus-assisted-emphasis-v1";
pub(super) const MAX_BLOCKS: usize = 16_384;
pub(super) const MAX_BLOCK_BYTES: usize = 32 * 1024;
pub(super) const MAX_DOCUMENT_TEXT_BYTES: usize = 25 * 1024 * 1024;
pub(super) const MAX_SNAPSHOT_BYTES: usize = 120 * 1024 * 1024;
pub(super) const MAX_MARKS: usize = 128;
pub(super) const MAX_REQUEST_BYTES: usize = 16 * 1024;
// IPC revisions must remain exact in JavaScript and fit the SQLite integer column.
pub(super) const MAX_REVISION: u64 = (1 << 53) - 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Error {
    Missing,
    Version,
    Model,
    Identity,
    Limit,
    Range,
    Locked,
    Stale,
    Evidence,
    Pending,
    Revision,
    Request,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ScalarRange {
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum MarkKind {
    Bold,
    Italic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Mark {
    pub range: ScalarRange,
    pub kind: MarkKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Run {
    pub range: ScalarRange,
    pub bold: bool,
    pub italic: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum Obligation {
    Emphasis,
    Typography,
    Link,
    Note,
    Attachment,
    SourceFormatting,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct BlockKey {
    pub ordinal: u32,
    pub source_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Expected {
    pub schema_version: u32,
    pub policy: String,
    pub variant_id: String,
    pub generation: String,
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "action", rename_all = "camelCase", deny_unknown_fields)]
pub(super) enum Edit {
    ReplaceMarks { marks: Vec<Mark> },
    Reset {},
    Approve {},
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct EditRequest {
    pub expected: Expected,
    pub block: BlockKey,
    pub edit: Edit,
}

/// Bound typed decoding before allocating mark arrays; a future IPC handler must not bypass this admission.
/// This is not a limit on the transport's own input-buffer allocation.
pub(super) fn decode_request(bytes: &[u8]) -> Result<EditRequest, Error> {
    if bytes.len() > MAX_REQUEST_BYTES {
        return Err(Error::Limit);
    }
    let request: EditRequest = serde_json::from_slice(bytes).map_err(|_| Error::Request)?;
    validate_expected(&request.expected)?;
    if !valid_identity(&request.block.source_identity) {
        return Err(Error::Identity);
    }
    if matches!(&request.edit, Edit::ReplaceMarks { marks } if marks.len() > MAX_MARKS) {
        return Err(Error::Limit);
    }
    Ok(request)
}

/// Finish accepts only the native generation/revision token, never client HTML or approval claims.
pub(super) fn decode_finish(bytes: &[u8]) -> Result<Expected, Error> {
    if bytes.len() > MAX_REQUEST_BYTES {
        return Err(Error::Limit);
    }
    let expected: Expected = serde_json::from_slice(bytes).map_err(|_| Error::Request)?;
    validate_expected(&expected)?;
    Ok(expected)
}

fn validate_expected(expected: &Expected) -> Result<(), Error> {
    if expected.schema_version != SCHEMA_VERSION || expected.policy != POLICY {
        return Err(Error::Version);
    }
    if !valid_document_ids(&expected.variant_id, &expected.generation) {
        return Err(Error::Identity);
    }
    if expected.revision > MAX_REVISION {
        return Err(Error::Revision);
    }
    Ok(())
}

/// Native-owned read snapshots; callers must not substitute client-provided strings here.
pub(super) struct Evidence<'a> {
    pub normalization_policy: &'a str,
    pub source_snapshot: &'a str,
    pub published_html: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Block {
    key: BlockKey,
    structure_identity: String,
    text: String,
    text_hash: String,
    locked: Vec<ScalarRange>,
    // Source-validated baseline marks can cover locked literals; edits never change this channel.
    fixed: Vec<Mark>,
    editable: Vec<Mark>,
    obligations: BTreeSet<Obligation>,
    approved: bool,
}

impl Block {
    /// Retain exact native English; geometry admission alone does not validate source hierarchy or meaning.
    pub(super) fn new(
        key: BlockKey,
        structure_identity: String,
        text: String,
        locked: Vec<ScalarRange>,
        fixed: Vec<Mark>,
        obligations: BTreeSet<Obligation>,
    ) -> Result<Self, Error> {
        if !valid_identity(&key.source_identity) || !valid_identity(&structure_identity) {
            return Err(Error::Identity);
        }
        let coordinates = Coordinates::new(&text)?;
        if locked.len() > MAX_MARKS || fixed.len() > MAX_MARKS {
            return Err(Error::Limit);
        }
        for range in &locked {
            coordinates.validate(*range, true)?;
        }
        validate_marks(&coordinates, &fixed, &[])?;
        Ok(Self {
            key,
            structure_identity,
            text_hash: fingerprint(&text),
            text,
            locked,
            fixed,
            editable: Vec::new(),
            approved: obligations.is_empty(),
            obligations,
        })
    }

    /// Approval resolves emphasis only; attachment, typography and import gaps remain independent blockers.
    pub(super) fn remaining(&self) -> BTreeSet<Obligation> {
        let mut remaining = self.obligations.clone();
        if self.approved {
            remaining.remove(&Obligation::Emphasis);
        } else {
            remaining.insert(Obligation::Emphasis);
        }
        remaining
    }

    /// A deterministic geometry preview, not permission to render arbitrary source structures.
    pub(super) fn runs(&self) -> Vec<Run> {
        canonical_runs(
            self.fixed.iter().chain(&self.editable).copied(),
            &self.locked,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Document {
    expected: Expected,
    normalization_policy: String,
    source_hash: String,
    published_hash: String,
    blocks: Vec<Block>,
}

impl Document {
    pub(super) fn expected(&self) -> &Expected {
        &self.expected
    }

    /// Build only from native OPUS evidence; legacy records must not acquire editable identities by guessing.
    pub(super) fn new(
        model: &str,
        variant_id: String,
        generation: String,
        evidence: Evidence<'_>,
        blocks: Vec<Block>,
    ) -> Result<Self, Error> {
        if !super::formatting::FormattingPolicy::supports_sentence_review(model) {
            return Err(Error::Model);
        }
        if !valid_document_ids(&variant_id, &generation) {
            return Err(Error::Identity);
        }
        validate_evidence_size(&evidence)?;
        if blocks.is_empty() || blocks.len() > MAX_BLOCKS {
            return Err(Error::Limit);
        }
        let mut ordinals = BTreeSet::new();
        let mut total = 0usize;
        for block in &blocks {
            // Repeated text/source hashes remain distinct only through their native source ordinal.
            if !ordinals.insert(block.key.ordinal) {
                return Err(Error::Identity);
            }
            total = total.checked_add(block.text.len()).ok_or(Error::Limit)?;
            if total > MAX_DOCUMENT_TEXT_BYTES {
                return Err(Error::Limit);
            }
        }
        Ok(Self {
            expected: Expected {
                schema_version: SCHEMA_VERSION,
                policy: POLICY.into(),
                variant_id,
                generation,
                revision: 0,
            },
            normalization_policy: evidence.normalization_policy.into(),
            source_hash: fingerprint(evidence.source_snapshot),
            published_hash: fingerprint(evidence.published_html),
            blocks,
        })
    }

    /// Recheck optimistic identity plus exact snapshots; Stage 4 must repeat this under the writer's lock.
    fn check(&self, expected: &Expected, evidence: Evidence<'_>) -> Result<(), Error> {
        if expected.schema_version != SCHEMA_VERSION || expected.policy != POLICY {
            return Err(Error::Version);
        }
        if expected.revision > MAX_REVISION {
            return Err(Error::Revision);
        }
        if expected != &self.expected {
            return Err(Error::Stale);
        }
        validate_evidence_size(&evidence)?;
        if evidence.normalization_policy != self.normalization_policy
            || fingerprint(evidence.source_snapshot) != self.source_hash
            || fingerprint(evidence.published_html) != self.published_hash
            || self
                .blocks
                .iter()
                .any(|b| fingerprint(&b.text) != b.text_hash)
        {
            return Err(Error::Evidence);
        }
        Ok(())
    }
}

/// Validate the entire operation before mutation. Missing/deleted bases never become new records.
pub(super) fn apply(
    current: Option<&mut Document>,
    request: &EditRequest,
    evidence: Evidence<'_>,
) -> Result<(), Error> {
    let document = current.ok_or(Error::Missing)?;
    document.check(&request.expected, evidence)?;
    let index = document
        .blocks
        .iter()
        .position(|b| b.key == request.block)
        .ok_or(Error::Identity)?;
    let next_revision = document
        .expected
        .revision
        .checked_add(1)
        .filter(|r| *r <= MAX_REVISION)
        .ok_or(Error::Revision)?;
    let block = &document.blocks[index];
    let (marks, approved) = match &request.edit {
        Edit::ReplaceMarks { marks } => {
            validate_marks(&Coordinates::new(&block.text)?, marks, &block.locked)?;
            (marks.clone(), false)
        }
        Edit::Reset {} => (Vec::new(), false),
        Edit::Approve {} => (block.editable.clone(), true),
    };
    document.blocks[index].editable = marks;
    document.blocks[index].approved = approved;
    document.expected.revision = next_revision;
    Ok(())
}

/// Pure publication eligibility only; it neither publishes nor establishes a database compare-and-swap.
pub(super) fn finish(
    current: Option<&Document>,
    expected: &Expected,
    evidence: Evidence<'_>,
) -> Result<(), Error> {
    let document = current.ok_or(Error::Missing)?;
    document.check(expected, evidence)?;
    if document.blocks.iter().any(|b| !b.remaining().is_empty()) {
        return Err(Error::Pending);
    }
    Ok(())
}

struct Coordinates {
    bytes: Vec<usize>,
    utf16: Vec<u32>,
    graphemes: BTreeSet<usize>,
}

impl Coordinates {
    /// One bounded text table keeps scalar, UTF-16 and grapheme checks on exactly the same untouched bytes.
    fn new(text: &str) -> Result<Self, Error> {
        if text.trim().is_empty() || text.len() > MAX_BLOCK_BYTES {
            return Err(Error::Limit);
        }
        let mut bytes = Vec::new();
        let mut utf16 = vec![0];
        let mut offset = 0;
        for (byte, ch) in text.char_indices() {
            bytes.push(byte);
            offset += ch.len_utf16() as u32;
            utf16.push(offset);
        }
        bytes.push(text.len());
        let graphemes = text
            .grapheme_indices(true)
            .map(|(byte, _)| byte)
            .chain(std::iter::once(text.len()))
            .collect();
        Ok(Self {
            bytes,
            utf16,
            graphemes,
        })
    }

    fn validate(&self, range: ScalarRange, empty: bool) -> Result<(), Error> {
        if range.start > range.end || (!empty && range.start == range.end) {
            return Err(Error::Range);
        }
        for scalar in [range.start, range.end] {
            let byte = self.bytes.get(scalar as usize).ok_or(Error::Range)?;
            if !self.graphemes.contains(byte) {
                return Err(Error::Range);
            }
        }
        Ok(())
    }
}

/// DOM/textarea selection offsets may split an emoji surrogate pair; reject rather than round them.
pub(super) fn selection_from_utf16(text: &str, start: u32, end: u32) -> Result<ScalarRange, Error> {
    let coordinates = Coordinates::new(text)?;
    let range = ScalarRange {
        start: coordinates
            .utf16
            .binary_search(&start)
            .map_err(|_| Error::Range)? as u32,
        end: coordinates
            .utf16
            .binary_search(&end)
            .map_err(|_| Error::Range)? as u32,
    };
    coordinates.validate(range, false)?;
    Ok(range)
}

/// Full interval intersection matters: endpoint checks alone miss edits enclosing a literal or note anchor.
fn validate_marks(
    coordinates: &Coordinates,
    marks: &[Mark],
    locked: &[ScalarRange],
) -> Result<(), Error> {
    if marks.len() > MAX_MARKS {
        return Err(Error::Limit);
    }
    for mark in marks {
        coordinates.validate(mark.range, false)?;
        if locked.iter().any(|lock| {
            if lock.start == lock.end {
                mark.range.start < lock.start && lock.start < mark.range.end
            } else {
                mark.range.start < lock.end && lock.start < mark.range.end
            }
        }) {
            return Err(Error::Locked);
        }
    }
    Ok(())
}

/// Sweep interval endpoints, merging adjacent equivalent B/I states without altering the English string.
fn canonical_runs(marks: impl Iterator<Item = Mark>, locked: &[ScalarRange]) -> Vec<Run> {
    let mut events = BTreeMap::<u32, [i32; 2]>::new();
    for mark in marks {
        let kind = usize::from(mark.kind == MarkKind::Italic);
        events.entry(mark.range.start).or_default()[kind] += 1;
        events.entry(mark.range.end).or_default()[kind] -= 1;
    }
    // Keep protected cut points even when adjacent edits have identical style.
    let cuts: BTreeSet<_> = locked.iter().flat_map(|r| [r.start, r.end]).collect();
    for cut in &cuts {
        events.entry(*cut).or_default();
    }
    let mut active = [0, 0];
    let mut previous = 0;
    let mut runs: Vec<Run> = Vec::new();
    for (end, delta) in events {
        if previous < end && active.iter().any(|n| *n > 0) {
            let next = Run {
                range: ScalarRange {
                    start: previous,
                    end,
                },
                bold: active[0] > 0,
                italic: active[1] > 0,
            };
            if let Some(last) = runs.last_mut().filter(|r| {
                r.range.end == previous
                    && !cuts.contains(&previous)
                    && r.bold == next.bold
                    && r.italic == next.italic
            }) {
                last.range.end = end;
            } else {
                runs.push(next);
            }
        }
        active[0] += delta[0];
        active[1] += delta[1];
        previous = end;
    }
    runs
}

fn valid_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}

fn valid_document_ids(variant_id: &str, generation: &str) -> bool {
    variant_id.len() <= 128
        && crate::document_uploads::validate_upload_id(variant_id).is_ok()
        && generation.len() == 32
        && generation.bytes().all(|c| c.is_ascii_hexdigit())
}

fn validate_evidence_size(evidence: &Evidence<'_>) -> Result<(), Error> {
    if !valid_identity(evidence.normalization_policy) {
        return Err(Error::Identity);
    }
    if evidence.source_snapshot.is_empty()
        || evidence.published_html.is_empty()
        || evidence.source_snapshot.len() > MAX_SNAPSHOT_BYTES
        || evidence.published_html.len() > MAX_SNAPSHOT_BYTES
    {
        return Err(Error::Limit);
    }
    Ok(())
}

/// Cryptographic byte identity protects native review evidence; the MT cache's FNV keys serve a different purpose.
pub(super) fn fingerprint(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

mod snapshot;
pub(super) use snapshot::{BlockPreview, Snapshot};

#[cfg(test)]
mod tests;
