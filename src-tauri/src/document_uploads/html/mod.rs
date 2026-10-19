//! HTML format module: parsing + sanitization.
//!
//! This is the HTML-specific layer. EPUB/PDF parser modules emit the same
//! format-neutral parsed document shape so the store, search, and reader stay
//! format-agnostic.

mod decode;
mod parser;
mod sanitize;
mod util;
pub(crate) use decode::decode_html_bytes;
pub(crate) use parser::{
    add_section_locators, has_complete_section_locators, parse_html_document, parsed_html_document,
};
pub(crate) use sanitize::{decode_entities, normalize_text, sanitize_html, strip_tags};
pub(crate) use util::extract_body_inner;
