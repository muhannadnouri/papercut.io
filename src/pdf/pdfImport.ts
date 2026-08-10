import type {
  PDFPageProxy,
  TextContent,
  TextItem,
} from 'pdfjs-dist/types/src/display/api'
import {
  deleteUploadedDocument,
  finalizeUploadedPdf,
  getUploadedPdfSource,
  storeUploadedPdfPageText,
  type PdfPageTextLayer,
  type UploadedDocument,
  type UploadedDocumentBatchProgress,
  type UploadedDocumentBatchResult,
} from '../uploads/DocumentUploads'
import { loadPdfJs, pdfJsAssetRoot, pdfLoadErrorMessage } from './pdfJs'
import {
  finalizedPdfTextStatus,
  hasPdfPageImages,
  hasUsableNativePdfText,
} from './ocr/pdfOcrReadiness'

const MAX_PDF_PAGES = 2_000
const THUMBNAIL_MAX_WIDTH = 480
const THUMBNAIL_MAX_HEIGHT = 720

interface PdfImportOptions {
  signal?: AbortSignal
  onProgress?: (progress: UploadedDocumentBatchProgress) => void
  titleOverride?: string
}

/**
 * Complete native-staged PDFs sequentially so PDF.js work remains bounded and
 * one bad PDF becomes a normal batch failure without discarding other imports.
 */
export async function indexImportedPdfs(
  result: UploadedDocumentBatchResult,
  options: PdfImportOptions = {},
): Promise<UploadedDocumentBatchResult> {
  const completed = result.imported.filter((document) => document.sourceKind !== 'pdf' || document.sections > 0)
  const pending = result.imported.filter((document) => document.sourceKind === 'pdf' && document.sections === 0)
  const failures = [...result.failures]

  for (let index = 0; index < pending.length; index += 1) {
    const document = pending[index]
    if (options.signal?.aborted) {
      await removePendingPdfs(pending.slice(index))
      return { ...result, imported: completed, failures, cancelled: true }
    }
    options.onProgress?.({
      phase: 'importing',
      processed: completed.length + failures.length,
      total: result.selected,
      imported: Math.max(0, completed.length - result.alreadyInLibrary.length),
      alreadyInLibrary: result.alreadyInLibrary.length,
      failed: failures.length,
      fileName: document.title,
    })

    try {
      completed.push(await extractAndIndexPdf(document, options.signal, options.titleOverride))
    } catch (error) {
      await deleteUploadedDocument(document.url).catch(() => undefined)
      if (options.signal?.aborted) {
        await removePendingPdfs(pending.slice(index + 1))
        return { ...result, imported: completed, failures, cancelled: true }
      }
      failures.push({
        fileName: document.title,
        error: pdfLoadErrorMessage(error),
      })
    }
  }

  return { ...result, imported: completed, failures }
}

async function extractAndIndexPdf(
  document: UploadedDocument,
  signal?: AbortSignal,
  titleOverride?: string,
): Promise<UploadedDocument> {
  throwIfAborted(signal)
  const pdfjs = await loadPdfJs()
  const assetRoot = pdfJsAssetRoot()
  const loadingTask = pdfjs.getDocument({
    data: await getUploadedPdfSource(document.url),
    standardFontDataUrl: `${assetRoot}standard_fonts/`,
    wasmUrl: `${assetRoot}wasm/`,
  })

  try {
    const pdf = await loadingTask.promise
    assertPdfPageCount(pdf.numPages)
    const metadata = await pdf.getMetadata().catch(() => null)
    const title = titleOverride ?? metadataTitle(metadata?.info)
    let thumbnail: number[] | undefined
    let hasUsableText = false
    let hasRecognitionCandidate = false

    for (let pageNumber = 1; pageNumber <= pdf.numPages; pageNumber += 1) {
      throwIfAborted(signal)
      const page = await pdf.getPage(pageNumber)
      try {
        const viewport = page.getViewport({ scale: 1 })
        if (pageNumber === 1) {
          thumbnail = await renderFirstPageThumbnail(page).catch((error) => {
            console.warn('Skipping PDF library thumbnail:', error)
            return undefined
          })
        }
        const content = await page.getTextContent({ disableNormalization: true })
        const usableText = hasUsableNativePdfText(content)
        hasUsableText ||= usableText
        if (!usableText) {
          const operatorList = await page.getOperatorList()
          hasRecognitionCandidate ||= hasPdfPageImages(operatorList.fnArray, pdfjs.OPS)
        }
        await storeUploadedPdfPageText(document.url, pageTextLayer(
          pdfjs.Util.transform,
          content,
          pageNumber - 1,
          viewport.width,
          viewport.height,
          viewport.transform,
        ))
      } finally {
        page.cleanup()
      }
    }

    return finalizeUploadedPdf(
      document.url,
      title,
      pdf.numPages,
      thumbnail,
      finalizedPdfTextStatus(hasUsableText, hasRecognitionCandidate),
    )
  } finally {
    await loadingTask.destroy()
  }
}

export function assertPdfPageCount(pageCount: number): void {
  if (pageCount > MAX_PDF_PAGES) {
    throw new Error(`PDF exceeds the ${MAX_PDF_PAGES}-page import limit`)
  }
}

type Transform = (m1: number[], m2: number[]) => number[]

/** Convert PDF.js items into the versioned page-coordinate contract shared by
 * search now and viewer/TTS highlighting later. */
export function pageTextLayer(
  transform: Transform,
  content: TextContent,
  pageIndex: number,
  width: number,
  height: number,
  viewportTransform: number[],
): PdfPageTextLayer {
  const blocks = content.items
    .filter((item): item is TextItem => 'str' in item)
    .map((item, order) => {
      const matrix = transform(viewportTransform, item.transform)
      const blockHeight = Math.hypot(matrix[2], matrix[3])
      return {
        text: item.str + (item.hasEOL ? '\n' : ''),
        bounds: [matrix[4], matrix[5] - blockHeight, Math.abs(item.width), blockHeight] as [number, number, number, number],
        order,
        confidence: null,
      }
    })

  return { schemaVersion: 1, pageIndex, width, height, blocks }
}

/** Fit one PDF page into the gallery's retained-cover bounds without upscaling. */
export function pdfThumbnailSize(width: number, height: number): {
  width: number
  height: number
  scale: number
} {
  if (!(width > 0) || !(height > 0)) {
    throw new Error('PDF first page has invalid dimensions')
  }
  const scale = Math.min(1, THUMBNAIL_MAX_WIDTH / width, THUMBNAIL_MAX_HEIGHT / height)
  return {
    width: Math.max(1, Math.round(width * scale)),
    height: Math.max(1, Math.round(height * scale)),
    scale,
  }
}

/** Render only page one and immediately release its canvas; cover creation is
 * best-effort so a WebView image-encoding limitation cannot reject an import. */
async function renderFirstPageThumbnail(page: PDFPageProxy): Promise<number[]> {
  const sourceViewport = page.getViewport({ scale: 1 })
  const size = pdfThumbnailSize(sourceViewport.width, sourceViewport.height)
  const viewport = page.getViewport({ scale: size.scale })
  const canvas = document.createElement('canvas')
  canvas.width = size.width
  canvas.height = size.height

  try {
    await page.render({ canvas, viewport, background: '#ffffff' }).promise
    const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, 'image/png'))
    if (!blob) throw new Error('WebView could not encode the PDF thumbnail')
    return Array.from(new Uint8Array(await blob.arrayBuffer()))
  } finally {
    canvas.width = 0
    canvas.height = 0
  }
}

function metadataTitle(info: unknown): string | undefined {
  if (!info || typeof info !== 'object' || !('Title' in info)) return undefined
  const title = (info as { Title?: unknown }).Title
  return typeof title === 'string' && title.trim() ? title.trim() : undefined
}

function throwIfAborted(signal?: AbortSignal): void {
  if (signal?.aborted) throw new DOMException('PDF import cancelled', 'AbortError')
}

async function removePendingPdfs(documents: UploadedDocument[]): Promise<void> {
  await Promise.all(documents.map((document) =>
    deleteUploadedDocument(document.url).catch(() => undefined)))
}
