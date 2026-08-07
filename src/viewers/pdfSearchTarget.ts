import type {
  EventBus,
  PDFViewer as PdfJsViewer,
} from 'pdfjs-dist/legacy/web/pdf_viewer.mjs'
import {
  clearSearchTargetHighlight,
  highlightFirstSearchTarget,
} from '../components/DocumentViewer/readerTarget'
import { renderedPdfSearchLayer, renderedPdfTextLayer } from '../pdf/ocr/pdfOcrTextLayer'
import type { SearchOpenTarget } from '../types/search'
import { pdfSearchTargetPage } from './pdfFind'
import type { ViewerFindApi } from './types'

const PDF_FIND_PENDING = 3

export type PdfSearchTargetProgress = {
  pageNumber: number
  phase: 'locating' | 'verifying'
}

interface PdfSearchTargetOptions {
  eventBus: EventBus
  getFindApi: () => ViewerFindApi | null
  onProgress: (progress: PdfSearchTargetProgress | null) => void
  pages: number
  pdfViewer: PdfJsViewer
  target: SearchOpenTarget
  viewer: HTMLElement
}

/** Navigate an indexed result to its PDF.js or OCR text layer.
 *
 * Page rendering is asynchronous, so this binding waits for either text-layer
 * event before falling back to whole-document Find. Its cleanup removes every
 * listener and highlight installed for the previous result.
 */
export function bindPdfSearchTarget({
  eventBus,
  getFindApi,
  onProgress,
  pages,
  pdfViewer,
  target,
  viewer,
}: PdfSearchTargetOptions): () => void {
  const text = target.text?.trim()
  if (!text) return () => {}

  clearSearchTargetHighlight(viewer)

  if (target.pageIndex === undefined) {
    const frame = requestAnimationFrame(() => getFindApi()?.search(text))
    return () => cancelAnimationFrame(frame)
  }

  const pageNumber = pdfSearchTargetPage(target, pages)
  if (pageNumber === null) return () => {}
  let complete = false
  let fallbackStarted = false
  onProgress({ pageNumber, phase: 'locating' })

  const finish = () => {
    if (complete) return
    complete = true
    onProgress(null)
  }
  const highlightTargetPage = () => {
    const searchLayer = renderedPdfSearchLayer(viewer, pageNumber)
    if (!searchLayer) return false
    const range = highlightFirstSearchTarget(searchLayer, text)
    if (!range) return false

    finish()
    range.startContainer.parentElement?.scrollIntoView({
      block: 'center',
      behavior: 'smooth',
    })
    return true
  }
  const fallbackToDocumentFind = () => {
    if (complete || fallbackStarted) return
    fallbackStarted = true
    onProgress({ pageNumber, phase: 'verifying' })
    getFindApi()?.search(text)
  }
  const handleTextLayerRendered = ({ pageNumber: renderedPage }: { pageNumber: number }) => {
    if (renderedPage !== pageNumber) return
    if (!highlightTargetPage()) fallbackToDocumentFind()
  }
  const handleFindSettled = ({ state }: { state: number }) => {
    if (fallbackStarted && state !== PDF_FIND_PENDING) finish()
  }

  eventBus.on('textlayerrendered', handleTextLayerRendered)
  eventBus.on('pdfocrtextlayerrendered', handleTextLayerRendered)
  eventBus.on('updatefindcontrolstate', handleFindSettled)
  pdfViewer.currentPageNumber = pageNumber
  const frame = requestAnimationFrame(() => {
    if (highlightTargetPage()) return
    const textLayer = renderedPdfTextLayer(viewer, pageNumber)
    if (textLayer?.textContent?.trim()) fallbackToDocumentFind()
  })

  return () => {
    complete = true
    cancelAnimationFrame(frame)
    eventBus.off('textlayerrendered', handleTextLayerRendered)
    eventBus.off('pdfocrtextlayerrendered', handleTextLayerRendered)
    eventBus.off('updatefindcontrolstate', handleFindSettled)
    clearSearchTargetHighlight(viewer)
    if (fallbackStarted) getFindApi()?.clear()
  }
}
