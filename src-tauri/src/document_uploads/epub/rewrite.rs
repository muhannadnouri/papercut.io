//! DOM-based EPUB fragment rewriting.
//!
//! Ammonia handles the security-oriented sanitizer pass. This module performs the
//! EPUB-specific adaptation pass on the sanitized DOM: anchor prefixing,
//! generated-reader hash links, and retained image asset markers.

use std::collections::{HashMap, HashSet};

use kuchikiki::{parse_html, traits::TendrilSink, NodeRef};

use super::assets::LoadedImageAssets;
use super::paths::{archive_base_dir, percent_decode, resolve_archive_path, split_href};

const WRAPPER_ID: &str = "papercut-epub-fragment-root";

/// Keep publisher spellings for lookup. Only repeated spellings are ambiguous.
pub(super) enum Anchor {
    Body,
    Element,
    Duplicate,
}
pub(super) type FragmentAnchors = HashMap<String, Anchor>;
pub(super) type ChapterTargets = HashMap<String, (usize, FragmentAnchors)>;

/// Collect sanitized `id`/`name` anchors before rewriting links.
///
/// Link rewriting is a two-pass process: first learn which fragment targets exist
/// per spine item, then rewrite hrefs. A DOM walk avoids the edge cases that made
/// the previous string scanner fragile around quoting and malformed tags.
pub(super) fn collect_fragment_anchors(html: &str, body_id: Option<String>) -> FragmentAnchors {
    let document = parse_fragment(html);
    let Some(root) = fragment_root(&document) else {
        return FragmentAnchors::new();
    };

    let mut anchors = FragmentAnchors::new();
    if let Some(id) = body_id {
        anchors.insert(id, Anchor::Body);
    }
    if let Ok(nodes) = root.select("*") {
        for node in nodes {
            if node.as_node() == &root {
                continue;
            }
            let attrs = node.attributes.borrow();
            let id = attrs.get("id");
            let name = attrs.get("name").filter(|name| Some(*name) != id);
            for value in id.into_iter().chain(name) {
                anchors
                    .entry(value.to_owned())
                    .and_modify(|entry| *entry = Anchor::Duplicate)
                    .or_insert(Anchor::Element);
            }
        }
    }
    anchors
}

/// Resolve local image references from one sanitized spine fragment.
pub(super) fn collect_image_paths(html: &str, current_path: &str) -> HashSet<String> {
    let document = parse_fragment(html);
    let Some(root) = fragment_root(&document) else {
        return HashSet::new();
    };
    root.select("img[src]")
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|node| {
            let attrs = node.attributes.borrow();
            let src = attrs.get("src")?;
            if is_unsafe_url(src) || src.starts_with("http://") || src.starts_with("https://") {
                return None;
            }
            resolve_archive_path(&archive_base_dir(current_path), src).ok()
        })
        .collect()
}

/// Rewrite sanitized chapter HTML so all retained links/resources are local.
///
/// The DOM parser gives browser-like handling for attributes and malformed-but-
/// recoverable XHTML while keeping the generated reader as ordinary HTML.
pub(super) fn rewrite_epub_fragment(
    html: &str,
    current_path: &str,
    chapter_index: usize,
    targets: &ChapterTargets,
    image_assets: &LoadedImageAssets,
) -> String {
    let document = parse_fragment(html);
    let Some(root) = fragment_root(&document) else {
        return html.to_string();
    };

    if let Ok(nodes) = root.select("*") {
        for node in nodes {
            if node.as_node() == &root {
                continue;
            }
            let tag_name = node.name.local.to_string().to_ascii_lowercase();
            let mut attrs = node.attributes.borrow_mut();
            remove_active_or_layout_attrs(&mut attrs);

            if let Some(value) = attrs.get("id").map(ToOwned::to_owned) {
                attrs.insert("id", prefix_anchor(chapter_index, &value));
            }
            if let Some(value) = attrs.get("name").map(ToOwned::to_owned) {
                attrs.insert("name", prefix_anchor(chapter_index, &value));
            }

            match tag_name.as_str() {
                "a" => rewrite_anchor_attrs(&mut attrs, current_path, targets),
                "img" => rewrite_image_attrs(&mut attrs, current_path, image_assets),
                _ => rewrite_generic_attrs(&mut attrs),
            }
        }
    }

    serialize_children(&root)
}

/// Wrap a fragment so `kuchikiki` can parse it through the normal HTML pipeline.
///
/// We serialize only the wrapper's children, so the artificial document/body/div
/// never leaks into stored reader HTML.
fn parse_fragment(html: &str) -> NodeRef {
    parse_html()
        .one(format!(
            "<!doctype html><html><body><div id=\"{WRAPPER_ID}\">{html}</div></body></html>"
        ))
        .document_node
}

fn fragment_root(document: &NodeRef) -> Option<NodeRef> {
    document
        .select_first(&format!("#{WRAPPER_ID}"))
        .ok()
        .map(|node| node.as_node().clone())
}

fn serialize_children(node: &NodeRef) -> String {
    let mut bytes = Vec::new();
    for child in node.children() {
        if child.serialize(&mut bytes).is_err() {
            return String::new();
        }
    }
    String::from_utf8(bytes).unwrap_or_default()
}

fn remove_active_or_layout_attrs(attrs: &mut kuchikiki::Attributes) {
    let to_remove: Vec<String> = attrs
        .map
        .keys()
        .map(|name| name.local.to_string())
        .filter(|name| {
            let lower = name.to_ascii_lowercase();
            lower.starts_with("on") || matches!(lower.as_str(), "style" | "srcset")
        })
        .collect();
    for name in to_remove {
        attrs.remove(name.as_str());
    }
}

fn rewrite_anchor_attrs(
    attrs: &mut kuchikiki::Attributes,
    current_path: &str,
    targets: &ChapterTargets,
) {
    let Some(href) = attrs.get("href").map(ToOwned::to_owned) else {
        return;
    };
    match resolve_target(&href, current_path, targets, false) {
        Some(value) => {
            attrs.insert("href", format!("#{value}"));
        }
        None => {
            attrs.remove("href");
        }
    }
}

fn rewrite_image_attrs(
    attrs: &mut kuchikiki::Attributes,
    current_path: &str,
    image_assets: &LoadedImageAssets,
) {
    let Some(src) = attrs.get("src").map(ToOwned::to_owned) else {
        return;
    };
    match rewrite_image_src(&src, current_path, image_assets) {
        Ok(file_name) => {
            attrs.remove("data-papercut-image-error");
            attrs.remove("src");
            attrs.insert("data-papercut-asset", file_name.clone());
            attrs.insert("loading", "lazy".into());
            attrs.insert("decoding", "async".into());
        }
        Err(reason) => {
            attrs.insert("data-papercut-image-error", reason.into());
            attrs.remove("src");
        }
    }
}

fn rewrite_generic_attrs(attrs: &mut kuchikiki::Attributes) {
    if attrs.get("href").is_some_and(is_unsafe_url) {
        attrs.remove("href");
    }
    attrs.remove("src");
}

/// Return the generated reader ID. Ordinary links fall back to the chapter
/// start for missing fragments; outline entries require an exact destination.
pub(super) fn resolve_target(
    href: &str,
    current_path: &str,
    targets: &ChapterTargets,
    exact: bool,
) -> Option<String> {
    if is_unsafe_url(href)
        || href.starts_with("http://")
        || href.starts_with("https://")
        || href.starts_with("mailto:")
    {
        return None;
    }
    let (path_part, fragment) = split_href(href);
    if exact
        && path_part
            .split('/')
            .next()
            .is_some_and(|part| part.contains(':'))
    {
        return None;
    }
    let target_path = if path_part.is_empty() {
        current_path.to_string()
    } else {
        resolve_archive_path(&archive_base_dir(current_path), path_part).ok()?
    };
    let (index, anchors) = targets.get(&target_path)?;
    if let Some(fragment) = fragment.filter(|v| !v.is_empty()).map(percent_decode) {
        match anchors.get(&fragment) {
            Some(Anchor::Body) => return Some(format!("chapter-{index}")),
            Some(Anchor::Element) => return Some(prefix_anchor(*index, &fragment)),
            Some(Anchor::Duplicate) if !exact => return Some(prefix_anchor(*index, &fragment)),
            _ if exact => return None,
            _ => {}
        }
    }
    Some(format!("chapter-{index}"))
}

/// Rewrite a local image `src` to its generated stored filename, or drop it.
///
/// Remote images are excluded for offline behavior and privacy; unsupported or
/// oversized local images retain a reason for the reader's placeholder.
fn rewrite_image_src(
    src: &str,
    current_path: &str,
    image_assets: &LoadedImageAssets,
) -> Result<String, &'static str> {
    let lower = src.trim().to_ascii_lowercase();
    if lower.starts_with("http:") || lower.starts_with("https:") || lower.starts_with("//") {
        return Err("remote");
    }
    if lower.starts_with("data:") {
        return Err("unsupported");
    }
    if is_unsafe_url(src) {
        return Err("invalid-path");
    }
    let path =
        resolve_archive_path(&archive_base_dir(current_path), src).map_err(|_| "invalid-path")?;
    image_assets
        .paths
        .get(&path)
        .cloned()
        .ok_or_else(|| image_assets.errors.get(&path).copied().unwrap_or("missing"))
}

fn is_unsafe_url(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    lower.starts_with("javascript:") || lower.starts_with("data:text/html")
}

/// Prefix an imported anchor with chapter index to prevent cross-chapter id collisions.
fn prefix_anchor(chapter_index: usize, value: &str) -> String {
    // Reserve a prefix for encoded UTF-8 bytes, including literal IDs starting with that prefix.
    // A percent sign in an ID is literal. Only the incoming href is URL-decoded.
    if !value.is_empty()
        && !value.starts_with("_u_")
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b':' | b'.'))
    {
        return format!("ch{chapter_index}-{value}");
    }
    use std::fmt::Write;
    let mut id = format!("ch{chapter_index}-_u_");
    for byte in value.bytes() {
        write!(id, "{byte:02x}").expect("writing to a String");
    }
    id
}

/// Mark images before sanitization can erase their original URL or SVG markup.
pub(super) fn annotate_image_issues(html: &str) -> String {
    let document = parse_fragment(html);
    let Some(root) = fragment_root(&document) else {
        return html.to_string();
    };
    if let Ok(nodes) = root.select("*") {
        let nodes: Vec<_> = nodes
            .filter(|node| node.name.local.rsplit(':').next() == Some("svg"))
            .collect();
        for element in nodes {
            let node = element.as_node();
            if node.ancestors().skip(1).any(|parent| {
                parent
                    .as_element()
                    .is_some_and(|element| element.name.local.rsplit(':').next() == Some("svg"))
            }) {
                continue;
            }
            let label = element
                .attributes
                .borrow()
                .get("aria-label")
                .map(str::to_string)
                .or_else(|| {
                    node.descendants()
                        .find(|child| {
                            child.as_element().is_some_and(|element| {
                                element.name.local.rsplit(':').next() == Some("title")
                            })
                        })
                        .map(|title| title.text_contents())
                })
                .unwrap_or_default();
            let placeholder = parse_fragment(&format!(
                "<img data-papercut-image-error=\"svg\" alt=\"{}\">",
                super::render::escape_attr(&label)
            ));
            if let Ok(image) = placeholder.select_first("img") {
                if let Some(id) = element.attributes.borrow().get("id") {
                    image.attributes.borrow_mut().insert("id", id.to_string());
                }
                node.insert_before(image.as_node().clone());
                node.detach();
            }
        }
    }
    if let Ok(images) = root.select("img") {
        for image in images {
            let mut attrs = image.attributes.borrow_mut();
            if attrs
                .get("data-papercut-image-error")
                .is_some_and(|reason| {
                    crate::document_uploads::parsed::IMAGE_ERROR_REASONS.contains(&reason)
                })
            {
                continue;
            }
            let src = attrs.get("src").unwrap_or("").trim().to_ascii_lowercase();
            let reason =
                if src.starts_with("http:") || src.starts_with("https:") || src.starts_with("//") {
                    "remote"
                } else if src.starts_with("data:") {
                    "unsupported"
                } else if is_unsafe_url(&src) {
                    "invalid-path"
                } else {
                    "unavailable"
                };
            attrs.insert("data-papercut-image-error", reason.into());
        }
    }
    serialize_children(&root)
}

#[cfg(test)]
mod image_issue_tests {
    use super::*;
    use crate::document_uploads::html::sanitize_html;
    use crate::document_uploads::parsed::{image_import_warnings, ParsedDocumentAsset};

    #[test]
    fn image_reasons_survive_sanitization_without_changing_reader_text() {
        let raw = r#"<p>Readable text.</p><img src="good.png"><img src="missing.png" alt="Missing figure"><img src="huge.png"><img src="limit.png"><img src="photo.bmp"><img src="https://example.com/image.png"><img src="../../../outside.png"><img src="data:image/png;base64,eA=="><img><svg xmlns="http://www.w3.org/2000/svg"><title>Lost diagram</title><image href="no-file.png"/></svg><s:svg><s:title>Prefixed diagram</s:title></s:svg>"#;
        let mut assets = LoadedImageAssets::default();
        assets.insert(
            "OPS/good.png".into(),
            ParsedDocumentAsset::new("image/png", b"png".to_vec()).unwrap(),
        );
        for (path, reason) in [
            ("OPS/huge.png", "size-limit"),
            ("OPS/limit.png", "total-limit"),
            ("OPS/photo.bmp", "unsupported"),
        ] {
            assets.errors.insert(path.into(), reason);
        }
        let annotated = super::super::sanitize_epub_fragment(raw);
        let html = rewrite_epub_fragment(&annotated, "OPS/ch.xhtml", 0, &HashMap::new(), &assets);
        let warnings = image_import_warnings(&html);
        assert_eq!(warnings.values().sum::<usize>(), 10);
        assert_eq!(warnings.get("unsupported"), Some(&2));
        assert_eq!(warnings.get("svg"), Some(&2));
        for reason in [
            "missing",
            "size-limit",
            "total-limit",
            "remote",
            "invalid-path",
            "unavailable",
        ] {
            assert_eq!(warnings.get(reason), Some(&1), "{reason}");
        }
        assert_eq!(image_import_warnings(&sanitize_html(&html)), warnings);
        assert_eq!(html.matches("data-papercut-asset=").count(), 1);
        assert!(html.contains("alt=\"Missing figure\""));
        assert!(html.contains("alt=\"Lost diagram\""));
        assert!(!html.contains("https://example.com"));
        assert_eq!(
            crate::document_uploads::html::normalize_text(
                &crate::document_uploads::html::strip_tags(&html)
            ),
            "Readable text."
        );
    }
}
