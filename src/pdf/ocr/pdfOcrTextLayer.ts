import type { PdfPageTextLayer } from '../../uploads/DocumentUploads'

const OCR_LAYER_CLASS = 'pdf-ocr-text-layer'

/** Add selectable transparent OCR words over one textless PDF.js page.
 * Coordinates stay in the stored OCR viewport and one parent transform keeps
 * every word aligned as PDF.js changes the rendered page scale. */
export function renderPdfOcrTextLayer(
  page: HTMLElement,
  textLayer: HTMLElement,
  layer: PdfPageTextLayer,
): HTMLElement | null {
  page.querySelector(`.${OCR_LAYER_CLASS}`)?.remove()
  if (layer.blocks.length === 0) return null

  const scale = pdfOcrLayerScale(
    layer.width,
    layer.height,
    textLayer.offsetWidth,
    textLayer.offsetHeight,
  )
  const overlay = page.ownerDocument.createElement('div')
  overlay.className = OCR_LAYER_CLASS
  overlay.style.width = `${layer.width}px`
  overlay.style.height = `${layer.height}px`
  overlay.style.transform = `scale(${scale.x}, ${scale.y})`

  for (const block of [...layer.blocks].sort((a, b) => a.order - b.order)) {
    const [left, top, width, height] = block.bounds
    if (!block.text || width <= 0 || height <= 0) continue
    const word = page.ownerDocument.createElement('span')
    word.dir = 'auto'
    word.textContent = block.text
    word.style.left = `${left}px`
    word.style.top = `${top}px`
    word.style.width = `${width}px`
    word.style.height = `${height}px`
    word.style.fontSize = `${height}px`
    overlay.append(word)
  }

  page.append(overlay)
  return overlay
}

export function clearPdfOcrTextLayers(viewer: HTMLElement): void {
  viewer.querySelectorAll(`.${OCR_LAYER_CLASS}`).forEach((layer) => layer.remove())
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
