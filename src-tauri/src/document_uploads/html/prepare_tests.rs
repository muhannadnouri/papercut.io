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

#[test]
fn plaintext_roundtrip_falls_back_before_it_can_append_literal_closing_tags() {
    for literal in ["literal <stuff>", "Été عربيّ </body><b>literal</b>"] {
        let raw = format!("<p id='before' style='font-weight:bold'>Before</p><plaintext>{literal}");
        let prepared = prepare_html_document(&raw);
        let baseline = parse_html_document(&raw);
        assert_eq!(prepared.view_html, baseline.view_html, "{literal}");
        assert_eq!(
            prepared
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
        assert_eq!(
            prepared.source_formatting.unwrap().warnings,
            ["literal-roundtrip"]
        );
    }
    let plain = prepare_html_document("<p>Before</p><plaintext>literal <stuff>");
    assert!(plain.source_formatting.unwrap().warnings.is_empty());
}

#[test]
fn removed_address_container_does_not_silently_remove_supported_italics() {
    let raw = r##"<div id='outer'><address style='font-style:italic'><span id='own'>Été عربيّ</span><a id='r' role='doc-noteref' href='#n'><sup>1</sup></a></address><div style='font-style:italic'><address><span id='inherited'>Inherited</span><span id='reset' style='font-style:normal'> Plain</span></address></div><address style='font-style:normal'><span id='normal'>Normal</span></address><aside id='n' role='doc-footnote'>Note <a role='doc-backlink' href='#r'>↩</a></aside></div>"##;
    let prepared = prepare_html_document(raw);
    let dom = parse_html().one(prepared.view_html).document_node;
    let baseline = parse_html()
        .one(parse_html_document(raw).view_html)
        .document_node;
    assert_eq!(
        dom.select_first("#outer").unwrap().text_contents(),
        baseline.select_first("#outer").unwrap().text_contents()
    );
    for (id, text) in [("own", "Été عربيّ"), ("inherited", "Inherited")] {
        assert_eq!(
            dom.select_first(&format!("#{id} > i"))
                .unwrap()
                .text_contents(),
            text
        );
        assert_eq!(dom.select(&format!("#{id}")).unwrap().count(), 1);
    }
    for id in ["reset", "normal"] {
        assert!(dom.select_first(&format!("#{id} i")).is_err());
    }
    assert!(dom.select_first("address").is_err());
    assert!(dom
        .select_first("#r[role='doc-noteref'][href='#n'] > sup > i")
        .is_ok());
    assert!(dom
        .select_first("#n a[role='doc-backlink'][href='#r']")
        .is_ok());
    assert!(prepared.source_formatting.unwrap().warnings.is_empty());
}

#[test]
fn table_whitespace_stays_in_place_through_the_final_sanitizer() {
    let raw = "<div id='outer'>Before<table id='t' style='font-weight:bold'>\n \t<caption id='caption' style='font-style:italic'>Caption</caption>\n<colgroup id='cols'>\n<col>\t</colgroup>\n<thead id='head'>\n<tr id='hr'>\n<th>Heading</th>\n</tr>\n</thead>\n<tbody id='body'>\n<tr id='br'>\n<td id='cell'>Cell</td>\n</tr>\n</tbody>\n<tfoot id='foot'>\n<tr id='fr'>\n<td>Footer</td>\n</tr>\n</tfoot>\n</table>After</div>";
    let prepared = prepare_html_document(raw);
    let dom = parse_html().one(prepared.view_html).document_node;
    let baseline = parse_html()
        .one(parse_html_document(raw).view_html)
        .document_node;
    assert_eq!(
        dom.select_first("#outer").unwrap().text_contents(),
        baseline.select_first("#outer").unwrap().text_contents()
    );
    // The existing sanitizer removes tfoot; the HTML parser repairs its rows into tbody.
    for selector in [
        "#t",
        "#cols",
        "#head",
        "#hr",
        "#body",
        "#br",
        "#fr",
        "#t > tbody:last-child",
    ] {
        let actual = dom.select_first(selector).unwrap();
        let expected = baseline.select_first(selector).unwrap();
        let direct_text = |node: &kuchikiki::NodeRef| {
            node.children()
                .filter_map(|child| child.as_text().map(|text| text.borrow().to_string()))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            direct_text(actual.as_node()),
            direct_text(expected.as_node()),
            "{selector}"
        );
        assert!(
            actual.as_node().children().all(|child| child
                .as_element()
                .is_none_or(|element| !matches!(element.name.local.as_ref(), "b" | "i"))),
            "{selector}"
        );
    }
    assert_eq!(
        dom.select_first("#caption > i > b")
            .unwrap()
            .text_contents(),
        "Caption"
    );
    assert_eq!(
        dom.select_first("#cell > b").unwrap().text_contents(),
        "Cell"
    );
    assert!(prepared.source_formatting.unwrap().warnings.is_empty());
}

#[test]
fn non_ascii_table_text_is_not_mistaken_for_structural_whitespace() {
    let raw = "<div id='outer' style='font-style:italic'>Before<table id='t'>\u{a0}<tbody><tr><td>Cell</td></tr></tbody></table>After</div>";
    let prepared = prepare_html_document(raw);
    let dom = parse_html().one(prepared.view_html).document_node;
    let baseline = parse_html()
        .one(parse_html_document(raw).view_html)
        .document_node;
    assert_eq!(
        dom.select_first("#outer").unwrap().text_contents(),
        baseline.select_first("#outer").unwrap().text_contents()
    );
    assert!(dom
        .select("#outer > i")
        .unwrap()
        .any(|node| node.text_contents().contains('\u{a0}')));
    assert!(prepared.source_formatting.unwrap().warnings.is_empty());
}

#[test]
fn select_formatting_falls_back_without_changing_final_source_or_sections() {
    for selection in [
        "<select style='font-style:italic'><option>First</option></select>",
        "<select><optgroup style='font-weight:bold' label='Group'><option>First</option></optgroup></select>",
        "<select><option style='font-style:italic'>First</option></select>",
        "<div style='font-style:italic'><select><option>First</option></select></div>",
    ] {
        let raw = format!("<p id='before' style='font-weight:bold'>Before</p>{selection}<p id='after'>After</p>");
        let prepared = prepare_html_document(&raw);
        let baseline = parse_html_document(&raw);
        assert_eq!(prepared.view_html, baseline.view_html, "{selection}");
        assert_eq!(prepared.sections.iter().map(|s| (&s.text, &s.body_ranges)).collect::<Vec<_>>(), baseline.sections.iter().map(|s| (&s.text, &s.body_ranges)).collect::<Vec<_>>());
        assert_eq!(prepared.source_formatting.unwrap().warnings, ["unsupported-context"]);
    }
}

#[test]
fn select_context_without_a_needed_wrapper_does_not_warn_or_block_other_emphasis() {
    for selection in [
        "<select><option>First</option></select>",
        "<select style='font-style:normal;font-weight:normal'><option>First</option></select>",
        "<div style='font-style:italic'><select style='font-style:normal'><option>First</option></select></div>",
        "<select style='font-weight:bold'><option></option></select>",
    ] {
        let raw = format!("<p id='before' style='font-weight:bold'>Before</p>{selection}");
        let prepared = prepare_html_document(&raw);
        let dom = parse_html().one(prepared.view_html).document_node;
        assert_eq!(dom.select_first("#before > b").unwrap().text_contents(), "Before");
        assert!(prepared.source_formatting.unwrap().warnings.is_empty(), "{selection}");
    }
}
