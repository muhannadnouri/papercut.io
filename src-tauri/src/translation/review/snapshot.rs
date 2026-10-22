//! Immutable native evidence plus a mutable, revision-bound emphasis draft.
//! Paths identify source/English nodes even when whole paragraphs repeat.

use kuchikiki::NodeRef;

use super::*;
use crate::translation::{
    dom::DomPlan, formatting::FormattingPolicy, html::parse_html_document,
    storage::PersistTranslationRequest,
};

const MAX_NODES: usize = 262_144;
const MAX_DEPTH: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Mapping {
    ordinal: u32,
    source_path: Vec<usize>,
    target_path: Vec<usize>,
    needs_review: bool,
    source_formatting: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in crate::translation) struct Snapshot {
    pub(in crate::translation) source_html: String,
    pub(in crate::translation) baseline_html: String,
    pub(in crate::translation) document: Document,
    normalization_policy: String,
    mappings: Vec<Mapping>,
}

impl Snapshot {
    /// Capture only after the existing writer has sanitized/indexed the native render.
    pub(in crate::translation) fn capture(
        request: &PersistTranslationRequest,
        variant_id: String,
        generation: String,
        published_html: &str,
    ) -> Result<Self, Error> {
        if request.formatting != FormattingPolicy::SentenceReview {
            return Err(Error::Model);
        }
        let source_html = crate::document_uploads::sanitize_html(&request.source.view_html);
        let normalization_policy = request
            .source
            .source_formatting
            .as_ref()
            .map(|report| report.policy.as_str())
            .unwrap_or("legacy-source-no-policy")
            .to_owned();
        let source = checked_plan(&source_html)?;
        let target = checked_plan(published_html)?;
        if source.blocks.len() != request.source.blocks.len()
            || source.blocks.len() != request.translated_sections.len()
            || source.blocks.len() != target.blocks.len()
        {
            return Err(Error::Evidence);
        }
        let mut mappings = Vec::with_capacity(source.blocks.len());
        let mut blocks = Vec::with_capacity(source.blocks.len());
        for (((source_block, original), translated), target_block) in source
            .blocks
            .iter()
            .zip(&request.source.blocks)
            .zip(&request.translated_sections)
            .zip(&target.blocks)
        {
            if source_block.ordinal != original.ordinal
                || source_block.inline.text != original.text
                || original.ordinal != translated.source_ordinal
                || original.is_heading != translated.is_heading
                || original.is_heading != source_block.is_heading
                || target_block.inline.text != translated.text
            {
                return Err(Error::Evidence);
            }
            let mapping = Mapping {
                ordinal: original.ordinal.try_into().map_err(|_| Error::Limit)?,
                source_path: path(&source.document, &source_block.node)?,
                target_path: path(&target.document, &target_block.node)?,
                needs_review: translated.fragments.iter().any(|f| f.needs_review),
                source_formatting: request
                    .source
                    .source_formatting
                    .as_ref()
                    .is_some_and(|report| !report.warnings.is_empty()),
            };
            blocks.push(capture_block(
                &source_block.node,
                &target_block.node,
                &mapping,
            )?);
            mappings.push(mapping);
        }
        let document = Document::new(
            &request.model_id,
            variant_id,
            generation,
            Evidence {
                normalization_policy: &normalization_policy,
                source_snapshot: &source_html,
                published_html,
            },
            blocks,
        )?;
        let snapshot = Self {
            source_html,
            baseline_html: published_html.to_owned(),
            document,
            normalization_policy,
            mappings,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }

    /// Deserialize is not admission: rebuild immutable text/structure/locks before accepting a draft.
    pub(in crate::translation) fn validate(&self) -> Result<(), Error> {
        self.document
            .check(&self.document.expected, self.evidence())?;
        if !valid_document_ids(
            &self.document.expected.variant_id,
            &self.document.expected.generation,
        ) {
            return Err(Error::Identity);
        }
        let source = checked_plan(&self.source_html)?;
        let target = checked_plan(&self.baseline_html)?;
        if self.mappings.is_empty()
            || self.mappings.len() > MAX_BLOCKS
            || self.mappings.len() != source.blocks.len()
            || self.mappings.len() != target.blocks.len()
            || self.mappings.len() != self.document.blocks.len()
        {
            return Err(Error::Evidence);
        }
        let mut total = 0usize;
        for (((mapping, source_block), target_block), saved) in self
            .mappings
            .iter()
            .zip(&source.blocks)
            .zip(&target.blocks)
            .zip(&self.document.blocks)
        {
            if source_block.ordinal != mapping.ordinal as usize
                || path(&source.document, &source_block.node)? != mapping.source_path
                || path(&target.document, &target_block.node)? != mapping.target_path
            {
                return Err(Error::Evidence);
            }
            let mut rebuilt = capture_block(&source_block.node, &target_block.node, mapping)?;
            rebuilt.editable = saved.editable.clone();
            rebuilt.approved = saved.approved;
            if &rebuilt != saved {
                return Err(Error::Evidence);
            }
            validate_marks(
                &Coordinates::new(&saved.text)?,
                &saved.editable,
                &saved.locked,
            )?;
            total = total.checked_add(saved.text.len()).ok_or(Error::Limit)?;
        }
        if total > MAX_DOCUMENT_TEXT_BYTES {
            return Err(Error::Limit);
        }
        Ok(())
    }

    pub(in crate::translation) fn with_document(&self, document: Document) -> Result<Self, Error> {
        let mut expected = self.document.expected.clone();
        expected.revision = document.expected.revision;
        if document.expected != expected {
            return Err(Error::Stale);
        }
        let mut snapshot = self.clone();
        snapshot.document = document;
        snapshot.validate()?;
        Ok(snapshot)
    }

    /// A failed edit leaves even the in-memory snapshot unchanged.
    pub(in crate::translation) fn apply(&mut self, request: &EditRequest) -> Result<(), Error> {
        self.validate()?;
        let evidence = Evidence {
            normalization_policy: &self.normalization_policy,
            source_snapshot: &self.source_html,
            published_html: &self.baseline_html,
        };
        super::apply(Some(&mut self.document), request, evidence)
    }

    /// Split existing English text nodes only; source shells, anchors and literal nodes never move.
    pub(in crate::translation) fn reconstruct(&self) -> Result<String, Error> {
        self.validate()?;
        if self.document.blocks.iter().all(|b| b.editable.is_empty()) {
            return Ok(self.baseline_html.clone());
        }
        let document = parse_html_document(&self.baseline_html);
        for (mapping, block) in self.mappings.iter().zip(&self.document.blocks) {
            let node = at_path(&document, &mapping.target_path)?;
            let mut offset = 0u32;
            let runs = canonical_runs(block.editable.iter().copied(), &block.locked);
            for text_node in node
                .inclusive_descendants()
                .filter(|n| n.as_text().is_some())
                .collect::<Vec<_>>()
            {
                let text = text_node
                    .as_text()
                    .ok_or(Error::Evidence)?
                    .borrow()
                    .to_string();
                let end = offset
                    .checked_add(text.chars().count() as u32)
                    .ok_or(Error::Limit)?;
                replace_text(&text_node, &text, offset, end, &runs)?;
                offset = end;
            }
            if offset != block.text.chars().count() as u32 {
                return Err(Error::Evidence);
            }
        }
        serialize(&document)
    }

    /// A separate namespace keeps full-source notes/backlinks usable beside the English preview.
    pub(in crate::translation) fn source_preview(&self) -> Result<String, Error> {
        self.validate()?;
        let document = parse_html_document(&self.source_html);
        let baseline = parse_html_document(&self.baseline_html);
        let identifiers = |document: &NodeRef| {
            document
                .inclusive_descendants()
                .filter_map(|n| n.as_element().map(|e| e.attributes.borrow().clone()))
                .flat_map(|attrs| {
                    ["id", "name"]
                        .into_iter()
                        .filter_map(move |name| attrs.get(name).map(str::to_owned))
                })
                .collect::<BTreeSet<_>>()
        };
        let existing = identifiers(&baseline);
        let source_ids = identifiers(&document);
        // Native generation tokens make collisions unlikely, but authored ids must never defeat isolation.
        let base = format!(
            "papercut-review-source-{}-",
            self.document.expected.generation
        );
        let mut blocked = BTreeSet::new();
        for id in &existing {
            if let Some(suffix) = id.strip_prefix(&base) {
                if source_ids.contains(suffix) {
                    blocked.insert(0);
                }
                if let Some((number, original)) = suffix.split_once('-') {
                    if let Ok(number_value) = number.parse::<usize>() {
                        if number_value > 0
                            && number_value.to_string() == number
                            && source_ids.contains(original)
                        {
                            blocked.insert(number_value);
                        }
                    }
                }
            }
        }
        let suffix = (0..=blocked.len())
            .find(|n| !blocked.contains(n))
            .ok_or(Error::Evidence)?;
        let prefix = if suffix == 0 {
            base
        } else {
            format!("{base}{suffix}-")
        };
        for node in document.inclusive_descendants() {
            if let Some(element) = node.as_element() {
                let mut attrs = element.attributes.borrow_mut();
                for name in ["id", "name"] {
                    if let Some(value) = attrs.get(name).map(str::to_owned) {
                        attrs.insert(name, format!("{prefix}{value}"));
                    }
                }
                if let Some(value) = attrs
                    .get("href")
                    .and_then(|v| v.strip_prefix('#'))
                    .map(str::to_owned)
                {
                    attrs.insert("href", format!("#{prefix}{value}"));
                }
            }
        }
        serialize(&document)
    }

    fn evidence(&self) -> Evidence<'_> {
        Evidence {
            normalization_policy: &self.normalization_policy,
            source_snapshot: &self.source_html,
            published_html: &self.baseline_html,
        }
    }
}

/// Native persisted HTML must already satisfy the reader sanitizer, including all URL rules.
fn checked_plan(html: &str) -> Result<DomPlan, Error> {
    if html.is_empty() || html.len() > MAX_SNAPSHOT_BYTES {
        return Err(Error::Limit);
    }
    let document = parse_html_document(html);
    let mut stack = vec![(document.clone(), 0usize)];
    let mut count = 0usize;
    while let Some((node, depth)) = stack.pop() {
        count += 1;
        if count > MAX_NODES || depth > MAX_DEPTH {
            return Err(Error::Limit);
        }
        stack.extend(node.children().map(|child| (child, depth + 1)));
    }
    if serialize(&document)?
        != serialize(&parse_html_document(
            crate::document_uploads::sanitize_html(html),
        ))?
    {
        return Err(Error::Evidence);
    }
    let plan = DomPlan::new(html);
    if plan.blocks.len() > MAX_BLOCKS {
        return Err(Error::Limit);
    }
    Ok(plan)
}

fn capture_block(source: &NodeRef, target: &NodeRef, mapping: &Mapping) -> Result<Block, Error> {
    let mut geometry = Geometry::default();
    collect(target, &mut geometry)?;
    let mut obligations = BTreeSet::new();
    if mapping.needs_review {
        obligations.insert(Obligation::Emphasis);
        for node in source.inclusive_descendants() {
            if let Some(element) = node.as_element() {
                let tag = element.name.local.as_ref();
                if crate::document_uploads::is_note_marker(&node) {
                    obligations.insert(Obligation::Note);
                } else if tag == "a" {
                    obligations.insert(Obligation::Link);
                } else if matches!(tag, "img" | "audio" | "video" | "table") {
                    obligations.insert(Obligation::Attachment);
                } else if matches!(tag, "u" | "s" | "sub" | "sup" | "mark") {
                    obligations.insert(Obligation::Typography);
                }
            }
        }
    }
    if mapping.source_formatting {
        obligations.insert(Obligation::SourceFormatting);
    }
    Block::new(
        BlockKey {
            ordinal: mapping.ordinal,
            source_identity: fingerprint(&format!(
                "{}:{:?}:{}",
                mapping.ordinal,
                mapping.source_path,
                serialize(source)?
            )),
        },
        fingerprint(&serialize(target)?),
        geometry.text,
        geometry.locked,
        geometry.fixed,
        obligations,
    )
}

#[derive(Default)]
struct Geometry {
    text: String,
    scalars: u32,
    fixed: Vec<Mark>,
    locked: Vec<ScalarRange>,
}

/// Existing links/literals are locked intervals; attachment and structural cuts cannot be crossed.
fn collect(node: &NodeRef, geometry: &mut Geometry) -> Result<(), Error> {
    if let Some(text) = node.as_text() {
        if geometry.text.len().saturating_add(text.borrow().len()) > MAX_BLOCK_BYTES {
            return Err(Error::Limit);
        }
        geometry.text.push_str(&text.borrow());
        geometry.scalars = geometry
            .scalars
            .checked_add(text.borrow().chars().count() as u32)
            .ok_or(Error::Limit)?;
        return Ok(());
    }
    let start = geometry.scalars;
    let tag = node.as_element().map(|e| e.name.local.to_string());
    let protected = crate::document_uploads::is_note_marker(node)
        || node.as_element().is_some_and(|e| {
            matches!(
                e.name.local.as_ref(),
                "a" | "code" | "kbd" | "samp" | "pre" | "u" | "s" | "sub" | "sup" | "mark"
            ) || e
                .attributes
                .borrow()
                .get("translate")
                .is_some_and(|v| v.eq_ignore_ascii_case("no"))
        });
    for child in node.children() {
        collect(&child, geometry)?;
    }
    let end = geometry.scalars;
    if protected
        || tag
            .as_deref()
            .is_some_and(|t| matches!(t, "img" | "br" | "hr" | "table" | "td" | "th"))
    {
        geometry.locked.push(ScalarRange { start, end });
    }
    let kind = match tag.as_deref() {
        Some("b" | "strong") => Some(MarkKind::Bold),
        Some("i" | "em") => Some(MarkKind::Italic),
        _ => None,
    };
    if let Some(kind) = kind.filter(|_| start < end) {
        geometry.fixed.push(Mark {
            range: ScalarRange { start, end },
            kind,
        });
    }
    if geometry.locked.len() > MAX_MARKS || geometry.fixed.len() > MAX_MARKS {
        return Err(Error::Limit);
    }
    Ok(())
}

fn path(root: &NodeRef, node: &NodeRef) -> Result<Vec<usize>, Error> {
    let mut child = node.clone();
    let mut result = Vec::new();
    while child != *root {
        let parent = child.parent().ok_or(Error::Evidence)?;
        result.push(
            parent
                .children()
                .position(|n| n == child)
                .ok_or(Error::Evidence)?,
        );
        child = parent;
        if result.len() > MAX_DEPTH {
            return Err(Error::Limit);
        }
    }
    result.reverse();
    Ok(result)
}

fn at_path(root: &NodeRef, path: &[usize]) -> Result<NodeRef, Error> {
    if path.len() > MAX_DEPTH {
        return Err(Error::Limit);
    }
    let mut node = root.clone();
    for index in path {
        node = node.children().nth(*index).ok_or(Error::Evidence)?;
    }
    Ok(node)
}

/// Escaped text nodes retain exact characters; b/i wrappers are created by native constants only.
fn replace_text(
    node: &NodeRef,
    text: &str,
    start: u32,
    end: u32,
    runs: &[Run],
) -> Result<(), Error> {
    let mut boundaries = BTreeSet::from([start, end]);
    for run in runs {
        for point in [run.range.start, run.range.end] {
            if start < point && point < end {
                boundaries.insert(point);
            }
        }
    }
    if !runs
        .iter()
        .any(|r| r.range.start < end && start < r.range.end)
    {
        return Ok(());
    }
    let characters: Vec<_> = text.chars().collect();
    let points: Vec<_> = boundaries.into_iter().collect();
    for pair in points.windows(2) {
        let value: String = characters[(pair[0] - start) as usize..(pair[1] - start) as usize]
            .iter()
            .collect();
        let state = runs
            .iter()
            .find(|r| r.range.start <= pair[0] && pair[1] <= r.range.end);
        let mut addition = NodeRef::new_text(value);
        for tag in ["i", "b"] {
            let enabled = state.is_some_and(|r| if tag == "b" { r.bold } else { r.italic });
            if enabled {
                let fragment = parse_html_document(format!("<span><{tag}></{tag}></span>"));
                let wrapper = fragment
                    .select_first(tag)
                    .map_err(|_| Error::Evidence)?
                    .as_node()
                    .clone();
                wrapper.append(addition);
                addition = wrapper;
            }
        }
        node.insert_before(addition);
    }
    node.detach();
    Ok(())
}

fn serialize(node: &NodeRef) -> Result<String, Error> {
    let mut bytes = Vec::new();
    node.serialize(&mut bytes).map_err(|_| Error::Evidence)?;
    String::from_utf8(bytes).map_err(|_| Error::Evidence)
}

#[cfg(test)]
mod tests;
