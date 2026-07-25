import type { PDFViewer } from 'pdfjs-dist/legacy/web/pdf_viewer.mjs'
import type { PdfTtsSourceSpan } from '../tts/types'

const PDF_TTS_HIGHLIGHT_NAME = 'pdf-tts-current'

interface PdfTextHighlighterMapping {
  textDivs: Node[] | null
  textContentItemsStr: string[] | null
}

interface PdfPageViewWithTextMapping {
  _textHighlighter?: PdfTextHighlighterMapping
}

/** Clear only Papercut's TTS range without disturbing PDF.js Find highlights. */
export function clearPdfTtsHighlight(doc: Document): void {
  const registry = doc.defaultView?.CSS.highlights
  if (!registry) return
  registry.get(PDF_TTS_HIGHLIGHT_NAME)?.clear()
  registry.delete(PDF_TTS_HIGHLIGHT_NAME)
}

/**
 * Map persisted PDF.js item offsets onto the currently rendered text layers.
 *
 * PDF.js virtualizes pages, so absent text layers are skipped and this function
 * is called again on `textlayerrendered`. Its pinned TextHighlighter mapping is
 * used instead of guessing from DOM children, which Find may split into spans.
 */
export function applyPdfTtsHighlight(
  pdfViewer: PDFViewer,
  spans: PdfTtsSourceSpan[],
  doc: Document,
): Range | null {
  const view = doc.defaultView
  const registry = view?.CSS.highlights
  if (!view || !registry) return null

  clearPdfTtsHighlight(doc)
  const highlight = new view.Highlight()
  let firstRange: Range | null = null

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
    highlight.add(range)
    firstRange ??= range
  }

  if (firstRange) registry.set(PDF_TTS_HIGHLIGHT_NAME, highlight)
  return firstRange
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
