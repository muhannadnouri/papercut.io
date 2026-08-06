import type { DocumentImportStatus } from '../../hooks/useUploadedLibrary'
import type { PdfRecognitionIssues } from './recognizePdf'

/** Keep OCR feedback attached to the PDF that started the shared job. */
export function isPdfRecognitionStatusForDocument(
  status: DocumentImportStatus,
  documentUrl: string,
): boolean {
  return status.format === 'pdf-ocr' && status.documentUrl === documentUrl
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
