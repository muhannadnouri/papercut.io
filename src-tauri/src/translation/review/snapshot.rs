//! Immutable native evidence plus a mutable, revision-bound emphasis draft.
//! Paths identify source/English nodes even when whole paragraphs repeat.

use kuchikiki::NodeRef;
use std::collections::HashMap;

use super::*;
use crate::translation::{
    dom::DomPlan, formatting::FormattingPolicy, html::parse_html_document,
    storage::PersistTranslationRequest,
};

const MAX_NODES: usize = 262_144;
const MAX_DEPTH: usize = 128;

#[cfg(test)]
thread_local! {
    static PATH_VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

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

/// Comparison fragments contain native-sanitized evidence; the UI must still render an inert semantic clone.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::translation) struct BlockPreview {
    source_html: String,
    inherited_marks: Vec<Mark>,
    blockers: Vec<String>,
}

pub(in crate::translation) struct EditorPreview {
    pub source_html: String,
    pub target_html: String,
    pub blocks: Vec<BlockPreview>,
    pub publication_error: Option<String>,
}

/// Parsed evidence is borrowed for one operation; deserialized snapshots never carry admission flags.
struct ValidatedSnapshot<'a> {
    snapshot: &'a Snapshot,
    source: DomPlan,
    target: DomPlan,
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
        let (source_paths, _) = block_paths(&source)?;
        let (target_paths, _) = block_paths(&target)?;
        if source.blocks.len() != request.source.blocks.len()
            || source.blocks.len() != request.translated_sections.len()
            || source.blocks.len() != target.blocks.len()
        {
            return Err(Error::Evidence);
        }
        let mut mappings = Vec::with_capacity(source.blocks.len());
        let mut blocks = Vec::with_capacity(source.blocks.len());
        for (index, (((source_block, original), translated), target_block)) in source
            .blocks
            .iter()
            .zip(&request.source.blocks)
            .zip(&request.translated_sections)
            .zip(&target.blocks)
            .enumerate()
        {
            if source_block.ordinal != original.ordinal
                || source_block.inline.text != original.text
                || original.ordinal != translated.source_ordinal
                || original.is_heading != translated.is_heading
                || original.is_heading != source_block.is_heading
                // Reader coverage collapses whitespace; immutable text and edit coordinates stay exact.
                || target_block.inline.text != crate::document_uploads::normalize_text(&translated.text)
            {
                return Err(Error::Evidence);
            }
            let mapping = Mapping {
                ordinal: original.ordinal.try_into().map_err(|_| Error::Limit)?,
                source_path: source_paths[index].clone(),
                target_path: target_paths[index].clone(),
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
        self.validated().map(|_| ())
    }

    /// Keep the checked native trees so preview work cannot detach their block identities by reparsing.
    fn validated(&self) -> Result<ValidatedSnapshot<'_>, Error> {
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
        let (source_paths, _) = block_paths(&source)?;
        let (target_paths, _) = block_paths(&target)?;
        if self.mappings.is_empty()
            || self.mappings.len() > MAX_BLOCKS
            || self.mappings.len() != source.blocks.len()
            || self.mappings.len() != target.blocks.len()
            || self.mappings.len() != self.document.blocks.len()
        {
            return Err(Error::Evidence);
        }
        let mut total = 0usize;
        for (index, (((mapping, source_block), target_block), saved)) in self
            .mappings
            .iter()
            .zip(&source.blocks)
            .zip(&target.blocks)
            .zip(&self.document.blocks)
            .enumerate()
        {
            if source_block.ordinal != mapping.ordinal as usize
                || source_paths[index] != mapping.source_path
                || target_paths[index] != mapping.target_path
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
        Ok(ValidatedSnapshot {
            snapshot: self,
            source,
            target,
        })
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
    #[cfg(test)]
    pub(in crate::translation) fn reconstruct(&self) -> Result<String, Error> {
        self.validated()?.reconstruct()
    }

    /// Both response previews share one evidence validation and the same parsed source/English trees.
    #[cfg(test)]
    pub(in crate::translation) fn previews(&self) -> Result<(String, String), Error> {
        let validated = self.validated()?;
        Ok((validated.source_preview()?, validated.reconstruct()?))
    }

    /// Present blockers from immutable evidence even before approval, using one validated parse and mark pass.
    pub(in crate::translation) fn editor_preview(&self) -> Result<EditorPreview, Error> {
        let validated = self.validated()?;
        let source_html = validated.source_preview()?;
        // Fragments share the full source preview's namespace, and never become reader HTML.
        let blocks = validated.block_previews()?;
        let publication_guard = self
            .document
            .blocks
            .iter()
            .all(|block| block.remaining().is_empty())
            .then(|| {
                (
                    consumer_signature(&validated.target.document),
                    document_identifiers(&validated.target.document),
                )
            });
        validated.apply_marks()?;
        let target_html = if self
            .document
            .blocks
            .iter()
            .all(|block| block.editable.is_empty())
        {
            self.baseline_html.clone()
        } else {
            serialize(&validated.target.document)?
        };
        let publication_error = if let Some((before, identifiers)) = publication_guard {
            validated
                .publication_marked(&self.document.expected, before, identifiers)
                .err()
                .map(|error| {
                    match error {
                        Error::Pending => "publication-blocked",
                        Error::Limit => "publication-limit",
                        _ => "publication-invalid",
                    }
                    .to_owned()
                })
        } else {
            None
        };
        Ok(EditorPreview {
            source_html,
            target_html,
            blocks,
            publication_error,
        })
    }

    /// Publication removes fallback prose only after native approval and consumer-equivalence checks.
    pub(in crate::translation) fn publication(&self, expected: &Expected) -> Result<String, Error> {
        let validated = self.validated()?;
        super::finish(Some(&self.document), expected, self.evidence())?;
        let before = consumer_signature(&validated.target.document);
        let identifiers = document_identifiers(&validated.target.document);
        validated.apply_marks()?;
        validated.publication_marked(expected, before, identifiers)
    }

    /// A separate namespace keeps full-source notes/backlinks usable beside the English preview.
    #[cfg(test)]
    pub(in crate::translation) fn source_preview(&self) -> Result<String, Error> {
        self.validated()?.source_preview()
    }

    fn evidence(&self) -> Evidence<'_> {
        Evidence {
            normalization_policy: &self.normalization_policy,
            source_snapshot: &self.source_html,
            published_html: &self.baseline_html,
        }
    }
}

impl ValidatedSnapshot<'_> {
    /// Mark application is shared with the editor; this gate still enforces approval and every publication invariant.
    fn publication_marked(
        self,
        expected: &Expected,
        before: Vec<ConsumerPart>,
        identifiers: BTreeMap<(String, String), usize>,
    ) -> Result<String, Error> {
        super::finish(
            Some(&self.snapshot.document),
            expected,
            self.snapshot.evidence(),
        )?;
        for (mapping, block) in self.snapshot.mappings.iter().zip(&self.target.blocks) {
            if !mapping.needs_review {
                continue;
            }
            let (label, original) = retained_original(&block.node)?;
            if source_only_blocker(&original).is_some() {
                return Err(Error::Pending);
            }
            let original_element = original.as_element().ok_or(Error::Evidence)?;
            let target_element = block.node.as_element().ok_or(Error::Evidence)?;
            for name in ["id", "name"] {
                if let Some(value) = original_element.attributes.borrow().get(name) {
                    let mut target = target_element.attributes.borrow_mut();
                    if target.get(name).is_some_and(|existing| existing != value) {
                        return Err(Error::Evidence);
                    }
                    target.insert(name, value.to_owned());
                }
            }
            label.detach();
            original.detach();
        }
        if self
            .target
            .document
            .select("[data-papercut-translation-review]")
            .map_err(|_| Error::Evidence)?
            .next()
            .is_some()
            || consumer_signature(&self.target.document) != before
            || document_identifiers(&self.target.document) != identifiers
        {
            return Err(Error::Evidence);
        }
        let output = serialize(&self.target.document)?;
        // Added wrappers must obey the same native snapshot bounds and reader sanitizer as the baseline.
        checked_plan(&output)?;
        let before =
            crate::document_uploads::parse_reader_html_document(&self.snapshot.baseline_html);
        let after = crate::document_uploads::parse_reader_html_document(&output);
        if before.sections.len() != after.sections.len()
            || before.sections.iter().zip(&after.sections).any(|(a, b)| {
                a.heading != b.heading
                    || a.text != b.text
                    || a.body_ranges != b.body_ranges
                    || a.page_index != b.page_index
            })
        {
            return Err(Error::Evidence);
        }
        Ok(output)
    }

    /// Derive presentation context without migrating immutable geometry in older snapshot generations.
    fn block_previews(&self) -> Result<Vec<BlockPreview>, Error> {
        self.snapshot
            .document
            .blocks
            .iter()
            .zip(&self.snapshot.mappings)
            .zip(&self.target.blocks)
            .zip(&self.source.blocks)
            .map(|(((block, mapping), target), source)| {
                let mut blockers = block
                    .remaining()
                    .into_iter()
                    .filter_map(|obligation| match obligation {
                        Obligation::Emphasis => None,
                        Obligation::Typography => Some("typography"),
                        Obligation::Link => Some("link"),
                        Obligation::Note => Some("note"),
                        Obligation::Attachment => Some("attachment"),
                        Obligation::SourceFormatting => Some("sourceFormatting"),
                    })
                    .map(str::to_owned)
                    .collect::<Vec<_>>();
                if mapping.needs_review {
                    let (_, original) = retained_original(&target.node)?;
                    if let Some(code) = source_only_blocker(&original) {
                        blockers.push(code.to_owned());
                    }
                }
                let mut source_html = serialize(&source.node)?;
                for kind in inherited_emphasis(&source.node) {
                    let tag = if kind == MarkKind::Bold {
                        "strong"
                    } else {
                        "em"
                    };
                    source_html = format!("<{tag}>{source_html}</{tag}>");
                }
                let end = block
                    .text
                    .chars()
                    .count()
                    .try_into()
                    .map_err(|_| Error::Limit)?;
                let inherited_marks = inherited_emphasis(&target.node)
                    .into_iter()
                    .map(|kind| Mark {
                        range: ScalarRange { start: 0, end },
                        kind,
                    })
                    .collect();
                Ok(BlockPreview {
                    source_html,
                    inherited_marks,
                    blockers,
                })
            })
            .collect()
    }

    /// Consume this operation's fresh tree; another preview always starts from the immutable baseline.
    #[cfg(test)]
    fn reconstruct(self) -> Result<String, Error> {
        if self
            .snapshot
            .document
            .blocks
            .iter()
            .all(|b| b.editable.is_empty())
        {
            return Ok(self.snapshot.baseline_html.clone());
        }
        self.apply_marks()?;
        serialize(&self.target.document)
    }

    /// B/I only split existing native text nodes; block owners and exact English never change.
    fn apply_marks(&self) -> Result<(), Error> {
        for (target, block) in self
            .target
            .blocks
            .iter()
            .zip(&self.snapshot.document.blocks)
        {
            let node = &target.node;
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
        Ok(())
    }

    /// Rewrite only the separate source tree, leaving English anchors and saved evidence untouched.
    fn source_preview(&self) -> Result<String, Error> {
        let document = &self.source.document;
        let baseline = &self.target.document;
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
        let existing = identifiers(baseline);
        let source_ids = identifiers(document);
        // Native generation tokens make collisions unlikely, but authored ids must never defeat isolation.
        let base = format!(
            "papercut-review-source-{}-",
            self.snapshot.document.expected.generation
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
        serialize(document)
    }
}

/// Strict ancestors style the isolated preview; owned/descendant marks already belong to saved geometry.
fn inherited_emphasis(node: &NodeRef) -> Vec<MarkKind> {
    let mut bold = false;
    let mut italic = false;
    for ancestor in node.ancestors() {
        if let Some(element) = ancestor.as_element() {
            match element.name.local.as_ref() {
                "b" | "strong" => bold = true,
                "i" | "em" => italic = true,
                _ => {}
            }
        }
    }
    [(bold, MarkKind::Bold), (italic, MarkKind::Italic)]
        .into_iter()
        .filter_map(|(present, kind)| present.then_some(kind))
        .collect()
}

/// The retained pair is renderer-owned; arbitrary preceding siblings cannot stand in for native evidence.
fn retained_original(target: &NodeRef) -> Result<(NodeRef, NodeRef), Error> {
    let label = target.previous_sibling().ok_or(Error::Evidence)?;
    let original = label.previous_sibling().ok_or(Error::Evidence)?;
    if review_marker(&label).as_deref() != Some("label")
        || review_marker(&original).as_deref() != Some("original")
    {
        return Err(Error::Evidence);
    }
    Ok((label, original))
}

/// The editor can resolve emphasis only; source-only structure remains a blocker even for old drafts.
fn source_only_blocker(original: &NodeRef) -> Option<&'static str> {
    let literal = |node: &NodeRef| {
        node.as_element().is_some_and(|element| {
            matches!(element.name.local.as_ref(), "pre" | "code" | "kbd" | "samp")
                || element
                    .attributes
                    .borrow()
                    .get("translate")
                    .is_some_and(|value| value.eq_ignore_ascii_case("no"))
        })
    };
    if original.as_element().is_some_and(|element| {
        element.name.local.as_ref() == "pre"
            || element
                .attributes
                .borrow()
                .get("translate")
                .is_some_and(|value| value.eq_ignore_ascii_case("no"))
    }) {
        return Some("sourceOnlyLiteral");
    }
    for node in original.descendants() {
        if node
            .inclusive_ancestors()
            .any(|ancestor| review_marker(&ancestor).as_deref() == Some("label"))
        {
            continue;
        }
        if literal(&node) {
            return Some("sourceOnlyLiteral");
        }
        if let Some(element) = node.as_element() {
            let attributes = element.attributes.borrow();
            if !matches!(
                element.name.local.as_ref(),
                "b" | "strong" | "i" | "em" | "span"
            ) || attributes.get("id").is_some()
                || attributes.get("name").is_some()
                || attributes.get("role").is_some()
                || attributes.get("epub:type").is_some()
            {
                return Some("sourceOnlyStructure");
            }
        }
    }
    None
}

fn review_marker(node: &NodeRef) -> Option<String> {
    node.as_element().and_then(|element| {
        element
            .attributes
            .borrow()
            .get("data-papercut-translation-review")
            .map(str::to_owned)
    })
}

/// Keep identity multiplicity: losing an anchor or introducing duplicate authored ids fails closed.
fn document_identifiers(document: &NodeRef) -> BTreeMap<(String, String), usize> {
    let mut identifiers = BTreeMap::new();
    for node in document.inclusive_descendants() {
        if let Some(element) = node.as_element() {
            for name in ["id", "name"] {
                if let Some(value) = element.attributes.borrow().get(name) {
                    *identifiers
                        .entry((name.to_owned(), value.to_owned()))
                        .or_default() += 1;
                }
            }
        }
    }
    identifiers
}

#[derive(Debug, PartialEq, Eq)]
enum ConsumerPart {
    Open(String, Vec<(String, String)>),
    Close,
    Text(String),
}

/// A stricter input than TTS chunking: exact text and all non-emphasis DOM boundaries stay identical.
fn consumer_signature(document: &NodeRef) -> Vec<ConsumerPart> {
    fn collect(node: &NodeRef, signature: &mut Vec<ConsumerPart>) {
        if review_marker(node).is_some() {
            return;
        }
        if let Some(text) = node.as_text() {
            if let Some(ConsumerPart::Text(previous)) = signature.last_mut() {
                previous.push_str(&text.borrow());
            } else {
                signature.push(ConsumerPart::Text(text.borrow().to_string()));
            }
            return;
        }
        let element = node.as_element();
        let retained = element.is_some_and(|element| {
            !matches!(element.name.local.as_ref(), "b" | "i" | "strong" | "em")
        });
        if retained {
            let element = element.unwrap();
            let attributes = element
                .attributes
                .borrow()
                .map
                .iter()
                .filter(|(name, _)| {
                    !matches!(name.local.as_ref(), "id" | "name" | "data-papercut-section")
                })
                .map(|(name, value)| (name.local.to_string(), value.value.clone()))
                .collect();
            signature.push(ConsumerPart::Open(
                element.name.local.to_string(),
                attributes,
            ));
        }
        for child in node.children() {
            collect(&child, signature);
        }
        if retained {
            signature.push(ConsumerPart::Close);
        }
    }
    let mut signature = Vec::new();
    collect(document, &mut signature);
    signature
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

/// Each sibling list is enumerated once; persisted paths keep their existing all-child indices.
fn block_paths(plan: &DomPlan) -> Result<(Vec<Vec<usize>>, usize), Error> {
    // Pointers identify nodes only inside this parsed tree, never in saved evidence or IPC.
    let blocks: HashMap<_, _> = plan
        .blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (std::rc::Rc::as_ptr(&block.node.0) as usize, index))
        .collect();
    let mut paths = vec![None; plan.blocks.len()];
    let mut visited = 0usize;
    collect_paths(
        &plan.document,
        &mut Vec::new(),
        &blocks,
        &mut paths,
        &mut visited,
    )?;
    let paths = paths
        .into_iter()
        .map(|path| path.ok_or(Error::Evidence))
        .collect::<Result<_, _>>()?;
    Ok((paths, visited))
}

/// Carry child indices down the tree instead of searching each node's preceding siblings.
fn collect_paths(
    node: &NodeRef,
    path: &mut Vec<usize>,
    blocks: &HashMap<usize, usize>,
    paths: &mut [Option<Vec<usize>>],
    visited: &mut usize,
) -> Result<(), Error> {
    *visited += 1;
    #[cfg(test)]
    PATH_VISITS.with(|visits| visits.set(visits.get() + 1));
    if *visited > MAX_NODES || path.len() > MAX_DEPTH {
        return Err(Error::Limit);
    }
    if let Some(index) = blocks.get(&(std::rc::Rc::as_ptr(&node.0) as usize)) {
        paths[*index] = Some(path.clone());
    }
    for (index, child) in node.children().enumerate() {
        path.push(index);
        collect_paths(&child, path, blocks, paths, visited)?;
        path.pop();
    }
    Ok(())
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

#[cfg(test)]
mod traversal_tests;

#[cfg(test)]
mod publication_tests;

#[cfg(test)]
mod editor_tests;
