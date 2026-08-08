import type { DocumentImportStatus } from '../../hooks/useUploadedLibrary'
import type { UploadedDocumentTextStatus } from '../../uploads/DocumentUploads'
import type { PdfRecognitionIssues } from './recognizePdf'
import { PDF_OCR_INTERRUPTED } from './pdfRecognitionJob'

export type PdfRecognitionIndicatorState = 'running' | 'attention' | 'error' | 'none'

/** Keep OCR feedback attached to the PDF that started the shared job. */
export function isPdfRecognitionStatusForDocument(
  status: DocumentImportStatus,
  documentUrl: string,
): boolean {
  return status.format === 'pdf-ocr' && status.documentUrl === documentUrl
}

/** Map the shared OCR lifecycle to one unambiguous reader-toolbar indicator. */
export function pdfRecognitionIndicatorState(
  status: DocumentImportStatus,
  documentUrl: string,
  textStatus?: UploadedDocumentTextStatus,
): PdfRecognitionIndicatorState {
  if (isPdfRecognitionStatusForDocument(status, documentUrl)) {
    if (status.status === 'recognizing') return 'running'
    if (status.status === 'error') {
      return status.message === PDF_OCR_INTERRUPTED ? 'attention' : 'error'
    }
    if (status.status === 'recognized') {
      return pdfRecognitionIssueAction(status.recognitionIssues) ? 'attention' : 'none'
    }
  }

  return textStatus === 'recognition-required' ? 'attention' : 'none'
}

/** Retry technical failures, but let users accept usable partial or review-only OCR. */
export function pdfRecognitionIssueAction(
  issues?: PdfRecognitionIssues,
): 'retry' | 'accept' | null {
  if ((issues?.failedPages.length ?? 0) > 0) return 'retry'
  if ((issues?.unrecognizedPages.length ?? 0) > 0 ||
      (issues?.lowConfidencePages.length ?? 0) > 0) return 'accept'
  return null
}

/** Count only pages handled by the action currently offered to the user. */
export function pdfRecognitionActionPageCount(issues?: PdfRecognitionIssues): number {
  return pdfRecognitionIssueAction(issues) === 'retry'
    ? issues?.failedPages.length ?? 0
    : (issues?.unrecognizedPages.length ?? 0) +
      (issues?.lowConfidencePages.length ?? 0)
}
