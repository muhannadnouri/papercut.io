import pdfWorkerUrl from 'pdfjs-dist/legacy/build/pdf.worker.min.mjs?url'

let pdfJsPromise: Promise<typeof import('pdfjs-dist/legacy/build/pdf.mjs')> | undefined

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
