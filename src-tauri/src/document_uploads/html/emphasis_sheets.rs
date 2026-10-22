//! Bounded, non-rendering stylesheet admission shared with inline declarations.

use cssparser::{
    parse_important, AtRuleParser, BasicParseErrorKind, CowRcStr, DeclarationListParser,
    DeclarationParser, ParseError, ParseErrorKind, Parser, ParserInput, ParserState,
    QualifiedRuleParser, RuleListParser, Token,
};
use kuchikiki::{ElementData, NodeDataRef, Selectors, Specificity};

use super::emphasis_policy::{self as policy, Budget, Declaration, ParsedValue, Property, Work};

pub(super) struct Sheet {
    pub(super) rules: Vec<Rule>,
}

pub(super) struct Rule {
    selectors: Selectors,
    components: Vec<usize>,
    pub(super) selected: [Option<Declaration>; 2],
    pub(super) html_type_folding: bool,
}

impl Rule {
    /// Charge compound complexity before matching; a long selector cannot multiply the work cap.
    pub(super) fn select(
        &self,
        element: &NodeDataRef<ElementData>,
        budget: &mut Budget,
    ) -> Result<Option<Specificity>, &'static str> {
        let mut specificity = None;
        // Kuchikiki rescans names/classes per selector; 64-byte units bound hostile string work.
        let attributes = element.attributes.borrow();
        let attribute_bytes = element.name.local.len()
            + attributes.get("class").map_or(0, str::len)
            + attributes.get("id").map_or(0, str::len);
        let attribute_work = 1 + attribute_bytes / 64;
        for (selector, components) in self.selectors.0.iter().zip(&self.components) {
            charge(
                budget,
                Work {
                    node_work: components.saturating_mul(attribute_work),
                    ..Work::default()
                },
            )?;
            if selector.matches(element) {
                specificity = Some(specificity.map_or(selector.specificity(), |previous| {
                    std::cmp::max(previous, selector.specificity())
                }));
            }
        }
        Ok(specificity)
    }
}

/// Admit a complete bounded sheet before retaining any rules; unsupported input fails atomically.
#[cfg(test)]
pub(super) fn parse(css: &str, budget: &mut Budget) -> Result<Sheet, &'static str> {
    budget.sheet(css.len()).map_err(reason)?;
    parse_charged(css, budget)
}

/// EPUB charges bounded archive bytes before decoding; do not count the same sheet twice.
pub(super) fn parse_charged(css: &str, budget: &mut Budget) -> Result<Sheet, &'static str> {
    let mut input = ParserInput::new(css);
    let mut input = Parser::new(&mut input);
    // The stylesheet constructor silently skips a leading @charset; here every at-rule must reject.
    let mut parser = RuleListParser::new_for_nested_rule(&mut input, Rules { budget });
    let mut rules = Vec::new();
    while !parser.input.is_exhausted() {
        charge(
            parser.parser.budget,
            Work {
                rules: 1,
                ..Work::default()
            },
        )?;
        let Some(rule) = parser.next() else {
            break;
        };
        rules.push(rule.map_err(|(error, _)| error_reason(error, "unsupported-rule"))?);
    }
    Ok(Sheet { rules })
}

/// Keep property-level importance and last-wins order identical for sheet and inline declarations.
pub(super) fn declarations(
    input: &mut Parser<'_, '_>,
    budget: &mut Budget,
) -> Result<[Option<Declaration>; 2], &'static str> {
    let mut selected: [Option<Declaration>; 2] = [None, None];
    let mut declarations = DeclarationListParser::new(input, InlineDeclarations);
    while !declarations.input.is_exhausted() {
        charge(
            budget,
            Work {
                declarations: 1,
                ..Work::default()
            },
        )?;
        let Some(parsed) = declarations.next() else {
            break;
        };
        if let Some(declaration) =
            parsed.map_err(|(error, _)| error_reason(error, "unsupported-value"))?
        {
            let index = usize::from(declaration.property == Property::Style);
            if selected[index].is_none_or(|previous| declaration.important || !previous.important) {
                selected[index] = Some(declaration);
            }
        }
    }
    Ok(selected)
}

/// Stable codes are safe to persist without copying publisher CSS into reports.
pub(super) fn charge(budget: &mut Budget, work: Work) -> Result<(), &'static str> {
    budget.charge(work).map_err(reason)
}

pub(super) fn reason(limit: policy::Limit) -> &'static str {
    match limit {
        policy::Limit::SheetBytes => "sheet-byte-limit",
        policy::Limit::CssBytes => "css-byte-limit",
        policy::Limit::Sheets => "sheet-limit",
        policy::Limit::Rules => "rule-limit",
        policy::Limit::Declarations => "declaration-limit",
        policy::Limit::NodeWork => "node-work-limit",
    }
}

fn error_reason(error: ParseError<'_, &'static str>, fallback: &'static str) -> &'static str {
    match error.kind {
        ParseErrorKind::Custom(reason) => reason,
        _ => fallback,
    }
}

struct Rules<'a> {
    budget: &'a mut Budget,
}

impl<'i> QualifiedRuleParser<'i> for Rules<'_> {
    type Prelude = (Selectors, Vec<usize>, bool);
    type QualifiedRule = Rule;
    type Error = &'static str;

    fn parse_prelude<'t>(
        &mut self,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
        let start = input.position();
        let mut components = Vec::new();
        let mut count = 0;
        let mut gap = false;
        let mut html_type_folding = false;
        loop {
            // is_exhausted skips whitespace and would accidentally admit descendant combinators.
            let token = match input.next_including_whitespace() {
                Ok(token) => token.clone(),
                Err(error) if matches!(error.kind, BasicParseErrorKind::EndOfInput) => break,
                Err(error) => return Err(error.into()),
            };
            if matches!(token, Token::WhiteSpace(_)) {
                gap |= count != 0;
                continue;
            }
            if matches!(token, Token::Comma) {
                if count == 0 {
                    return Err(input.new_custom_error("unsupported-selector"));
                }
                components.push(count);
                count = 0;
                gap = false;
                continue;
            }
            if gap {
                return Err(input.new_custom_error("unsupported-selector"));
            }
            match token {
                Token::Ident(name) if count == 0 => {
                    html_type_folding |= name.bytes().any(|byte| byte.is_ascii_uppercase());
                }
                Token::IDHash(_) => (),
                Token::Delim('.') => {
                    // Whitespace after a dot is a combinator/syntax error, not a class identifier.
                    if !matches!(input.next_including_whitespace()?, Token::Ident(_)) {
                        return Err(input.new_custom_error("unsupported-selector"));
                    }
                }
                _ => return Err(input.new_custom_error("unsupported-selector")),
            }
            // Count components as rule work before compile/allocation, bounding both lists and compounds.
            charge(
                self.budget,
                Work {
                    rules: 1,
                    ..Work::default()
                },
            )
            .map_err(|reason| input.new_custom_error(reason))?;
            count += 1;
        }
        if count == 0 {
            return Err(input.new_custom_error("unsupported-selector"));
        }
        components.push(count);
        let selectors = Selectors::compile(input.slice_from(start))
            .map_err(|_| input.new_custom_error("unsupported-selector"))?;
        Ok((selectors, components, html_type_folding))
    }

    fn parse_block<'t>(
        &mut self,
        (selectors, components, html_type_folding): Self::Prelude,
        _: &ParserState,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::QualifiedRule, ParseError<'i, Self::Error>> {
        let selected =
            declarations(input, self.budget).map_err(|reason| input.new_custom_error(reason))?;
        Ok(Rule {
            selectors,
            components,
            selected,
            html_type_folding,
        })
    }
}

impl<'i> AtRuleParser<'i> for Rules<'_> {
    type Prelude = ();
    type AtRule = Rule;
    type Error = &'static str;

    fn parse_prelude<'t>(
        &mut self,
        _: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self::Prelude, ParseError<'i, Self::Error>> {
        Err(input.new_custom_error("unsupported-rule"))
    }
}

struct InlineDeclarations;

impl<'i> DeclarationParser<'i> for InlineDeclarations {
    type Declaration = Option<Declaration>;
    type Error = &'static str;

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
            return Err(input.new_custom_error("unsupported-shorthand"));
        } else {
            // Unrelated values are never rendered/fetched and still reach the unchanged sanitizer.
            while input.next().is_ok() {}
            return Ok(None);
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
        policy::declaration(property, value, important)
            .map(Some)
            .map_err(|_| input.new_custom_error("unsupported-value"))
    }
}

impl<'i> AtRuleParser<'i> for InlineDeclarations {
    type Prelude = ();
    type AtRule = Option<Declaration>;
    type Error = &'static str;
}

#[cfg(test)]
#[path = "emphasis_sheets_tests.rs"]
mod tests;
