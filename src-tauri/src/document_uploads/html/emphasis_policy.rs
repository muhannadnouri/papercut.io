//! Pure admission policy shared by import-time normalization and future local-sheet support.
//! Parsing, DOM changes and importer calls live outside this policy module.

pub(crate) const POLICY: &str = "source-emphasis-v1";
pub(super) const MAX_SHEET_BYTES: usize = 64 * 1024;
pub(super) const MAX_CSS_BYTES: usize = 256 * 1024;
pub(super) const MAX_SHEETS: usize = 32;
pub(super) const MAX_RULES: usize = 2048;
pub(super) const MAX_DECLARATIONS: usize = 8192;
pub(super) const MAX_NODE_WORK: usize = 1_000_000;
pub(super) const MAX_DOM_DEPTH: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Property {
    Weight,
    Style,
    FontShorthand,
    AllShorthand,
}

/// Tokens come from a real CSS parser, never string splitting here.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum ParsedValue {
    Normal,
    Bold,
    Number(f64),
    Italic,
    Oblique,
    Inherit,
    Initial,
    Unset,
    RelativeWeight,
    AngledOblique,
    Variable,
    Revert,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SpecifiedValue {
    On,
    Off,
    Inherit,
}

impl SpecifiedValue {
    /// Inheritance carries the computed value, not a parent's `!important` priority.
    pub(super) fn resolve(self, inherited: bool) -> bool {
        match self {
            Self::On => true,
            Self::Off => false,
            Self::Inherit => inherited,
        }
    }
}

/// Preserve declaration importance for the later per-element cascade owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Declaration {
    pub(super) property: Property,
    pub(super) value: SpecifiedValue,
    pub(super) important: bool,
}

/// Admit the deliberately lossy B/I subset; unknown affecting values cannot certify coverage.
pub(super) fn declaration(
    property: Property,
    value: ParsedValue,
    important: bool,
) -> Result<Declaration, Unsupported> {
    use ParsedValue as V;
    let specified = match (property, value) {
        (Property::FontShorthand | Property::AllShorthand, _) => {
            return Err(Unsupported::Shorthand)
        }
        (_, V::Normal | V::Initial) => SpecifiedValue::Off,
        (_, V::Inherit | V::Unset) => SpecifiedValue::Inherit,
        (Property::Weight, V::Bold) | (Property::Style, V::Italic | V::Oblique) => {
            SpecifiedValue::On
        }
        (Property::Weight, V::Number(weight))
            if weight.is_finite() && (100.0..=900.0).contains(&weight) =>
        {
            if weight >= 600.0 {
                SpecifiedValue::On
            } else {
                SpecifiedValue::Off
            }
        }
        _ => return Err(Unsupported::Value),
    };
    Ok(Declaration {
        property,
        value: specified,
        important,
    })
}

/// Identifier spelling/escapes belong to the CSS parser; this admits only token kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SelectorToken {
    Type,
    Class,
    Id,
    Combinator,
    Pseudo,
    Attribute,
    Universal,
    Namespace,
}

/// A selector list contains nonempty compounds: optional leading type, then classes/IDs.
pub(super) fn selectors(list: &[&[SelectorToken]]) -> Result<(), Unsupported> {
    if list.is_empty()
        || list.iter().any(|compound| {
            compound.is_empty()
                || compound.iter().enumerate().any(|(index, token)| {
                    !matches!(token, SelectorToken::Class | SelectorToken::Id)
                        && !(index == 0 && *token == SelectorToken::Type)
                })
        })
    {
        return Err(Unsupported::Selector);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mode {
    InlineOnly,
    LocalSheets,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Activation {
    Unconditional,
    ConditionalMedia,
    Disabled,
    Alternate,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Availability {
    Interpreted,
    Unread,
    Missing,
    Disallowed,
    OverBudget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Unsupported {
    Value,
    Shorthand,
    Selector,
    SheetActivation,
    UninterpretedSheet,
    SemanticReset,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Coverage {
    Complete,
    Unsupported(Unsupported),
}

impl Coverage {
    /// Inline-only mode cannot know whether an unread author rule overrides inline styles.
    pub(super) fn sheet(mode: Mode, activation: Activation, availability: Availability) -> Self {
        if activation != Activation::Unconditional {
            Self::Unsupported(Unsupported::SheetActivation)
        } else if mode == Mode::InlineOnly || availability != Availability::Interpreted {
            Self::Unsupported(Unsupported::UninterpretedSheet)
        } else {
            Self::Complete
        }
    }

    /// Keep the first blocker; later successes cannot recertify incomplete source coverage.
    pub(super) fn merge(self, next: Self) -> Self {
        match self {
            Self::Complete => next,
            Self::Unsupported(_) => self,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct FontState {
    pub(super) bold: bool,
    pub(super) italic: bool,
}

/// V1 keeps authored semantic emphasis instead of stripping it to emulate a CSS reset.
pub(super) fn semantic_reset(authored: FontState, effective: FontState) -> Coverage {
    if (authored.bold && !effective.bold) || (authored.italic && !effective.italic) {
        Coverage::Unsupported(Unsupported::SemanticReset)
    } else {
        Coverage::Complete
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Limit {
    SheetBytes,
    CssBytes,
    Sheets,
    Rules,
    Declarations,
    NodeWork,
}

/// Counts one entire HTML/EPUB import, including all chapters; never reset per chapter.
/// CSS bytes include inline declarations, and cached sheets need charging only once.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Work {
    pub(super) css_bytes: usize,
    pub(super) sheets: usize,
    pub(super) rules: usize,
    pub(super) declarations: usize,
    /// Charge each visited node and each attempted selector-node match before doing it.
    pub(super) node_work: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Budget {
    pub(super) used: Work,
}

impl Budget {
    /// A ZIP reader may inflate only bytes which can still be admitted by this import.
    pub(super) fn sheet_byte_allowance(&self) -> usize {
        MAX_SHEET_BYTES.min(MAX_CSS_BYTES - self.used.css_bytes)
    }

    /// Precharge a complete sheet atomically before decoding/parsing its bounded bytes.
    pub(super) fn sheet(&mut self, bytes: usize) -> Result<(), Limit> {
        if bytes > MAX_SHEET_BYTES {
            return Err(Limit::SheetBytes);
        }
        self.charge(Work {
            css_bytes: bytes,
            sheets: 1,
            ..Work::default()
        })
    }

    /// Check every counter before committing any: overflow/failure never spends a partial charge.
    pub(super) fn charge(&mut self, work: Work) -> Result<(), Limit> {
        let next = Work {
            css_bytes: checked(
                self.used.css_bytes,
                work.css_bytes,
                MAX_CSS_BYTES,
                Limit::CssBytes,
            )?,
            sheets: checked(self.used.sheets, work.sheets, MAX_SHEETS, Limit::Sheets)?,
            rules: checked(self.used.rules, work.rules, MAX_RULES, Limit::Rules)?,
            declarations: checked(
                self.used.declarations,
                work.declarations,
                MAX_DECLARATIONS,
                Limit::Declarations,
            )?,
            node_work: checked(
                self.used.node_work,
                work.node_work,
                MAX_NODE_WORK,
                Limit::NodeWork,
            )?,
        };
        self.used = next;
        Ok(())
    }
}

/// Checked arithmetic and the semantic cap are the same admission decision.
fn checked(used: usize, added: usize, maximum: usize, limit: Limit) -> Result<usize, Limit> {
    used.checked_add(added)
        .filter(|next| *next <= maximum)
        .ok_or(limit)
}

#[cfg(test)]
#[path = "emphasis_policy_tests.rs"]
mod tests;
