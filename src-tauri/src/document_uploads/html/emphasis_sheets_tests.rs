use super::*;
use kuchikiki::{parse_html, traits::TendrilSink};

#[test]
fn admitted_compounds_lists_comments_and_escapes_use_real_selector_matching() {
    let mut budget = Budget::default();
    let sheet = parse(
        r"p.\62 old/**/#target, .café { f\6f nt-weight: b\6f ld; FONT-style: italic }",
        &mut budget,
    )
    .unwrap();
    let document = parse_html()
        .one("<p class='bold' id='target'>One</p><div class='café'>Two</div><p>Three</p>");
    let matching: Vec<_> = document.document_node.select("p, div").unwrap().collect();
    let rule = &sheet.rules[0];
    assert!(rule.select(&matching[0], &mut budget).unwrap().is_some());
    assert!(rule.select(&matching[1], &mut budget).unwrap().is_some());
    assert!(rule.select(&matching[2], &mut budget).unwrap().is_none());
    assert_eq!(rule.selected[0].unwrap().value, policy::SpecifiedValue::On);
    assert_eq!(rule.selected[1].unwrap().value, policy::SpecifiedValue::On);
}

#[test]
fn only_matching_selectors_contribute_specificity() {
    let mut budget = Budget::default();
    let sheet = parse("#absent, p.x, .x {font-weight:bold}", &mut budget).unwrap();
    let document = parse_html().one("<p class='x'>Text</p>");
    let element = document.document_node.select_first("p").unwrap();
    let expected = Selectors::compile("p.x").unwrap().0[0].specificity();
    assert!(sheet.rules[0].select(&element, &mut budget).unwrap() == Some(expected));
}

#[test]
fn declaration_importance_and_order_are_property_specific() {
    let mut budget = Budget::default();
    let sheet = parse(
        ".x {font-weight:bold !important; font-weight:normal; font-style:normal !important; font-style:italic !important; color:red}",
        &mut budget,
    )
    .unwrap();
    let selected = &sheet.rules[0].selected;
    assert_eq!(selected[0].unwrap().value, policy::SpecifiedValue::On);
    assert!(selected[0].unwrap().important);
    assert_eq!(selected[1].unwrap().value, policy::SpecifiedValue::On);
    assert!(selected[1].unwrap().important);
    assert_eq!(budget.used.declarations, 5);
}

#[test]
fn every_selector_outside_the_compound_subset_rejects_the_whole_sheet() {
    for selector in [
        "p .x",
        "p\n.x",
        "p\t.x",
        "p > .x",
        "p+.x",
        "p~.x",
        "[class=x]",
        "p:hover",
        "*",
        "svg|p",
        "|p",
        ".x p",
        ". x",
        ".x,",
        ",.x",
        ".x,,p",
        "#123",
        "",
    ] {
        let css = format!(".safe {{font-weight:bold}} {selector} {{font-style:italic}}");
        assert_eq!(
            parse(&css, &mut Budget::default()).err(),
            Some("unsupported-selector"),
            "{selector:?}"
        );
    }
}

#[test]
fn all_at_rules_including_leading_charset_and_escaped_import_are_explicit_failures() {
    for css in [
        "@import 'other.css';",
        r"@\69mport 'other.css';",
        "@charset 'utf-8'; p {font-weight:bold}",
        "@media all {p {font-weight:bold}}",
        "@supports (display:block) {p {font-weight:bold}}",
        "@font-face {font-family:other; src:url(other.woff)}",
        "@layer layer;",
    ] {
        assert_eq!(
            parse(css, &mut Budget::default()).err(),
            Some("unsupported-rule"),
            "{css}"
        );
    }
}

#[test]
fn unknown_affecting_values_and_shorthands_reject_but_unrelated_css_stays_inert() {
    for declaration in [
        "font-weight:bolder",
        "font-weight:var(--weight)",
        "font-weight:1000",
        "font-style:oblique 10deg",
        "font-style:revert",
        "font-weight:bold junk",
    ] {
        assert_eq!(
            parse(&format!("p {{{declaration}}}"), &mut Budget::default()).err(),
            Some("unsupported-value"),
            "{declaration}"
        );
    }
    for declaration in ["font:italic 12px serif", "all:initial"] {
        assert_eq!(
            parse(&format!("p {{{declaration}}}"), &mut Budget::default()).err(),
            Some("unsupported-shorthand")
        );
    }
    let sheet = parse(
        "p {background:url(https://invalid.example/image); color:var(--color); font-weight:600}",
        &mut Budget::default(),
    )
    .unwrap();
    assert_eq!(
        sheet.rules[0].selected[0].unwrap().value,
        policy::SpecifiedValue::On
    );
}

#[test]
fn per_sheet_and_import_wide_caps_are_checked_before_parsing() {
    let mut budget = Budget::default();
    assert_eq!(
        parse(&" ".repeat(policy::MAX_SHEET_BYTES + 1), &mut budget).err(),
        Some("sheet-byte-limit")
    );
    assert_eq!(budget.used, Work::default());
    budget.used.css_bytes = policy::MAX_CSS_BYTES;
    assert_eq!(parse("p{}", &mut budget).err(), Some("css-byte-limit"));
    assert_eq!(budget.used.sheets, 0);
    budget.used.css_bytes = 0;
    budget.used.sheets = policy::MAX_SHEETS;
    assert_eq!(parse("p{}", &mut budget).err(), Some("sheet-limit"));
}

#[test]
fn selector_lists_and_compounds_cannot_hide_work_behind_one_rule() {
    for selector in ["p.x.y", "p,.x,.y"] {
        let mut budget = Budget::default();
        budget.used.rules = policy::MAX_RULES - 3;
        assert_eq!(
            parse(&format!("{selector} {{font-weight:bold}}"), &mut budget).err(),
            Some("rule-limit")
        );
        assert_eq!(budget.used.rules, policy::MAX_RULES);
        assert_eq!(budget.used.declarations, 0);
    }
}

#[test]
fn declaration_and_match_work_caps_precede_their_operations() {
    let mut budget = Budget::default();
    budget.used.declarations = policy::MAX_DECLARATIONS;
    assert_eq!(
        parse("p {font-weight:bold}", &mut budget).err(),
        Some("declaration-limit")
    );
    let mut budget = Budget::default();
    let sheet = parse("p.x#target {font-weight:bold}", &mut budget).unwrap();
    let document = parse_html().one("<p class='x' id='target'>Text</p>");
    let element = document.document_node.select_first("p").unwrap();
    budget.used.node_work = policy::MAX_NODE_WORK - 2;
    assert_eq!(
        sheet.rules[0].select(&element, &mut budget).err(),
        Some("node-work-limit")
    );
    assert_eq!(budget.used.node_work, policy::MAX_NODE_WORK - 2);
}

#[test]
fn already_charged_epub_sheet_does_not_spend_byte_or_sheet_budget_twice() {
    let mut budget = Budget::default();
    let css = "p {font-style:italic}";
    budget.sheet(css.len()).unwrap();
    parse_charged(css, &mut budget).unwrap();
    assert_eq!(budget.used.sheets, 1);
    assert_eq!(budget.used.css_bytes, css.len());
    assert_eq!(budget.used.rules, 2);
    assert_eq!(budget.used.declarations, 1);
}

#[test]
fn long_class_attributes_cannot_amplify_the_selector_match_budget() {
    let mut budget = Budget::default();
    let sheet = parse(".missing{font-weight:bold}", &mut budget).unwrap();
    let document = parse_html().one(format!("<p class='{}'>Text</p>", "x ".repeat(4096)));
    let element = document.document_node.select_first("p").unwrap();
    budget.used.node_work = policy::MAX_NODE_WORK - 128;
    assert_eq!(
        sheet.rules[0].select(&element, &mut budget).err(),
        Some("node-work-limit")
    );
    assert_eq!(budget.used.node_work, policy::MAX_NODE_WORK - 128);
    budget.used.node_work = 0;
    assert!(sheet.rules[0]
        .select(&element, &mut budget)
        .unwrap()
        .is_none());
    assert_eq!(budget.used.node_work, 129);
}
