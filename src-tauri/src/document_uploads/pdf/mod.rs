//! PDF-owned source and derived page-text storage.
//!
//! PDF.js extraction and rendering live in the frontend. This module owns only
//! the native storage boundary so the PDF architecture can be removed without
//! spreading binary-format details through the HTML/EPUB pipeline.

mod page_text;
mod source;

pub(crate) use source::{restore_transferred_pdf, SOURCE_FILE_NAME};
