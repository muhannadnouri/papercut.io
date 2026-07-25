import type { PDFViewer } from 'pdfjs-dist/legacy/web/pdf_viewer.mjs'
import type { PdfTtsSourceSpan } from '../tts/types'

const PDF_TTS_HIGHLIGHT_LAYER_CLASS = 'pdf-tts-highlight-layer'

interface PdfTextHighlighterMapping {
  textDivs: Node[] | null
  textContentItemsStr: string[] | null
}

interface PdfPageViewWithTextMapping {
  _textHighlighter?: PdfTextHighlighterMapping
}

export interface PdfTtsHighlightRect {
  bottom: number
  left: number
  right: number
  top: number
}

/** Clear only Papercut's TTS range without disturbing PDF.js Find highlights. */
export function clearPdfTtsHighlight(doc: Document): void {
  doc.querySelectorAll(`.${PDF_TTS_HIGHLIGHT_LAYER_CLASS}`).forEach((layer) => layer.remove())
}

/**
 * Map persisted PDF.js item offsets onto the currently rendered text layers.
 *
 * PDF.js virtualizes pages, so absent text layers are skipped and this function
 * is called again on `textlayerrendered`. Its pinned TextHighlighter mapping is
 * used instead of guessing from DOM children, which Find may split into spans.
 * Same-line range rectangles are joined visually because PDF.js positions most
 * words as separate elements and CSS text highlights otherwise leave seams.
 */
export function applyPdfTtsHighlight(
  pdfViewer: PDFViewer,
  spans: PdfTtsSourceSpan[],
  doc: Document,
): Range | null {
  clearPdfTtsHighlight(doc)
  let firstRange: Range | null = null
  const pageRects = new Map<HTMLElement, PdfTtsHighlightRect[]>()

  for (const span of spans) {
    const pageView = pdfViewer.getPageView(span.pageIndex) as PdfPageViewWithTextMapping | undefined
    const mapping = pageView?._textHighlighter
    const node = mapping?.textDivs?.[span.blockOrder]
    const sourceText = mapping?.textContentItemsStr?.[span.blockOrder]
    if (!node?.isConnected || sourceText === undefined) continue

    const start = textPoint(node, Math.min(span.startOffset, sourceText.length))
    const end = textPoint(node, Math.min(span.endOffset, sourceText.length))
    if (!start || !end) continue

    const range = doc.createRange()
    range.setStart(start.node, start.offset)
    range.setEnd(end.node, end.offset)
    if (range.collapsed) continue
    firstRange ??= range

    const page = start.node.parentElement?.closest<HTMLElement>('.page')
    if (!page) continue
    const rects = pageRects.get(page) ?? []
    for (const rect of range.getClientRects()) {
      if (rect.width <= 0 || rect.height <= 0) continue
      rects.push({
        bottom: rect.bottom,
        left: rect.left,
        right: rect.right,
        top: rect.top,
      })
    }
    pageRects.set(page, rects)
  }

  for (const [page, rects] of pageRects) renderHighlightBands(page, rects, doc)
  return firstRange
}

/**
 * Join neighboring word rectangles without spanning columns or separate lines.
 *
 * The gap ceiling scales with text height so the rule follows PDF zoom while
 * still rejecting large table/column gaps that happen to share a baseline.
 */
export function mergePdfTtsHighlightRects(
  rects: PdfTtsHighlightRect[],
): PdfTtsHighlightRect[] {
  const bands: PdfTtsHighlightRect[] = []
  for (const rect of [...rects].sort((a, b) => a.top - b.top || a.left - b.left)) {
    const match = bands.find((band) => {
      const overlap = Math.min(band.bottom, rect.bottom) - Math.max(band.top, rect.top)
      const minHeight = Math.min(band.bottom - band.top, rect.bottom - rect.top)
      const horizontalGap = Math.max(band.left - rect.right, rect.left - band.right, 0)
      return overlap >= minHeight * 0.45 &&
        horizontalGap <= Math.max(band.bottom - band.top, rect.bottom - rect.top) * 1.25
    })
    if (!match) {
      bands.push({ ...rect })
      continue
    }
    match.bottom = Math.max(match.bottom, rect.bottom)
    match.left = Math.min(match.left, rect.left)
    match.right = Math.max(match.right, rect.right)
    match.top = Math.min(match.top, rect.top)
  }
  return bands.sort((a, b) => a.top - b.top || a.left - b.left)
}

function renderHighlightBands(
  page: HTMLElement,
  rects: PdfTtsHighlightRect[],
  doc: Document,
): void {
  const pageRect = page.getBoundingClientRect()
  const layer = doc.createElement('div')
  layer.className = PDF_TTS_HIGHLIGHT_LAYER_CLASS
  layer.setAttribute('aria-hidden', 'true')

  for (const rect of mergePdfTtsHighlightRects(rects)) {
    const band = doc.createElement('span')
    band.className = 'pdf-tts-highlight-band'
    band.style.left = `${rect.left - pageRect.left}px`
    band.style.top = `${rect.top - pageRect.top}px`
    band.style.width = `${rect.right - rect.left}px`
    band.style.height = `${rect.bottom - rect.top}px`
    layer.append(band)
  }
  if (layer.childElementCount > 0) page.append(layer)
}

/** Resolve an offset against nested nodes after PDF.js Find rewrites a text div. */
function textPoint(root: Node, targetOffset: number): { node: Text; offset: number } | null {
  const doc = root.ownerDocument
  if (!doc) return null
  if (root.nodeType === Node.TEXT_NODE) {
    const node = root as Text
    return { node, offset: Math.min(targetOffset, node.length) }
  }
  const walker = doc.createTreeWalker(root, NodeFilter.SHOW_TEXT)
  let remaining = targetOffset
  let last: Text | null = null

  while (walker.nextNode()) {
    const node = walker.currentNode as Text
    last = node
    if (remaining <= node.length) return { node, offset: remaining }
    remaining -= node.length
  }
  return last ? { node: last, offset: last.length } : null
}
