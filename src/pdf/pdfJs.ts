import pdfWorkerUrl from 'pdfjs-dist/legacy/build/pdf.worker.min.mjs?url'

let pdfJsPromise: Promise<typeof import('pdfjs-dist/legacy/build/pdf.mjs')> | undefined
let pdfViewerPromise: Promise<typeof import('pdfjs-dist/legacy/web/pdf_viewer.mjs')> | undefined

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
