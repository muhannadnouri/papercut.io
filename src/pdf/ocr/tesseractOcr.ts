import type { LoggerMessage, Page, Worker } from 'tesseract.js'
import type { PdfPageTextLayer } from '../../uploads/DocumentUploads'

export type PdfOcrProgress = Pick<LoggerMessage, 'progress' | 'status'>
export type PdfOcrLanguage = 'eng' | 'ara'

function tesseractAssetRoot(): string {
  return new URL('tesseract/', document.baseURI).href
}

/** Create one reusable offline worker for a packaged language; callers should
 * keep it for the entire job so model startup is paid only once per document. */
export async function createPdfOcrWorker(
  language: PdfOcrLanguage,
  onProgress?: (progress: PdfOcrProgress) => void,
  automaticLayout = false,
): Promise<Worker> {
  const { createWorker, OEM, PSM } = await import('tesseract.js')
  const root = tesseractAssetRoot()
  const worker = await createWorker(language, OEM.LSTM_ONLY, {
    workerPath: `${root}worker.min.js`,
    corePath: `${root}core/`,
    langPath: `${root}lang`,
    logger: ({ progress, status }) => onProgress?.({ progress, status }),
  })
  if (automaticLayout) {
    await worker.setParameters({ tessedit_pageseg_mode: PSM.AUTO })
  }
  return worker
}

/** Recognize one rendered page and normalize it into Papercut's shared sidecar. */
export async function recognizePdfPage(
  worker: Worker,
  canvas: HTMLCanvasElement,
  pageIndex: number,
  pageWidth: number,
  pageHeight: number,
): Promise<PdfPageTextLayer> {
  const result = await worker.recognize(canvas, {}, { blocks: true })
  return ocrPageTextLayer(
    result.data,
    pageIndex,
    pageWidth,
    pageHeight,
    canvas.width,
    canvas.height,
  )
}

/** Preserve Tesseract reading order while converting image pixels to page coordinates. */
export function ocrPageTextLayer(
  page: Pick<Page, 'blocks'>,
  pageIndex: number,
  pageWidth: number,
  pageHeight: number,
  imageWidth: number,
  imageHeight: number,
): PdfPageTextLayer {
  if (!(pageWidth > 0) || !(pageHeight > 0) || !(imageWidth > 0) || !(imageHeight > 0)) {
    throw new Error('OCR page dimensions are invalid')
  }
  const scaleX = pageWidth / imageWidth
  const scaleY = pageHeight / imageHeight
  const lines = (page.blocks ?? []).flatMap((block) => block.paragraphs.flatMap((paragraph) => paragraph.lines))
  let order = 0

  return {
    schemaVersion: 1,
    pageIndex,
    width: pageWidth,
    height: pageHeight,
    blocks: lines.flatMap((line) => line.words.map((word, index) => ({
      text: word.text + (index === line.words.length - 1 ? '\n' : ' '),
      bounds: [
        word.bbox.x0 * scaleX,
        word.bbox.y0 * scaleY,
        (word.bbox.x1 - word.bbox.x0) * scaleX,
        (word.bbox.y1 - word.bbox.y0) * scaleY,
      ],
      order: order++,
      confidence: Math.max(0, Math.min(1, word.confidence / 100)),
    }))),
  }
}
