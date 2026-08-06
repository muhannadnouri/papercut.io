import pdfWorkerUrl from 'pdfjs-dist/legacy/build/pdf.worker.min.mjs?url'

let pdfJsPromise: Promise<typeof import('pdfjs-dist/legacy/build/pdf.mjs')> | undefined
let pdfViewerPromise: Promise<typeof import('pdfjs-dist/legacy/web/pdf_viewer.mjs')> | undefined

const PDF_LOAD_ERROR_MESSAGES: Record<string, string> = {
  AbortException: 'PDF loading was interrupted.',
  InvalidPDFException: 'This PDF is damaged or invalid.',
  PasswordException: 'Password-protected PDFs are not supported.',
  ResponseException: 'The PDF could not be loaded.',
}

export function pdfJsAssetRoot(): string {
  return new URL('pdfjs/', document.baseURI).href
}

/** Load the selected parser lazily so non-PDF app startup stays unchanged. */
export function loadPdfJs() {
  pdfJsPromise ??= import('pdfjs-dist/legacy/build/pdf.mjs').then((pdfjs) => {
    pdfjs.GlobalWorkerOptions.workerSrc = pdfWorkerUrl
    return pdfjs
  })
  return pdfJsPromise
}

/**
 * Load viewer primitives after the legacy PDF.js build has populated the
 * global expected by its viewer module.
 */
export function loadPdfViewer() {
  pdfViewerPromise ??= loadPdfJs().then(() => import('pdfjs-dist/legacy/web/pdf_viewer.mjs'))
  return pdfViewerPromise
}

/**
 * Convert PDF.js parser failures into stable messages at import and view boundaries.
 * Matching exception names keeps this helper synchronous without eagerly loading
 * the large PDF.js module during ordinary non-PDF app startup.
 */
export function pdfLoadErrorMessage(error: unknown): string {
  if (!(error instanceof Error)) return String(error)
  if (error.name === 'ResponseException' && hasMissingFlag(error)) {
    return 'The PDF file is missing or unavailable.'
  }
  return PDF_LOAD_ERROR_MESSAGES[error.name] ?? error.message
}

function hasMissingFlag(error: Error): boolean {
  return 'missing' in error && error.missing === true
}
