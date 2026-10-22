//! HTML format module: parsing + sanitization.
//!
//! This is the HTML-specific layer. EPUB/PDF parser modules emit the same
//! format-neutral parsed document shape so the store, search, and reader stay
//! format-agnostic.

mod decode;
pub(super) mod emphasis;
// Pure policy also documents conservative rejection states beyond the active subset.
#[allow(dead_code)]
mod emphasis_policy;
mod emphasis_sheets;
mod parser;
mod prepare;
mod sanitize;
#[cfg(test)]
mod stylesheet_tests;
mod util;
pub(crate) use decode::decode_html_bytes;
pub(super) use emphasis_policy::POLICY as SOURCE_EMPHASIS_POLICY;
pub(crate) use parser::{
    add_section_locators, has_complete_section_locators, parse_html_document, parsed_html_document,
    reader_block_nodes,
};
pub(crate) use prepare::prepare_html_document;
pub(crate) use sanitize::{
    decode_entities, normalize_text, sanitize_html, strip_tags, tag_separates_text, valid_translate,
};
pub(crate) use util::extract_body_inner;

#[cfg(test)]
mod source_mode_tests;
