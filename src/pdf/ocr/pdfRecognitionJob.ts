import type { PdfRecognitionIssues } from './recognizePdf'
import type { PdfOcrLanguage } from './tesseractOcr'

const STORAGE_KEY = 'papercut.pdfRecognitionJob.v1'
const SESSION_ID = globalThis.crypto?.randomUUID?.() ?? String(Date.now())

export const PDF_OCR_INTERRUPTED = 'pdf-ocr-interrupted'

export interface PdfRecognitionJob {
  documentUrl: string
  language: PdfOcrLanguage
  improveIssues?: PdfRecognitionIssues
  sessionId: string
}

/** Read the small recovery marker; recognized page payloads remain in native sidecars. */
export function getPdfRecognitionJob(): PdfRecognitionJob | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    return raw ? parsePdfRecognitionJob(JSON.parse(raw)) : null
  } catch {
    return null
  }
}

/** Return only work left by an earlier app session, never the current live job. */
export function getInterruptedPdfRecognitionJob(): PdfRecognitionJob | null {
  const job = getPdfRecognitionJob()
  return job?.sessionId !== SESSION_ID ? job : null
}

/** Persist enough intent to resume while each completed page is stored atomically. */
export function savePdfRecognitionJob(
  documentUrl: string,
  language: PdfOcrLanguage,
  improveIssues?: PdfRecognitionIssues,
): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({
      documentUrl,
      language,
      improveIssues,
      sessionId: SESSION_ID,
    } satisfies PdfRecognitionJob))
  } catch {
    // Recovery metadata is best effort; unavailable browser storage must not stop OCR.
  }
}

/** Clear only the matching job so stale cleanup cannot remove newer work. */
export function clearPdfRecognitionJob(documentUrl: string): void {
  try {
    if (getPdfRecognitionJob()?.documentUrl === documentUrl) {
      localStorage.removeItem(STORAGE_KEY)
    }
  } catch {
    // Best-effort cleanup mirrors the storage write behavior above.
  }
}

export function parsePdfRecognitionJob(value: unknown): PdfRecognitionJob | null {
  if (!value || typeof value !== 'object') return null
  const job = value as Partial<PdfRecognitionJob>
  if (typeof job.documentUrl !== 'string' ||
      (job.language !== 'eng' && job.language !== 'ara') ||
      typeof job.sessionId !== 'string' ||
      !isRecognitionIssues(job.improveIssues)) return null
  return job as PdfRecognitionJob
}

function isRecognitionIssues(value: unknown): value is PdfRecognitionIssues | undefined {
  if (value === undefined) return true
  if (!value || typeof value !== 'object') return false
  const issues = value as Partial<PdfRecognitionIssues>
  return isPageList(issues.failedPages) &&
    isPageList(issues.unrecognizedPages) &&
    isPageList(issues.lowConfidencePages)
}

function isPageList(value: unknown): value is number[] {
  return Array.isArray(value) && value.every((page) => Number.isInteger(page) && page > 0)
}
