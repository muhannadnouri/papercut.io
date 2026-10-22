use super::*;
use kuchikiki::{parse_html, traits::TendrilSink, NodeRef};
use std::io::Write;
use zip::write::SimpleFileOptions;

/// Keep archive admission in the test: resources can be absent or deliberately undeclared.
fn book(
    chapters: &[String],
    resources: &[(&str, Option<&str>, Option<&str>)],
    chapter_media_type: &str,
) -> Vec<u8> {
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    archive.start_file("mimetype", options).unwrap();
    archive.write_all(b"application/epub+zip").unwrap();
    archive.start_file(CONTAINER_PATH, options).unwrap();
    archive
        .write_all(
            b"<container><rootfiles><rootfile full-path='OPS/book.opf'/></rootfiles></container>",
        )
        .unwrap();
    let chapters_manifest: String = (0..chapters.len())
        .map(|i| {
            format!("<item id='c{i}' href='text/c{i}.xhtml' media-type='{chapter_media_type}'/>")
        })
        .collect();
    let resources_manifest: String = resources
        .iter()
        .enumerate()
        .filter_map(|(i, (path, mime, _))| {
            mime.map(|mime| format!("<item id='r{i}' href='{path}' media-type='{mime}'/>"))
        })
        .collect();
    let spine: String = (0..chapters.len())
        .map(|i| format!("<itemref idref='c{i}'/>"))
        .collect();
    archive.start_file("OPS/book.opf", options).unwrap();
    archive.write_all(format!("<package><metadata><title>Stylesheet fixture</title></metadata><manifest>{chapters_manifest}{resources_manifest}</manifest><spine>{spine}</spine></package>").as_bytes()).unwrap();
    for (i, chapter) in chapters.iter().enumerate() {
        archive
            .start_file(format!("OPS/text/c{i}.xhtml"), options)
            .unwrap();
        archive.write_all(chapter.as_bytes()).unwrap();
    }
    for (path, _, body) in resources {
        if let Some(body) = body {
            archive.start_file(format!("OPS/{path}"), options).unwrap();
            archive.write_all(body.as_bytes()).unwrap();
        }
    }
    archive.finish().unwrap().into_inner()
}

fn chapter(head: &str, body: &str) -> String {
    format!("<html><head>{head}</head><body>{body}</body></html>")
}

/// Assert after reader sanitization too, rather than accepting intermediate wrappers only.
fn imported(
    chapters: &[String],
    resources: &[(&str, Option<&str>, Option<&str>)],
) -> (ParsedDocument, NodeRef) {
    let parsed = parse_epub_document(
        &book(chapters, resources, "application/xhtml+xml"),
        "Fallback",
    )
    .unwrap();
    let safe = super::super::html::sanitize_html(&parsed.view_html);
    assert_eq!(super::super::html::sanitize_html(&safe), safe);
    let dom = parse_html().one(safe).document_node;
    (parsed, dom)
}

#[test]
fn linked_and_embedded_sheets_follow_document_order_without_losing_notes_or_assets() {
    let chapters = [
        chapter("<link rel='stylesheet' href='../styles/early.css'/><style>.x{font-weight:normal;font-style:italic}.embedded{font-style:italic}</style><link rel='stylesheet' href='../styles/late.css'/>",
            "<p id='first' class='x'>Première <a id='r' role='doc-noteref' href='c1.xhtml#n'><sup>1</sup></a></p><p id='second' class='embedded'>Second</p><img src='../images/diagram.svg' alt='Diagram'/><code translate='no'>user_id</code>"),
        chapter("", "<aside id='n' role='doc-footnote'>Note <a role='doc-backlink' href='c0.xhtml#r'>↩</a></aside>"),
    ];
    let (parsed, dom) = imported(
        &chapters,
        &[
            (
                "styles/early.css",
                Some("text/css"),
                Some(".x{font-weight:bold}"),
            ),
            (
                "styles/late.css",
                Some("text/css"),
                Some(".x{font-style:normal;font-weight:bold}"),
            ),
            (
                "images/diagram.svg",
                Some("image/svg+xml"),
                Some("<svg xmlns='http://www.w3.org/2000/svg'><path d='M0 0'/></svg>"),
            ),
        ],
    );
    assert_eq!(
        dom.select_first("#ch0-first > b").unwrap().text_contents(),
        "Première "
    );
    assert!(dom.select_first("#ch0-first i").is_err());
    assert_eq!(
        dom.select_first("#ch0-second > i").unwrap().text_contents(),
        "Second"
    );
    assert!(dom
        .select_first("#ch0-r[role='doc-noteref'][href='#ch1-n'] > sup > b")
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
    assert_eq!(parsed.assets.len(), 1);
    let image = dom.select_first("img[alt='Diagram']").unwrap();
    assert_eq!(
        image.attributes.borrow().get("data-papercut-asset"),
        Some(parsed.assets[0].file_name.as_str())
    );
    assert!(image.attributes.borrow().get("src").is_none());
    assert!(parsed.view_html.find("Première").unwrap() < parsed.view_html.find("Second").unwrap());
    assert!(parsed.source_formatting.unwrap().warnings.is_empty());
}

#[test]
fn a_chapters_embedded_rules_do_not_leak_into_the_next_chapter() {
    let (parsed, dom) = imported(
        &[
            chapter(
                "<style>.x{font-weight:bold}</style>",
                "<p id='p' class='x'>First</p>",
            ),
            chapter(
                "<style>.x{font-style:italic}</style>",
                "<p id='p' class='x'>Second</p>",
            ),
            chapter("", "<p id='p' class='x'>Third</p>"),
        ],
        &[],
    );
    assert_eq!(
        dom.select_first("#ch0-p > b").unwrap().text_contents(),
        "First"
    );
    assert!(dom.select_first("#ch0-p i").is_err());
    assert_eq!(
        dom.select_first("#ch1-p > i").unwrap().text_contents(),
        "Second"
    );
    assert!(dom.select_first("#ch1-p b").is_err());
    assert!(dom.select_first("#ch2-p b, #ch2-p i").is_err());
    assert!(parsed.source_formatting.unwrap().warnings.is_empty());
}

#[test]
fn manifest_css_is_resolved_relative_to_the_chapter_and_percent_decoded() {
    let (parsed, dom) = imported(
        &[chapter(
            "<link rel='stylesheet' href='../styles/book%20style.css'/>",
            "<p class='x'>Été عربيّ</p>",
        )],
        &[
            ("styles/book%20style.css", Some("text/css"), None),
            (
                "styles/book style.css",
                None,
                Some("p.x{font-weight:bold;font-style:italic}"),
            ),
        ],
    );
    assert_eq!(
        dom.select_first("p > i > b").unwrap().text_contents(),
        "Été عربيّ"
    );
    assert!(parsed.source_formatting.unwrap().warnings.is_empty());
}

#[test]
fn xhtml_uppercase_type_selectors_warn_without_reordering_source_or_stopping_later_chapters() {
    for selector in ["P", r"\50"] {
        let first = format!("<html xmlns='http://www.w3.org/1999/xhtml'><head><style>{selector}{{font-weight:bold}}</style></head><body><p id='p' style='font-style:italic'>First <strong>authored</strong></p></body></html>");
        let second = "<html xmlns='http://www.w3.org/1999/xhtml'><head><link rel='stylesheet' href='../styles/upper.css'/></head><body><p id='p' style='font-style:italic'>Second</p></body></html>".into();
        let third = "<html xmlns='http://www.w3.org/1999/xhtml'><head/><body><p id='p' style='font-style:italic'>Third</p></body></html>".into();
        let (parsed, dom) = imported(
            &[first, second, third],
            &[(
                "styles/upper.css",
                Some("text/css"),
                Some("P{font-weight:bold}"),
            )],
        );
        assert_eq!(
            parsed.source_formatting.unwrap().warnings,
            ["unsupported-document-mode"],
            "{selector}"
        );
        for (index, text) in [(0, "First authored"), (1, "Second")] {
            let paragraph = dom.select_first(&format!("#ch{index}-p")).unwrap();
            assert_eq!(paragraph.text_contents(), text);
            assert!(paragraph.as_node().select_first("b, i").is_err());
        }
        assert_eq!(
            dom.select_first("#ch0-p > strong").unwrap().text_contents(),
            "authored"
        );
        assert_eq!(
            dom.select_first("#ch2-p > i").unwrap().text_contents(),
            "Third"
        );
        let paragraphs: Vec<_> = dom
            .select(".epub-chapter p")
            .unwrap()
            .map(|paragraph| paragraph.text_contents())
            .collect();
        assert_eq!(paragraphs, ["First authored", "Second", "Third"]);
        assert!(dom.select_first("style, link, script").is_err());
    }
}

#[test]
fn xhtml_local_css_preserves_case_sensitive_class_and_id_matching_through_reader_sanitization() {
    let html = "<html xmlns='http://www.w3.org/1999/xhtml'><head><link rel='stylesheet' href='../styles/case.css'/></head><body><p id='Target' class='Emphasis'>Été عربيّ</p><p id='target' class='emphasis'>Plain</p></body></html>".into();
    let (parsed, dom) = imported(
        &[html],
        &[(
            "styles/case.css",
            Some("text/css"),
            Some(r"p.\45 mphasis{font-weight:bold} #\54 arget{font-style:italic}"),
        )],
    );
    assert!(parsed.source_formatting.unwrap().warnings.is_empty());
    assert_eq!(
        dom.select_first("#ch0-Target > i > b")
            .unwrap()
            .text_contents(),
        "Été عربيّ"
    );
    let plain = dom.select_first("#ch0-target").unwrap();
    assert_eq!(plain.text_contents(), "Plain");
    assert!(plain.as_node().select_first("b, i").is_err());
    let paragraphs: Vec<_> = dom
        .select(".epub-chapter p")
        .unwrap()
        .map(|paragraph| paragraph.text_contents())
        .collect();
    assert_eq!(paragraphs, ["Été عربيّ", "Plain"]);
    assert!(dom.select_first("style, link, script").is_err());
}

#[test]
fn an_explicit_html_manifest_item_uses_html_type_matching_even_with_an_xhtml_filename() {
    let html = "<!DOCTYPE html><html><head><link rel='stylesheet' href='../styles/case.css'/></head><body><p id='Target' class='Emphasis'>Été عربيّ</p><p id='target' class='emphasis'>Plain</p></body></html>".into();
    let bytes = book(
        &[html],
        &[(
            "styles/case.css",
            Some("text/css"),
            Some(r"P.\45 mphasis{font-weight:bold} #\54 arget{font-style:italic}"),
        )],
        "text/html",
    );
    let parsed = parse_epub_document(&bytes, "Fallback").unwrap();
    let safe = super::super::html::sanitize_html(&parsed.view_html);
    assert_eq!(super::super::html::sanitize_html(&safe), safe);
    let dom = parse_html().one(safe).document_node;
    assert!(parsed.source_formatting.unwrap().warnings.is_empty());
    assert_eq!(
        dom.select_first("#ch0-Target > i > b")
            .unwrap()
            .text_contents(),
        "Été عربيّ"
    );
    let plain = dom.select_first("#ch0-target").unwrap();
    assert_eq!(plain.text_contents(), "Plain");
    assert!(plain.as_node().select_first("b, i").is_err());
    let paragraphs: Vec<_> = dom
        .select(".epub-chapter p")
        .unwrap()
        .map(|paragraph| paragraph.text_contents())
        .collect();
    assert_eq!(paragraphs, ["Été عربيّ", "Plain"]);
    assert!(dom.select_first("style, link, script").is_err());
}

#[test]
fn unread_or_untrusted_stylesheet_links_keep_readable_source_with_incomplete_coverage() {
    for href in [
        "../styles/missing.css",
        "../styles/unlisted.css",
        "../styles/wrong-mime.css",
        "https://invalid.test/style.css",
        "//invalid.test/style.css",
        "file:///tmp/style.css",
        "/OPS/styles/valid.css",
        "../../../style.css",
        "../../../%2e%2e/style.css",
    ] {
        let (parsed, dom) = imported(
            &[chapter(
                &format!("<link rel='stylesheet' href='{href}'/>"),
                "<p id='p' style='font-weight:bold'>Readable text.</p>",
            )],
            &[
                ("styles/missing.css", Some("text/css"), None),
                ("styles/unlisted.css", None, Some("p{font-weight:bold}")),
                (
                    "styles/wrong-mime.css",
                    Some("application/octet-stream"),
                    Some("p{font-weight:bold}"),
                ),
                (
                    "styles/valid.css",
                    Some("text/css"),
                    Some("p{font-weight:bold}"),
                ),
            ],
        );
        assert_eq!(
            parsed.source_formatting.unwrap().warnings,
            ["unread-stylesheet"],
            "{href}"
        );
        assert_eq!(
            dom.select_first("#ch0-p").unwrap().text_contents(),
            "Readable text."
        );
        assert!(dom.select_first("b, i").is_err(), "{href}");
        assert!(!parsed.view_html.contains("invalid.test"));
        assert!(!parsed.view_html.contains("file:///"));
    }
}

#[test]
fn imports_in_embedded_or_local_css_are_rejected_without_importing_another_sheet() {
    for head in [
        "<style>@import '../styles/nested.css'; p{font-weight:bold}</style>",
        "<link rel='stylesheet' href='../styles/import.css'/>",
    ] {
        let (parsed, dom) = imported(
            &[chapter(head, "<p style='font-weight:bold'>Readable.</p>")],
            &[
                (
                    "styles/import.css",
                    Some("text/css"),
                    Some("@import url('nested.css'); p{font-weight:bold}"),
                ),
                (
                    "styles/nested.css",
                    Some("text/css"),
                    Some("p{font-style:italic}"),
                ),
            ],
        );
        assert_eq!(
            parsed.source_formatting.unwrap().warnings,
            ["unsupported-rule"]
        );
        assert!(dom.select_first("b, i").is_err());
        assert_eq!(dom.select_first("p").unwrap().text_contents(), "Readable.");
    }
}

#[test]
fn conditional_alternate_or_disabled_sheets_do_not_claim_complete_coverage() {
    for head in [
        "<style media='print'>p{font-weight:bold}</style>",
        "<link rel='stylesheet' media='print' href='../styles/book.css'/>",
        "<link rel='alternate stylesheet' title='Other' href='../styles/book.css'/>",
        "<link rel='stylesheet' disabled='' href='../styles/book.css'/>",
    ] {
        let (parsed, dom) = imported(
            &[chapter(head, "<p style='font-weight:bold'>Readable.</p>")],
            &[(
                "styles/book.css",
                Some("text/css"),
                Some("p{font-weight:bold}"),
            )],
        );
        assert_eq!(
            parsed.source_formatting.unwrap().warnings,
            ["unsupported-activation"],
            "{head}"
        );
        assert_eq!(dom.select_first("p").unwrap().text_contents(), "Readable.");
        assert!(dom.select_first("b, i").is_err());
    }
}

#[test]
fn one_cached_local_sheet_is_not_charged_again_for_each_spine_reference() {
    let chapters: Vec<_> = (0..40)
        .map(|i| {
            chapter(
                "<link rel='stylesheet' href='../styles/shared.css'/>",
                &format!("<p class='x'>Chapter {i}</p>"),
            )
        })
        .collect();
    let (parsed, dom) = imported(
        &chapters,
        &[(
            "styles/shared.css",
            Some("text/css"),
            Some(".x{font-weight:bold}"),
        )],
    );
    assert_eq!(dom.select(".epub-chapter p > b").unwrap().count(), 40);
    assert!(parsed.source_formatting.unwrap().warnings.is_empty());
}

#[test]
fn distinct_local_sheet_budget_is_shared_by_the_whole_spine() {
    let paths: Vec<_> = (0..34).map(|i| format!("styles/s{i}.css")).collect();
    let resources: Vec<_> = paths
        .iter()
        .map(|path| {
            (
                path.as_str(),
                Some("text/css"),
                Some(".x{font-weight:bold}"),
            )
        })
        .collect();
    let chapters: Vec<_> = (0..34)
        .map(|i| {
            chapter(
                &format!("<link rel='stylesheet' href='../styles/s{i}.css'/>"),
                &format!("<p class='x'>Chapter {i}</p>"),
            )
        })
        .collect();
    let (parsed, dom) = imported(&chapters, &resources);
    assert_eq!(dom.select(".epub-chapter p > b").unwrap().count(), 32);
    for index in [32, 33] {
        assert!(dom.select_first(&format!("#chapter-{index} b")).is_err());
        assert_eq!(
            dom.select_first(&format!("#chapter-{index} p"))
                .unwrap()
                .text_contents(),
            format!("Chapter {index}")
        );
    }
    assert_eq!(parsed.source_formatting.unwrap().warnings, ["sheet-limit"]);
}

#[test]
fn local_sheet_declarations_spend_one_import_wide_budget() {
    let css = format!(".x{{{}font-weight:bold}}", "color:red;".repeat(4095));
    let (parsed, dom) = imported(
        &[
            chapter(
                "<link rel='stylesheet' href='../styles/first.css'/>",
                "<p class='x'>First</p>",
            ),
            chapter(
                "<link rel='stylesheet' href='../styles/second.css'/>",
                "<p class='x'>Second</p>",
            ),
            chapter(
                "<link rel='stylesheet' href='../styles/third.css'/>",
                "<p class='x'>Third</p>",
            ),
            chapter("", "<p style='font-style:italic'>Fourth</p>"),
        ],
        &[
            ("styles/first.css", Some("text/css"), Some(&css)),
            ("styles/second.css", Some("text/css"), Some(&css)),
            (
                "styles/third.css",
                Some("text/css"),
                Some(".x{font-style:italic}"),
            ),
        ],
    );
    assert_eq!(
        dom.select_first("#chapter-0 b").unwrap().text_contents(),
        "First"
    );
    assert_eq!(
        dom.select_first("#chapter-1 b").unwrap().text_contents(),
        "Second"
    );
    for (index, text) in [(2, "Third"), (3, "Fourth")] {
        assert!(dom
            .select_first(&format!("#chapter-{index} b, #chapter-{index} i"))
            .is_err());
        assert_eq!(
            dom.select_first(&format!("#chapter-{index} p"))
                .unwrap()
                .text_contents(),
            text
        );
    }
    assert_eq!(
        parsed.source_formatting.unwrap().warnings,
        ["declaration-limit"]
    );
}

#[test]
fn selector_match_work_remains_bounded_across_chapters_even_for_a_cached_sheet() {
    let css = format!(
        "{}.target{{font-weight:bold}}",
        ".never{font-weight:bold}".repeat(999)
    );
    let body = "<p class='target'>Words.</p>".repeat(520);
    let chapters = [
        chapter(
            "<link rel='stylesheet' href='../styles/shared.css'/>",
            &body,
        ),
        chapter(
            "<link rel='stylesheet' href='../styles/shared.css'/>",
            &body,
        ),
    ];
    let (parsed, dom) = imported(
        &chapters,
        &[("styles/shared.css", Some("text/css"), Some(&css))],
    );
    assert_eq!(dom.select("#chapter-0 p > b").unwrap().count(), 520);
    assert!(dom.select_first("#chapter-1 b").is_err());
    assert_eq!(dom.select("#chapter-1 p").unwrap().count(), 520);
    assert_eq!(
        dom.select_first("#chapter-1").unwrap().text_contents(),
        "Words.".repeat(520)
    );
    assert_eq!(
        parsed.source_formatting.unwrap().warnings,
        ["node-work-limit"]
    );
}

#[test]
fn externalizing_inline_svg_keeps_the_global_source_stylesheet_warning() {
    for (header, svg) in [
        ("", "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 1 1'><style>p{font-weight:normal!important}</style><rect width='1' height='1'/></svg>"),
        ("", "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 1 1'><template><style>p{font-weight:normal!important}</style></template><rect width='1' height='1'/></svg>"),
        ("<?xml-stylesheet type='text/css' href='../styles/first.css'?>", ""),
    ] {
        let html = format!("{header}<html xmlns='http://www.w3.org/1999/xhtml'><head/><body>{svg}<p id='p' style='font-weight:bold'>Words</p></body></html>");
        let (parsed, dom) = imported(&[html], &[]);
        assert_eq!(parsed.source_formatting.unwrap().warnings, ["unsupported-stylesheet"]);
        assert!(dom.select_first("#ch0-p b").is_err());
        assert_eq!(dom.select_first("#ch0-p").unwrap().text_contents(), "Words");
        assert_eq!(parsed.assets.len(), usize::from(!svg.is_empty()));
    }
}

#[test]
fn extracted_svg_retains_html_stylesheet_link_evidence_through_final_sanitization() {
    for svg_body in [
        "<link xmlns='http://www.w3.org/1999/xhtml' rel='stylesheet' href='../styles/reset.css'/>",
        "<foreignObject><div xmlns='http://www.w3.org/1999/xhtml'><link rel='alternate STYLESHEET' media='print' href='../styles/reset.css'/></div></foreignObject>",
        "<template xmlns='urn:foreign'><link xmlns='http://www.w3.org/1999/xhtml' rel='stylesheet' href='../styles/reset.css'/></template>",
        "<template xmlns=''><link xmlns='http://www.w3.org/1999/xhtml' rel='stylesheet' href='../styles/reset.css'/></template>",
    ] {
        let html = format!("<html xmlns='http://www.w3.org/1999/xhtml'><head/><body><svg xmlns='http://www.w3.org/2000/svg' aria-label='Diagram'>{svg_body}<rect width='1' height='1'/></svg><p id='p' style='font-weight:bold'>Words <a id='r' href='#n' role='doc-noteref'><sup>1</sup></a></p><aside id='n' role='doc-footnote'>Note</aside></body></html>");
        let (parsed, dom) = imported(
            &[html],
            &[("styles/reset.css", Some("text/css"), Some("p{font-weight:normal!important}"))],
        );
        assert_eq!(parsed.source_formatting.unwrap().warnings, ["unsupported-stylesheet"]);
        assert!(dom.select_first("#ch0-p b, #ch0-p i").is_err());
        assert_eq!(dom.select_first("#ch0-p").unwrap().text_contents(), "Words 1");
        assert!(dom.select_first("#ch0-r[href='#ch0-n'][role='doc-noteref'] > sup").is_ok());
        assert_eq!(parsed.assets.len(), 1);
        assert!(dom.select_first("img[alt='Diagram']").is_ok());
        assert!(dom.select_first("svg, style, link, script").is_err());
    }
}

#[test]
fn html_chapter_svg_stylesheet_evidence_uses_html_name_and_integration_point_rules() {
    for svg_body in [
        "<foreignObject><LINK xmlns='http://www.w3.org/1999/xhtml' REL='stylesheet' HREF='../styles/reset.css'/></foreignObject>",
        "<foreignObject><link rel='stylesheet' href='../styles/reset.css'/></foreignObject>",
        "<STYLE>p{font-weight:normal!important}</STYLE>",
    ] {
        let html = format!("<!DOCTYPE html><html xmlns='http://www.w3.org/1999/xhtml'><head/><body><svg xmlns='http://www.w3.org/2000/svg' aria-label='Diagram'>{svg_body}<rect width='1' height='1'/></svg><p id='p' style='font-weight:bold;font-style:italic'>Words <strong>authored</strong> <a id='r' href='#n' role='doc-noteref'><sup>1</sup></a></p><aside id='n' role='doc-footnote'>Note <a role='doc-backlink' href='#r'>↩</a></aside></body></html>");
        let bytes = book(
            &[html],
            &[(
                "styles/reset.css",
                Some("text/css"),
                Some("p{font-weight:normal!important}"),
            )],
            "text/html",
        );
        let parsed = parse_epub_document(&bytes, "Fallback").unwrap();
        let safe = super::super::html::sanitize_html(&parsed.view_html);
        assert_eq!(super::super::html::sanitize_html(&safe), safe);
        let dom = parse_html().one(safe).document_node;
        assert_eq!(
            parsed.source_formatting.unwrap().warnings,
            ["unsupported-stylesheet"],
            "{svg_body}"
        );
        let paragraph = dom.select_first("#ch0-p").unwrap();
        assert_eq!(paragraph.text_contents(), "Words authored 1");
        assert!(paragraph.as_node().select_first("b, i").is_err());
        assert_eq!(
            dom.select_first("#ch0-p > strong").unwrap().text_contents(),
            "authored"
        );
        assert!(dom
            .select_first("#ch0-r[href='#ch0-n'][role='doc-noteref'] > sup")
            .is_ok());
        assert!(dom
            .select_first("#ch0-n a[role='doc-backlink'][href='#ch0-r']")
            .is_ok());
        assert!(parsed.view_html.find("Words").unwrap() < parsed.view_html.find("Note").unwrap());
        assert_eq!(parsed.assets.len(), 1, "{svg_body}: {}", parsed.view_html);
        let image = dom.select_first("img[alt='Diagram']").unwrap();
        assert_eq!(
            image.attributes.borrow().get("data-papercut-asset"),
            Some(parsed.assets[0].file_name.as_str())
        );
        assert!(dom.select_first("svg, style, link, script").is_err());
    }
}

#[test]
fn html_svg_preload_link_is_not_mistaken_for_an_active_stylesheet() {
    let html = "<!DOCTYPE html><html><head/><body><svg xmlns='http://www.w3.org/2000/svg' aria-label='Diagram'><foreignObject><LINK REL='preload' HREF='../styles/missing.css'/></foreignObject><rect width='1' height='1'/></svg><p id='p' style='font-weight:bold;font-style:italic'>Words</p></body></html>".into();
    let bytes = book(&[html], &[], "text/html");
    let parsed = parse_epub_document(&bytes, "Fallback").unwrap();
    let safe = super::super::html::sanitize_html(&parsed.view_html);
    assert_eq!(super::super::html::sanitize_html(&safe), safe);
    let dom = parse_html().one(safe).document_node;
    assert!(parsed.source_formatting.unwrap().warnings.is_empty());
    assert_eq!(
        dom.select_first("#ch0-p > i > b").unwrap().text_contents(),
        "Words"
    );
    assert_eq!(parsed.assets.len(), 1, "{}", parsed.view_html);
    assert!(dom.select_first("img[alt='Diagram']").is_ok());
    assert!(dom.select_first("svg, style, link, script").is_err());
}

#[test]
fn svg_non_stylesheet_links_and_genuine_html_inert_scopes_do_not_block_normalization() {
    for svg_body in [
        "<link xmlns='http://www.w3.org/1999/xhtml' rel='preload' as='style' href='../styles/missing.css'/>",
        "<link rel='stylesheet' href='../styles/missing.css'/>",
        "<link xmlns='urn:foreign' rel='stylesheet' href='../styles/missing.css'/>",
        "<template xmlns='http://www.w3.org/1999/xhtml'><link rel='stylesheet' href='../styles/missing.css'/></template>",
        "<foreignObject><noscript xmlns='http://www.w3.org/1999/xhtml'><link rel='stylesheet' href='../styles/missing.css'/></noscript></foreignObject>",
        "<script xmlns='http://www.w3.org/1999/xhtml'><link rel='stylesheet' href='../styles/missing.css'/></script>",
    ] {
        let html = format!("<html xmlns='http://www.w3.org/1999/xhtml'><head/><body><svg xmlns='http://www.w3.org/2000/svg' aria-label='Diagram'>{svg_body}<rect width='1' height='1'/></svg><p id='p' style='font-weight:bold'>Words</p></body></html>");
        let (parsed, dom) = imported(&[html], &[]);
        assert!(parsed.source_formatting.unwrap().warnings.is_empty(), "{svg_body}");
        assert_eq!(dom.select_first("#ch0-p > b").unwrap().text_contents(), "Words");
        assert_eq!(parsed.assets.len(), 1);
        assert!(dom.select_first("img[alt='Diagram']").is_ok());
        assert!(dom.select_first("svg, style, link, script").is_err());
    }
}

#[test]
fn xml_stylesheet_instruction_evidence_survives_entity_and_malformed_xml_fallbacks() {
    for (prologue, body) in [
        (
            "<!DOCTYPE html [<!ENTITY unused 'not expanded'>]>",
            "<p id='p' style='font-weight:bold'>Words</p>",
        ),
        ("", "<p id='p' style='font-weight:bold'>Words"),
    ] {
        let html = format!("<?xml-stylesheet href='https://example.invalid/unread.css' type='text/css'?>{prologue}<html xmlns='http://www.w3.org/1999/xhtml'><head/><body>{body}</body></html>");
        let (parsed, dom) = imported(&[html], &[]);
        assert_eq!(
            parsed.source_formatting.unwrap().warnings,
            ["unsupported-stylesheet"]
        );
        assert_eq!(dom.select_first("#ch0-p").unwrap().text_contents(), "Words");
        assert!(dom.select_first("#ch0-p b, #ch0-p i").is_err());
        assert!(dom.select_first("style, link, script").is_err());
        assert!(parsed.assets.is_empty());
    }
}

#[test]
fn xml_fallback_without_a_stylesheet_instruction_does_not_invent_its_warning() {
    for html in [
        "<!DOCTYPE html [<!ENTITY unused 'not expanded'>]><html xmlns='http://www.w3.org/1999/xhtml'><head/><body><p id='p'>Words</p></body></html>",
        "<html xmlns='http://www.w3.org/1999/xhtml'><head/><body><p id='p'>Words</body></html>",
        "<?xml-stylesheet-other href='unread.css'?><html xmlns='http://www.w3.org/1999/xhtml'><head/><body><p id='p'>Words</body></html>",
    ] {
        let (parsed, dom) = imported(&[html.into()], &[]);
        assert!(!parsed.source_formatting.unwrap().warnings.iter().any(|warning| warning == "unsupported-stylesheet"));
        assert_eq!(dom.select_first("#ch0-p").unwrap().text_contents(), "Words");
        assert!(parsed.assets.is_empty());
    }
}
