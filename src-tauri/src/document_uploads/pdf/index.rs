//! PDF page-text sidecar and FTS indexing orchestration.

use serde::Deserialize;
use tauri::Runtime;

use super::super::cover::{write_pdf_thumbnail, PNG_COVER_FILE_NAME, THUMBNAIL_MEDIA_TYPE};
use super::super::parsed::ParsedDocumentCover;
use super::super::parsed::{ParsedDocument, ParsedSection};
use super::super::storage::{
    upload_dir, upload_reference_from_url, upload_source_path, StoredSourceKind,
};
use super::super::store::{find_upload_by_id, open_db, upsert_document, PdfTextStatus};
use super::super::types::UploadedDocument;
use super::narration::{
    reconstruct_narration_segments, reconstruct_search_text, PdfNarrationSegment,
};
use super::page_text::{
    has_ocr_text_marker, read_page_text_layer, sync_ocr_text_marker, write_page_text_layer,
    PageTextLayer,
};

pub(crate) const MAX_PDF_PAGES: u32 = 2_000;
const MAX_TITLE_CHARS: usize = 512;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PdfPageTextRequest {
    document_url: String,
    layer: PageTextLayer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PdfPageTextReadRequest {
    document_url: String,
    page_index: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PdfFinalizeRequest {
    document_url: String,
    title: Option<String>,
    page_count: u32,
    thumbnail: Option<Vec<u8>>,
    #[serde(default)]
    text_status: PdfTextStatus,
}

/// Validate and store one page at a time so extraction never sends a complete
/// large document's text and coordinates through one IPC message.
pub(crate) fn store_pdf_page_text<R: Runtime>(
    app: &tauri::AppHandle<R>,
    request: PdfPageTextRequest,
) -> Result<(), String> {
    let (id, _) = validated_pdf_upload(app, &request.document_url)?;
    if request.layer.page_index >= MAX_PDF_PAGES {
        return Err(format!("PDF exceeds the {MAX_PDF_PAGES}-page import limit"));
    }
    write_page_text_layer(&upload_dir(app, &id)?, &request.layer)
}

/// Return one validated derived page layer for viewer integrations without
/// reparsing the canonical PDF or loading the rest of the document.
pub(crate) fn get_pdf_page_text_layer<R: Runtime>(
    app: &tauri::AppHandle<R>,
    request: PdfPageTextReadRequest,
) -> Result<PageTextLayer, String> {
    let (id, _) = validated_pdf_upload(app, &request.document_url)?;
    if request.page_index >= MAX_PDF_PAGES {
        return Err(format!("PDF exceeds the {MAX_PDF_PAGES}-page limit"));
    }
    read_page_text_layer(&upload_dir(app, &id)?, request.page_index)
}

/// Tell the viewer whether finalized sidecars require indexed OCR-aware Find.
pub(crate) fn pdf_has_ocr_text<R: Runtime>(
    app: &tauri::AppHandle<R>,
    document_url: &str,
) -> Result<bool, String> {
    let (id, _) = validated_pdf_upload(app, document_url)?;
    Ok(has_ocr_text_marker(&upload_dir(app, &id)?))
}

/// Load validated sidecars and reconstruct logical prose for narration.
///
/// Papercut currently materializes a complete audiobook chunk list before
/// synthesis. Returning compact page/block runs avoids reparsing the canonical
/// PDF or sending full geometry while retaining the locator data needed by the
/// active-page highlight path.
pub(crate) fn get_pdf_narration_segments<R: Runtime>(
    app: &tauri::AppHandle<R>,
    document_url: &str,
) -> Result<Vec<PdfNarrationSegment>, String> {
    let (id, _) = validated_pdf_upload(app, document_url)?;
    let db = open_db(app)?;
    let document =
        find_upload_by_id(&db, &id)?.ok_or_else(|| "PDF upload metadata is missing".to_string())?;
    if StoredSourceKind::from_str(&document.source_kind)? != StoredSourceKind::Pdf {
        return Err("Uploaded PDF source metadata does not match its URL".into());
    }
    if document.text_status == "processing" || document.text_status == "recognition-required" {
        return Err("PDF text has not been indexed".into());
    }
    let page_count =
        u32::try_from(document.sections).map_err(|_| "PDF page count is invalid".to_string())?;
    if page_count == 0 || page_count > MAX_PDF_PAGES {
        return Err("PDF text has not been indexed".into());
    }

    let dir = upload_dir(app, &id)?;
    reconstruct_narration_segments(page_count, |page_index| {
        read_page_text_layer(&dir, page_index)
    })
}

/// Commit all page rows and their FTS entries only after every expected sidecar
/// exists and passes validation. The source PDF remains canonical and sidecars
/// can be rebuilt by a later extractor version.
pub(crate) fn finalize_pdf_index<R: Runtime>(
    app: &tauri::AppHandle<R>,
    request: PdfFinalizeRequest,
) -> Result<UploadedDocument, String> {
    if request.page_count == 0 || request.page_count > MAX_PDF_PAGES {
        return Err(format!(
            "PDF page count must be between 1 and {MAX_PDF_PAGES}"
        ));
    }
    let (id, source_kind) = validated_pdf_upload(app, &request.document_url)?;
    let mut db = open_db(app)?;
    let existing =
        find_upload_by_id(&db, &id)?.ok_or_else(|| "PDF upload metadata is missing".to_string())?;
    let title = normalized_title(request.title.as_deref(), &existing.title);
    let dir = upload_dir(app, &id)?;
    let mut sections = Vec::with_capacity(request.page_count as usize);
    let mut has_ocr_text = false;

    for page_index in 0..request.page_count {
        let layer = read_page_text_layer(&dir, page_index)?;
        has_ocr_text |= layer.blocks.iter().any(|block| block.confidence.is_some());
        let text = reconstruct_search_text(layer);
        sections.push(ParsedSection {
            heading: None,
            text,
            page_index: Some(page_index),
        });
    }
    sync_ocr_text_marker(&dir, has_ocr_text)?;

    let cover = match request.thumbnail {
        Some(bytes) => {
            if let Err(error) = write_pdf_thumbnail(&dir, &bytes) {
                log::warn!("Skipping unusable imported PDF thumbnail: {error}");
                None
            } else {
                Some(ParsedDocumentCover {
                    media_type: THUMBNAIL_MEDIA_TYPE,
                    file_name: PNG_COVER_FILE_NAME,
                    bytes,
                })
            }
        }
        None if existing.cover_media_type.as_deref() == Some(THUMBNAIL_MEDIA_TYPE)
            && dir.join(PNG_COVER_FILE_NAME).is_file() =>
        {
            Some(ParsedDocumentCover {
                media_type: THUMBNAIL_MEDIA_TYPE,
                file_name: PNG_COVER_FILE_NAME,
                bytes: Vec::new(),
            })
        }
        None => None,
    };
    let parsed = ParsedDocument {
        title: title.clone(),
        format: "pdf".into(),
        view_html: String::new(),
        sections,
        cover,
    };
    upsert_document(
        &mut db,
        &id,
        &request.document_url,
        &parsed,
        existing.original_file_name.as_deref(),
        source_kind,
        existing.imported_at_ms,
        existing.bytes,
        request.text_status,
    )?;

    find_upload_by_id(&db, &id)?.ok_or_else(|| "Indexed PDF metadata is missing".to_string())
}

fn validated_pdf_upload<R: Runtime>(
    app: &tauri::AppHandle<R>,
    document_url: &str,
) -> Result<(String, StoredSourceKind), String> {
    let (id, source_kind) = upload_reference_from_url(document_url)?;
    if source_kind != StoredSourceKind::Pdf {
        return Err("Document is not an uploaded PDF".into());
    }
    if !upload_source_path(app, &id, source_kind)?.is_file() {
        return Err("PDF source file is missing".into());
    }
    Ok((id, source_kind))
}

fn normalized_title(candidate: Option<&str>, fallback: &str) -> String {
    let candidate = candidate.map(str::trim).filter(|title| !title.is_empty());
    candidate
        .unwrap_or(fallback)
        .chars()
        .take(MAX_TITLE_CHARS)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::normalized_title;

    #[test]
    fn pdf_title_uses_trimmed_metadata_or_fallback() {
        assert_eq!(
            normalized_title(Some("  A PDF Title  "), "file"),
            "A PDF Title"
        );
        assert_eq!(normalized_title(Some("  "), "file"), "file");
    }
}
