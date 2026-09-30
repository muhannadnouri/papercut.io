//! Move inline SVG out of the reader DOM before HTML sanitization discards it.

use std::io::{Read, Seek};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use roxmltree::{Document, Node};
use zip::ZipArchive;

use super::assets::{
    image_format, inline_raster_parts, read_zip_bytes_limited, LoadedImageAssets, ManifestItem,
    MAX_IMAGE_BYTES,
};
use super::paths::{archive_base_dir, resolve_archive_path};
use super::render::escape_attr;
use crate::document_uploads::parsed::ParsedDocumentAsset;

const SVG_NS: &str = "http://www.w3.org/2000/svg";
const XLINK_NS: &str = "http://www.w3.org/1999/xlink";

/// Preserve complete SVG compositions, including their viewport and local raster
/// images. Generated SVG is used only as an external img, like manifest SVGs.
/// Malformed XHTML keeps the existing forgiving HTML import path.
pub(super) fn externalize_inline_svgs<R: Read + Seek>(
    raw: &str,
    chapter_path: &str,
    archive: &mut ZipArchive<R>,
    manifest: &[ManifestItem],
    assets: &mut LoadedImageAssets,
) -> String {
    // EPUB XHTML commonly declares a doctype. roxmltree never fetches external
    // DTDs; reject entity declarations so source ranges remain unexpanded.
    if raw.contains("<!ENTITY") {
        return raw.to_string();
    }
    let Ok(document) = Document::parse_with_options(
        raw,
        roxmltree::ParsingOptions {
            allow_dtd: true,
            ..Default::default()
        },
    ) else {
        return raw.to_string();
    };
    let mut replacements = Vec::new();
    for node in document.descendants().filter(|node| {
        node.has_tag_name((SVG_NS, "svg"))
            && !node
                .ancestors()
                .skip(1)
                .any(|parent| parent.has_tag_name((SVG_NS, "svg")))
    }) {
        if node.range().len() as u64 > MAX_IMAGE_BYTES
            || node
                .ancestors()
                .any(|parent| parent.attribute((super::XML_NAMESPACE, "base")).is_some())
        {
            continue;
        }
        let svg = standalone_svg(node, raw);
        let Some(svg) = embed_raster_images(&svg, chapter_path, archive, manifest) else {
            continue;
        };
        let Some(asset) = ParsedDocumentAsset::new("image/svg+xml", svg.into_bytes()) else {
            continue;
        };
        let name = asset.file_name.clone();
        let Ok(path) = resolve_archive_path(&archive_base_dir(chapter_path), &name) else {
            continue;
        };
        if !assets.insert(path, asset) {
            continue;
        }
        let title = node
            .children()
            .find(|child| child.has_tag_name((SVG_NS, "title")))
            .map(|title| {
                title
                    .descendants()
                    .filter_map(|child| child.text().filter(|_| child.is_text()))
                    .collect::<String>()
            })
            .unwrap_or_default();
        let alt = node.attribute("aria-label").unwrap_or(&title);
        let mut img = format!(
            "<img src=\"{}\" alt=\"{}\"",
            escape_attr(&name),
            escape_attr(alt)
        );
        if let Some(id) = node.attribute("id") {
            img.push_str(&format!(" id=\"{}\"", escape_attr(id)));
        }
        img.push_str(" />");
        replacements.push((node.range(), img));
    }
    let mut output = raw.to_string();
    for (range, replacement) in replacements.into_iter().rev() {
        output.replace_range(range, &replacement);
    }
    output
}

/// Rebuild only the opening tag to carry inherited namespace declarations.
/// Preserve the XML subtree verbatim, including CSS, CDATA, and geometry.
fn standalone_svg(node: Node<'_, '_>, raw: &str) -> String {
    let source = &raw[node.range()];
    let name = source[1..]
        .split(|ch: char| ch.is_whitespace() || ch == '/' || ch == '>')
        .next()
        .unwrap();
    let mut svg = format!("<{name}");
    for namespace in node.namespaces() {
        let prefix = namespace
            .name()
            .map(|name| format!(":{name}"))
            .unwrap_or_default();
        svg.push_str(&format!(
            " xmlns{prefix}=\"{}\"",
            escape_attr(namespace.uri())
        ));
    }
    for attribute in node.attributes() {
        svg.push_str(&format!(
            " {}=\"{}\"",
            &raw[attribute.range_qname()],
            escape_attr(attribute.value())
        ));
    }
    svg.push('>');
    if let (Some(first), Some(last)) = (node.first_child(), node.last_child()) {
        svg.push_str(&raw[first.range().start..last.range().end]);
    }
    svg.push_str(&format!("</{name}>"));
    svg
}

/// An SVG loaded as an image cannot fetch sibling files. Embed only bounded,
/// manifest-declared local rasters; never fetch remote or arbitrary local paths.
/// ponytail: nested SVG files and CSS resources need a bounded dependency resolver
/// if future books require them; no recursive resource loading here.
fn embed_raster_images<R: Read + Seek>(
    svg: &str,
    chapter_path: &str,
    archive: &mut ZipArchive<R>,
    manifest: &[ManifestItem],
) -> Option<String> {
    if svg.len() as u64 > MAX_IMAGE_BYTES {
        return None;
    }
    let document = Document::parse(svg).ok()?;
    if document
        .descendants()
        .any(|node| node.attribute((super::XML_NAMESPACE, "base")).is_some())
    {
        return None;
    }
    let mut size = svg.len();
    let mut replacements = Vec::new();
    for node in document
        .descendants()
        .filter(|node| node.has_tag_name((SVG_NS, "image")))
    {
        let href = node
            .attribute("href")
            .or_else(|| node.attribute((XLINK_NS, "href")))?;
        let (media_type, bytes) = if let Some((media_type, encoded)) = inline_raster_parts(href) {
            (media_type, BASE64_STANDARD.decode(encoded).ok()?)
        } else {
            let path = resolve_archive_path(&archive_base_dir(chapter_path), href).ok()?;
            let item = manifest.iter().find(|item| item.href == path)?;
            let (media_type, _) = image_format(&item.media_type, &item.href)?;
            if media_type == "image/svg+xml" {
                return None;
            }
            (
                media_type,
                read_zip_bytes_limited(archive, &path, MAX_IMAGE_BYTES)?,
            )
        };
        let data = format!("data:{media_type};base64,{}", BASE64_STANDARD.encode(bytes));
        for attribute in node.attributes().filter(|attribute| {
            attribute.name() == "href" && matches!(attribute.namespace(), None | Some(XLINK_NS))
        }) {
            let range = attribute.range_value();
            size = size.checked_sub(range.len())?.checked_add(data.len())?;
            if size as u64 > MAX_IMAGE_BYTES {
                return None;
            }
            replacements.push((range, data.clone()));
        }
    }
    let mut output = svg.to_string();
    // Attribute iteration order is not part of the XML parser's contract.
    replacements.sort_by_key(|(range, _)| range.start);
    for (range, replacement) in replacements.into_iter().rev() {
        output.replace_range(range, &replacement);
    }
    Some(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write};

    fn archive() -> ZipArchive<Cursor<Vec<u8>>> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file("OPS/cover.jpg", zip::write::FileOptions::default())
            .unwrap();
        zip.write_all(b"raster fixture").unwrap();
        ZipArchive::new(zip.finish().unwrap()).unwrap()
    }

    #[test]
    fn preserves_composition_namespaces_labels_and_local_images_outside_reader_dom() {
        let raw = r#"<!DOCTYPE html><html xmlns="http://www.w3.org/1999/xhtml" xmlns:s="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"><body>
          <s:svg id="cover" aria-label="A &quot;cover&quot;" viewBox="0 0 100 200" preserveAspectRatio="xMidYMid">
            <s:style><![CDATA[text { fill: red; }]]></s:style>
            <s:image width="100" height="200" xlink:href="../cover.jpg"/>
            <s:image x="10" y="20" width="30" height="40" href="../cover.jpg" xlink:href="missing.jpg"/>
            <s:svg x="5" y="6"><s:text>Overlay &amp; text</s:text></s:svg>
            <s:script>/*never-in-reader*/</s:script>
          </s:svg><p>Body text.</p>
          <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><title>A diagram</title><rect width="10" height="10"/></svg>
        </body></html>"#;
        let manifest = vec![ManifestItem {
            href: "OPS/cover.jpg".into(),
            media_type: "image/jpeg".into(),
        }];
        let mut assets = LoadedImageAssets::default();
        let converted = externalize_inline_svgs(
            raw,
            "OPS/text/chapter.xhtml",
            &mut archive(),
            &manifest,
            &mut assets,
        );
        assert_eq!(assets.files.len(), 2); // Nested SVG stays inside its composition.
        assert!(converted.contains("alt=\"A &quot;cover&quot;\""));
        assert!(converted.contains("id=\"cover\""));
        assert!(converted.contains("alt=\"A diagram\""));
        assert!(!converted.contains("never-in-reader"));
        let svg = String::from_utf8(assets.files[0].bytes.clone()).unwrap();
        let doc = Document::parse(&svg).unwrap();
        assert_eq!(doc.root_element().attribute("viewBox"), Some("0 0 100 200"));
        assert_eq!(
            doc.root_element().attribute("preserveAspectRatio"),
            Some("xMidYMid")
        );
        assert!(svg.contains("<![CDATA[text { fill: red; }]]>"));
        assert!(svg.contains("Overlay &amp; text"));
        assert_eq!(svg.matches("data:image/jpeg;base64,").count(), 3);
        assert!(!svg.contains("../cover.jpg"));
        assert!(!svg.contains("missing.jpg"));
        let sanitized = super::super::sanitize_epub_fragment(&converted);
        let paths =
            super::super::rewrite::collect_image_paths(&sanitized, "OPS/text/chapter.xhtml");
        assert!(assets.paths.keys().all(|path| paths.contains(path)));
    }

    #[test]
    fn rejects_unresolved_resources_and_keeps_embedded_rasters() {
        for href in [
            "https://example.com/a.jpg",
            "../../../cover.jpg",
            "missing.jpg",
            "data:image/svg+xml;base64,PHN2Zy8+",
        ] {
            let svg = format!(r#"<svg xmlns="{SVG_NS}"><image href="{href}"/></svg>"#);
            assert!(embed_raster_images(&svg, "OPS/text/ch.xhtml", &mut archive(), &[]).is_none());
        }
        let svg =
            format!(r#"<svg xmlns="{SVG_NS}"><image href="data:image/png;base64,eA=="/></svg>"#);
        assert_eq!(
            embed_raster_images(&svg, "OPS/ch.xhtml", &mut archive(), &[]),
            Some(svg)
        );
        let raw = format!(r#"<html xml:base="elsewhere/"><svg xmlns="{SVG_NS}"/></html>"#);
        let mut assets = LoadedImageAssets::default();
        assert_eq!(
            externalize_inline_svgs(&raw, "OPS/ch.xhtml", &mut archive(), &[], &mut assets),
            raw
        );
        assert!(assets.files.is_empty());
        for raw in [
            "<broken",
            "<!DOCTYPE html [<!ENTITY x 'expanded'>]><html>&x;</html>",
        ] {
            assert_eq!(
                externalize_inline_svgs(raw, "OPS/ch.xhtml", &mut archive(), &[], &mut assets),
                raw
            );
            assert!(assets.files.is_empty());
        }
    }
}
