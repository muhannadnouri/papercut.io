use super::*;

#[test]
fn policy_and_work_limits_are_frozen_before_parser_integration() {
    assert_eq!(POLICY, "source-emphasis-v1");
    assert_eq!(MAX_SHEET_BYTES, 65_536);
    assert_eq!(MAX_CSS_BYTES, 262_144);
    assert_eq!(MAX_SHEETS, 32);
    assert_eq!(MAX_RULES, 2048);
    assert_eq!(MAX_DECLARATIONS, 8192);
    assert_eq!(MAX_NODE_WORK, 1_000_000);
}

#[test]
fn numeric_weights_have_finite_inclusive_bounds_and_a_frozen_bold_threshold() {
    for (weight, expected) in [
        (100.0, false),
        (599.999, false),
        (600.0, true),
        (900.0, true),
    ] {
        let admitted = declaration(Property::Weight, ParsedValue::Number(weight), false).unwrap();
        assert_eq!(admitted.value.resolve(false), expected);
    }
    for weight in [99.999, 900.001, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            declaration(Property::Weight, ParsedValue::Number(weight), false),
            Err(Unsupported::Value)
        );
    }
}

#[test]
fn property_specific_values_do_not_leak_between_bold_and_italic() {
    for (property, value, expected) in [
        (Property::Weight, ParsedValue::Normal, false),
        (Property::Weight, ParsedValue::Bold, true),
        (Property::Style, ParsedValue::Normal, false),
        (Property::Style, ParsedValue::Italic, true),
        (Property::Style, ParsedValue::Oblique, true),
    ] {
        assert_eq!(
            declaration(property, value, true).unwrap(),
            Declaration {
                property,
                value: if expected {
                    SpecifiedValue::On
                } else {
                    SpecifiedValue::Off
                },
                important: true,
            }
        );
    }
    for (property, value) in [
        (Property::Weight, ParsedValue::Italic),
        (Property::Weight, ParsedValue::Oblique),
        (Property::Style, ParsedValue::Bold),
        (Property::Style, ParsedValue::Number(700.0)),
    ] {
        assert_eq!(declaration(property, value, false), Err(Unsupported::Value));
    }
}

#[test]
fn inherit_and_unset_use_parent_value_while_initial_resets_it() {
    for property in [Property::Weight, Property::Style] {
        for inherited in [false, true] {
            for value in [ParsedValue::Inherit, ParsedValue::Unset] {
                assert_eq!(
                    declaration(property, value, false)
                        .unwrap()
                        .value
                        .resolve(inherited),
                    inherited
                );
            }
            assert!(!declaration(property, ParsedValue::Initial, true)
                .unwrap()
                .value
                .resolve(inherited));
        }
    }
}

#[test]
fn child_normal_is_not_overridden_by_inherited_parent_importance() {
    let parent = declaration(Property::Weight, ParsedValue::Bold, true).unwrap();
    let child = declaration(Property::Weight, ParsedValue::Normal, false).unwrap();
    assert!(parent.important);
    assert!(!child.important);
    assert!(!child.value.resolve(parent.value.resolve(false)));
}

#[test]
fn unsupported_affecting_values_and_shorthands_never_certify_a_font_value() {
    for property in [Property::Weight, Property::Style] {
        for value in [
            ParsedValue::RelativeWeight,
            ParsedValue::AngledOblique,
            ParsedValue::Variable,
            ParsedValue::Revert,
            ParsedValue::Other,
        ] {
            assert_eq!(declaration(property, value, true), Err(Unsupported::Value));
        }
    }
    for property in [Property::FontShorthand, Property::AllShorthand] {
        assert_eq!(
            declaration(property, ParsedValue::Normal, true),
            Err(Unsupported::Shorthand)
        );
    }
}

#[test]
fn selector_lists_admit_only_nonempty_type_class_id_compounds() {
    use SelectorToken as S;
    assert_eq!(selectors(&[&[S::Type], &[S::Class], &[S::Id]]), Ok(()));
    assert_eq!(selectors(&[&[S::Type, S::Class, S::Class, S::Id]]), Ok(()));
    for list in [
        vec![],
        vec![vec![]],
        vec![vec![S::Type, S::Type]],
        vec![vec![S::Class, S::Type]],
    ] {
        let refs: Vec<_> = list.iter().map(Vec::as_slice).collect();
        assert_eq!(selectors(&refs), Err(Unsupported::Selector));
    }
    for token in [
        S::Combinator,
        S::Pseudo,
        S::Attribute,
        S::Universal,
        S::Namespace,
    ] {
        assert_eq!(selectors(&[&[S::Class, token]]), Err(Unsupported::Selector));
    }
}

#[test]
fn unread_important_sheet_keeps_inline_bold_unresolved() {
    // A future parser may encounter `.x {font-weight:normal!important}` in this unread sheet.
    let inline = declaration(Property::Weight, ParsedValue::Bold, false).unwrap();
    assert!(inline.value.resolve(false));
    assert_eq!(
        Coverage::sheet(
            Mode::InlineOnly,
            Activation::Unconditional,
            Availability::Unread
        ),
        Coverage::Unsupported(Unsupported::UninterpretedSheet)
    );
    assert_eq!(
        Coverage::sheet(
            Mode::InlineOnly,
            Activation::Unconditional,
            Availability::Interpreted
        ),
        Coverage::Unsupported(Unsupported::UninterpretedSheet)
    );
}

#[test]
fn sheet_activation_and_availability_must_both_be_known() {
    for activation in [
        Activation::ConditionalMedia,
        Activation::Disabled,
        Activation::Alternate,
        Activation::Unknown,
    ] {
        assert_eq!(
            Coverage::sheet(Mode::LocalSheets, activation, Availability::Interpreted),
            Coverage::Unsupported(Unsupported::SheetActivation)
        );
    }
    for availability in [
        Availability::Unread,
        Availability::Missing,
        Availability::Disallowed,
        Availability::OverBudget,
    ] {
        assert_eq!(
            Coverage::sheet(Mode::LocalSheets, Activation::Unconditional, availability),
            Coverage::Unsupported(Unsupported::UninterpretedSheet)
        );
    }
    assert_eq!(
        Coverage::sheet(
            Mode::LocalSheets,
            Activation::Unconditional,
            Availability::Interpreted
        ),
        Coverage::Complete
    );
}

#[test]
fn later_success_cannot_recertify_unsupported_coverage() {
    let missing = Coverage::Unsupported(Unsupported::UninterpretedSheet);
    assert_eq!(Coverage::Complete.merge(missing), missing);
    assert_eq!(missing.merge(Coverage::Complete), missing);
    assert_eq!(
        missing.merge(Coverage::Unsupported(Unsupported::Selector)),
        missing
    );
}

#[test]
fn authored_semantic_resets_require_review_instead_of_stripping_authored_tags() {
    for authored in [
        FontState {
            bold: true,
            italic: false,
        },
        FontState {
            bold: false,
            italic: true,
        },
    ] {
        assert_eq!(
            semantic_reset(authored, FontState::default()),
            Coverage::Unsupported(Unsupported::SemanticReset)
        );
        assert_eq!(semantic_reset(authored, authored), Coverage::Complete);
    }
    assert_eq!(
        semantic_reset(FontState::default(), FontState::default()),
        Coverage::Complete
    );
    assert_eq!(
        semantic_reset(
            FontState::default(),
            FontState {
                bold: true,
                italic: true
            }
        ),
        Coverage::Complete
    );
}

#[test]
fn per_sheet_and_total_bytes_have_inclusive_caps_without_partial_charges() {
    let mut budget = Budget::default();
    for _ in 0..4 {
        budget.sheet(MAX_SHEET_BYTES).unwrap();
    }
    let full = budget;
    assert_eq!(budget.sheet(1), Err(Limit::CssBytes));
    assert_eq!(budget, full);
    assert_eq!(budget.sheet(MAX_SHEET_BYTES + 1), Err(Limit::SheetBytes));
    assert_eq!(budget, full);
}

#[test]
fn empty_sheets_still_count_toward_the_sheet_limit() {
    let mut budget = Budget::default();
    for _ in 0..MAX_SHEETS {
        budget.sheet(0).unwrap();
    }
    let full = budget;
    assert_eq!(budget.sheet(0), Err(Limit::Sheets));
    assert_eq!(budget, full);
}

#[test]
fn every_work_counter_has_an_inclusive_cap_and_atomic_rejection() {
    let full = Work {
        css_bytes: MAX_CSS_BYTES,
        sheets: MAX_SHEETS,
        rules: MAX_RULES,
        declarations: MAX_DECLARATIONS,
        node_work: MAX_NODE_WORK,
    };
    for (extra, limit) in [
        (
            Work {
                css_bytes: 1,
                ..Work::default()
            },
            Limit::CssBytes,
        ),
        (
            Work {
                sheets: 1,
                ..Work::default()
            },
            Limit::Sheets,
        ),
        (
            Work {
                rules: 1,
                ..Work::default()
            },
            Limit::Rules,
        ),
        (
            Work {
                declarations: 1,
                ..Work::default()
            },
            Limit::Declarations,
        ),
        (
            Work {
                node_work: 1,
                ..Work::default()
            },
            Limit::NodeWork,
        ),
    ] {
        let mut budget = Budget::default();
        budget.charge(full).unwrap();
        let before = budget;
        assert_eq!(budget.charge(extra), Err(limit));
        assert_eq!(budget, before);
    }
}

#[test]
fn a_late_counter_failure_or_integer_overflow_cannot_spend_earlier_counters() {
    let mut budget = Budget::default();
    budget
        .charge(Work {
            node_work: MAX_NODE_WORK,
            ..Work::default()
        })
        .unwrap();
    let before = budget;
    for amount in [1, usize::MAX] {
        assert_eq!(
            budget.charge(Work {
                css_bytes: 1,
                sheets: 1,
                rules: 1,
                declarations: 1,
                node_work: amount
            }),
            Err(Limit::NodeWork)
        );
        assert_eq!(budget, before);
    }
}
