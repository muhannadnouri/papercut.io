//! Import-only inline font normalization; sanitization remains the caller's security boundary.

use cssparser::{
    parse_important, AtRuleParser, CowRcStr, DeclarationListParser, DeclarationParser, ParseError,
    Parser, ParserInput, Token,
};
use kuchikiki::{iter::NodeEdge, parse_html, traits::TendrilSink, NodeRef};

use super::emphasis_policy::{
    self as policy, Budget, Declaration, FontState, ParsedValue, Property, Work,
};

pub(crate) struct Normalized {
    pub(crate) html: String,
    /// Stable bounded reason for persisted import metadata, never raw CSS or document text.
    pub(crate) warning: Option<&'static str>,
}

/// One instance owns the budget for an entire HTML or multi-chapter EPUB import.
#[derive(Default)]
pub(crate) struct Normalizer {
    budget: Budget,
}

#[derive(Clone, Copy, Default)]
struct Frame {
    effective: FontState,
    authored: FontState,
    inert: bool,
}

impl Normalizer {
    /// Plan all text wrappers first: unsupported CSS or exhaustion cannot leave a half-normalized chapter.
    pub(crate) fn normalize(&mut self, html: &str) -> Normalized {
        let document = parse_html().one(html).document_node;
        let planned = self.plan(&document);
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

    /// Carry computed properties, not declaration importance, down an iterative DOM traversal.
    fn plan(&mut self, document: &NodeRef) -> Result<Vec<(NodeRef, FontState)>, &'static str> {
        let mut stack = vec![Frame::default()];
        let mut wrappers = Vec::new();
        let mut literal_roundtrip = false;
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
                        if tag == "style"
                            || (tag == "link"
                                && attrs.get("rel").is_some_and(|rel| {
                                    rel.split_ascii_whitespace()
                                        .any(|part| part.eq_ignore_ascii_case("stylesheet"))
                                }))
                        {
                            return Err("unread-stylesheet");
                        }
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
                            if let Some(style) = attrs.get("style") {
                                let declarations = self.declarations(style)?;
                                for declaration in declarations.into_iter().flatten() {
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
        if literal_roundtrip && !wrappers.is_empty() {
            return Err("literal-roundtrip");
        }
        Ok(wrappers)
    }

    /// A real CSS tokenizer resolves comments/escapes before the deliberately small value policy.
    fn declarations(&mut self, style: &str) -> Result<[Option<Declaration>; 2], &'static str> {
        self.charge(Work {
            css_bytes: style.len(),
            ..Work::default()
        })?;
        let mut input = ParserInput::new(style);
        let mut parser = Parser::new(&mut input);
        let mut selected: [Option<Declaration>; 2] = [None, None];
        let mut declarations = DeclarationListParser::new(&mut parser, InlineDeclarations);
        while !declarations.input.is_exhausted() {
            self.charge(Work {
                declarations: 1,
                ..Work::default()
            })?;
            let Some(parsed) = declarations.next() else {
                break;
            };
            if let Some(declaration) = parsed.map_err(|_| "unsupported-value")?? {
                let index = usize::from(declaration.property == Property::Style);
                if selected[index]
                    .is_none_or(|previous| declaration.important || !previous.important)
                {
                    selected[index] = Some(declaration);
                }
            }
        }
        Ok(selected)
    }

    fn charge(&mut self, work: Work) -> Result<(), &'static str> {
        self.budget.charge(work).map_err(|limit| match limit {
            policy::Limit::CssBytes => "css-byte-limit",
            policy::Limit::Declarations => "declaration-limit",
            policy::Limit::NodeWork => "node-work-limit",
            _ => "work-limit",
        })
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

struct InlineDeclarations;

impl<'i> DeclarationParser<'i> for InlineDeclarations {
    type Declaration = Result<Option<Declaration>, &'static str>;
    type Error = ();

    fn parse_value<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Declaration, ParseError<'i, Self::Error>> {
        let property = if name.eq_ignore_ascii_case("font-weight") {
            Property::Weight
        } else if name.eq_ignore_ascii_case("font-style") {
            Property::Style
        } else if name.eq_ignore_ascii_case("font") || name.eq_ignore_ascii_case("all") {
            while input.next().is_ok() {}
            return Ok(Err("unsupported-shorthand"));
        } else {
            // Unrelated properties are inert here and still pass through the unchanged sanitizer.
            while input.next().is_ok() {}
            return Ok(Ok(None));
        };
        let value = match input.next()?.clone() {
            Token::Ident(value) => match value.to_ascii_lowercase().as_str() {
                "normal" => ParsedValue::Normal,
                "bold" => ParsedValue::Bold,
                "italic" => ParsedValue::Italic,
                "oblique" => ParsedValue::Oblique,
                "inherit" => ParsedValue::Inherit,
                "initial" => ParsedValue::Initial,
                "unset" => ParsedValue::Unset,
                _ => ParsedValue::Other,
            },
            Token::Number { value, .. } => ParsedValue::Number(f64::from(value)),
            _ => ParsedValue::Other,
        };
        let important = input.try_parse(parse_important).is_ok();
        input.expect_exhausted()?;
        Ok(policy::declaration(property, value, important)
            .map(Some)
            .map_err(|_| "unsupported-value"))
    }
}

impl<'i> AtRuleParser<'i> for InlineDeclarations {
    type Prelude = ();
    type AtRule = Result<Option<Declaration>, &'static str>;
    type Error = ();
}

#[cfg(test)]
#[path = "emphasis_tests.rs"]
mod tests;
