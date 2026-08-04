import type { PDFDocumentLoadingTask, PDFPageProxy } from 'pdfjs-dist/legacy/build/pdf.mjs'
import {
  finalizeUploadedPdf,
  getUploadedPdfAssetUrl,
  storeUploadedPdfPageText,
  type UploadedDocument,
} from '../../uploads/DocumentUploads'
import { loadPdfJs, pdfJsAssetRoot } from '../pdfJs'
import { hasPdfPageImages, hasUsableNativePdfText } from './pdfOcrReadiness'
import { createEnglishPdfOcrWorker, recognizeEnglishPdfPage } from './tesseractOcr'

const MAX_OCR_RENDER_PIXELS = 8_000_000
const TARGET_OCR_SCALE = 2.5

export const PDF_OCR_NO_TEXT = 'pdf-ocr-no-text'

export interface PdfRecognitionProgress {
  phase: 'preparing' | 'recognizing' | 'indexing'
  pageNumber: number
  pageCount: number
}

interface PdfRecognitionOptions {
  signal?: AbortSignal
  onProgress?: (progress: PdfRecognitionProgress) => void
}

/** Recognize only image-backed pages whose native extraction is not usable.
 * Existing native and blank page sidecars stay untouched, and finalization
 * rebuilds the shared page FTS only after the bounded pass succeeds. */
export async function recognizeEnglishPdfDocument(
  document: UploadedDocument,
  options: PdfRecognitionOptions = {},
): Promise<UploadedDocument> {
  if (document.sourceKind !== 'pdf' || document.textStatus !== 'recognition-required') {
    throw new Error('Document does not require PDF text recognition')
  }

  throwIfAborted(options.signal)
  options.onProgress?.({ phase: 'preparing', pageNumber: 0, pageCount: document.sections })
  const [pdfjs, sourceUrl] = await Promise.all([
    loadPdfJs(),
    getUploadedPdfAssetUrl(document.url),
  ])
  const assetRoot = pdfJsAssetRoot()
  const loadingTask = pdfjs.getDocument({
    url: sourceUrl,
    disableStream: true,
    rangeChunkSize: 1_048_576,
    standardFontDataUrl: `${assetRoot}standard_fonts/`,
    wasmUrl: `${assetRoot}wasm/`,
  })
  let worker: Awaited<ReturnType<typeof createEnglishPdfOcrWorker>> | undefined
  let workerTermination: Promise<unknown> | undefined
  const cancel = () => {
    void loadingTask.destroy()
    if (worker && !workerTermination) workerTermination = worker.terminate()
  }
  options.signal?.addEventListener('abort', cancel, { once: true })

  try {
    const pdf = await loadingTask.promise
    throwIfAborted(options.signal)
    let recognizedCharacters = 0
    let recognitionStillRequired = false

    for (let pageNumber = 1; pageNumber <= pdf.numPages; pageNumber += 1) {
      throwIfAborted(options.signal)
      const page = await pdf.getPage(pageNumber)

      try {
        const content = await page.getTextContent({ disableNormalization: true })
        if (hasUsableNativePdfText(content)) continue
        const operatorList = await page.getOperatorList()
        if (!hasPdfPageImages(operatorList.fnArray, pdfjs.OPS)) continue

        options.onProgress?.({ phase: 'recognizing', pageNumber, pageCount: pdf.numPages })
        worker ??= await createEnglishPdfOcrWorker()
        throwIfAborted(options.signal)
        const pageCharacters = await recognizeAndStorePage(
          document.url,
          page,
          worker,
          pageNumber - 1,
          options.signal,
        )
        recognizedCharacters += pageCharacters
        recognitionStillRequired ||= pageCharacters === 0
      } finally {
        page.cleanup()
      }
    }

    if (recognizedCharacters === 0) throw new Error(PDF_OCR_NO_TEXT)
    options.onProgress?.({ phase: 'indexing', pageNumber: pdf.numPages, pageCount: pdf.numPages })
    return await finalizeUploadedPdf(
      document.url,
      document.title,
      pdf.numPages,
      undefined,
      recognitionStillRequired,
    )
  } finally {
    options.signal?.removeEventListener('abort', cancel)
    if (workerTermination) await workerTermination.catch(() => undefined)
    else await worker?.terminate().catch(() => undefined)
    await destroyLoadingTask(loadingTask)
  }
}

/** Render and store one OCR page, releasing its canvas before the next page. */
async function recognizeAndStorePage(
  documentUrl: string,
  page: PDFPageProxy,
  worker: Awaited<ReturnType<typeof createEnglishPdfOcrWorker>>,
  pageIndex: number,
  signal?: AbortSignal,
): Promise<number> {
  const sourceViewport = page.getViewport({ scale: 1 })
  const scale = pdfOcrRenderScale(sourceViewport.width, sourceViewport.height)
  const viewport = page.getViewport({ scale })
  const canvas = window.document.createElement('canvas')
  canvas.width = Math.max(1, Math.round(viewport.width))
  canvas.height = Math.max(1, Math.round(viewport.height))

  try {
    await page.render({ canvas, viewport, background: '#ffffff' }).promise
    throwIfAborted(signal)
    const layer = await recognizeEnglishPdfPage(
      worker,
      canvas,
      pageIndex,
      sourceViewport.width,
      sourceViewport.height,
    )
    await storeUploadedPdfPageText(documentUrl, layer)
    return layer.blocks.reduce(
      (total, block) => total + block.text.replace(/\s/g, '').length,
      0,
    )
  } finally {
    canvas.width = 0
    canvas.height = 0
  }
}

/** Prefer enough pixels for OCR accuracy while bounding large-page memory. */
export function pdfOcrRenderScale(width: number, height: number): number {
  if (!(width > 0) || !(height > 0)) throw new Error('PDF page dimensions are invalid')
  return Math.min(TARGET_OCR_SCALE, Math.sqrt(MAX_OCR_RENDER_PIXELS / (width * height)))
}

function throwIfAborted(signal?: AbortSignal): void {
  if (signal?.aborted) throw new DOMException('PDF text recognition cancelled', 'AbortError')
}

async function destroyLoadingTask(loadingTask: PDFDocumentLoadingTask): Promise<void> {
  await loadingTask.destroy().catch(() => undefined)
}
