use super::*;
use kuchikiki::{parse_html, traits::TendrilSink};

#[test]
fn extra_normalization_roundtrip_never_consumes_a_literal_leading_newline() {
    for tag in ["pre", "textarea", "listing"] {
        let raw = format!("<{tag}>\n\n\nA  B\n C</{tag}><p style='font-weight:bold'>Words</p>");
        let prepared = prepare_html_document(&raw);
        let legacy = parse_html_document(&raw);
        assert_eq!(
            prepared.source_formatting.unwrap().warnings,
            ["literal-roundtrip"]
        );
        assert_eq!(prepared.view_html, legacy.view_html, "{tag}");
        assert_eq!(
            prepared
                .sections
                .iter()
                .map(|s| &s.text)
                .collect::<Vec<_>>(),
            legacy.sections.iter().map(|s| &s.text).collect::<Vec<_>>()
        );
    }
    let raw = "<pre>\n\n\nA</pre><p>Unstyled</p>";
    assert!(prepare_html_document(raw)
        .source_formatting
        .unwrap()
        .warnings
        .is_empty());
}

#[test]
fn fresh_html_normalizes_before_the_same_sanitizer_and_section_extractor() {
    let html = r##"<html lang="fr"><head><title>Fixture</title></head>
    <body style="font-weight: bold"><p id="p">Été <span style="font-weight: normal; font-style: italic">calme</span>.
    <a id="r" role="doc-noteref" href="#n"><sup>1</sup></a><code translate="no">user_id</code></p>
    <aside id="n" role="doc-footnote"><p>Note <a role="doc-backlink" href="#r">↩</a></p></aside></body></html>"##;
    let parsed = prepare_html_document(html);
    let expected = parse_html_document(html);
    assert_eq!(parsed.title, expected.title);
    assert_eq!(
        parsed.sections.iter().map(|s| &s.text).collect::<Vec<_>>(),
        expected
            .sections
            .iter()
            .map(|s| &s.text)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        parsed
            .sections
            .iter()
            .map(|s| &s.body_ranges)
            .collect::<Vec<_>>(),
        expected
            .sections
            .iter()
            .map(|s| &s.body_ranges)
            .collect::<Vec<_>>()
    );
    let document = parse_html().one(parsed.view_html.clone()).document_node;
    assert_eq!(
        document.select_first("#p").unwrap().text_contents(),
        parse_html()
            .one(html)
            .document_node
            .select_first("#p")
            .unwrap()
            .text_contents()
    );
    assert_eq!(document.select("#p").unwrap().count(), 1);
    assert_eq!(
        document.select_first("#p > b").unwrap().text_contents(),
        "Été "
    );
    let calm = document.select_first("#p > span").unwrap();
    assert_eq!(calm.text_contents(), "calme");
    assert!(calm.as_node().select_first("i").is_ok());
    assert!(calm.as_node().select_first("b").is_err());
    assert!(document
        .select_first("a#r[role='doc-noteref'][href='#n'] > sup")
        .is_ok());
    assert!(document
        .select_first("#n a[role='doc-backlink'][href='#r']")
        .is_ok());
    assert_eq!(
        document
            .select_first("code[translate='no']")
            .unwrap()
            .text_contents(),
        "user_id"
    );
    assert_eq!(
        parsed.source_formatting.unwrap(),
        SourceFormatting {
            policy: POLICY.into(),
            warnings: vec![]
        }
    );
    assert!(!parsed.view_html.contains("style="));
    let safe = super::super::sanitize_html(&parsed.view_html);
    assert_eq!(super::super::sanitize_html(&safe), safe);
}

#[test]
fn unread_head_sheets_warn_and_do_not_guess_inline_equivalence() {
    for sheet in [
        "<style>.x { font-weight: normal !important }</style>",
        "<link rel='alternate stylesheet' href='https://invalid.test/style.css'>",
    ] {
        let parsed = prepare_html_document(&format!("<html><head>{sheet}</head><body><p class='x' style='font-weight:bold'>Readable</p></body></html>"));
        assert_eq!(
            parsed.source_formatting.unwrap().warnings,
            ["unread-stylesheet"]
        );
        assert!(!parsed.view_html.contains("<b>"));
        assert!(!parsed.view_html.contains("<style"));
        assert!(!parsed.view_html.contains("invalid.test"));
        assert_eq!(parsed.sections[0].text, "Readable");
    }
}

#[test]
fn normalization_never_bypasses_the_import_security_boundary() {
    let parsed = prepare_html_document(
        r#"<body style='font-style:italic' onload='alert(1)'>
        <p id='safe' onclick='alert(2)'>Safe <span style='font-weight:b\6f ld'>bold</span>
        <a href='java&#x73;cript:alert(3)'>link</a><img src='https://invalid.test/track'></p>
        <script>dangerous()</script><iframe src='file:///private'></iframe></body>"#,
    );
    assert!(parsed
        .source_formatting
        .as_ref()
        .unwrap()
        .warnings
        .is_empty());
    assert!(parsed.view_html.contains("<b>bold</b>"));
    for unsafe_text in [
        "onload",
        "onclick",
        "javascript:",
        "<script",
        "dangerous",
        "<iframe",
        "invalid.test",
        "file:",
        "style=",
    ] {
        assert!(!parsed.view_html.contains(unsafe_text), "{unsafe_text}");
    }
    let document = parse_html().one(parsed.view_html).document_node;
    assert!(document.select_first("#safe").is_ok());
}

#[test]
fn shared_reparse_does_not_apply_css_to_generated_or_restored_documents() {
    let raw = "<p style='font-weight:bold'>Generated text</p>";
    let imported = prepare_html_document(raw);
    let generated = parse_html_document(raw);
    assert!(imported.view_html.contains("<b>Generated text</b>"));
    assert!(!generated.view_html.contains("<b>"));
    assert!(generated.source_formatting.is_none());
    assert_eq!(
        parse_html_document(&imported.view_html).view_html,
        imported.view_html
    );
}
