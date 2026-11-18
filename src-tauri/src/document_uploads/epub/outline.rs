//! Publisher navigation is optional and refers to paths relative to the nav or NCX file.
use std::io::{Read, Seek};

use roxmltree::{Document, Node, ParsingOptions};
use zip::ZipArchive;

use super::super::parsed::OutlineEntry;
use super::rewrite::{resolve_target, ChapterTargets};
use super::{normalize_text, read_zip_text_limited, ParsedPackage};

pub(super) const MAX_ENTRIES: usize = 10_000;
pub(super) const MAX_DEPTH: usize = 32;
pub(super) const MAX_LABEL_BYTES: usize = 4096;

const XHTML: &str = "http://www.w3.org/1999/xhtml";
const EPUB: &str = "http://www.idpf.org/2007/ops";
const NCX: &str = "http://www.daisy.org/z3986/2005/ncx/";

/// Prefer EPUB 3 navigation, then try the declared NCX. An empty result lets import continue.
pub(super) fn load<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    package: &ParsedPackage,
    targets: &ChapterTargets,
) -> Vec<OutlineEntry> {
    for (path, format) in [(&package.nav, Format::Nav), (&package.ncx, Format::Ncx)] {
        let Some(path) = path else { continue };
        let Ok(xml) = read_zip_text_limited(archive, path, 4 * 1024 * 1024) else {
            continue;
        };
        if let Some(entries) = format.parse(&xml, path, targets) {
            if !entries.is_empty() {
                return entries;
            }
        }
    }
    Vec::new()
}

#[derive(Clone, Copy)]
enum Format {
    Nav,
    Ncx,
}

impl Format {
    /// Read the contents hierarchy from the EPUB 3 TOC nav or EPUB 2 NCX navMap.
    fn parse(self, xml: &str, path: &str, targets: &ChapterTargets) -> Option<Vec<OutlineEntry>> {
        // Accept common XHTML/NCX doctypes without enabling publisher-defined entities.
        if xml.contains("<!ENTITY") {
            return None;
        }
        let doc = Document::parse_with_options(
            xml,
            ParsingOptions {
                allow_dtd: true,
                nodes_limit: 100_000,
                ..Default::default()
            },
        )
        .ok()?;
        let list = match self {
            Self::Nav => doc
                .descendants()
                .find(|n| {
                    n.has_tag_name((XHTML, "nav"))
                        && n.attribute((EPUB, "type"))
                            .unwrap_or_default()
                            .split_ascii_whitespace()
                            .any(|t| t == "toc")
                })?
                .children()
                .find(|n| n.has_tag_name((XHTML, "ol")))?,
            Self::Ncx => doc
                .root_element()
                .children()
                .find(|n| n.has_tag_name((NCX, "navMap")))?,
        };
        let mut remaining = MAX_ENTRIES;
        self.entries(list, path, targets, 1, &mut remaining)
    }

    /// Keep reachable children under broken links. Exceeding a limit discards this resource's entire tree.
    fn entries(
        self,
        list: Node,
        path: &str,
        targets: &ChapterTargets,
        depth: usize,
        remaining: &mut usize,
    ) -> Option<Vec<OutlineEntry>> {
        let mut entries = Vec::new();
        let tag = match self {
            Self::Nav => (XHTML, "li"),
            Self::Ncx => (NCX, "navPoint"),
        };
        for item in list.children().filter(|n| n.has_tag_name(tag)) {
            if depth > MAX_DEPTH {
                return None;
            }
            *remaining = remaining.checked_sub(1)?;
            let (label, href, nested) = match self {
                Self::Nav => {
                    let label = item
                        .children()
                        .find(|n| n.has_tag_name((XHTML, "a")) || n.has_tag_name((XHTML, "span")));
                    (
                        label,
                        label
                            .filter(|n| n.has_tag_name((XHTML, "a")))
                            .and_then(|n| n.attribute("href")),
                        item.children().find(|n| n.has_tag_name((XHTML, "ol"))),
                    )
                }
                Self::Ncx => (
                    item.children()
                        .find(|n| n.has_tag_name((NCX, "navLabel")))
                        .and_then(|n| n.children().find(|n| n.has_tag_name((NCX, "text")))),
                    item.children()
                        .find(|n| n.has_tag_name((NCX, "content")))
                        .and_then(|n| n.attribute("src")),
                    Some(item),
                ),
            };
            let children = match nested {
                Some(list) => self.entries(list, path, targets, depth + 1, remaining)?,
                None => Vec::new(),
            };
            let text: String = label
                .into_iter()
                .flat_map(|n| n.descendants())
                .filter_map(|n| {
                    if n.is_text() {
                        n.text()
                    } else if n.has_tag_name((XHTML, "img")) {
                        n.attribute("alt")
                    } else {
                        None
                    }
                })
                .collect();
            let label = normalize_text(&text);
            if label.len() > MAX_LABEL_BYTES {
                return None;
            }
            if label.is_empty() {
                entries.extend(children);
                continue;
            }
            let target = href
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .and_then(|href| resolve_target(href, path, targets, true));
            if target.is_some() || !children.is_empty() {
                entries.push(OutlineEntry {
                    label,
                    target,
                    children,
                });
            }
        }
        Some(entries)
    }
}

#[cfg(test)]
mod limits_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document_uploads::epub::rewrite::collect_fragment_anchors;
    use std::io::Cursor;

    #[test]
    fn keeps_only_exact_local_destinations_and_groups_with_working_children() {
        let xml = r##"<!DOCTYPE html><html xmlns="http://www.w3.org/1999/xhtml" xmlns:e="http://www.idpf.org/2007/ops">
          <nav e:type="page-list"><ol><li><a href="a.xhtml">Page one</a></li></ol></nav>
          <nav e:type="toc other"><ol><li><a href="a.xhtml#missing">Part</a><ol>
            <li><a href="a.xhtml#same">First</a></li>
            <li><a href="b.xhtml#s%61me"><em>Second</em> entry</a></li>
            <li><a href="b.xhtml#two-words">Similar ID</a></li>
            <li><a href="a.xhtml#x%20y">Ambiguous ID</a></li>
            <li><a href="other.xhtml">Outside spine</a></li>
            <li><a href="https://example.com/">External</a></li>
          </ol></li></ol></nav></html>"##;
        let targets = ChapterTargets::from([
            (
                "a.xhtml".into(),
                (
                    2,
                    collect_fragment_anchors(
                        r#"<p id="same"></p><p id="x y"></p><p id="x-y"></p>"#,
                    ),
                ),
            ),
            (
                "b.xhtml".into(),
                (
                    4,
                    collect_fragment_anchors(r#"<p id="same"></p><p id="two words"></p>"#),
                ),
            ),
        ]);
        let entries = Format::Nav.parse(xml, "toc.xhtml", &targets).unwrap();
        assert_eq!(entries.len(), 1);
        let group = &entries[0];
        assert_eq!(group.label, "Part");
        assert_eq!(group.target, None);
        assert_eq!(
            group
                .children
                .iter()
                .map(|n| (n.label.as_str(), n.target.as_deref()))
                .collect::<Vec<_>>(),
            [
                ("First", Some("ch2-same")),
                ("Second entry", Some("ch4-same"))
            ]
        );
    }

    #[test]
    fn empty_nav_uses_declared_ncx() {
        let bytes = include_bytes!("../../../../scripts/fixtures/epub/publisher-outline-2.epub");
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let opf = read_zip_text_limited(&mut archive, "OPS/book.opf", super::super::MAX_OPF_BYTES)
            .unwrap();
        // The declared XHTML exists and parses, but contains no toc nav.
        let opf = opf.replace("id=\"notes\"", "id=\"notes\" properties=\"other nav\"");
        let package = super::super::parse_opf(&opf, "OPS/book.opf", "Fallback").unwrap();
        assert_eq!(package.nav.as_deref(), Some("OPS/text/notes.xhtml"));
        let targets =
            ChapterTargets::from([("OPS/text/notes.xhtml".into(), (0, Default::default()))]);
        let entries = load(&mut archive, &package, &targets);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].target.as_deref(), Some("chapter-0"));
        assert!(entries[0].children.is_empty());
    }
}
