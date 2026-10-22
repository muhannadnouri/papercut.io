use super::*;
use crate::document_uploads::parsed::ParsedDocument;
use kuchikiki::{parse_html, traits::TendrilSink, NodeRef};

/// Exercise fresh import and reader sanitization, not just the CSS parser's intermediate result.
fn imported(html: &str) -> (ParsedDocument, NodeRef) {
    // These cascade fixtures exercise standards CSS; source-mode fallbacks have their own tests.
    let parsed = prepare_html_document(&format!("<!doctype html>{html}"));
    let safe = sanitize_html(&parsed.view_html);
    assert_eq!(sanitize_html(&safe), safe);
    let dom = parse_html().one(safe).document_node;
    (parsed, dom)
}

#[test]
fn the_final_import_respects_selector_specificity_order_importance_and_inheritance() {
    let html = r#"<html><head><style>
        .list, #list-id {font-weight:bold}
        .weight{font-weight:bold} p{font-weight:normal;font-style:normal}
        #specific{font-weight:normal}.weak{font-weight:bold}
        .later{font-style:normal}.later{font-style:italic}
        .important{font-weight:bold!important}
        .inline{font-style:italic}
        #parent{font-weight:bold!important;font-style:italic!important}
        .reset{font-weight:normal;font-style:normal}
        .unset{font-weight:unset;font-style:initial}
        </style></head><body>
        <p id='list-class' class='list'>Class list</p><p id='list-id'>ID list</p>
        <p id='class-over-type' class='weight'>Class beats type</p>
        <p id='specific' class='weak'>ID beats class</p>
        <p id='source-order' class='later'>Later rule</p>
        <p id='important' class='important' style='font-weight:normal'>Sheet important</p>
        <p id='inline-important' class='important' style='font-weight:normal!important'>Inline important</p>
        <p id='inline' class='inline' style='font-style:normal'>Inline normal</p>
        <div id='parent'><span id='inherited'>Inherited</span><span id='reset' class='reset'>Own reset</span><span id='unset' class='unset'>Unset weight</span></div>
        <p id='sibling'>Sibling</p></body></html>"#;
    let (parsed, dom) = imported(html);
    for (id, bold, italic) in [
        ("list-class", true, false),
        ("list-id", true, false),
        ("class-over-type", true, false),
        ("specific", false, false),
        ("source-order", false, true),
        ("important", true, false),
        ("inline-important", false, false),
        ("inline", false, false),
        ("inherited", true, true),
        ("reset", false, false),
        ("unset", true, false),
        ("sibling", false, false),
    ] {
        let node = dom.select_first(&format!("#{id}")).unwrap();
        assert_eq!(node.as_node().select_first("b").is_ok(), bold, "{id}");
        assert_eq!(node.as_node().select_first("i").is_ok(), italic, "{id}");
    }
    assert!(parsed.source_formatting.unwrap().warnings.is_empty());
}

#[test]
fn embedded_css_preserves_source_hierarchy_text_notes_and_literals_through_the_final_sanitizer() {
    let html = r##"<html lang='fr'><head><title>Source</title><style>
        .copy{font-weight:bold;background-image:url('https://invalid.test/track')}
        .word{font-style:italic}
        </style></head><body onload='dangerous()'><div id='outer'>
        <p id='p' class='copy'>Été عربيّ <span id='inner' class='word'>calme <em>authored</em></span>.
        <a id='r' role='doc-noteref' href='#n'><sup>1</sup></a><a href='javascript:dangerous()'>link</a></p>
        <pre id='literal' translate='no'>  user_id
        x &lt; y</pre><aside id='n' role='doc-footnote'>Note <a role='doc-backlink' href='#r'>↩</a></aside>
        </div><script>dangerous()</script><img src='https://invalid.test/track'/></body></html>"##;
    let (parsed, dom) = imported(html);
    let baseline = parse_html_document(html);
    let baseline_dom = parse_html().one(baseline.view_html).document_node;
    for id in ["outer", "p", "inner", "literal", "r", "n"] {
        let selector = format!("#{id}");
        assert_eq!(dom.select(&selector).unwrap().count(), 1);
        assert_eq!(
            dom.select_first(&selector).unwrap().text_contents(),
            baseline_dom
                .select_first(&selector)
                .unwrap()
                .text_contents(),
            "{id}"
        );
    }
    assert!(dom
        .select_first("#outer > p#p > span#inner > i > b")
        .is_ok());
    assert!(dom.select_first("#inner > em > b").is_ok());
    assert!(dom
        .select_first("#r[role='doc-noteref'][href='#n'] > sup > b")
        .is_ok());
    assert!(dom
        .select_first("#n a[role='doc-backlink'][href='#r']")
        .is_ok());
    assert!(dom.select_first("#literal b, #literal i").is_err());
    assert_eq!(
        parsed
            .sections
            .iter()
            .map(|s| (&s.text, &s.body_ranges))
            .collect::<Vec<_>>(),
        baseline
            .sections
            .iter()
            .map(|s| (&s.text, &s.body_ranges))
            .collect::<Vec<_>>()
    );
    for forbidden in [
        "<style",
        "style=",
        "onload",
        "javascript:",
        "dangerous",
        "invalid.test",
        "<script",
    ] {
        assert!(!parsed.view_html.contains(forbidden), "{forbidden}");
    }
    assert!(parsed.source_formatting.unwrap().warnings.is_empty());
}

#[test]
fn empty_and_unconditional_embedded_styles_accept_real_css_identifier_escapes() {
    let (parsed, dom) = imported(
        r#"<html><head><style></style><style media=' ALL ' type='TEXT/CSS'>
        .\65 mph, #escaped {font-we\69 ght:b\6f ld;font-style:it\61 lic}
        </style></head><body><p id='class' class='emph'>Été</p><p id='escaped'>عربيّ</p></body></html>"#,
    );
    for (id, text) in [("class", "Été"), ("escaped", "عربيّ")] {
        assert_eq!(
            dom.select_first(&format!("#{id} > i > b"))
                .unwrap()
                .text_contents(),
            text
        );
    }
    assert!(parsed.source_formatting.unwrap().warnings.is_empty());
}

#[test]
fn unsupported_rules_values_or_activation_fall_back_atomically_to_the_existing_import() {
    for (head, warning) in [
        (
            "<style>p > span{font-weight:bold}</style>",
            "unsupported-selector",
        ),
        (
            "<style>p:hover{font-weight:bold}</style>",
            "unsupported-selector",
        ),
        (
            "<style>[lang]{font-weight:bold}</style>",
            "unsupported-selector",
        ),
        (
            "<style>@media screen{p{font-weight:bold}}</style>",
            "unsupported-rule",
        ),
        (
            "<style>@import 'missing.css';p{font-weight:bold}</style>",
            "unsupported-rule",
        ),
        (
            "<style>@charset 'UTF-8';p{font-weight:bold}</style>",
            "unsupported-rule",
        ),
        (
            "<style>p{font:bold 12px serif}</style>",
            "unsupported-shorthand",
        ),
        (
            "<style>p{font-weight:var(--weight)}</style>",
            "unsupported-value",
        ),
        (
            "<style media='screen'>p{font-weight:bold}</style>",
            "unsupported-activation",
        ),
        (
            "<style disabled=''>p{font-weight:bold}</style>",
            "unsupported-activation",
        ),
        (
            "<style type='text/plain'>p{font-weight:bold}</style>",
            "unsupported-activation",
        ),
    ] {
        let html = format!("<html><head>{head}</head><body><p id='p' style='font-weight:bold'>Readable É <span>text.</span></p></body></html>");
        let (parsed, dom) = imported(&html);
        let baseline = parse_html_document(&html);
        assert_eq!(parsed.view_html, baseline.view_html, "{head}");
        assert_eq!(
            parsed.source_formatting.unwrap().warnings,
            [warning],
            "{head}"
        );
        assert_eq!(
            dom.select_first("#p").unwrap().text_contents(),
            "Readable É text."
        );
        assert!(dom.select_first("b, i").is_err(), "{head}");
    }
    // A CSS reset cannot silently replace an author's retained semantic emphasis.
    let html = "<style>b{font-weight:normal}</style><p><b>Authored</b></p>";
    let (parsed, dom) = imported(html);
    assert_eq!(parsed.view_html, parse_html_document(html).view_html);
    assert_eq!(
        parsed.source_formatting.unwrap().warnings,
        ["semantic-reset"]
    );
    assert_eq!(dom.select_first("b").unwrap().text_contents(), "Authored");
}

#[test]
fn genuinely_inert_content_cannot_format_active_html() {
    let (parsed, dom) = imported(
        r#"<body>
        <template><style>p{font-weight:bold}</style></template>
        <noscript><style>p{font-weight:bold}</style></noscript>
        <script>const sample = '<style>p{font-weight:bold}</style>';</script>
        <p id='p' style='font-style:italic'>Active text.</p></body>"#,
    );
    assert_eq!(
        dom.select_first("#p > i").unwrap().text_contents(),
        "Active text."
    );
    assert!(dom.select_first("#p b").is_err());
    assert!(parsed.source_formatting.unwrap().warnings.is_empty());
}

#[test]
fn an_embedded_sheet_after_readable_text_still_applies_to_the_whole_document() {
    let (parsed, dom) = imported("<body><p id='before' class='x'>Before</p><style>.x{font-weight:bold}</style><p id='after' class='x'>After</p></body>");
    assert_eq!(
        dom.select_first("#before > b").unwrap().text_contents(),
        "Before"
    );
    assert_eq!(
        dom.select_first("#after > b").unwrap().text_contents(),
        "After"
    );
    assert!(parsed.view_html.find("Before").unwrap() < parsed.view_html.find("After").unwrap());
    assert!(parsed.source_formatting.unwrap().warnings.is_empty());
}

#[test]
fn ordinary_html_never_consumes_even_an_existing_host_stylesheet() {
    struct TestDirectory(std::path::PathBuf);
    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let directory = TestDirectory(std::env::temp_dir().join(format!(
        "papercut-stylesheet-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )));
    std::fs::create_dir(&directory.0).unwrap();
    let path = directory.0.join("style.css");
    std::fs::write(&path, "p{font-weight:bold}").unwrap();
    for href in [
        path.display().to_string(),
        format!("file://{}", path.display()),
        "style.css".into(),
        "https://invalid.test/style.css".into(),
    ] {
        let html = format!("<html><head><link rel='stylesheet' href='{href}'/></head><body><p style='font-weight:bold'>Readable.</p></body></html>");
        let (parsed, dom) = imported(&html);
        assert_eq!(parsed.view_html, parse_html_document(&html).view_html);
        assert_eq!(
            parsed.source_formatting.unwrap().warnings,
            ["unread-stylesheet"]
        );
        assert_eq!(dom.select_first("p").unwrap().text_contents(), "Readable.");
        assert!(dom.select_first("b, i").is_err());
    }
}

#[test]
fn active_svg_styles_cannot_silently_override_inline_html_emphasis() {
    for sheet in [
        "<style>p{font-weight:normal!important}</style>",
        "<template><style>p{font-weight:normal!important}</style></template>",
        "<noscript><style>p{font-weight:normal!important}</style></noscript>",
    ] {
        let html = format!("<svg>{sheet}</svg><p id='p' style='font-weight:bold'>Words</p>");
        let (parsed, dom) = imported(&html);
        assert_eq!(parsed.view_html, parse_html_document(&html).view_html);
        assert_eq!(
            parsed.source_formatting.unwrap().warnings,
            ["unsupported-stylesheet"]
        );
        assert!(dom.select_first("#p b").is_err());
    }
}
