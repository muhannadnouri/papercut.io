use super::*;
use kuchikiki::{parse_html, traits::TendrilSink};
use std::io::Write;
use zip::write::SimpleFileOptions;

/// Build a minimal synthetic spine, avoiding external files and publisher-specific fixtures.
fn book(chapters: &[&str]) -> Vec<u8> {
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    archive.start_file("mimetype", options).unwrap();
    archive.write_all(b"application/epub+zip").unwrap();
    archive.start_file(CONTAINER_PATH, options).unwrap();
    archive
        .write_all(
            b"<container><rootfiles><rootfile full-path='book.opf'/></rootfiles></container>",
        )
        .unwrap();
    let manifest: String = (0..chapters.len())
        .map(|i| format!("<item id='c{i}' href='c{i}.xhtml' media-type='application/xhtml+xml'/>"))
        .collect();
    let spine: String = (0..chapters.len())
        .map(|i| format!("<itemref idref='c{i}'/>"))
        .collect();
    archive.start_file("book.opf", options).unwrap();
    archive.write_all(format!("<package><metadata><title>Inline fixture</title></metadata><manifest>{manifest}</manifest><spine>{spine}</spine></package>").as_bytes()).unwrap();
    for (i, chapter) in chapters.iter().enumerate() {
        archive.start_file(format!("c{i}.xhtml"), options).unwrap();
        archive.write_all(chapter.as_bytes()).unwrap();
    }
    archive.finish().unwrap().into_inner()
}

#[test]
fn full_chapter_ancestry_is_normalized_before_body_extraction_and_note_rewriting() {
    let parsed = parse_epub_document(&book(&[
        r#"<html style='font-style:italic'><head><title>Chapter</title></head><body style='font-weight:bold'><p id='p'>First <span style='font-weight:normal;font-style:normal'>plain</span><a id='r' role='doc-noteref' href='c1.xhtml#n'><sup>1</sup></a><code translate='no'>user_id</code></p></body></html>"#,
        r#"<html><head/><body><aside id='n' role='doc-footnote'><p>Note <a role='doc-backlink' href='c0.xhtml#r'>↩</a></p></aside><p>Second.</p></body></html>"#,
    ]), "Fallback").unwrap();
    let safe = super::super::html::sanitize_html(&parsed.view_html);
    let dom = parse_html().one(safe.clone()).document_node;
    assert_eq!(dom.select("#ch0-p").unwrap().count(), 1);
    assert_eq!(
        dom.select_first("#ch0-p > i > b").unwrap().text_contents(),
        "First "
    );
    let plain = dom.select_first("#ch0-p > span").unwrap();
    assert_eq!(plain.text_contents(), "plain");
    assert!(plain.as_node().select_first("b, i").is_err());
    assert!(dom
        .select_first("#ch0-r[role='doc-noteref'][href='#ch1-n'] > sup")
        .is_ok());
    assert!(dom
        .select_first("#ch1-n a[role='doc-backlink'][href='#ch0-r']")
        .is_ok());
    assert_eq!(
        dom.select_first("code[translate='no']")
            .unwrap()
            .text_contents(),
        "user_id"
    );
    assert!(safe.find("First ").unwrap() < safe.find("Second.").unwrap());
    assert_eq!(super::super::html::sanitize_html(&safe), safe);
    assert_eq!(
        parsed.source_formatting.unwrap().warnings,
        Vec::<String>::new()
    );
}

#[test]
fn head_css_is_resolved_or_reported_before_extraction_discards_the_head() {
    for (sheet, warning) in [
        ("<style>.x{font-weight:normal!important}</style>", None),
        (
            "<link rel='stylesheet' href='missing.css'/>",
            Some("unread-stylesheet"),
        ),
        (
            "<link rel='stylesheet' href='https://invalid.test/style.css'/>",
            Some("unread-stylesheet"),
        ),
    ] {
        let parsed = parse_epub_document(&book(&[&format!("<html><head>{sheet}</head><body><p class='x' style='font-weight:bold'>Chapter.</p></body></html>")]), "Fallback").unwrap();
        let report = parsed.source_formatting.unwrap();
        assert_eq!(
            report.warnings,
            warning.into_iter().map(str::to_owned).collect::<Vec<_>>()
        );
        assert!(!parsed.view_html.contains("<b>"));
        assert!(parsed.sections.iter().any(|s| s.text == "Chapter."));
        let safe = super::super::html::sanitize_html(&parsed.view_html);
        assert!(!safe.contains("invalid.test"));
        assert!(!safe.contains("missing.css"));
    }
}

#[test]
fn xml_chapter_metadata_survives_html_serialization_of_void_elements() {
    let parsed = parse_epub_document(&book(&[
        r#"<html lang='ar' dir='rtl' translate='no'><head><meta charset='utf-8'/></head><body lang='ar-SA' style='font-weight:bold'><p>نصّ<br/>ثانٍ</p></body></html>"#,
    ]), "Fallback").unwrap();
    let dom = parse_html().one(parsed.view_html).document_node;
    let chapter = dom.select_first("#chapter-0").unwrap();
    let attributes = chapter.attributes.borrow();
    assert_eq!(attributes.get("lang"), Some("ar-SA"));
    assert_eq!(attributes.get("dir"), Some("rtl"));
    assert_eq!(attributes.get("translate"), Some("no"));
    assert!(chapter.as_node().select_first("p > b").is_ok());
    assert_eq!(chapter.as_node().select("br").unwrap().count(), 1);
    assert!(parsed.source_formatting.unwrap().warnings.is_empty());
}

#[test]
fn xhtml_empty_nonvoid_elements_do_not_capture_following_formatted_text() {
    let parsed = parse_epub_document(&book(&["<html><head/><body><p><a id='point'/><span id='empty' style='font-weight:bold'/>Plain <span style='font-style:italic'>italic</span></p></body></html>"]), "Fallback").unwrap();
    let dom = parse_html().one(parsed.view_html).document_node;
    assert_eq!(dom.select_first("#ch0-point").unwrap().text_contents(), "");
    assert_eq!(dom.select_first("#ch0-empty").unwrap().text_contents(), "");
    assert!(dom.select_first("b").is_err());
    assert_eq!(dom.select_first("i").unwrap().text_contents(), "italic");
}

#[test]
fn literal_newlines_keep_the_existing_epub_import_roundtrip() {
    // XML keeps the first literal LF; HTML consumes it, so source-context admission now rejects first.
    let prefix = "<html><head/><body><pre>\n\n\nA  B\n C</pre>";
    let styled = format!("{prefix}<p style='font-weight:bold'>Words</p></body></html>");
    let plain = format!("{prefix}<p>Words</p></body></html>");
    let prepared = parse_epub_document(&book(&[&styled]), "Fallback").unwrap();
    let control = parse_epub_document(&book(&[&plain]), "Fallback").unwrap();
    assert_eq!(prepared.view_html, control.view_html);
    assert_eq!(
        prepared.source_formatting.unwrap().warnings,
        ["unsupported-document-mode"]
    );
    assert!(control.source_formatting.unwrap().warnings.is_empty());
}

#[test]
fn carriage_return_references_keep_the_existing_complete_epub_import() {
    for tag in ["textarea", "listing", "pre"] {
        for reference in ["&#13;", "&#xD;"] {
            for (head, style) in [
                ("", " style='font-weight:bold'"),
                ("<style>#other{font-style:italic}</style>", ""),
            ] {
                let styled = format!("<html><head>{head}</head><body><div id='outer'><p id='other'{style}>Before</p><{tag}>{reference}{reference}A  B\n C</{tag}></div></body></html>");
                let plain = styled.replace(style, "").replace(head, "");
                let last = "<html><head/><body><p>Last chapter.</p></body></html>";
                let prepared = parse_epub_document(&book(&[&styled, last]), "Fallback").unwrap();
                let baseline = parse_epub_document(&book(&[&plain, last]), "Fallback").unwrap();
                assert_eq!(prepared.view_html, baseline.view_html, "{tag}, {reference}");
                assert_eq!(
                    prepared.source_formatting.unwrap().warnings,
                    ["literal-roundtrip"],
                    "{tag}, {reference}, {head}"
                );
                let text = |html: &str| parse_html().one(html).document_node.text_contents();
                assert_eq!(text(&prepared.view_html), text(&baseline.view_html));
                assert_eq!(
                    prepared
                        .sections
                        .iter()
                        .map(|s| (&s.heading, &s.text, s.page_index, &s.body_ranges))
                        .collect::<Vec<_>>(),
                    baseline
                        .sections
                        .iter()
                        .map(|s| (&s.heading, &s.text, s.page_index, &s.body_ranges))
                        .collect::<Vec<_>>()
                );
                assert!(baseline.source_formatting.unwrap().warnings.is_empty());
            }
        }
    }
}

#[test]
fn interior_carriage_return_references_do_not_disable_epub_normalization() {
    for tag in ["pre", "textarea", "listing"] {
        for reference in ["&#13;", "&#xD;"] {
            let styled = format!("<html><head/><body><div id='outer'><p id='other' style='font-weight:bold'>Before</p><{tag}>Prefix {reference}Literal</{tag}></div></body></html>");
            let plain = styled.replace(" style='font-weight:bold'", "");
            let prepared = parse_epub_document(&book(&[&styled]), "Fallback").unwrap();
            let baseline = parse_epub_document(&book(&[&plain]), "Fallback").unwrap();
            assert!(
                prepared.source_formatting.unwrap().warnings.is_empty(),
                "{tag}, {reference}"
            );
            let dom = parse_html().one(prepared.view_html).document_node;
            assert_eq!(
                dom.select_first("#ch0-other > b").unwrap().text_contents(),
                "Before"
            );
            assert_eq!(
                dom.text_contents(),
                parse_html()
                    .one(baseline.view_html)
                    .document_node
                    .text_contents()
            );
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
        }
    }
}

#[test]
fn declaration_budget_is_shared_across_chapters_and_exhaustion_keeps_import_readable() {
    let declarations = "color:red;".repeat(4095);
    let chapter = format!(
        "<html><head/><body><p style='{declarations}font-weight:bold'>Readable.</p></body></html>"
    );
    let parsed = parse_epub_document(&book(&[&chapter, &chapter, &chapter]), "Fallback").unwrap();
    assert_eq!(
        parsed.source_formatting.unwrap().warnings,
        ["declaration-limit"]
    );
    let dom = parse_html().one(parsed.view_html).document_node;
    assert!(dom.select_first("#chapter-0 b").is_ok());
    assert!(dom.select_first("#chapter-1 b").is_ok());
    assert!(dom.select_first("#chapter-2 b").is_err());
    assert_eq!(
        dom.select_first("#chapter-2").unwrap().text_contents(),
        "Readable."
    );
    assert_eq!(dom.select(".epub-chapter").unwrap().count(), 3);
}

#[test]
fn plaintext_chapter_fallback_matches_the_existing_final_import_roundtrip() {
    // XML has a closing plaintext tag; HTML consumes it as text, so context mismatch rejects first.
    let styled = "<html><head/><body><p id='before' style='font-weight:bold'>Before</p><plaintext>literal &lt;stuff&gt;</plaintext></body></html>";
    let plain = styled.replace(" style='font-weight:bold'", "");
    let prepared = parse_epub_document(&book(&[styled]), "Fallback").unwrap();
    let baseline = parse_epub_document(&book(&[&plain]), "Fallback").unwrap();
    assert_eq!(prepared.view_html, baseline.view_html);
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
        ["unsupported-document-mode"]
    );
    assert!(baseline.source_formatting.unwrap().warnings.is_empty());
}

#[test]
fn address_italics_survive_chapter_sanitization_and_cross_chapter_note_rewriting() {
    let parsed = parse_epub_document(&book(&[
        "<html><head/><body><div id='outer'><address style='font-style:italic'><span id='own'>Été عربيّ</span><a id='r' role='doc-noteref' href='c1.xhtml#n'><sup>1</sup></a></address><div style='font-style:italic'><address><span id='inherited'>Inherited</span><span id='reset' style='font-style:normal'> Plain</span></address></div><address style='font-style:normal'><span id='normal'>Normal</span></address></div></body></html>",
        "<html><head/><body><aside id='n' role='doc-footnote'>Note <a role='doc-backlink' href='c0.xhtml#r'>↩</a></aside></body></html>",
    ]), "Fallback").unwrap();
    let safe = super::super::html::sanitize_html(&parsed.view_html);
    let dom = parse_html().one(safe).document_node;
    assert_eq!(
        dom.select_first("#ch0-outer").unwrap().text_contents(),
        "Été عربيّ1Inherited PlainNormal"
    );
    for (id, text) in [("own", "Été عربيّ"), ("inherited", "Inherited")] {
        assert_eq!(
            dom.select_first(&format!("#ch0-{id} > i"))
                .unwrap()
                .text_contents(),
            text
        );
        assert_eq!(dom.select(&format!("#ch0-{id}")).unwrap().count(), 1);
    }
    for id in ["reset", "normal"] {
        assert!(dom.select_first(&format!("#ch0-{id} i")).is_err());
    }
    assert!(dom.select_first("address").is_err());
    assert!(dom
        .select_first("#ch0-r[role='doc-noteref'][href='#ch1-n'] > sup > i")
        .is_ok());
    assert!(dom
        .select_first("#ch1-n a[role='doc-backlink'][href='#ch0-r']")
        .is_ok());
    assert!(parsed.source_formatting.unwrap().warnings.is_empty());
}

#[test]
fn table_structure_and_whitespace_survive_the_complete_epub_import() {
    let styled = "<html><head/><body><div id='outer'>Before<table id='t' style='font-weight:bold'>\n \t<caption id='caption' style='font-style:italic'>Caption</caption>\n<colgroup id='cols'>\n<col/>\t</colgroup>\n<thead id='head'>\n<tr id='hr'>\n<th>Heading</th>\n</tr>\n</thead>\n<tbody id='body'>\n<tr id='br'>\n<td id='cell'>Cell</td>\n</tr>\n</tbody>\n<tfoot id='foot'>\n<tr id='fr'>\n<td>Footer</td>\n</tr>\n</tfoot>\n</table>After</div></body></html>";
    let plain = styled
        .replace(" style='font-weight:bold'", "")
        .replace(" style='font-style:italic'", "");
    let prepared = parse_epub_document(&book(&[styled]), "Fallback").unwrap();
    let baseline = parse_epub_document(&book(&[&plain]), "Fallback").unwrap();
    let safe = super::super::html::sanitize_html(&prepared.view_html);
    let dom = parse_html().one(safe).document_node;
    let expected = parse_html()
        .one(super::super::html::sanitize_html(&baseline.view_html))
        .document_node;
    assert_eq!(
        dom.select_first("#ch0-outer").unwrap().text_contents(),
        expected.select_first("#ch0-outer").unwrap().text_contents()
    );
    // Retain the unchanged sanitizer's tfoot-to-tbody repair, including all whitespace.
    for selector in [
        "#ch0-t",
        "#ch0-cols",
        "#ch0-head",
        "#ch0-hr",
        "#ch0-body",
        "#ch0-br",
        "#ch0-fr",
        "#ch0-t > tbody:last-child",
    ] {
        let actual = dom.select_first(selector).unwrap();
        let expected = expected.select_first(selector).unwrap();
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
        dom.select_first("#ch0-caption > i > b")
            .unwrap()
            .text_contents(),
        "Caption"
    );
    assert_eq!(
        dom.select_first("#ch0-cell > b").unwrap().text_contents(),
        "Cell"
    );
    assert!(prepared.source_formatting.unwrap().warnings.is_empty());
}

#[test]
fn select_chapter_fallback_preserves_readable_source_and_does_not_warn_for_plain_options() {
    for selection in [
        "<select style='font-style:italic'><option>First</option></select>",
        "<select><optgroup style='font-weight:bold' label='Group'><option>First</option></optgroup></select>",
        "<select><option style='font-style:italic'>First</option></select>",
        "<div style='font-style:italic'><select><option>First</option></select></div>",
    ] {
        let styled = format!("<html><head/><body><p id='before' style='font-weight:bold'>Before</p>{selection}<p id='after'>After</p></body></html>");
        let plain = styled.replace(" style='font-weight:bold'", "").replace(" style='font-style:italic'", "");
        let prepared = parse_epub_document(&book(&[&styled]), "Fallback").unwrap();
        let baseline = parse_epub_document(&book(&[&plain]), "Fallback").unwrap();
        assert_eq!(prepared.view_html, baseline.view_html, "{selection}");
        assert_eq!(prepared.sections.iter().map(|s| (&s.text, &s.body_ranges)).collect::<Vec<_>>(), baseline.sections.iter().map(|s| (&s.text, &s.body_ranges)).collect::<Vec<_>>());
        assert_eq!(prepared.source_formatting.unwrap().warnings, ["unsupported-context"]);
        assert!(baseline.source_formatting.unwrap().warnings.is_empty());
    }
    for selection in [
        "<select><option>First</option></select>",
        "<div style='font-style:italic'><select style='font-style:normal'><option>First</option></select></div>",
    ] {
        let raw = format!("<html><head/><body><p id='before' style='font-weight:bold'>Before</p>{selection}</body></html>");
        let prepared = parse_epub_document(&book(&[&raw]), "Fallback").unwrap();
        let dom = parse_html().one(prepared.view_html).document_node;
        assert_eq!(dom.select_first("#ch0-before > b").unwrap().text_contents(), "Before");
        assert!(prepared.source_formatting.unwrap().warnings.is_empty());
    }
}
