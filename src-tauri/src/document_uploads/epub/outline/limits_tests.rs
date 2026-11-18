//! Exercise the optional-resource limits through the normal EPUB import path.
use super::{EPUB, NCX, XHTML};
use crate::document_uploads::epub::parse_epub_document;
use crate::document_uploads::parsed::ParsedDocument;
use std::io::{Cursor, Write};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

const LEAF: &str = r#"<li><a href="../text/notes.xhtml#ridge">Ridge</a></li>"#;

fn nav(items: &str, padding: &str) -> String {
    format!(r#"<nav xmlns="{XHTML}" xmlns:e="{EPUB}" e:type="toc"><ol>{items}</ol>{padding}</nav>"#)
}

/// Both resources are declared even when omitted so the tests exercise missing ZIP entries.
fn import(nav: Option<&str>, ncx: Option<&str>) -> ParsedDocument {
    let bytes = include_bytes!("../../../../../scripts/fixtures/epub/publisher-outline-3.epub");
    let mut fixture = ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut book = ZipWriter::new(Cursor::new(Vec::new()));
    for path in ["mimetype", "META-INF/container.xml", "OPS/text/notes.xhtml"] {
        book.raw_copy_file(fixture.by_name(path).unwrap()).unwrap();
    }
    let opf = r#"<package version="3.0"><manifest>
      <item id="nav" href="guide/list.xhtml" properties="nav" media-type="application/xhtml+xml"/>
      <item id="ncx" href="guide/list.ncx" media-type="application/x-dtbncx+xml"/>
      <item id="notes" href="text/notes.xhtml" media-type="application/xhtml+xml"/>
    </manifest><spine toc="ncx"><itemref idref="notes"/></spine></package>"#;
    for (path, text) in [
        ("OPS/book.opf", Some(opf)),
        ("OPS/guide/list.xhtml", nav),
        ("OPS/guide/list.ncx", ncx),
    ] {
        if let Some(text) = text {
            book.start_file(
                path,
                SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
            )
            .unwrap();
            book.write_all(text.as_bytes()).unwrap();
        }
    }
    let bytes = book.finish().unwrap().into_inner();
    let parsed = parse_epub_document(&bytes, "Fixture").unwrap();
    assert!(parsed.view_html.contains("Morning on the ridge"));
    assert!(parsed
        .sections
        .iter()
        .any(|s| s.text.contains("Follow the river.")));
    parsed
}

#[test]
fn decompressed_size_limit_falls_back_to_ncx() {
    let mut xml = nav(LEAF, "");
    xml.push_str(&" ".repeat(4 * 1024 * 1024 - xml.len()));
    let ncx = format!(
        r#"<ncx xmlns="{NCX}"><navMap><navPoint><navLabel><text>Fallback</text></navLabel><content src="../text/notes.xhtml#valley"/></navPoint></navMap></ncx>"#
    );
    for (expected_label, expected_target) in [("Ridge", "ch0-ridge"), ("Fallback", "ch0-valley")] {
        let parsed = import(Some(&xml), Some(&ncx));
        let [entry] = parsed.outline.as_slice() else {
            panic!("expected one destination")
        };
        assert_eq!(entry.label, expected_label);
        assert_eq!(entry.target.as_deref(), Some(expected_target));
        xml.push(' ');
    }
    assert!(import(Some(&xml), None).outline.is_empty());
}

#[test]
fn xml_node_limit_includes_nodes_outside_the_list() {
    // The document, nav, ol, li, a, and text account for six nodes.
    let mut padding = "<!--x-->".repeat(100_000 - 6);
    assert_eq!(import(Some(&nav(LEAF, &padding)), None).outline.len(), 1);
    padding.push_str("<!--x-->");
    assert!(import(Some(&nav(LEAF, &padding)), None).outline.is_empty());
}

#[test]
fn entry_limit_applies_across_sibling_groups() {
    let group = |count| format!("<li><span>Group</span><ol>{}</ol></li>", LEAF.repeat(count));
    let items = group(4_999) + &group(4_999);
    let parsed = import(Some(&nav(&items, "")), None);
    assert_eq!(parsed.outline.len(), 2);
    assert!(parsed.outline.iter().all(|g| g.children.len() == 4_999));
    assert!(import(Some(&nav(&(items + LEAF), "")), None)
        .outline
        .is_empty());
}

#[test]
fn nesting_limit_accepts_the_last_level() {
    let mut items = LEAF.to_owned();
    for _ in 1..32 {
        items = format!("<li><span>Group</span><ol>{items}</ol></li>");
    }
    let parsed = import(Some(&nav(&items, "")), None);
    let mut level = parsed.outline.as_slice();
    for _ in 1..32 {
        assert_eq!(level.len(), 1);
        level = &level[0].children;
    }
    assert_eq!(level[0].target.as_deref(), Some("ch0-ridge"));
    let deeper = format!("<li><span>Group</span><ol>{items}</ol></li>");
    assert!(import(Some(&nav(&deeper, "")), None).outline.is_empty());
}

#[test]
fn normalized_label_limit_counts_utf8_bytes() {
    let mut label = "é".repeat(2048);
    let xml = nav(&LEAF.replace("Ridge", &format!("   {label}   ")), "");
    assert_eq!(import(Some(&xml), None).outline[0].label, label);
    label.push('a');
    assert!(import(Some(&nav(&LEAF.replace("Ridge", &label), "")), None)
        .outline
        .is_empty());
}

#[test]
fn readable_chapter_survives_missing_or_malformed_navigation_resources() {
    for bad in [None, Some("<broken")] {
        assert!(import(bad, bad).outline.is_empty());
    }
}
