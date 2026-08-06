import type { PDFDocumentLoadingTask, PDFPageProxy } from 'pdfjs-dist/legacy/build/pdf.mjs'
import {
  finalizeUploadedPdf,
  getUploadedPdfAssetUrl,
  getUploadedPdfPageText,
  storeUploadedPdfPageText,
  type PdfPageTextLayer,
  type UploadedDocument,
} from '../../uploads/DocumentUploads'
import { loadPdfJs, pdfJsAssetRoot } from '../pdfJs'
import { hasPdfPageImages, hasUsableNativePdfText } from './pdfOcrReadiness'
import {
  createPdfOcrWorker,
  recognizePdfPage,
  type PdfOcrLanguage,
} from './tesseractOcr'

const PDF_POINTS_PER_INCH = 72
const TARGET_OCR_DPI = 300
const MAX_OCR_RENDER_PIXELS = 9_000_000
const LOW_OCR_CONFIDENCE = 0.5

export const PDF_OCR_NO_TEXT = 'pdf-ocr-no-text'

export interface PdfRecognitionProgress {
  phase: 'preparing' | 'recognizing' | 'indexing'
  pageNumber: number
  pageCount: number
}

export interface PdfRecognitionIssues {
  failedPages: number[]
  unrecognizedPages: number[]
  lowConfidencePages: number[]
}

export interface PdfRecognitionResult {
  document: UploadedDocument
  issues: PdfRecognitionIssues
}

export class PdfRecognitionNoTextError extends Error {
  readonly issues: PdfRecognitionIssues

  constructor(issues: PdfRecognitionIssues) {
    super(PDF_OCR_NO_TEXT)
    this.name = 'PdfRecognitionNoTextError'
    this.issues = issues
  }
}

interface PdfRecognitionOptions {
  signal?: AbortSignal
  onProgress?: (progress: PdfRecognitionProgress) => void
}

/** Recognize only image-backed pages whose native extraction is not usable.
 * Existing native and blank page sidecars stay untouched, and finalization
 * rebuilds the shared page FTS only after the bounded pass succeeds. */
export async function recognizePdfDocument(
  document: UploadedDocument,
  language: PdfOcrLanguage,
  options: PdfRecognitionOptions = {},
): Promise<PdfRecognitionResult> {
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
  let worker: Awaited<ReturnType<typeof createPdfOcrWorker>> | undefined
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
    const issues: PdfRecognitionIssues = {
      failedPages: [],
      unrecognizedPages: [],
      lowConfidencePages: [],
    }

    for (let pageNumber = 1; pageNumber <= pdf.numPages; pageNumber += 1) {
      throwIfAborted(options.signal)
      const page = await pdf.getPage(pageNumber)

      try {
        const content = await page.getTextContent({ disableNormalization: true })
        if (hasUsableNativePdfText(content)) continue
        const operatorList = await page.getOperatorList()
        if (!hasPdfPageImages(operatorList.fnArray, pdfjs.OPS)) continue

        const existing = await getUploadedPdfPageText(document.url, pageNumber - 1)
          .catch(() => null)
        const existingQuality = existing ? pdfOcrPageQuality(existing) : null
        if (existingQuality &&
            existingQuality.confidence !== null &&
            existingQuality.characters > 0) {
          recognizedCharacters += existingQuality.characters
          if (existingQuality.confidence < LOW_OCR_CONFIDENCE) {
            issues.lowConfidencePages.push(pageNumber)
          }
          continue
        }

        options.onProgress?.({ phase: 'recognizing', pageNumber, pageCount: pdf.numPages })
        worker ??= await createPdfOcrWorker(language)
        throwIfAborted(options.signal)
        try {
          const layer = await recognizePage(page, worker, pageNumber - 1, options.signal)
          const quality = pdfOcrPageQuality(layer)
          if (quality.characters === 0) {
            issues.unrecognizedPages.push(pageNumber)
            continue
          }
          await storeUploadedPdfPageText(document.url, layer)
          recognizedCharacters += quality.characters
          if (quality.confidence !== null && quality.confidence < LOW_OCR_CONFIDENCE) {
            issues.lowConfidencePages.push(pageNumber)
          }
        } catch (error) {
          throwIfAborted(options.signal)
          console.warn(`Unable to recognize PDF page ${pageNumber}:`, error)
          issues.failedPages.push(pageNumber)
          if (existingQuality &&
              existingQuality.confidence !== null &&
              existingQuality.characters > 0) {
            recognizedCharacters += existingQuality.characters
          }
        }
      } finally {
        page.cleanup()
      }
    }

    issues.lowConfidencePages = [...new Set(issues.lowConfidencePages)]
    if (recognizedCharacters === 0) throw new PdfRecognitionNoTextError(issues)
    options.onProgress?.({ phase: 'indexing', pageNumber: pdf.numPages, pageCount: pdf.numPages })
    const updated = await finalizeUploadedPdf(
      document.url,
      document.title,
      pdf.numPages,
      undefined,
      issues.failedPages.length > 0 ||
        issues.unrecognizedPages.length > 0 ||
        issues.lowConfidencePages.length > 0,
    )
    return { document: updated, issues }
  } finally {
    options.signal?.removeEventListener('abort', cancel)
    if (workerTermination) await workerTermination.catch(() => undefined)
    else await worker?.terminate().catch(() => undefined)
    await destroyLoadingTask(loadingTask)
  }
}

/** Render one OCR page, releasing its bounded canvas before the next page. */
async function recognizePage(
  page: PDFPageProxy,
  worker: Awaited<ReturnType<typeof createPdfOcrWorker>>,
  pageIndex: number,
  signal?: AbortSignal,
): Promise<PdfPageTextLayer> {
  const sourceViewport = page.getViewport({ scale: 1 })
  const scale = pdfOcrRenderScale(sourceViewport.width, sourceViewport.height)
  const viewport = page.getViewport({ scale })
  const canvas = window.document.createElement('canvas')
  canvas.width = Math.max(1, Math.round(viewport.width))
  canvas.height = Math.max(1, Math.round(viewport.height))

  try {
    await page.render({ canvas, viewport, background: '#ffffff' }).promise
    throwIfAborted(signal)
    return await recognizePdfPage(
      worker,
      canvas,
      pageIndex,
      sourceViewport.width,
      sourceViewport.height,
    )
  } finally {
    canvas.width = 0
    canvas.height = 0
  }
}

/** Summarize persisted OCR without treating confidence as proof of correctness.
 * Character weighting prevents a short punctuation token from dominating the
 * page score; the conservative threshold asks for review rather than repeating
 * the same deterministic recognition pass. */
export function pdfOcrPageQuality(layer: PdfPageTextLayer): {
  characters: number
  confidence: number | null
} {
  let characters = 0
  let confidenceCharacters = 0
  let weightedConfidence = 0

  for (const block of layer.blocks) {
    const length = block.text.replace(/\s/gu, '').length
    characters += length
    if (block.confidence === null || length === 0) continue
    confidenceCharacters += length
    weightedConfidence += block.confidence * length
  }

  return {
    characters,
    confidence: confidenceCharacters > 0 ? weightedConfidence / confidenceCharacters : null,
  }
}

/** Render ordinary pages at 300 DPI while bounding unusually large pages to
 * one 9-megapixel canvas, keeping peak OCR memory independent of page count. */
export function pdfOcrRenderScale(width: number, height: number): number {
  if (!(width > 0) || !(height > 0)) throw new Error('PDF page dimensions are invalid')
  return Math.min(
    TARGET_OCR_DPI / PDF_POINTS_PER_INCH,
    Math.sqrt(MAX_OCR_RENDER_PIXELS / (width * height)),
  )
}

function throwIfAborted(signal?: AbortSignal): void {
  if (signal?.aborted) throw new DOMException('PDF text recognition cancelled', 'AbortError')
}

async function destroyLoadingTask(loadingTask: PDFDocumentLoadingTask): Promise<void> {
  await loadingTask.destroy().catch(() => undefined)
}
