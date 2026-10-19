import type { PdfPageTextLayer } from '../../uploads/DocumentUploads'

export const PDF_OCR_TEXT_LAYER_CLASS = 'pdf-ocr-text-layer'

type PdfOcrTextBlock = PdfPageTextLayer['blocks'][number]

export interface PdfOcrTextLine {
  text: string
  bounds: [number, number, number, number]
}

/** Distinguish persisted OCR words from native PDF.js text blocks. */
export function hasPdfOcrText(layer: PdfPageTextLayer): boolean {
  return layer.blocks.some((block) => block.confidence !== null)
}

/** Add selectable transparent OCR lines over one textless PDF.js page.
 *
 * Tesseract stores word boxes, but independently sized word spans produce
 * overlapping native selection glyphs. Joining each persisted line and fitting
 * its measured text to the union of those boxes mirrors PDF.js's text-layer
 * strategy while keeping one parent transform for page-scale changes.
 */
export function renderPdfOcrTextLayer(
  page: HTMLElement,
  textLayer: HTMLElement,
  layer: PdfPageTextLayer,
): HTMLElement | null {
  page.querySelector(`.${PDF_OCR_TEXT_LAYER_CLASS}`)?.remove()
  if (layer.blocks.length === 0) return null

  const scale = pdfOcrLayerScale(
    layer.width,
    layer.height,
    textLayer.offsetWidth,
    textLayer.offsetHeight,
  )
  const overlay = page.ownerDocument.createElement('div')
  overlay.className = PDF_OCR_TEXT_LAYER_CLASS
  overlay.style.width = `${layer.width}px`
  overlay.style.height = `${layer.height}px`
  overlay.style.transform = `scale(${scale.x}, ${scale.y})`

  const measurement = page.ownerDocument.createElement('canvas').getContext('2d')
  for (const line of pdfOcrTextLines(layer.blocks)) {
    const [left, top, width, height] = line.bounds
    const text = page.ownerDocument.createElement('span')
    text.dir = 'auto'
    text.textContent = line.text
    text.style.fontFamily = 'sans-serif'
    text.style.fontSize = `${height}px`
    text.style.left = `${left}px`
    text.style.top = `${top}px`
    if (measurement) {
      measurement.font = `${height}px sans-serif`
      const measuredWidth = measurement.measureText(line.text).width
      text.style.transform = `scaleX(${pdfOcrTextScale(width, measuredWidth)})`
    }
    overlay.append(text)
    const lineBreak = page.ownerDocument.createElement('br')
    lineBreak.setAttribute('role', 'presentation')
    overlay.append(lineBreak)
  }

  page.append(overlay)
  return overlay
}

export function clearPdfOcrTextLayers(viewer: HTMLElement): void {
  viewer.querySelectorAll(`.${PDF_OCR_TEXT_LAYER_CLASS}`).forEach((layer) => layer.remove())
}

/** Return a PDF.js text layer only after its asynchronous render is complete. */
export function renderedPdfTextLayer(
  viewer: HTMLElement,
  pageNumber: number,
): HTMLElement | null {
  const textLayer = viewer.querySelector<HTMLElement>(
    `.page[data-page-number="${pageNumber}"] .textLayer`,
  )
  return textLayer?.querySelector('.endOfContent') ? textLayer : null
}

export function renderedPdfOcrTextLayer(
  viewer: HTMLElement,
  pageNumber: number,
): HTMLElement | null {
  return viewer.querySelector<HTMLElement>(
    `.page[data-page-number="${pageNumber}"] .${PDF_OCR_TEXT_LAYER_CLASS}`,
  )
}

/** Prefer derived OCR text for mixed PDFs, then fall back to usable native text. */
export function renderedPdfSearchLayer(
  viewer: HTMLElement,
  pageNumber: number,
): HTMLElement | null {
  const ocrLayer = renderedPdfOcrTextLayer(viewer, pageNumber)
  if (ocrLayer) return ocrLayer
  const textLayer = renderedPdfTextLayer(viewer, pageNumber)
  return textLayer?.textContent?.trim() ? textLayer : null
}

/** Convert ordered OCR word boxes into selectable line runs without changing persistence. */
export function pdfOcrTextLines(blocks: readonly PdfOcrTextBlock[]): PdfOcrTextLine[] {
  const lines: PdfOcrTextLine[] = []
  let lineBlocks: PdfOcrTextBlock[] = []

  const finishLine = () => {
    if (lineBlocks.length === 0) return
    const left = Math.min(...lineBlocks.map((block) => block.bounds[0]))
    const top = Math.min(...lineBlocks.map((block) => block.bounds[1]))
    const right = Math.max(...lineBlocks.map((block) => block.bounds[0] + block.bounds[2]))
    const bottom = Math.max(...lineBlocks.map((block) => block.bounds[1] + block.bounds[3]))
    let text = ''
    for (const block of lineBlocks) {
      const value = block.text.replace(/[\r\n]+/gu, '')
      if (text && !/\s$/u.test(text) && !/^\s/u.test(value)) text += ' '
      text += value
    }
    text = text.trimEnd()
    if (text) lines.push({ text, bounds: [left, top, right - left, bottom - top] })
    lineBlocks = []
  }

  for (const block of [...blocks].sort((a, b) => a.order - b.order)) {
    const [left, top, width, height] = block.bounds
    if (
      !block.text || width <= 0 || height <= 0 ||
      ![left, top, width, height].every(Number.isFinite)
    ) continue
    lineBlocks.push(block)
    if (/[\r\n]/u.test(block.text)) finishLine()
  }
  finishLine()
  return lines
}

export function pdfOcrTextScale(targetWidth: number, measuredWidth: number): number {
  return targetWidth > 0 && measuredWidth > 0 ? targetWidth / measuredWidth : 1
}

export function pdfOcrLayerScale(
  sourceWidth: number,
  sourceHeight: number,
  renderedWidth: number,
  renderedHeight: number,
): { x: number; y: number } {
  if (sourceWidth <= 0 || sourceHeight <= 0 || renderedWidth <= 0 || renderedHeight <= 0) {
    return { x: 1, y: 1 }
  }
  return { x: renderedWidth / sourceWidth, y: renderedHeight / sourceHeight }
}
