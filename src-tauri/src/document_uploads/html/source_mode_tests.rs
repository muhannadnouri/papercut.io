use super::emphasis::{Normalized, Normalizer, SourceKind};
use super::*;
use kuchikiki::{parse_html, traits::TendrilSink};

#[test]
fn standards_html_matches_type_names_insensitively_but_keeps_class_and_id_case() {
    let raw = r#"<!doctype html><html><head><style>
        P.\45 mphasis {font-weight:bold}
        #\54 arget {font-style:italic}
        </style></head><body>
        <p id='Target' class='Emphasis'>Été عربيّ</p>
        <p id='target' class='emphasis'>Plain</p>
        </body></html>"#;
    let prepared = prepare_html_document(raw);
    let dom = parse_html().one(prepared.view_html.clone()).document_node;
    assert_eq!(
        dom.select_first("#Target > i > b").unwrap().text_contents(),
        "Été عربيّ"
    );
    assert!(dom.select_first("#target b, #target i").is_err());
    assert!(prepared.source_formatting.unwrap().warnings.is_empty());
    assert!(!prepared.view_html.contains("<style"));
    let safe = sanitize_html(&prepared.view_html);
    assert_eq!(sanitize_html(&safe), safe);
}

#[test]
fn legacy_html_does_not_silently_apply_standards_case_matching() {
    for doctype in [
        "",
        "<!DOCTYPE HTML PUBLIC '-//W3C//DTD HTML 4.01 Transitional//EN'>",
        "<!DOCTYPE HTML PUBLIC '-//W3C//DTD HTML 4.01 Transitional//EN' 'http://www.w3.org/TR/html4/loose.dtd'>",
    ] {
        let raw = format!(
            r#"{doctype}<html><head><style>
            .\58 {{font-weight:bold}} #\54 ARGET {{font-style:italic}}
            </style></head><body><p id='target' class='x'>Mixed <strong>authored</strong> case.</p></body></html>"#
        );
        let prepared = prepare_html_document(&raw);
        let baseline = parse_html_document(&raw);
        assert_eq!(prepared.view_html, baseline.view_html, "{doctype}");
        assert_eq!(
            prepared
                .sections
                .iter()
                .map(|section| (&section.text, &section.body_ranges))
                .collect::<Vec<_>>(),
            baseline
                .sections
                .iter()
                .map(|section| (&section.text, &section.body_ranges))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            prepared.source_formatting.unwrap().warnings,
            ["unsupported-document-mode"],
            "{doctype}"
        );
    }
}

#[test]
fn legacy_table_font_inheritance_is_not_guessed_by_the_standards_walker() {
    let raw = "<body style='font-weight:bold;font-style:italic'><p id='before'>Before</p><table><tbody><tr><td id='cell'>Cell</td></tr></tbody></table><p id='after'>After</p></body>";
    let prepared = prepare_html_document(raw);
    let baseline = parse_html_document(raw);
    assert_eq!(prepared.view_html, baseline.view_html);
    assert_eq!(
        prepared.source_formatting.unwrap().warnings,
        ["unsupported-document-mode"]
    );
    let dom = parse_html().one(prepared.view_html).document_node;
    assert_eq!(dom.select_first("#cell").unwrap().text_contents(), "Cell");
    assert!(dom.select_first("b, i").is_err());

    let standards = prepare_html_document(&format!("<!doctype html>{raw}"));
    let dom = parse_html().one(standards.view_html).document_node;
    for id in ["before", "cell", "after"] {
        assert!(dom.select_first(&format!("#{id} > i > b")).is_ok());
    }
    assert!(standards.source_formatting.unwrap().warnings.is_empty());
}

#[test]
fn legacy_html_without_affecting_css_keeps_semantic_markup_without_a_mode_warning() {
    for raw in [
        "<p id='p'>Plain É عربيّ</p>",
        "<p id='p'><strong>Bold</strong> <em>italic</em> text.</p>",
        "<p id='p' style='color:red'>Unrelated presentation</p>",
    ] {
        let prepared = prepare_html_document(raw);
        let baseline = parse_html_document(raw);
        assert_eq!(prepared.view_html, baseline.view_html, "{raw}");
        assert!(
            prepared.source_formatting.unwrap().warnings.is_empty(),
            "{raw}"
        );
    }
}

/// Embedded XHTML fixtures must never request an archive or host filesystem read.
fn normalized_xhtml(raw: &str) -> Normalized {
    Normalizer::default().normalize_with(
        raw,
        SourceKind::Xhtml,
        |_| panic!("An embedded-only fixture cannot resolve a stylesheet"),
        |_, _| panic!("An embedded-only fixture cannot read a stylesheet"),
    )
}

#[test]
fn xhtml_uppercase_type_selectors_are_not_matched_using_html_case_rules() {
    for selector in ["P", r"\50", r"\000050", "P.Emphasis"] {
        let raw = format!(
            "<html xmlns='http://www.w3.org/1999/xhtml'><head><style>{selector}{{font-weight:bold}}</style></head><body><p id='p' class='Emphasis'>Plain É عربيّ</p></body></html>"
        );
        let normalized = normalized_xhtml(&raw);
        assert_eq!(
            normalized.warning,
            Some("unsupported-document-mode"),
            "{selector}"
        );
        assert_eq!(normalized.html, raw);
        let prepared = parse_html_document(&normalized.html);
        let baseline = parse_html_document(&raw);
        assert_eq!(prepared.view_html, baseline.view_html);
        assert!(parse_html()
            .one(prepared.view_html)
            .document_node
            .select_first("#p b")
            .is_err());
    }
}

#[test]
fn xhtml_lowercase_types_keep_unicode_and_escaped_class_and_id_matching() {
    let raw = r#"<html xmlns='http://www.w3.org/1999/xhtml'><head><style>
        p.\63 afé#\54 arget {font-weight:bold;font-style:italic}
        </style></head><body>
        <p id='Target' class='café'>Été عربيّ</p>
        <p id='target' class='CAFÉ'>Plain</p>
        </body></html>"#;
    let normalized = normalized_xhtml(raw);
    assert_eq!(normalized.warning, None);
    let prepared = parse_html_document(&normalized.html);
    let dom = parse_html().one(prepared.view_html).document_node;
    assert_eq!(
        dom.select_first("#Target > i > b").unwrap().text_contents(),
        "Été عربيّ"
    );
    assert!(dom.select_first("#target b, #target i").is_err());

    // XML type-name checks must not confuse uppercase class/ID identifiers with element names.
    let named = normalized_xhtml(
        "<html xmlns='http://www.w3.org/1999/xhtml'><head><style>.P#P{font-weight:bold}</style></head><body><p class='P' id='P'>Exact</p></body></html>",
    );
    assert_eq!(named.warning, None);
    assert!(named.html.contains("<b>Exact</b>"));
}

#[test]
fn xhtml_source_qualification_cannot_silently_use_a_repaired_html_tree() {
    for body in [
        "<P>Readable</P>",
        "<p CLASS='x'>Readable</p>",
        "<p ID='p'>Readable</p>",
        "<p STYLE='font-style:italic'>Readable</p>",
        "<h:p xmlns:h='http://www.w3.org/1999/xhtml'>Readable</h:p>",
        "<p xmlns='urn:unrelated'>Readable</p>",
        "<p xml:id='p'>Readable</p>",
        "<p>Readable",
    ] {
        let raw = format!(
            "<html xmlns='http://www.w3.org/1999/xhtml'><head><style>p{{font-weight:bold}}</style></head><body>{body}</body></html>"
        );
        let normalized = normalized_xhtml(&raw);
        assert_eq!(
            normalized.warning,
            Some("unsupported-document-mode"),
            "{body}"
        );
        assert_eq!(normalized.html, raw, "{body}");
        let prepared = parse_html_document(&normalized.html);
        let baseline = parse_html_document(&raw);
        assert_eq!(prepared.view_html, baseline.view_html, "{body}");
        assert_eq!(
            prepared
                .sections
                .iter()
                .map(|section| (&section.text, &section.body_ranges))
                .collect::<Vec<_>>(),
            baseline
                .sections
                .iter()
                .map(|section| (&section.text, &section.body_ranges))
                .collect::<Vec<_>>(),
            "{body}"
        );
        assert!(prepared
            .sections
            .iter()
            .any(|section| section.text == "Readable"));
        assert!(parse_html()
            .one(prepared.view_html)
            .document_node
            .select_first("b, i")
            .is_err());
    }
}

#[test]
fn xhtml_source_qualification_without_affecting_css_does_not_invent_a_mode_warning() {
    for body in [
        "<P>Plain</P>",
        "<p ID='p'><strong>Authored</strong> emphasis</p>",
        "<p style='color:red'>Unrelated presentation</p>",
    ] {
        let raw = format!(
            "<html xmlns='http://www.w3.org/1999/xhtml'><head><!-- No font stylesheet --></head><body>{body}</body></html>"
        );
        let normalized = normalized_xhtml(&raw);
        assert_eq!(normalized.warning, None, "{body}");
        assert_eq!(normalized.html, raw, "{body}");
    }
}

#[test]
fn xhtml_html_repairs_cannot_invent_matching_containers_or_reparent_styled_text() {
    for raw in [
        "<html xmlns='http://www.w3.org/1999/xhtml'><style>body{font-weight:bold}</style><p>Words</p></html>",
        "<doc><style>body{font-weight:bold}</style><p>Words</p></doc>",
        "<html><head><style>p{font-weight:bold}</style></head><body><p>Before<div>Words</div>After</p></body></html>",
        "<html><head><style>tbody{font-weight:bold}</style></head><body><table><tr><td>Words</td></tr></table></body></html>",
        "<html><head><style>table{font-weight:bold}</style></head><body><table>Words<tr><td>Cell</td></tr></table></body></html>",
        "<html><head><style>p{font-weight:bold}</style></head><body><p id='a\nb'>Words</p></body></html>",
    ] {
        let normalized = normalized_xhtml(raw);
        assert_eq!(normalized.warning, Some("unsupported-document-mode"), "{raw}");
        assert_eq!(normalized.html, raw);
        let prepared = parse_html_document(&normalized.html);
        let baseline = parse_html_document(raw);
        assert_eq!(prepared.view_html, baseline.view_html, "{raw}");
        assert!(parse_html().one(prepared.view_html).document_node.select_first("b, i").is_err(), "{raw}");
        assert!(prepared.sections.iter().any(|section| section.text.contains("Words")));
    }

    // An explicit, unrepaired hierarchy keeps computed inheritance through tables and inline nodes.
    let raw = "<html><head><style>body{font-weight:bold}p{font-style:italic}</style></head><body><p>Before<span>Words</span>After</p><table><tbody><tr><td>Cell</td></tr></tbody></table></body></html>";
    let normalized = normalized_xhtml(raw);
    assert_eq!(normalized.warning, None);
    let prepared = parse_html_document(&normalized.html);
    let dom = parse_html().one(prepared.view_html).document_node;
    assert_eq!(
        dom.select_first("p > span > i > b")
            .unwrap()
            .text_contents(),
        "Words"
    );
    assert_eq!(dom.select_first("td > b").unwrap().text_contents(), "Cell");
}

#[test]
fn xhtml_source_comparison_obeys_the_existing_import_depth_limit() {
    let nesting = super::emphasis_policy::MAX_DOM_DEPTH + 1;
    let raw = format!(
        "<html><head/><body>{}<p style='font-weight:bold'>Words</p>{}</body></html>",
        "<div>".repeat(nesting),
        "</div>".repeat(nesting)
    );
    let normalized = normalized_xhtml(&raw);
    assert_eq!(normalized.warning, Some("dom-depth-limit"));
    assert_eq!(normalized.html, raw);
}

#[test]
fn original_xml_sheets_remain_visible_after_namespace_or_earlier_tree_mismatches() {
    for (root, sheet) in [
        ("html", "<h:style>p{font-weight:bold}</h:style>"),
        ("h:html", "<h:style>p{font-weight:bold}</h:style>"),
        (
            "html",
            "<h:link rel='alternate STYLESHEET' href='unread.css'/>",
        ),
        ("html", "<s:style>p{font-weight:bold}</s:style>"),
        (
            "html",
            "<style xmlns='http://www.w3.org/2000/svg'>p{font-weight:bold}</style>",
        ),
    ] {
        let raw = format!("<{root} xmlns='http://www.w3.org/1999/xhtml' xmlns:h='http://www.w3.org/1999/xhtml' xmlns:s='http://www.w3.org/2000/svg'><head>{sheet}</head><body><p>Words</p></body></{root}>");
        let normalized = normalized_xhtml(&raw);
        assert_eq!(
            normalized.warning,
            Some("unsupported-document-mode"),
            "{root}: {sheet}"
        );
        assert_eq!(normalized.html, raw);
    }
}

#[test]
fn only_genuine_html_inert_ancestors_suppress_original_xml_sheet_evidence() {
    for (container, namespace, warning) in [
        ("template", "http://www.w3.org/1999/xhtml", None),
        ("noscript", "http://www.w3.org/1999/xhtml", None),
        ("script", "http://www.w3.org/1999/xhtml", None),
        ("template", "urn:foreign", Some("unsupported-document-mode")),
        ("script", "urn:foreign", Some("unsupported-document-mode")),
    ] {
        let raw = format!("<html xmlns='http://www.w3.org/1999/xhtml' xmlns:h='http://www.w3.org/1999/xhtml' xmlns:f='{namespace}'><head/><body><f:{container}><h:style>p{{font-weight:bold}}</h:style></f:{container}><p>Words</p></body></html>");
        let normalized = normalized_xhtml(&raw);
        assert_eq!(normalized.warning, warning, "{container}: {namespace}");
        assert_eq!(normalized.html, raw);
    }
}

#[test]
fn unrelated_prefixed_xml_and_non_stylesheet_links_do_not_invent_a_sheet_warning() {
    for content in [
        "<h:p>Words</h:p>",
        "<f:style xmlns:f='urn:foreign'>Unrelated words</f:style>",
        "<h:link rel='preload' href='unread.css'/><p>Words</p>",
    ] {
        let raw = format!("<html xmlns='http://www.w3.org/1999/xhtml' xmlns:h='http://www.w3.org/1999/xhtml'><head/><body>{content}</body></html>");
        let normalized = normalized_xhtml(&raw);
        assert_eq!(normalized.warning, None, "{content}");
        assert_eq!(normalized.html, raw);
    }
}

#[test]
fn rejected_xml_never_certifies_sheets_that_html_cannot_recognize() {
    for raw in [
        "<!DOCTYPE html [<!ENTITY unused 'never expanded'>]><html xmlns='http://www.w3.org/1999/xhtml' xmlns:h='http://www.w3.org/1999/xhtml'><head><h:style>p{font-weight:bold}</h:style></head><body><p>Words</p></body></html>",
        "<html xmlns='http://www.w3.org/1999/xhtml' xmlns:h='http://www.w3.org/1999/xhtml'><head><h:style>p{font-weight:bold}</h:style></head><body><p>Words</body></html>",
        "<html xmlns='http://www.w3.org/1999/xhtml'><head/><body><p>Words</body></html>",
    ] {
        let normalized = normalized_xhtml(raw);
        assert_eq!(normalized.warning, Some("unsupported-document-mode"));
        assert_eq!(normalized.html, raw);
    }
}
