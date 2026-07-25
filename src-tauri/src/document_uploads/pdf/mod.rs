//! PDF-owned source and derived page-text storage.
//!
//! PDF.js extraction and rendering live in the frontend. This module owns only
//! the native storage boundary so the PDF architecture can be removed without
//! spreading binary-format details through the HTML/EPUB pipeline.

mod index;
mod page_text;
mod source;

pub(crate) use index::{
    finalize_pdf_index, store_pdf_page_text, PdfFinalizeRequest, PdfPageTextRequest,
};
pub(crate) use source::{
    get_pdf_source_bytes, import_pdf_source, restore_transferred_pdf, SOURCE_FILE_NAME,
};
