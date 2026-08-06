import type { DocumentImportStatus } from '../../hooks/useUploadedLibrary'

/** Keep OCR feedback attached to the PDF that started the shared job. */
export function isPdfRecognitionStatusForDocument(
  status: DocumentImportStatus,
  documentUrl: string,
): boolean {
  return status.format === 'pdf-ocr' && status.documentUrl === documentUrl
}
