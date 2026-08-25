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
    finalize_pdf_index, get_pdf_narration_segments, get_pdf_page_text_layer, pdf_has_ocr_text,
    store_pdf_page_text, PdfFinalizeRequest, PdfPageTextReadRequest, PdfPageTextRequest,
};
pub(crate) use narration::PdfNarrationSegment;
pub(crate) use page_text::PageTextLayer;
#[cfg(feature = "native-tts-core")]
pub(crate) use source::restore_audiobook_pdf;
pub(crate) use source::{
    get_pdf_source_bytes, get_pdf_source_path, import_pdf_source, restore_transferred_pdf,
    SOURCE_FILE_NAME,
};
