use super::super::emphasis_policy::{
    MAX_CSS_BYTES, MAX_DECLARATIONS, MAX_DOM_DEPTH, MAX_NODE_WORK,
};
use super::*;

#[test]
fn resets_of_retained_default_emphasis_warn_instead_of_claiming_visual_equivalence() {
    for source in [
        "<cite style='font-style:normal'>Plain</cite>",
        "<h1><span style='font-weight:normal'>Plain</span></h1>",
        "<table><tr><th style='font-weight:initial'>Plain</th></tr></table>",
        "<body style='font-style:normal'><var style='font-style:inherit'>Plain</var></body>",
    ] {
        let result = Normalizer::default().normalize(source);
        assert_eq!(result.warning, Some("semantic-reset"), "{source}");
        assert_eq!(result.html, source);
    }
    let source = "<body style='font-weight:normal;font-style:normal'><h1>Heading</h1><cite style='font-style:italic'>Citation</cite><table><tr><th>Header</th></tr></table></body>";
    let result = Normalizer::default().normalize(source);
    assert_eq!(result.warning, None);
    assert_eq!(result.html, source);
}

fn normalize(html: &str) -> String {
    let result = Normalizer::default().normalize(html);
    assert_eq!(result.warning, None, "{}", result.html);
    result.html
}

fn body(html: &str) -> String {
    parse_html()
        .one(html)
        .document_node
        .select_first("body")
        .unwrap()
        .as_node()
        .children()
        .map(|node| node.to_string())
        .collect()
}

#[test]
fn inline_inheritance_resets_and_nested_semantics_preserve_text_and_identity() {
    let source = r#"<p id="p" style="font-weight:bold"> A <span id="reset" style="font-weight:normal">plain</span> <em id="e">both</em> <span style="font-style:italic">both again</span> </p>"#;
    let result = normalize(source);
    let document = parse_html().one(result).document_node;
    assert_eq!(document.text_contents(), " A plain both both again ");
    assert_eq!(document.select("#p").unwrap().count(), 1);
    assert_eq!(document.select("#reset").unwrap().count(), 1);
    assert_eq!(document.select("#e").unwrap().count(), 1);
    assert_eq!(document.select("#reset b").unwrap().count(), 0);
    assert_eq!(document.select("#e b").unwrap().count(), 1);
    assert_eq!(document.select("p > b").unwrap().count(), 4);
    assert_eq!(document.select("span i b").unwrap().count(), 1);
}

#[test]
fn full_document_root_and_body_styles_reach_only_readable_descendants() {
    let source = r#"<html style="font-weight:bold"><head><title>Plain title</title></head><body style="font-style:italic"><p>Both</p><p style="font-weight:normal;font-style:normal">Neither</p></body></html>"#;
    let result = normalize(source);
    assert!(result.contains("<title>Plain title</title>"));
    assert!(result.contains("<p><i><b>Both</b></i></p>"));
    assert!(result.contains("font-style:normal\">Neither</p>"));
}

#[test]
fn comments_case_and_css_escapes_are_parsed_as_tokens() {
    let source = r#"<p style="FONT-\77 eight: b\6f ld/**/!/**/ImPoRtAnT; font-\73 tyle: \69 talic">Words</p>"#;
    assert!(body(&normalize(source)).contains("<i><b>Words</b></i>"));
}

#[test]
fn duplicate_longhands_respect_property_importance_and_source_order() {
    for (style, bold, italic) in [
        ("font-weight:normal;font-weight:bold", true, false),
        ("font-weight:bold!important;font-weight:normal", true, false),
        (
            "font-weight:bold!important;font-weight:normal!important",
            false,
            false,
        ),
        (
            "font-weight:bold;font-style:italic!important;font-style:normal",
            true,
            true,
        ),
        (
            "font-weight:bold;font-weight:normal;font-style:italic",
            false,
            true,
        ),
    ] {
        let result = normalize(&format!("<p style='{style}'>Words</p>"));
        let document = parse_html().one(result).document_node;
        assert_eq!(
            document.select("b").unwrap().count(),
            usize::from(bold),
            "{style}"
        );
        assert_eq!(
            document.select("i").unwrap().count(),
            usize::from(italic),
            "{style}"
        );
    }
}

#[test]
fn inherited_important_never_beats_a_child_declaration() {
    let result = normalize("<p style='font-weight:bold!important;font-style:italic!important'>Parent<span style='font-weight:normal;font-style:normal'>Child</span><span style='font-weight:inherit;font-style:unset'>Inherited</span></p>");
    let document = parse_html().one(result).document_node;
    let spans = document.select("span").unwrap().collect::<Vec<_>>();
    assert_eq!(
        spans[0].as_node().to_string(),
        "<span style=\"font-weight:normal;font-style:normal\">Child</span>"
    );
    assert!(spans[1]
        .as_node()
        .to_string()
        .contains("<i><b>Inherited</b></i>"));
}

#[test]
fn numeric_weights_and_css_wide_values_use_the_frozen_visual_policy() {
    for (weight, expected) in [("100", false), ("599", false), ("600", true), ("900", true)] {
        let result = normalize(&format!("<p style='font-weight:{weight}'>Words</p>"));
        assert_eq!(result.contains("<b>Words</b>"), expected);
    }
    let result = normalize("<p style='font-weight:bold;font-style:italic'><span style='font-weight:initial;font-style:normal'>Off</span><span style='font-weight:unset;font-style:inherit'>On</span></p>");
    assert!(result.contains(">Off</span>"));
    assert!(result.contains("<i><b>On</b></i>"));
    assert!(normalize("<p style='font-style:oblique'>Words</p>").contains("<i>Words</i>"));
}

#[test]
fn unsupported_affecting_values_and_malformed_declarations_are_atomic() {
    for style in [
        "font-weight:bolder",
        "font-weight:lighter",
        "font-weight:950",
        "font-weight:0",
        "font-weight:var(--weight)",
        "font-weight:revert",
        "font-weight:600 700",
        "font-weight:bold !urgent",
        "font-style:oblique 10deg",
        "font-style:bold",
        "font-style:var(--style)",
        "font-weight bold",
        "font-weight:",
        "@media screen { font-weight:bold }",
    ] {
        let source = format!("<p style='font-weight:bold'>Earlier</p><p style='{style}'>Later</p>");
        let result = Normalizer::default().normalize(&source);
        assert_eq!(result.warning, Some("unsupported-value"), "{style}");
        assert_eq!(result.html, source, "{style}");
    }
}

#[test]
fn font_and_all_shorthands_cannot_silently_override_supported_longhands() {
    for style in [
        "font-weight:bold;font:normal 12px serif",
        "font:inherit;font-weight:bold",
        "font-style:italic;all:initial",
        "all:unset!important;font-style:italic",
    ] {
        let source = format!("<p style='{style}'>Words</p>");
        let result = Normalizer::default().normalize(&source);
        assert_eq!(result.warning, Some("unsupported-shorthand"));
        assert_eq!(result.html, source);
    }
}

#[test]
fn linked_html_sheets_remain_unread_and_uncertain_activation_warns() {
    for (sheet, reason) in [
        (
            "<link rel='stylesheet' href='chapter.css'>",
            "unread-stylesheet",
        ),
        (
            "<link rel='alternate StyleSheet' media='print' href='other.css'>",
            "unsupported-activation",
        ),
    ] {
        let source = format!("<html><head>{sheet}</head><body><p class='x' style='font-weight:bold'>Words</p></body></html>");
        let result = Normalizer::default().normalize(&source);
        assert_eq!(result.warning, Some(reason));
        assert_eq!(result.html, source);
    }
    assert!(normalize(
        "<style></style><link rel='icon' href='cover.png'><p style='font-weight:bold'>Words</p>"
    )
    .contains("<b>Words</b>"));
}

#[test]
fn semantic_emphasis_is_preserved_and_conflicting_resets_remain_unresolved() {
    for source in [
        "<strong id='s' style='font-weight:normal'>Words</strong>",
        "<em id='e'><span style='font-style:initial'>Words</span></em>",
        "<b><span style='font-weight:100'>Words</span></b>",
        "<i style='font-style:unset'>Words</i>",
    ] {
        let result = Normalizer::default().normalize(source);
        assert_eq!(result.warning, Some("semantic-reset"));
        assert_eq!(result.html, source);
    }
    let source = "<strong id='s'><em id='e'>Words</em></strong>";
    assert_eq!(normalize(source), source);
}

#[test]
fn normalization_is_idempotent_without_cloning_containers_or_anchors() {
    let source = r##"<p id="p" style="font-weight:bold;font-style:italic">Words<a id="ref" href="#n" role="doc-noteref">1</a><span style="font-weight:normal">After</span></p><aside id="n">Note<a href="#ref">Back</a></aside>"##;
    let first = normalize(source);
    assert_eq!(normalize(&first), first);
    let document = parse_html().one(first).document_node;
    for selector in ["#p", "#ref", "#n", "a[href='#n']", "a[href='#ref']"] {
        assert_eq!(document.select(selector).unwrap().count(), 1);
    }
}

#[test]
fn exact_whitespace_unicode_literal_text_and_assets_survive() {
    let source = "<p style='font-weight:bold'>  Français\nالعَرَبِيَّة 👩‍👩‍👧‍👦 <code id='literal'>a &lt; b\t\n</code><a href='#n' id='r'>[1]</a><img src='picture.png' data-papercut-asset='asset.png' alt='Picture'> </p><aside id='n'>Note</aside>";
    let before = parse_html().one(source).document_node;
    let after = parse_html().one(normalize(source)).document_node;
    assert_eq!(after.text_contents(), before.text_contents());
    for selector in ["#literal", "#r", "#n", "img"] {
        let before = before.select_first(selector).unwrap();
        let after = after.select_first(selector).unwrap();
        assert_eq!(
            before.attributes.borrow().map,
            after.attributes.borrow().map
        );
        assert_eq!(before.text_contents(), after.text_contents());
    }
}

#[test]
fn raw_text_head_foreign_and_template_content_never_receive_wrappers() {
    let source = "<html style='font-weight:bold'><head><title>Title</title><script>if (a < b) value++;</script></head><body><textarea>Literal &amp; text</textarea><script>execute()</script><svg><text>Foreign</text></svg><template><p>Template</p></template><p>Readable</p></body></html>";
    let result = normalize(source);
    assert!(result.contains("<title>Title</title>"));
    assert!(result.contains("<script>if (a < b) value++;</script>"));
    assert!(result.contains("<textarea>Literal &amp; text</textarea>"));
    assert!(result.contains("<script>execute()</script>"));
    assert!(result.contains("<text>Foreign</text>"));
    assert!(!result.contains("<b>Template</b>"));
    assert!(result.contains("<p><b>Readable</b></p>"));
}

#[test]
fn unrelated_css_is_inert_and_unchanged_sanitizer_removes_executable_content() {
    let source = "<p style='color:red;background:URL(\"javascript:evil()\");font-weight:bold' onclick='evil()'>Readable<a href='javascript:evil()'>link</a></p><script>evil()</script>";
    let normalized = normalize(source);
    let sanitized = super::super::sanitize_html(&normalized);
    assert!(sanitized.contains("<b>Readable</b>"));
    assert!(!sanitized.contains("style="));
    assert!(!sanitized.contains("onclick"));
    assert!(!sanitized.contains("javascript:"));
    assert!(!sanitized.contains("<script"));
    assert_eq!(super::super::sanitize_html(&sanitized), sanitized);
}

#[test]
fn css_byte_bound_is_precharged_and_readable_fallback_is_untouched() {
    let css = format!("color:{}", "x".repeat(MAX_CSS_BYTES - 6));
    let source = format!("<p style='{css}'>Words</p>");
    assert_eq!(Normalizer::default().normalize(&source).warning, None);
    let source = format!("<p style='font-weight:bold'>Earlier</p><p style='{css}x'>Words</p>");
    let result = Normalizer::default().normalize(&source);
    assert_eq!(result.warning, Some("css-byte-limit"));
    assert_eq!(result.html, source);
}

#[test]
fn declaration_budget_is_import_wide_and_exhaustion_is_transactional() {
    let mut normalizer = Normalizer::default();
    let source = format!(
        "<p style='{}'>Words</p>",
        "color:red;".repeat(MAX_DECLARATIONS)
    );
    assert_eq!(normalizer.normalize(&source).warning, None);
    let source = "<p style='font-weight:bold'>Next chapter</p>";
    let result = normalizer.normalize(source);
    assert_eq!(result.warning, Some("declaration-limit"));
    assert_eq!(result.html, source);
}

#[test]
fn node_budget_is_import_wide_and_charges_generated_work_before_mutation() {
    let mut normalizer = Normalizer::default();
    normalizer.budget.used.node_work = MAX_NODE_WORK - 11;
    let source = "<p style='font-weight:bold'>Words</p>";
    // Discovery and planning each visit html/head/body/p/text, plus one wrapper.
    assert_eq!(normalizer.normalize(source).warning, None);
    let result = normalizer.normalize(source);
    assert_eq!(result.warning, Some("node-work-limit"));
    assert_eq!(result.html, source);
    let mut normalizer = Normalizer::default();
    normalizer.budget.used.node_work = MAX_NODE_WORK - 10;
    let result = normalizer.normalize(source);
    assert_eq!(result.warning, Some("node-work-limit"));
    assert_eq!(result.html, source);
}

#[test]
fn depth_admission_includes_generated_wrappers_before_recursive_serialization() {
    let nested = |count| {
        format!(
            "<body style='font-weight:bold'>{}Words{}</body>",
            "<div>".repeat(count),
            "</div>".repeat(count)
        )
    };
    assert!(normalize(&nested(MAX_DOM_DEPTH - 4)).contains("<b>Words</b>"));
    for count in [MAX_DOM_DEPTH - 3, MAX_DOM_DEPTH + 1] {
        let source = nested(count);
        let result = Normalizer::default().normalize(&source);
        assert_eq!(result.warning, Some("dom-depth-limit"));
        assert_eq!(result.html, source);
    }
}

#[test]
fn rejected_local_sheets_are_cached_and_counted_before_any_repeated_zip_work() {
    let mut normalizer = Normalizer::default();
    normalizer.budget.used.css_bytes = MAX_CSS_BYTES - 1;
    let source = "<link rel='stylesheet' href='style.css'><p style='font-weight:bold'>Words</p>";
    let mut reads = 0;
    for _ in 0..2 {
        let result = normalizer.normalize_with(
            source,
            |href| Ok(href.into()),
            |_, limit| {
                reads += 1;
                assert_eq!(limit, 1);
                // Even a faulty callback cannot bypass the pre-decode byte admission.
                Ok(b"p{}".to_vec())
            },
        );
        assert_eq!(result.warning, Some("css-byte-limit"));
        assert_eq!(result.html, source);
    }
    assert_eq!(reads, 1);
    assert_eq!(normalizer.budget.used.sheets, 1);
    assert_eq!(normalizer.budget.used.css_bytes, MAX_CSS_BYTES - 1);

    let mut normalizer = Normalizer::default();
    let mut reads = 0;
    for index in 0..policy::MAX_SHEETS + 2 {
        let source = format!("<link rel='stylesheet' href='{index}.css'><p>Words</p>");
        let result = normalizer.normalize_with(
            &source,
            |href| Ok(href.into()),
            |_, _| {
                reads += 1;
                Err("unread-stylesheet")
            },
        );
        assert_eq!(
            result.warning,
            Some(if index < policy::MAX_SHEETS {
                "unread-stylesheet"
            } else {
                "sheet-limit"
            })
        );
        assert_eq!(result.html, source);
    }
    assert_eq!(reads, policy::MAX_SHEETS);
    assert_eq!(normalizer.local_sheets.len(), policy::MAX_SHEETS);
}

#[test]
fn sheet_derived_wrappers_keep_literal_and_select_atomic_fallbacks() {
    for (body, warning) in [
        (
            "<p>Words</p><plaintext>literal <stuff>",
            "literal-roundtrip",
        ),
        ("<p>Words</p><pre>\n\nLiteral</pre>", "literal-roundtrip"),
        (
            "<p>Words</p><select><option>Choice</option></select>",
            "unsupported-context",
        ),
    ] {
        let source = format!("<style>body{{font-style:italic}}</style>{body}");
        let result = Normalizer::default().normalize(&source);
        assert_eq!(result.warning, Some(warning));
        assert_eq!(result.html, source);
    }
}

#[test]
fn invalid_utf8_local_css_stays_unread_and_cannot_clear_the_cached_warning() {
    let source = "<link rel='stylesheet' href='style.css'><p style='font-weight:bold'>Words</p>";
    let mut normalizer = Normalizer::default();
    let mut reads = 0;
    for _ in 0..2 {
        let result = normalizer.normalize_with(
            source,
            |href| Ok(href.into()),
            |_, _| {
                reads += 1;
                Ok(vec![0xff])
            },
        );
        assert_eq!(result.html, source);
        assert_eq!(result.warning, Some("unread-stylesheet"));
    }
    assert_eq!(reads, 1);
    assert_eq!(normalizer.budget.used.sheets, 1);
    assert_eq!(normalizer.budget.used.css_bytes, 1);
}

#[test]
fn external_utf8_bom_is_decoded_without_spending_or_matching_its_bytes_twice() {
    for selector in ["p", ".word", "#word"] {
        let source = "<link rel='stylesheet' href='style.css'><p id='word' class='word'>Words</p>";
        let css = format!("\u{feff}{selector}{{font-weight:bold}}");
        let mut normalizer = Normalizer::default();
        let result = normalizer.normalize_with(
            source,
            |href| Ok(href.into()),
            |_, _| Ok(css.as_bytes().to_vec()),
        );
        assert_eq!(result.warning, None);
        assert!(result.html.contains("<b>Words</b>"), "{selector}");
        assert_eq!(normalizer.budget.used.css_bytes, css.len());
    }
    let source = "<link rel='stylesheet' href='style.css' charset='windows-1252'><p>Words</p>";
    let result = Normalizer::default().normalize_with(
        source,
        |href| Ok(href.into()),
        |_, _| panic!("Unsupported encoding must not be read"),
    );
    assert_eq!(result.warning, Some("unsupported-activation"));
    assert_eq!(result.html, source);
}
