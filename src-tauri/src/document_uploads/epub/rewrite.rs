//! DOM-based EPUB fragment rewriting.
//!
//! Ammonia handles the security-oriented sanitizer pass. This module performs the
//! EPUB-specific adaptation pass on the sanitized DOM: anchor prefixing,
//! generated-reader hash links, and retained raster asset markers.

use std::collections::{HashMap, HashSet};

use kuchikiki::{parse_html, traits::TendrilSink, NodeRef};

use super::paths::{archive_base_dir, percent_decode, resolve_archive_path, split_href};

const WRAPPER_ID: &str = "papercut-epub-fragment-root";

/// Collect sanitized `id`/`name` anchors before rewriting links.
///
/// Link rewriting is a two-pass process: first learn which fragment targets exist
/// per spine item, then rewrite hrefs. A DOM walk avoids the edge cases that made
/// the previous string scanner fragile around quoting and malformed tags.
pub(super) fn collect_fragment_anchors(html: &str) -> HashSet<String> {
    let document = parse_fragment(html);
    let Some(root) = fragment_root(&document) else {
        return HashSet::new();
    };

    let mut anchors = HashSet::new();
    if let Ok(nodes) = root.select("*") {
        for node in nodes {
            if node.as_node() == &root {
                continue;
            }
            let attrs = node.attributes.borrow();
            if let Some(value) = attrs.get("id") {
                anchors.insert(safe_anchor(value));
            }
            if let Some(value) = attrs.get("name") {
                anchors.insert(safe_anchor(value));
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
    chapter_indexes: &HashMap<String, usize>,
    readable_paths: &HashSet<String>,
    anchor_indexes: &HashMap<String, HashSet<String>>,
    image_assets: &HashMap<String, String>,
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
                "a" => rewrite_anchor_attrs(
                    &mut attrs,
                    current_path,
                    chapter_indexes,
                    readable_paths,
                    anchor_indexes,
                ),
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
    chapter_indexes: &HashMap<String, usize>,
    readable_paths: &HashSet<String>,
    anchor_indexes: &HashMap<String, HashSet<String>>,
) {
    let Some(href) = attrs.get("href").map(ToOwned::to_owned) else {
        return;
    };
    match rewrite_internal_href(
        &href,
        current_path,
        chapter_indexes,
        readable_paths,
        anchor_indexes,
    ) {
        Some(value) => {
            attrs.insert("href", value);
        }
        None => {
            attrs.remove("href");
        }
    }
}

fn rewrite_image_attrs(
    attrs: &mut kuchikiki::Attributes,
    current_path: &str,
    image_assets: &HashMap<String, String>,
) {
    let Some(src) = attrs.get("src").map(ToOwned::to_owned) else {
        return;
    };
    match rewrite_image_src(&src, current_path, image_assets) {
        Some(file_name) => {
            attrs.remove("src");
            attrs.insert("data-papercut-asset", file_name.clone());
            attrs.insert("loading", "lazy".into());
            attrs.insert("decoding", "async".into());
        }
        None => {
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

/// Rewrite an EPUB-local link into a generated-reader hash.
///
/// Examples: `chapter.xhtml#note` becomes `#ch3-note`, while `chapter.xhtml`
/// becomes `#chapter-3`. If a fragment does not exist but the target chapter is
/// readable, we point to that chapter rather than leaving a dead footnote link.
fn rewrite_internal_href(
    href: &str,
    current_path: &str,
    chapter_indexes: &HashMap<String, usize>,
    readable_paths: &HashSet<String>,
    anchor_indexes: &HashMap<String, HashSet<String>>,
) -> Option<String> {
    if is_unsafe_url(href)
        || href.starts_with("http://")
        || href.starts_with("https://")
        || href.starts_with("mailto:")
    {
        return None;
    }
    let (path_part, fragment) = split_href(href);
    let target_path = if path_part.is_empty() {
        current_path.to_string()
    } else {
        resolve_archive_path(&archive_base_dir(current_path), path_part).ok()?
    };
    if !readable_paths.contains(&target_path) {
        return None;
    }
    let target_index = *chapter_indexes.get(&target_path)?;
    let chapter_href = || Some(format!("#chapter-{target_index}"));
    match fragment
        .map(percent_decode)
        .filter(|value| !value.is_empty())
    {
        Some(fragment) => {
            let anchor = safe_anchor(&fragment);
            if anchor_indexes
                .get(&target_path)
                .is_some_and(|anchors| anchors.contains(&anchor))
            {
                Some(format!("#{}", prefix_anchor(target_index, &fragment)))
            } else {
                chapter_href()
            }
        }
        None => chapter_href(),
    }
}

/// Rewrite a local image `src` to its generated stored filename, or drop it.
///
/// Remote images are excluded for offline behavior and privacy; unsupported or
/// oversized local images are simply omitted from the generated reader.
fn rewrite_image_src(
    src: &str,
    current_path: &str,
    image_assets: &HashMap<String, String>,
) -> Option<String> {
    if is_unsafe_url(src) || src.starts_with("http://") || src.starts_with("https://") {
        return None;
    }
    let path = resolve_archive_path(&archive_base_dir(current_path), src).ok()?;
    image_assets.get(&path).cloned()
}

fn is_unsafe_url(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    lower.starts_with("javascript:") || lower.starts_with("data:text/html")
}

/// Prefix an imported anchor with chapter index to prevent cross-chapter id collisions.
fn prefix_anchor(chapter_index: usize, value: &str) -> String {
    format!("ch{chapter_index}-{}", safe_anchor(value))
}

/// Convert arbitrary EPUB anchor text into a deterministic HTML id fragment.
///
/// We keep common id characters and replace everything else with `-`; callers use
/// the same normalization for both collected targets and rewritten hrefs.
fn safe_anchor(value: &str) -> String {
    let decoded = percent_decode(value);
    let mut out = String::with_capacity(decoded.len());
    for ch in decoded.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | ':' | '.') {
            out.push(ch);
        } else {
            out.push('-');
        }
    }
    if out.is_empty() {
        "anchor".into()
    } else {
        out
    }
}
