//! PDF-owned source and derived page-text storage.
//!
//! PDF.js extraction and rendering live in the frontend. This module owns only
//! the native storage boundary so the PDF architecture can be removed without
//! spreading binary-format details through the HTML/EPUB pipeline.

mod index;
mod narration;
mod page_text;
mod source;

pub(crate) use index::{
    finalize_pdf_index, get_pdf_narration_segments, store_pdf_page_text, PdfFinalizeRequest,
    PdfPageTextRequest,
};
pub(crate) use narration::PdfNarrationSegment;
pub(crate) use source::{
    get_pdf_source_bytes, get_pdf_source_path, import_pdf_source, restore_audiobook_pdf,
    restore_transferred_pdf, SOURCE_FILE_NAME,
};
