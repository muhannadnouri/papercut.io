//! PDF page-text sidecar and FTS indexing orchestration.

use serde::Deserialize;
use tauri::Runtime;

use super::super::parsed::{ParsedDocument, ParsedSection};
use super::super::storage::{
    upload_dir, upload_reference_from_url, upload_source_path, StoredSourceKind,
};
use super::super::store::{find_upload_by_id, open_db, upsert_document};
use super::super::types::UploadedDocument;
use super::page_text::{read_page_text_layer, write_page_text_layer, PageTextLayer};

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
pub(crate) struct PdfFinalizeRequest {
    document_url: String,
    title: Option<String>,
    page_count: u32,
}

/// Validate and store one page at a time so extraction never sends a complete
/// large document's text and coordinates through one IPC message.
pub(crate) fn store_pdf_page_text<R: Runtime>(
    app: &tauri::AppHandle<R>,
    request: PdfPageTextRequest,
) -> Result<(), String> {
    let (id, source_kind) = validated_pdf_upload(app, &request.document_url)?;
    if request.layer.page_index >= MAX_PDF_PAGES {
        return Err(format!("PDF exceeds the {MAX_PDF_PAGES}-page import limit"));
    }
    if source_kind != StoredSourceKind::Pdf {
        return Err("PDF page text can only be stored for a PDF upload".into());
    }
    write_page_text_layer(&upload_dir(app, &id)?, &request.layer)
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

    for page_index in 0..request.page_count {
        let mut layer = read_page_text_layer(&dir, page_index)?;
        layer.blocks.sort_by_key(|block| block.order);
        let text = layer
            .blocks
            .into_iter()
            .map(|block| block.text)
            .collect::<String>()
            .trim()
            .to_string();
        sections.push(ParsedSection {
            heading: None,
            text,
            page_index: Some(page_index),
        });
    }

    let parsed = ParsedDocument {
        title: title.clone(),
        format: "pdf".into(),
        view_html: String::new(),
        sections,
        cover: None,
    };
    upsert_document(
        &mut db,
        &id,
        &request.document_url,
        &parsed,
        source_kind,
        existing.imported_at_ms,
        existing.bytes,
    )?;

    Ok(UploadedDocument {
        id,
        url: request.document_url,
        title,
        format: "pdf".into(),
        source_kind: source_kind.as_str().into(),
        imported_at_ms: existing.imported_at_ms,
        bytes: existing.bytes,
        sections: request.page_count as usize,
        cover_media_type: None,
    })
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
