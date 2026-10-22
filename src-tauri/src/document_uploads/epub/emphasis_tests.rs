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
        r#"<html><body><aside id='n' role='doc-footnote'><p>Note <a role='doc-backlink' href='c0.xhtml#r'>↩</a></p></aside><p>Second.</p></body></html>"#,
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
fn unread_head_css_is_not_lost_when_extraction_discards_the_head() {
    for sheet in [
        "<style>.x{font-weight:normal!important}</style>",
        "<link rel='stylesheet' href='missing.css'/>",
        "<link rel='stylesheet' href='https://invalid.test/style.css'/>",
    ] {
        let parsed = parse_epub_document(&book(&[&format!("<html><head>{sheet}</head><body><p class='x' style='font-weight:bold'>Chapter.</p></body></html>")]), "Fallback").unwrap();
        let report = parsed.source_formatting.unwrap();
        assert_eq!(report.warnings, ["unread-stylesheet"]);
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
    let parsed = parse_epub_document(&book(&["<html><body><p><a id='point'/><span id='empty' style='font-weight:bold'/>Plain <span style='font-style:italic'>italic</span></p></body></html>"]), "Fallback").unwrap();
    let dom = parse_html().one(parsed.view_html).document_node;
    assert_eq!(dom.select_first("#ch0-point").unwrap().text_contents(), "");
    assert_eq!(dom.select_first("#ch0-empty").unwrap().text_contents(), "");
    assert!(dom.select_first("b").is_err());
    assert_eq!(dom.select_first("i").unwrap().text_contents(), "italic");
}

#[test]
fn literal_newlines_keep_the_existing_epub_import_roundtrip() {
    let prefix = "<html><body><pre>\n\n\nA  B\n C</pre>";
    let styled = format!("{prefix}<p style='font-weight:bold'>Words</p></body></html>");
    let plain = format!("{prefix}<p>Words</p></body></html>");
    let prepared = parse_epub_document(&book(&[&styled]), "Fallback").unwrap();
    let control = parse_epub_document(&book(&[&plain]), "Fallback").unwrap();
    assert_eq!(prepared.view_html, control.view_html);
    assert_eq!(
        prepared.source_formatting.unwrap().warnings,
        ["literal-roundtrip"]
    );
    assert!(control.source_formatting.unwrap().warnings.is_empty());
}

#[test]
fn declaration_budget_is_shared_across_chapters_and_exhaustion_keeps_import_readable() {
    let declarations = "color:red;".repeat(4095);
    let chapter = format!(
        "<html><body><p style='{declarations}font-weight:bold'>Readable.</p></body></html>"
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
