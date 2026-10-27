import type {
  EventBus,
  PDFViewer,
} from 'pdfjs-dist/legacy/web/pdf_viewer.mjs'
import {
  pdfOcrSourcePoint,
  renderedPdfOcrTextLayer,
  renderedPdfTextLayer,
} from '../../pdf/ocr/pdfOcrTextLayer'
import type { PdfPageViewWithTextMapping } from '../../pdf/pdfTtsHighlight'
import type {
  ViewerTextTargetApi,
  ViewerTextTargetResult,
  ViewerViewportBounds,
} from '../../viewers/types'
import {
  readerSelection,
  selectedTextStart,
} from './selectionTarget'
import {
  clippedBounds,
  intersect,
  visibleText,
} from './textTargetGeometry'

const invalidatingEvents = ['textlayerrendered', 'pdfocrtextlayerrendered',
  'updatetextlayermatches', 'scalechanging', 'rotationchanging', 'pagesdestroy']

/** Optional adapter over the pinned PDF.js visibility/item maps; never render pages for capture. */
export function createPdfTextTargetApi(
  viewer: PDFViewer,
  container: HTMLElement,
  eventBus: EventBus,
  getViewport: () => ViewerViewportBounds | null,
): { api: ViewerTextTargetApi;
  dispose: () => void } {
  const doc = container.ownerDocument
  let revision = 0
  let disposed = false
  const invalidate = () => {
    revision++
  }
  for (const event of invalidatingEvents) {
    eventBus.on(event, invalidate)
  }
  const visibilityChanged = () => {
    if (doc.visibilityState === 'hidden') {
      invalidate()
    }
  }
  doc.addEventListener('visibilitychange', visibilityChanged)
  const isCurrent = (version: number) => !disposed && container.isConnected && revision === version

  const capturePoint = (
    pageIndex: number, page: PdfPageViewWithTextMapping & { div: HTMLElement },
    layer: HTMLElement, ocr: boolean, blockOrder: number, element: Node, offset: number,
  ): ViewerTextTargetResult => {
    const version = revision
    const mapping = page._textHighlighter
    const sourceText = ocr ? element.textContent : mapping?.textContentItemsStr?.[blockOrder]
    if (!element.isConnected || !layer.contains(element) || typeof sourceText !== 'string' ||
      element.textContent !== sourceText) {
      return {
        status: 'unavailable',
        reason: 'unmapped',
      }
    }
    const source = ocr ? pdfOcrSourcePoint(element as Element, offset)
      : {
        pageIndex,
        blockOrder,
        offset,
        sourceText,
      }
    if (!source || source.pageIndex !== pageIndex || source.offset >= source.sourceText.length) {
      return {
        status: 'unavailable',
        reason: 'unmapped',
      }
    }
    const textSnapshot = element.textContent
    return {
      status: 'captured',
      point: {
        format: 'pdf',
        pageIndex,
        blockOrder: source.blockOrder,
        offset: source.offset,
      },
      sourceText: source.sourceText,
      isCurrent: () => isCurrent(version) && container.contains(layer) && layer.contains(element) &&
        element.textContent === textSnapshot && (ocr
        ? renderedPdfOcrTextLayer(page.div, pageIndex + 1) === layer
        : renderedPdfTextLayer(page.div, pageIndex + 1) === layer && page._textHighlighter === mapping &&
            mapping?.textDivs?.[blockOrder] === element && mapping.textContentItemsStr?.[blockOrder] === sourceText),
    }
  }

  const api: ViewerTextTargetApi = {
    captureSelection(): ViewerTextTargetResult {
      if (disposed || !container.isConnected) {
        return {
          status: 'unavailable',
          reason: 'stale',
        }
      }
      if (doc.visibilityState === 'hidden') {
        return {
          status: 'unavailable',
          reason: 'not-ready',
        }
      }
      const snapshot = readerSelection(container)
      if (!snapshot) {
        return {
          status: 'unavailable',
          reason: 'no-selection',
        }
      }
      const start = selectedTextStart(snapshot.range)
      if (!start || !visibleText(start.node, container)) {
        return {
          status: 'unavailable',
          reason: 'unmapped',
        }
      }
      const pageElement = start.node.parentElement?.closest<HTMLElement>('.page[data-page-number]')
      const pageIndex = Number(pageElement?.dataset.pageNumber) - 1
      if (!Number.isSafeInteger(pageIndex) || pageIndex < 0 || pageIndex >= viewer.pagesCount) {
        return {
          status: 'unavailable',
          reason: 'unmapped',
        }
      }
      const page = viewer.getPageView(pageIndex) as PdfPageViewWithTextMapping & { div: HTMLElement }
      if (page?.div !== pageElement) {
        return {
          status: 'unavailable',
          reason: 'stale',
        }
      }
      const ocr = renderedPdfOcrTextLayer(page.div, pageIndex + 1)
      const layer = ocr ?? renderedPdfTextLayer(page.div, pageIndex + 1)
      if (!layer) {
        return {
          status: 'unavailable',
          reason: 'not-ready',
        }
      }
      const elements = ocr ? [...ocr.children].filter(el => el.tagName === 'SPAN') : page._textHighlighter?.textDivs
      const blockOrder = elements?.findIndex(element => element === start.node || element.contains(start.node)) ?? -1
      const element = elements?.[blockOrder]
      if (!element) {
        return {
          status: 'unavailable',
          reason: 'unmapped',
        }
      }
      const prefix = doc.createRange()
      prefix.selectNodeContents(element)
      prefix.setEnd(start.node, start.offset)
      const capture = capturePoint(pageIndex, page, layer, Boolean(ocr), blockOrder, element, prefix.toString().length)
      return capture.status === 'captured' ? {
        ...capture,
        isCurrent: () => capture.isCurrent() && snapshot.isCurrent(),
      } : capture
    },
    async captureViewport(): Promise<ViewerTextTargetResult> {
      if (disposed || !container.isConnected) {
        return {
          status: 'unavailable',
          reason: 'stale',
        }
      }
      if (!viewer.pagesCount || doc.visibilityState === 'hidden') {
        return {
          status: 'unavailable',
          reason: 'not-ready',
        }
      }
      const viewport = getViewport()
      const bounds = viewport && clippedBounds(container, viewport)
      if (!bounds) {
        return {
          status: 'unavailable',
          reason: 'no-visible-text',
        }
      }
      // PDF.js sorts by visible area; restore page/source order for spreads and RTL layouts.
      const pages = (viewer._getVisiblePages() as { views: Array<{ id: number }> }).views
      for (const { id } of [...pages].sort((a, b) => a.id - b.id)) {
        const pageIndex = id - 1
        const page = viewer.getPageView(pageIndex) as PdfPageViewWithTextMapping & { div: HTMLElement }
        if (!page?.div?.isConnected || !container.contains(page.div)) {
          return {
            status: 'unavailable',
            reason: 'not-ready',
          }
        }
        if (!clippedBounds(page.div, bounds)) {
          continue
        }
        const ocr = renderedPdfOcrTextLayer(page.div, id)
        const native = renderedPdfTextLayer(page.div, id)
        const layer = ocr ?? native
        if (!layer) {
          return {
            status: 'unavailable',
            reason: 'not-ready',
          }
        }
        const mapping = page._textHighlighter
        const elements = ocr ? [...ocr.children].filter(el => el.tagName === 'SPAN')
          : mapping?.textDivs
        if (!elements || (!ocr && !mapping?.textContentItemsStr)) {
          return {
            status: 'unavailable',
            reason: 'not-ready',
          }
        }

        for (const [blockOrder, element] of elements.entries()) {
          if (!element.isConnected || !layer.contains(element)) {
            if (ocr || mapping?.textContentItemsStr?.[blockOrder]?.trim()) {
              return {
                status: 'unavailable',
                reason: 'unmapped',
              }
            }
            continue
          }
          const sourceText = ocr ? element.textContent : mapping!.textContentItemsStr![blockOrder]
          if (sourceText === undefined || element.textContent !== sourceText) {
            return {
              status: 'unavailable',
              reason: 'unmapped',
            }
          }
          const walker = doc.createTreeWalker(element, NodeFilter.SHOW_TEXT)
          const nodes: Text[] = []
          if (element.nodeType === Node.TEXT_NODE) {
            nodes.push(element as Text)
          } else {
            while (walker.nextNode()) {
              nodes.push(walker.currentNode as Text)
            }
          }
          let preceding = 0
          for (const node of nodes) {
            const clip = node.parentElement && clippedBounds(node.parentElement, bounds)
            const offset = clip && visibleText(node, container) ? firstVisiblePdfOffset(node, clip) : null
            if (offset !== null) {
              return capturePoint(pageIndex, page, layer, Boolean(ocr), blockOrder, element, preceding + offset)
            }
            preceding += node.length
          }
        }
      }
      return {
        status: 'unavailable',
        reason: 'no-visible-text',
      }
    },
  }
  return {
    api,
    dispose() {
      disposed = true
      invalidate()
      for (const event of invalidatingEvents) {
        eventBus.off(event, invalidate)
      }
      doc.removeEventListener('visibilitychange', visibilityChanged)
    },
  }
}

/** PDF.js transforms/overlays defeat native caret hits; read visible glyphs in source order. */
function firstVisiblePdfOffset(node: Text, bounds: ViewerViewportBounds): number | null {
  const range = node.ownerDocument.createRange()
  range.selectNodeContents(node)
  if (![...range.getClientRects()].some(rect => intersect(bounds, rect))) {
    return null
  }
  // shortcut: a huge PDF text item can require a glyph scan; optimize only if measured capture cost exceeds the budget.
  for (let offset = 0; offset < node.length; offset++) {
    if (/\s/u.test(node.data[offset])) {
      continue
    }
    range.setStart(node, offset)
    range.setEnd(node, offset + 1)
    if ([...range.getClientRects()].some(rect => intersect(bounds, rect))) {
      return offset
    }
  }
  return null
}
