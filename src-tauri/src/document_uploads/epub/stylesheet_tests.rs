use super::*;
use kuchikiki::{parse_html, traits::TendrilSink, NodeRef};
use std::io::Write;
use zip::write::SimpleFileOptions;

/// Keep archive admission in the test: resources can be absent or deliberately undeclared.
fn book(chapters: &[String], resources: &[(&str, Option<&str>, Option<&str>)]) -> Vec<u8> {
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
            format!("<item id='c{i}' href='text/c{i}.xhtml' media-type='application/xhtml+xml'/>")
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
    let parsed = parse_epub_document(&book(chapters, resources), "Fallback").unwrap();
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
