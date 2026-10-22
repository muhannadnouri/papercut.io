//! Import-only source font normalization; sanitization remains the caller's security boundary.

use std::{collections::HashMap, rc::Rc};

use cssparser::{Parser, ParserInput};
use kuchikiki::{iter::NodeEdge, parse_html, traits::TendrilSink, NodeRef, Specificity};

use super::emphasis_policy::{self as policy, Budget, Declaration, FontState, Property, Work};
use super::emphasis_sheets::{self as sheets, Sheet};

/// The two importers have different matching rules even when both use the same safe HTML DOM.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceKind {
    Html,
    Xhtml,
}

pub(crate) struct Normalized {
    pub(crate) html: String,
    /// Stable bounded reason for persisted import metadata, never raw CSS or document text.
    pub(crate) warning: Option<&'static str>,
}

/// One instance owns the budget for an entire HTML or multi-chapter EPUB import.
#[derive(Default)]
pub(crate) struct Normalizer {
    budget: Budget,
    local_sheets: HashMap<String, Result<Rc<Sheet>, &'static str>>,
}

#[derive(Clone, Copy, Default)]
struct Frame {
    effective: FontState,
    authored: FontState,
    inert: bool,
}

impl Normalizer {
    /// HTML reads embedded CSS only; linked files never escape to the host filesystem/network.
    pub(crate) fn normalize(&mut self, html: &str) -> Normalized {
        self.normalize_with(
            html,
            SourceKind::Html,
            |_| Err("unread-stylesheet"),
            |_, _| Err("unread-stylesheet"),
        )
    }

    /// The EPUB importer supplies archive-only URL admission and bounded bytes, never DOM mutation.
    pub(crate) fn normalize_with(
        &mut self,
        html: &str,
        kind: SourceKind,
        mut resolve: impl FnMut(&str) -> Result<String, &'static str>,
        mut read: impl FnMut(&str, usize) -> Result<Vec<u8>, &'static str>,
    ) -> Normalized {
        let document = parse_html().one(html).document_node;
        // Compare original XML with the reader DOM before trusting folded names or repaired trees.
        let source_warning = if kind == SourceKind::Xhtml {
            self.xhtml_context(html, &document).err()
        } else {
            None
        };
        if let Some(warning) =
            source_warning.filter(|warning| *warning != "unsupported-document-mode")
        {
            return Normalized {
                html: html.to_owned(),
                warning: Some(warning),
            };
        }
        let legacy_html = kind == SourceKind::Html
            && document.as_document().expect("Document node").quirks_mode()
                != kuchikiki::ParseOpts::default().tree_builder.quirks_mode;
        let planned = self
            .collect_sheets(&document, &mut resolve, &mut read)
            .and_then(|sheets| {
                for rule in sheets.iter().flat_map(|sheet| &sheet.rules) {
                    if rule.selected.iter().any(Option::is_some)
                        && (legacy_html
                            || source_warning.is_some()
                            || (kind == SourceKind::Xhtml && rule.html_type_folding))
                    {
                        return Err("unsupported-document-mode");
                    }
                }
                self.plan(&document, &sheets, legacy_html, source_warning)
            });
        let wrappers = match planned {
            Ok(wrappers) => wrappers,
            Err(warning) => {
                return Normalized {
                    html: html.to_owned(),
                    warning: Some(warning),
                };
            }
        };
        if wrappers.is_empty() {
            return Normalized {
                html: html.to_owned(),
                warning: None,
            };
        }
        let template = document
            .select_first("html")
            .expect("HTML parser supplies a root");
        for (node, font) in wrappers {
            let mut parent = node.clone();
            for (enabled, tag) in [(font.bold, "b"), (font.italic, "i")] {
                if enabled {
                    let mut name = template.name.clone();
                    name.local = tag.into();
                    let wrapper = NodeRef::new_element(name, []);
                    parent.insert_before(wrapper.clone());
                    parent.detach();
                    wrapper.append(parent);
                    parent = wrapper;
                }
            }
        }
        Normalized {
            html: document.to_string(),
            warning: None,
        }
    }

    /// Admit only XML names/namespaces unchanged by the shared HTML parser; never expand entities.
    fn xhtml_context(&mut self, html: &str, parsed: &NodeRef) -> Result<(), &'static str> {
        if html.contains("<!ENTITY") {
            return Err("unsupported-document-mode");
        }
        let document = roxmltree::Document::parse_with_options(
            html,
            roxmltree::ParsingOptions {
                allow_dtd: true,
                ..Default::default()
            },
        )
        .map_err(|_| "unsupported-document-mode")?;
        let namespace = document.root_element().tag_name().namespace();
        if !matches!(namespace, None | Some("http://www.w3.org/1999/xhtml")) {
            return Err("unsupported-document-mode");
        }
        // Match ordered elements/text and depth: HTML repair must not invent matching containers,
        // reparent styled text, or interpret XML character data using different raw-text rules.
        let mut parsed_nodes = parsed
            .traverse_inclusive()
            .scan(0usize, |depth, edge| {
                let (node, start) = match edge {
                    NodeEdge::Start(node) => (node, true),
                    NodeEdge::End(node) => (node, false),
                };
                let parent = *depth;
                if node.as_element().is_some() || node.as_document().is_some() {
                    if start {
                        *depth += 1;
                    } else {
                        *depth -= 1;
                    }
                }
                Some(
                    (start && (node.as_element().is_some() || node.as_text().is_some()))
                        .then_some((node, parent)),
                )
            })
            .flatten();
        // Preorder plus parent links needs only a depth-bounded stack, never a wide child queue.
        let mut ancestors = Vec::new();
        for node in document.descendants() {
            while ancestors
                .last()
                .is_some_and(|parent| Some(*parent) != node.parent())
            {
                ancestors.pop();
            }
            if ancestors.len() > policy::MAX_DOM_DEPTH {
                return Err("dom-depth-limit");
            }
            self.charge(Work {
                node_work: 1 + node.attributes().len(),
                ..Work::default()
            })?;
            let parsed_node = if node.is_element() || node.is_text() {
                let (parsed_node, depth) =
                    parsed_nodes.next().ok_or("unsupported-document-mode")?;
                if depth != ancestors.len() {
                    return Err("unsupported-document-mode");
                }
                Some(parsed_node)
            } else {
                None
            };
            if node.is_element() {
                // Extracted SVG is already an img; surviving foreign XML cannot use HTML matching.
                let name = node.tag_name().name();
                let lexical_name = html[node.range().start + 1..]
                    .split(|c: char| c.is_ascii_whitespace() || matches!(c, '/' | '>'))
                    .next();
                if node.tag_name().namespace() != namespace
                    || name.bytes().any(|byte| byte.is_ascii_uppercase())
                    || lexical_name != Some(name)
                {
                    return Err("unsupported-document-mode");
                }
                let element = parsed_node
                    .as_ref()
                    .and_then(|node| node.as_element())
                    .ok_or("unsupported-document-mode")?;
                if element.name.ns.as_ref() != "http://www.w3.org/1999/xhtml"
                    || element.name.local.as_ref() != name
                {
                    return Err("unsupported-document-mode");
                }
                let attributes = element.attributes.borrow();
                for attribute in node.attributes() {
                    if attribute
                        .name()
                        .bytes()
                        .any(|byte| byte.is_ascii_uppercase())
                        || attributes.get(&html[attribute.range_qname()]) != Some(attribute.value())
                        || match attribute.namespace() {
                            None => false,
                            Some("http://www.w3.org/XML/1998/namespace") => {
                                !matches!(attribute.name(), "lang" | "space")
                            }
                            Some("http://www.idpf.org/2007/ops") => attribute.name() != "type",
                            _ => true,
                        }
                    {
                        return Err("unsupported-document-mode");
                    }
                }
            } else if node.is_text()
                && parsed_node
                    .as_ref()
                    .and_then(|node| node.as_text())
                    .is_none_or(|text| text.borrow().as_str() != node.text().unwrap_or_default())
            {
                return Err("unsupported-document-mode");
            }
            ancestors.push(node);
        }
        if parsed_nodes.next().is_some() {
            return Err("unsupported-document-mode");
        }
        Ok(())
    }

    /// Carry computed properties, not declaration importance, down an iterative DOM traversal.
    fn plan(
        &mut self,
        document: &NodeRef,
        sheets: &[Rc<Sheet>],
        legacy_html: bool,
        source_warning: Option<&'static str>,
    ) -> Result<Vec<(NodeRef, FontState)>, &'static str> {
        let mut stack = vec![Frame::default()];
        let mut wrappers = Vec::new();
        let mut literal_roundtrip = false;
        let mut table = false;
        let mut inline_fonts = false;
        for edge in document.traverse() {
            match edge {
                NodeEdge::Start(node) => {
                    // Kuchikiki serialization is recursive; do not add an unbounded serialization path.
                    if stack.len() > policy::MAX_DOM_DEPTH {
                        return Err("dom-depth-limit");
                    }
                    self.charge(Work {
                        node_work: 1,
                        ..Work::default()
                    })?;
                    // Another HTML roundtrip can consume an untouched literal's leading LF.
                    // Fall back instead of changing literal bytes to compensate for serialization.
                    if node
                        .as_text()
                        .is_some_and(|text| text.borrow().starts_with('\n'))
                    {
                        literal_roundtrip |= node.parent().is_some_and(|parent| {
                            parent.as_element().is_some_and(|element| {
                                element.name.ns.as_ref() == "http://www.w3.org/1999/xhtml"
                                    && matches!(
                                        element.name.local.as_ref(),
                                        "pre" | "textarea" | "listing"
                                    )
                            }) && parent.first_child().is_some_and(|first| first == node)
                        });
                    }
                    let mut state = *stack.last().expect("Traversal has a root frame");
                    if let Some(element) = node.as_element() {
                        let tag = element.name.local.as_ref();
                        let attrs = element.attributes.borrow();
                        // Plaintext has no closing tag: serialization would invent readable text.
                        literal_roundtrip |= tag == "plaintext"
                            && element.name.ns.as_ref() == "http://www.w3.org/1999/xhtml";
                        state.inert |= element.name.ns.as_ref() != "http://www.w3.org/1999/xhtml"
                            || matches!(
                                tag,
                                "head"
                                    | "script"
                                    | "style"
                                    | "title"
                                    | "textarea"
                                    | "xmp"
                                    | "iframe"
                                    | "noembed"
                                    | "noframes"
                                    | "plaintext"
                                    | "template"
                                    | "noscript"
                            );
                        // Retained defaults survive sanitization and cannot be reset safely;
                        // removed containers such as address still need generated CSS wrappers.
                        let own_bold = matches!(
                            tag,
                            "b" | "strong" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "th"
                        );
                        let own_italic = matches!(tag, "i" | "em" | "cite" | "dfn" | "var");
                        let inherited = state.effective;
                        state.effective.bold |= own_bold;
                        state.effective.italic |= own_italic;
                        state.authored.bold |= own_bold;
                        state.authored.italic |= own_italic;
                        if !state.inert {
                            table |= tag == "table";
                            // Sheet occurrences retain document order; only winners carry importance.
                            let mut selected: [Option<(Declaration, Option<Specificity>)>; 2] =
                                [None, None];
                            let element_ref =
                                node.clone().into_element_ref().expect("Element node");
                            for sheet in sheets {
                                for rule in &sheet.rules {
                                    if let Some(specificity) =
                                        rule.select(&element_ref, &mut self.budget)?
                                    {
                                        for (index, declaration) in rule.selected.iter().enumerate()
                                        {
                                            if let Some(declaration) = declaration {
                                                cascade(
                                                    &mut selected[index],
                                                    *declaration,
                                                    Some(specificity),
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                            if let Some(style) = attrs.get("style") {
                                let declarations = self.declarations(style)?;
                                if declarations.iter().any(Option::is_some) {
                                    inline_fonts = true;
                                    if let Some(warning) = source_warning {
                                        return Err(warning);
                                    }
                                }
                                for (index, declaration) in declarations.into_iter().enumerate() {
                                    if let Some(declaration) = declaration {
                                        cascade(&mut selected[index], declaration, None);
                                    }
                                }
                            }
                            for (declaration, _) in selected.into_iter().flatten() {
                                match declaration.property {
                                    Property::Weight => {
                                        state.effective.bold =
                                            declaration.value.resolve(inherited.bold)
                                    }
                                    Property::Style => {
                                        state.effective.italic =
                                            declaration.value.resolve(inherited.italic)
                                    }
                                    _ => unreachable!("Only longhands are admitted"),
                                }
                            }
                            if policy::semantic_reset(state.authored, state.effective)
                                != policy::Coverage::Complete
                            {
                                return Err("semantic-reset");
                            }
                        }
                    } else if node.as_text().is_some_and(|text| !text.borrow().is_empty())
                        && !state.inert
                    {
                        let font = FontState {
                            bold: state.effective.bold && !state.authored.bold,
                            italic: state.effective.italic && !state.authored.italic,
                        };
                        let count = usize::from(font.bold) + usize::from(font.italic);
                        if count != 0 && can_wrap(&node)? {
                            if stack.len() + count > policy::MAX_DOM_DEPTH {
                                return Err("dom-depth-limit");
                            }
                            self.charge(Work {
                                node_work: count,
                                ..Work::default()
                            })?;
                            wrappers.push((node.clone(), font));
                        }
                    }
                    stack.push(state);
                }
                NodeEdge::End(_) => {
                    stack.pop();
                }
            }
        }
        // Legacy table defaults reset inherited font styles; do not emulate them with text wrappers.
        if legacy_html && table && inline_fonts {
            return Err("unsupported-document-mode");
        }
        if literal_roundtrip && !wrappers.is_empty() {
            return Err("literal-roundtrip");
        }
        Ok(wrappers)
    }

    /// Discover sheets before resolving any text, including styles declared after the body text.
    fn collect_sheets(
        &mut self,
        document: &NodeRef,
        resolve: &mut impl FnMut(&str) -> Result<String, &'static str>,
        read: &mut impl FnMut(&str, usize) -> Result<Vec<u8>, &'static str>,
    ) -> Result<Vec<Rc<Sheet>>, &'static str> {
        let mut result = Vec::new();
        let mut stack = vec![(false, false)];
        for edge in document.traverse() {
            match edge {
                NodeEdge::Start(node) => {
                    if stack.len() > policy::MAX_DOM_DEPTH {
                        return Err("dom-depth-limit");
                    }
                    self.charge(Work {
                        node_work: 1,
                        ..Work::default()
                    })?;
                    let (mut inert, mut foreign) =
                        *stack.last().expect("Traversal has a root frame");
                    if let Some(element) = node.as_element() {
                        let tag = element.name.local.as_ref();
                        foreign |= element.name.ns.as_ref() != "http://www.w3.org/1999/xhtml";
                        inert |= element.name.ns.as_ref() == "http://www.w3.org/1999/xhtml"
                            && matches!(tag, "template" | "noscript" | "script");
                        let attrs = element.attributes.borrow();
                        // SVG styles can affect sibling HTML; unknown foreign sheets are not inert CSS.
                        if !inert
                            && foreign
                            && (tag == "style"
                                || (tag == "link"
                                    && attrs.get("rel").is_some_and(|rel| {
                                        rel.split_ascii_whitespace()
                                            .any(|v| v.eq_ignore_ascii_case("stylesheet"))
                                    })))
                        {
                            return Err("unsupported-stylesheet");
                        }
                        if !inert && !foreign {
                            // Browser base selection/inherited xml:base are outside this archive-only URL model.
                            if (tag == "base" && attrs.get("href").is_some())
                                || attrs.get("xml:base").is_some()
                                || attrs.map.iter().any(|(name, _)| {
                                    name.local.as_ref() == "base"
                                        && name.ns.as_ref()
                                            == "http://www.w3.org/XML/1998/namespace"
                                })
                            {
                                return Err("unsupported-base");
                            }
                            let rel = attrs.get("rel").unwrap_or("");
                            let linked = tag == "link"
                                && rel
                                    .split_ascii_whitespace()
                                    .any(|v| v.eq_ignore_ascii_case("stylesheet"));
                            if tag == "style" || linked {
                                if attrs.get("disabled").is_some()
                                    || attrs.get("scoped").is_some()
                                    || attrs.get("title").is_some_and(|v| !v.is_empty())
                                    || rel
                                        .split_ascii_whitespace()
                                        .any(|v| v.eq_ignore_ascii_case("alternate"))
                                    || attrs.get("media").is_some_and(|v| {
                                        !v.trim_ascii().is_empty()
                                            && !v.trim_ascii().eq_ignore_ascii_case("all")
                                    })
                                    || attrs.get("type").is_some_and(|v| {
                                        !v.trim_ascii().is_empty()
                                            && !v.trim_ascii().eq_ignore_ascii_case("text/css")
                                    })
                                    || (linked
                                        && attrs.get("charset").is_some_and(|v| {
                                            !v.is_empty()
                                                && !v.eq_ignore_ascii_case("utf-8")
                                                && !v.eq_ignore_ascii_case("utf8")
                                        }))
                                {
                                    return Err("unsupported-activation");
                                }
                                let sheet = if linked {
                                    let key =
                                        resolve(attrs.get("href").ok_or("unread-stylesheet")?)?;
                                    self.local_sheet(&key, read)?
                                } else {
                                    // Charge the text length before allocating a combined sheet string.
                                    let bytes = node
                                        .children()
                                        .filter_map(|n| n.as_text().map(|t| t.borrow().len()))
                                        .sum();
                                    self.budget.sheet(bytes).map_err(sheets::reason)?;
                                    Rc::new(sheets::parse_charged(
                                        &node.text_contents(),
                                        &mut self.budget,
                                    )?)
                                };
                                // Repeated includes do not reparse cached sheets, but do cost traversal and matching.
                                self.charge(Work {
                                    node_work: 1,
                                    ..Work::default()
                                })?;
                                result.push(sheet);
                            }
                        }
                    }
                    stack.push((inert, foreign));
                }
                NodeEdge::End(_) => {
                    stack.pop();
                }
            }
        }
        Ok(result)
    }

    /// Count failed attempts too, and cache their warning so later chapters cannot repeat ZIP work.
    fn local_sheet(
        &mut self,
        key: &str,
        read: &mut impl FnMut(&str, usize) -> Result<Vec<u8>, &'static str>,
    ) -> Result<Rc<Sheet>, &'static str> {
        if let Some(sheet) = self.local_sheets.get(key) {
            return sheet.clone();
        }
        self.budget.sheet(0).map_err(sheets::reason)?;
        let limit = self.budget.sheet_byte_allowance();
        let result = (|| {
            let bytes = read(key, limit)?;
            // Defense in depth for the importer callback: reject before decoding or parsing.
            if bytes.len() > limit {
                return Err(if limit == policy::MAX_SHEET_BYTES {
                    "sheet-byte-limit"
                } else {
                    "css-byte-limit"
                });
            }
            self.charge(Work {
                css_bytes: bytes.len(),
                ..Work::default()
            })?;
            let css = std::str::from_utf8(&bytes).map_err(|_| "unread-stylesheet")?;
            // An external UTF-8 BOM is an encoding signature, not part of the first selector.
            let css = css.strip_prefix('\u{feff}').unwrap_or(css);
            sheets::parse_charged(css, &mut self.budget).map(Rc::new)
        })();
        self.local_sheets.insert(key.into(), result.clone());
        result
    }

    /// Inline bytes use the same declaration tokenizer and importance policy as local sheets.
    fn declarations(&mut self, style: &str) -> Result<[Option<Declaration>; 2], &'static str> {
        self.charge(Work {
            css_bytes: style.len(),
            ..Work::default()
        })?;
        let mut input = ParserInput::new(style);
        sheets::declarations(&mut Parser::new(&mut input), &mut self.budget)
    }

    fn charge(&mut self, work: Work) -> Result<(), &'static str> {
        sheets::charge(&mut self.budget, work)
    }
}

/// Importance outranks specificity; inline wins ties at either importance, then later source wins.
fn cascade(
    selected: &mut Option<(Declaration, Option<Specificity>)>,
    declaration: Declaration,
    specificity: Option<Specificity>,
) {
    let priority = |d: Declaration, s: Option<Specificity>| (d.important, s.is_none(), s);
    if selected
        .is_none_or(|(previous, s)| priority(declaration, specificity) >= priority(previous, s))
    {
        *selected = Some((declaration, specificity));
    }
}

/// Keep wrappers within HTML content models: table space must stay in place,
/// and select/option formatting would be discarded by the following parser.
fn can_wrap(node: &NodeRef) -> Result<bool, &'static str> {
    let Some(parent) = node.parent() else {
        return Ok(true);
    };
    let Some(element) = parent
        .as_element()
        .filter(|element| element.name.ns.as_ref() == "http://www.w3.org/1999/xhtml")
    else {
        return Ok(true);
    };
    let tag = element.name.local.as_ref();
    if matches!(tag, "select" | "option" | "optgroup") {
        return Err("unsupported-context");
    }
    let table_space = matches!(
        tag,
        "table" | "tbody" | "thead" | "tfoot" | "tr" | "colgroup"
    ) && node.as_text().is_some_and(|text| {
        text.borrow()
            .bytes()
            .all(|byte| matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0c))
    });
    Ok(!table_space)
}

#[cfg(test)]
#[path = "emphasis_tests.rs"]
mod tests;
