//! EPUB import parser.
//!
//! Reads EPUB container metadata and OPF spine order, extracts spine XHTML into a
//! sanitized generated reading HTML document, then reuses the shared section
//! extraction path so search and TTS stay format-agnostic.

use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read};

use roxmltree::Document;
use zip::ZipArchive;

use super::html::{extract_body_inner, normalize_text, parsed_html_document, strip_tags};
use super::parsed::ParsedDocument;

mod assets;
mod paths;
mod render;
mod rewrite;

use assets::{load_image_assets, ManifestItem};
use paths::{opf_base_dir, resolve_archive_path};
use render::{render_chapter, render_reading_html};
use rewrite::{collect_fragment_anchors, rewrite_epub_fragment};

const CONTAINER_PATH: &str = "META-INF/container.xml";
const MAX_MIMETYPE_BYTES: u64 = 128;
const MAX_CONTAINER_XML_BYTES: u64 = 1024 * 1024;
const MAX_OPF_BYTES: u64 = 8 * 1024 * 1024;
const MAX_CHAPTER_TEXT_BYTES: u64 = 20 * 1024 * 1024;
const MAX_TOTAL_CHAPTER_TEXT_BYTES: u64 = 120 * 1024 * 1024;

/// Parse an EPUB archive into Papercut's format-neutral document shape.
///
/// EPUB import deliberately produces one sanitized generated HTML document instead
/// of retaining the raw archive. That keeps search, Find, and TTS highlight logic
/// on the same path as HTML uploads while leaving room for a richer viewer later.
pub(crate) fn parse_epub_document(
    bytes: &[u8],
    fallback_title: &str,
) -> Result<ParsedDocument, String> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|err| format!("EPUB is not a readable ZIP archive: {err}"))?;
    validate_mimetype(&mut archive)?;

    let container = read_zip_text_limited(&mut archive, CONTAINER_PATH, MAX_CONTAINER_XML_BYTES)?;
    let opf_path = parse_container_path(&container)?;
    let opf = read_zip_text_limited(&mut archive, &opf_path, MAX_OPF_BYTES)?;
    let package = parse_opf(&opf, &opf_path, fallback_title)?;

    if package.spine_paths.is_empty() {
        return Err("EPUB package has no readable spine items".into());
    }

    let image_assets = load_image_assets(&mut archive, &package.manifest);
    let chapter_indexes: HashMap<String, usize> = package
        .spine_paths
        .iter()
        .enumerate()
        .map(|(index, path)| (path.clone(), index))
        .collect();

    let mut chapter_drafts = Vec::new();
    let mut total_chapter_text_bytes = 0u64;
    for (index, chapter_path) in package.spine_paths.iter().enumerate() {
        let raw = read_zip_text_limited(&mut archive, chapter_path, MAX_CHAPTER_TEXT_BYTES)?;
        total_chapter_text_bytes = total_chapter_text_bytes
            .checked_add(raw.len() as u64)
            .ok_or_else(|| "EPUB chapter text is too large to import".to_string())?;
        if total_chapter_text_bytes > MAX_TOTAL_CHAPTER_TEXT_BYTES {
            return Err("EPUB chapter text is too large to import".into());
        }
        let body = extract_body_inner(&raw).unwrap_or(raw.as_str());
        let sanitized = sanitize_epub_fragment(body);
        if normalize_text(&strip_tags(&sanitized)).is_empty() {
            continue;
        }
        chapter_drafts.push(ChapterDraft {
            index,
            path: chapter_path.clone(),
            anchors: collect_fragment_anchors(&sanitized),
            sanitized,
        });
    }

    let readable_paths: HashSet<String> = chapter_drafts
        .iter()
        .map(|chapter| chapter.path.clone())
        .collect();
    let anchor_indexes: HashMap<String, HashSet<String>> = chapter_drafts
        .iter()
        .map(|chapter| (chapter.path.clone(), chapter.anchors.clone()))
        .collect();

    let mut chapters = Vec::new();
    for chapter in chapter_drafts {
        let rewritten = rewrite_epub_fragment(
            &chapter.sanitized,
            &chapter.path,
            chapter.index,
            &chapter_indexes,
            &readable_paths,
            &anchor_indexes,
            &image_assets,
        );
        chapters.push(render_chapter(chapter.index, &chapter.path, &rewritten));
    }

    if chapters.is_empty() {
        return Err("EPUB did not contain readable text".into());
    }

    let view_html = render_reading_html(&package.title, &chapters);
    let parsed = parsed_html_document(package.title, "epub", view_html);
    if parsed.sections.is_empty() {
        return Err("EPUB did not contain readable sections".into());
    }
    Ok(parsed)
}

struct ParsedPackage {
    title: String,
    spine_paths: Vec<String>,
    manifest: Vec<ManifestItem>,
}

struct ChapterDraft {
    index: usize,
    path: String,
    sanitized: String,
    anchors: HashSet<String>,
}

/// Verify the EPUB-required `mimetype` entry before trusting the ZIP contents.
///
/// This is a fast user-facing guard, not a full EPUB conformance check. Many
/// malformed ZIPs can still fail later with a more specific container/OPF error.
fn validate_mimetype<R: Read + std::io::Seek>(archive: &mut ZipArchive<R>) -> Result<(), String> {
    let value = match read_zip_text_limited(archive, "mimetype", MAX_MIMETYPE_BYTES) {
        Ok(value) => value,
        Err(err) if err.contains("is missing or unreadable") => {
            return Err("EPUB is missing required mimetype entry".into());
        }
        Err(err) => return Err(err),
    };
    if value.trim() != "application/epub+zip" {
        return Err("Selected file is not an EPUB archive".into());
    }
    Ok(())
}

/// Read `META-INF/container.xml` and locate the OPF package document.
///
/// The OPF path becomes the base for all manifest-relative hrefs.
fn parse_container_path(xml: &str) -> Result<String, String> {
    let doc = Document::parse(xml).map_err(|err| format!("Invalid EPUB container.xml: {err}"))?;
    let rootfile = doc
        .descendants()
        .find(|node| node.has_tag_name("rootfile"))
        .ok_or_else(|| "EPUB container is missing rootfile".to_string())?;
    rootfile
        .attribute("full-path")
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| "EPUB container rootfile is missing full-path".into())
}

/// Parse OPF metadata, manifest, and spine into normalized archive paths.
///
/// The spine controls reading order; ZIP entry order is ignored. Manifest hrefs
/// are resolved eagerly so later chapter/image/link code works with one canonical
/// archive-path form.
fn parse_opf(opf: &str, opf_path: &str, fallback_title: &str) -> Result<ParsedPackage, String> {
    let doc =
        Document::parse(opf).map_err(|err| format!("Invalid EPUB package document: {err}"))?;
    let base = opf_base_dir(opf_path);
    let title = doc
        .descendants()
        .find(|node| node.tag_name().name() == "title")
        .and_then(|node| node.text())
        .map(normalize_text)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback_title.to_string());

    let mut manifest_by_id = HashMap::new();
    let mut manifest = Vec::new();
    for item in doc
        .descendants()
        .filter(|node| node.tag_name().name() == "item")
    {
        let Some(id) = item.attribute("id") else {
            continue;
        };
        let Some(href) = item.attribute("href") else {
            continue;
        };
        let media_type = item.attribute("media-type").unwrap_or_default().to_string();
        let item = ManifestItem {
            href: resolve_archive_path(&base, href)?,
            media_type,
        };
        manifest_by_id.insert(id.to_string(), item.clone());
        manifest.push(item);
    }

    let mut spine_paths = Vec::new();
    for itemref in doc
        .descendants()
        .filter(|node| node.tag_name().name() == "itemref")
    {
        let Some(idref) = itemref.attribute("idref") else {
            continue;
        };
        let Some(item) = manifest_by_id.get(idref) else {
            continue;
        };
        if is_readable_spine_item(&item.media_type, &item.href) {
            spine_paths.push(item.href.clone());
        }
    }

    Ok(ParsedPackage {
        title,
        spine_paths,
        manifest,
    })
}

/// Decide whether a spine item is text-like enough for the generated reader.
///
/// EPUBs in the wild often have incomplete or inconsistent media types, so we
/// accept common XHTML/HTML extensions as a pragmatic fallback.
fn is_readable_spine_item(media_type: &str, href: &str) -> bool {
    matches!(media_type, "application/xhtml+xml" | "text/html")
        || href.ends_with(".xhtml")
        || href.ends_with(".html")
        || href.ends_with(".htm")
}

/// Read a ZIP member as UTF-8 text only when its decompressed size fits a cap.
///
/// The upload limit caps compressed EPUB bytes, not inflated ZIP entries. Checking
/// both declared and actual bytes keeps malformed EPUBs from expanding into an
/// unbounded allocation while still producing errors that name the failing entry.
fn read_zip_text_limited<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    path: &str,
    max_bytes: u64,
) -> Result<String, String> {
    let mut file = archive
        .by_name(path)
        .map_err(|err| format!("EPUB entry {path} is missing or unreadable: {err}"))?;
    if file.size() > max_bytes {
        return Err(format!("EPUB entry {path} exceeds the text size limit"));
    }

    let mut bytes = Vec::with_capacity(file.size().min(max_bytes) as usize);
    file.by_ref()
        .take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|err| format!("Failed to read EPUB entry {path}: {err}"))?;
    if bytes.len() as u64 > max_bytes {
        return Err(format!("EPUB entry {path} exceeds the text size limit"));
    }

    String::from_utf8(bytes)
        .map_err(|err| format!("EPUB entry {path} is not valid UTF-8 text: {err}"))
}

/// Sanitize one XHTML body fragment while preserving anchorable ids/names.
///
/// IDs are needed for TOC links, footnotes, and backlinks. We preserve them here
/// and prefix them later so duplicate ids across chapters cannot collide.
fn sanitize_epub_fragment(fragment: &str) -> String {
    let mut builder = ammonia::Builder::default();
    builder.add_generic_attributes(&["id", "name"]);
    builder
        .clean(&normalize_empty_pre_elements(fragment))
        .to_string()
}

/// Preserve XHTML's empty `<pre/>` meaning before parsing fragments as HTML.
///
/// Project Gutenberg EPUBs can include `<pre/>` as a separator artifact. HTML
/// parsers treat `pre` as non-void, so leaving that syntax alone can make the
/// following prose render inside a large code/preformatted block.
fn normalize_empty_pre_elements(fragment: &str) -> String {
    let lower = fragment.to_ascii_lowercase();
    let mut out = String::with_capacity(fragment.len());
    let mut pos = 0usize;

    while let Some(start_rel) = lower[pos..].find("<pre") {
        let start = pos + start_rel;
        out.push_str(&fragment[pos..start]);

        let after_name = start + "<pre".len();
        if !is_tag_name_boundary(fragment[after_name..].chars().next()) {
            out.push('<');
            pos = start + 1;
            continue;
        }

        let Some(end_rel) = lower[start..].find('>') else {
            out.push_str(&fragment[start..]);
            return out;
        };
        let end = start + end_rel;
        let tag_inner = fragment[start + 1..end].trim_end();
        if let Some(open_inner) = tag_inner.strip_suffix('/') {
            out.push('<');
            out.push_str(open_inner.trim_end());
            out.push_str("></pre>");
        } else {
            out.push_str(&fragment[start..=end]);
        }
        pos = end + 1;
    }

    out.push_str(&fragment[pos..]);
    out
}

fn is_tag_name_boundary(ch: Option<char>) -> bool {
    matches!(ch, Some('>' | '/' | ' ' | '\t' | '\n' | '\r' | '\u{000C}'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::FileOptions;
    use zip::CompressionMethod;

    #[test]
    fn resolves_manifest_paths_against_opf_dir() {
        assert_eq!(
            resolve_archive_path("OPS", "chapter.xhtml").unwrap(),
            "OPS/chapter.xhtml"
        );
        assert_eq!(
            resolve_archive_path("OPS/text", "../chapter.xhtml").unwrap(),
            "OPS/chapter.xhtml"
        );
        assert_eq!(
            resolve_archive_path("", "text/chapter%201.xhtml#frag").unwrap(),
            "text/chapter 1.xhtml"
        );
    }

    #[test]
    fn rejects_paths_that_escape_root() {
        assert!(resolve_archive_path("", "../chapter.xhtml").is_err());
    }

    #[test]
    fn rejects_text_zip_entries_over_limit() {
        let mut archive = Vec::new();
        {
            let cursor = Cursor::new(&mut archive);
            let mut zip = zip::ZipWriter::new(cursor);
            let deflated = FileOptions::default().compression_method(CompressionMethod::Deflated);
            zip.start_file("OPS/text/chapter.xhtml", deflated).unwrap();
            zip.write_all(b"123456789").unwrap();
            zip.finish().unwrap();
        }

        let mut zip = ZipArchive::new(Cursor::new(archive)).unwrap();
        let err = read_zip_text_limited(&mut zip, "OPS/text/chapter.xhtml", 8).unwrap_err();
        assert!(err.contains("exceeds the text size limit"));
    }

    #[test]
    fn normalizes_empty_xhtml_pre_before_html_parsing() {
        let normalized = normalize_empty_pre_elements("<pre/><p>After</p>");

        assert_eq!(normalized, "<pre></pre><p>After</p>");
    }

    #[test]
    fn normalizes_empty_xhtml_pre_with_attributes() {
        let normalized = normalize_empty_pre_elements("<pre id=\"pg\" />Text");

        assert_eq!(normalized, "<pre id=\"pg\"></pre>Text");
    }

    #[test]
    fn leaves_non_pre_and_non_empty_pre_tags_alone() {
        assert_eq!(
            normalize_empty_pre_elements("<prefix/><pre>Code</pre>"),
            "<prefix/><pre>Code</pre>"
        );
    }

    #[test]
    fn epub_fixture_rewrites_toc_cross_chapter_links_and_images() {
        let bytes = fixture_epub();
        let parsed = parse_epub_document(&bytes, "Fallback").unwrap();
        assert!(parsed.view_html.contains("href=\"#ch1-start\""));
        assert!(parsed.view_html.contains("href=\"#chapter-2\""));
        assert!(parsed.view_html.contains("id=\"ch1-start\""));
        assert!(parsed.view_html.contains("src=\"data:image/png;base64,"));
        assert!(parsed.view_html.contains("alt=\"Cover\""));
        assert!(!parsed.view_html.contains("OPS/text/chapter1.xhtml#start"));
        assert!(!parsed.view_html.contains("../images/cover.png"));
    }

    #[test]
    fn epub2_fixture_rewrites_toc_footnotes_backlinks_and_missing_fragments() {
        let bytes = epub2_footnote_fixture();
        let parsed = parse_epub_document(&bytes, "Fallback").unwrap();
        assert!(parsed.view_html.contains("href=\"#chapter-1\""));
        assert!(parsed
            .view_html
            .contains("href=\"#ch2-uHnykLWY4y94vVftvHPPheF\""));
        assert!(parsed
            .view_html
            .contains("id=\"ch2-uHnykLWY4y94vVftvHPPheF\""));
        assert!(parsed.view_html.contains("href=\"#ch1-note2ref\""));
        assert!(parsed.view_html.contains("id=\"ch1-note2ref\""));
        assert!(parsed.view_html.contains("href=\"#chapter-2\""));
        assert!(!parsed.view_html.contains("notes.htm#"));
        assert!(!parsed.view_html.contains("ch01.htm#"));
    }

    #[test]
    fn epub_fixture_sections_follow_generated_reader_text() {
        let parsed = parse_epub_document(&fixture_epub(), "Fallback").unwrap();
        assert_eq!(parsed.format, "epub");
        assert_eq!(parsed.title, "Fixture");
        let text = parsed
            .sections
            .iter()
            .map(|section| section.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Start"));
        assert!(text.contains("First chapter."));
        assert!(text.contains("Second chapter."));
    }

    #[test]
    fn epub_fixture_drops_active_and_remote_content() {
        let parsed = parse_epub_document(&hostile_epub_fixture(), "Fallback").unwrap();
        assert!(parsed.view_html.contains("Safe text"));
        assert!(!parsed.view_html.contains("<script"));
        assert!(!parsed.view_html.contains("onclick"));
        assert!(!parsed.view_html.contains("javascript:"));
        assert!(!parsed.view_html.contains("https://example.com"));
        assert!(!parsed.view_html.contains("data:text/html"));
    }

    #[test]
    fn epub_fixture_rejects_empty_spine_text() {
        let mut archive = Vec::new();
        {
            let cursor = Cursor::new(&mut archive);
            let mut zip = zip::ZipWriter::new(cursor);
            let stored = FileOptions::default().compression_method(CompressionMethod::Stored);
            let deflated = FileOptions::default().compression_method(CompressionMethod::Deflated);
            zip.start_file("mimetype", stored).unwrap();
            zip.write_all(b"application/epub+zip").unwrap();
            zip.start_file(CONTAINER_PATH, deflated).unwrap();
            zip.write_all(container_xml().as_bytes()).unwrap();
            zip.start_file("OPS/package.opf", deflated).unwrap();
            zip.write_all(opf_xml().as_bytes()).unwrap();
            zip.start_file("OPS/nav.xhtml", deflated).unwrap();
            zip.write_all(b"<html><body></body></html>").unwrap();
            zip.start_file("OPS/text/chapter1.xhtml", deflated).unwrap();
            zip.write_all(b"<html><body><img src='../images/cover.png'/></body></html>")
                .unwrap();
            zip.start_file("OPS/text/chapter2.xhtml", deflated).unwrap();
            zip.write_all(b"<html><body></body></html>").unwrap();
            zip.finish().unwrap();
        }
        assert!(parse_epub_document(&archive, "Fallback").is_err());
    }

    /// Build a compact EPUB 2-style archive with TOC links, footnotes, and backlinks.
    ///
    /// Mirrors the older Calibre/MIA pattern from the Engels sample: flat archive
    /// paths, `.htm` spine items, and notes stored in a final `notes.htm` chapter.
    fn epub2_footnote_fixture() -> Vec<u8> {
        let mut archive = Vec::new();
        {
            let cursor = Cursor::new(&mut archive);
            let mut zip = zip::ZipWriter::new(cursor);
            let stored = FileOptions::default().compression_method(CompressionMethod::Stored);
            let deflated = FileOptions::default().compression_method(CompressionMethod::Deflated);
            zip.start_file("mimetype", stored).unwrap();
            zip.write_all(b"application/epub+zip").unwrap();
            zip.start_file(CONTAINER_PATH, deflated).unwrap();
            zip.write_all(flat_container_xml().as_bytes()).unwrap();
            zip.start_file("content.opf", deflated).unwrap();
            zip.write_all(epub2_opf_xml().as_bytes()).unwrap();
            zip.start_file("index_split1.htm", deflated).unwrap();
            zip.write_all(
                b"<html><body><h3>Contents</h3><p><a href='ch01.htm'>Chapter 1</a></p></body></html>",
            )
            .unwrap();
            zip.start_file("ch01.htm", deflated).unwrap();
            zip.write_all(
                b"<html><body><h1 id='calibre_toc_2'>Chapter 1</h1><p>Louis XI<sup><a href='notes.htm#uHnykLWY4y94vVftvHPPheF' id='note2ref'>[2]</a></sup> and <a href='notes.htm#missing'>Missing</a>.</p></body></html>",
            )
            .unwrap();
            zip.start_file("notes.htm", deflated).unwrap();
            zip.write_all(
                b"<html><body><h1 id='calibre_pb_0'>Notes</h1><p><a href='ch01.htm#note2ref' id='uHnykLWY4y94vVftvHPPheF'>2.</a> Louis XI note.</p></body></html>",
            )
            .unwrap();
            zip.finish().unwrap();
        }
        archive
    }

    /// Build a compact archive with content that must not survive sanitization.
    fn hostile_epub_fixture() -> Vec<u8> {
        let mut archive = Vec::new();
        {
            let cursor = Cursor::new(&mut archive);
            let mut zip = zip::ZipWriter::new(cursor);
            let stored = FileOptions::default().compression_method(CompressionMethod::Stored);
            let deflated = FileOptions::default().compression_method(CompressionMethod::Deflated);
            zip.start_file("mimetype", stored).unwrap();
            zip.write_all(b"application/epub+zip").unwrap();
            zip.start_file(CONTAINER_PATH, deflated).unwrap();
            zip.write_all(container_xml().as_bytes()).unwrap();
            zip.start_file("OPS/package.opf", deflated).unwrap();
            zip.write_all(single_chapter_opf_xml().as_bytes()).unwrap();
            zip.start_file("OPS/text/chapter1.xhtml", deflated).unwrap();
            zip.write_all(
                b"<html><body><h1 id='start'>Safe</h1><p onclick='x()'>Safe text <a href='javascript:alert(1)'>bad</a><a href='https://example.com'>remote</a><img src='https://example.com/a.png'/><img src='data:text/html;base64,abc'/></p><script>alert(1)</script></body></html>",
            )
            .unwrap();
            zip.finish().unwrap();
        }
        archive
    }

    /// Build a minimal nested-path EPUB fixture with a local manifest image.
    fn fixture_epub() -> Vec<u8> {
        let mut archive = Vec::new();
        {
            let cursor = Cursor::new(&mut archive);
            let mut zip = zip::ZipWriter::new(cursor);
            let stored = FileOptions::default().compression_method(CompressionMethod::Stored);
            let deflated = FileOptions::default().compression_method(CompressionMethod::Deflated);
            zip.start_file("mimetype", stored).unwrap();
            zip.write_all(b"application/epub+zip").unwrap();
            zip.start_file(CONTAINER_PATH, deflated).unwrap();
            zip.write_all(container_xml().as_bytes()).unwrap();
            zip.start_file("OPS/package.opf", deflated).unwrap();
            zip.write_all(opf_xml().as_bytes()).unwrap();
            zip.start_file("OPS/nav.xhtml", deflated).unwrap();
            zip.write_all(
                b"<html><body><nav><a href='text/chapter1.xhtml#start'>Start</a><a href='text/chapter2.xhtml'>Next</a></nav></body></html>",
            )
            .unwrap();
            zip.start_file("OPS/text/chapter1.xhtml", deflated).unwrap();
            zip.write_all(
                b"<html><body><h1 id='start'>Start</h1><p>First chapter.</p><img src='../images/cover.png' alt='Cover'/></body></html>",
            )
            .unwrap();
            zip.start_file("OPS/text/chapter2.xhtml", deflated).unwrap();
            zip.write_all(b"<html><body><p>Second chapter.</p></body></html>")
                .unwrap();
            zip.start_file("OPS/images/cover.png", deflated).unwrap();
            zip.write_all(b"\x89PNG\r\n\x1a\nimage").unwrap();
            zip.finish().unwrap();
        }
        archive
    }

    fn flat_container_xml() -> &'static str {
        r#"<?xml version="1.0"?><container><rootfiles><rootfile full-path="content.opf"/></rootfiles></container>"#
    }

    fn epub2_opf_xml() -> &'static str {
        r#"<?xml version="1.0"?><package><metadata><title>EPUB 2 Fixture</title></metadata><manifest><item id="toc" href="index_split1.htm" media-type="application/xhtml+xml"/><item id="c1" href="ch01.htm" media-type="application/xhtml+xml"/><item id="notes" href="notes.htm" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="toc"/><itemref idref="c1"/><itemref idref="notes"/></spine></package>"#
    }

    fn container_xml() -> &'static str {
        r#"<?xml version="1.0"?><container><rootfiles><rootfile full-path="OPS/package.opf"/></rootfiles></container>"#
    }

    fn single_chapter_opf_xml() -> &'static str {
        r#"<?xml version="1.0"?><package><metadata><title>Hostile Fixture</title></metadata><manifest><item id="c1" href="text/chapter1.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="c1"/></spine></package>"#
    }

    fn opf_xml() -> &'static str {
        r#"<?xml version="1.0"?><package><metadata><title>Fixture</title></metadata><manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml"/><item id="c1" href="text/chapter1.xhtml" media-type="application/xhtml+xml"/><item id="c2" href="text/chapter2.xhtml" media-type="application/xhtml+xml"/><item id="img" href="images/cover.png" media-type="image/png"/></manifest><spine><itemref idref="nav"/><itemref idref="c1"/><itemref idref="c2"/></spine></package>"#
    }
}
