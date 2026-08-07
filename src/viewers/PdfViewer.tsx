import { useCallback, useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { useTranslation } from 'react-i18next'
import type {
  PDFDocumentLoadingTask,
  PDFDocumentProxy,
} from 'pdfjs-dist/legacy/build/pdf.mjs'
import type {
  EventBus,
  PDFFindController,
  PDFLinkService,
  PDFViewer as PdfJsViewer,
} from 'pdfjs-dist/legacy/web/pdf_viewer.mjs'
import {
  loadPdfJs,
  loadPdfViewer,
  pdfJsAssetRoot,
  pdfLoadErrorMessage,
} from '../pdf/pdfJs'
import { hasUsablePdfText } from '../pdf/ocr/pdfOcrReadiness'
import {
  findUploadedPdfText,
  getUploadedPdfAssetUrl,
  getUploadedPdfPageText,
  isUploadedPdfDocumentUrl,
  uploadedPdfHasOcrText,
  type PdfPageTextLayer,
} from '../uploads/DocumentUploads'
import {
  clearPdfOcrTextLayers,
  hasPdfOcrText,
  PDF_OCR_TEXT_LAYER_CLASS,
  renderPdfOcrTextLayer,
} from '../pdf/ocr/pdfOcrTextLayer'
import {
  createPdfOcrFindAdapter,
  type PdfOcrFindMatch,
} from '../pdf/ocr/pdfOcrFind'
import {
  applyPdfTtsHighlight,
  clearPdfTtsHighlight,
} from '../pdf/pdfTtsHighlight'
import {
  clearSearchTargetHighlight,
  highlightFirstSearchTarget,
  highlightSearchTarget,
} from '../components/DocumentViewer/readerTarget'
import { createPdfFindAdapter, pdfSearchTargetPage } from './pdfFind'
import { createPdfBookmarkApi } from './pdfBookmark'
import {
  PdfControls,
  type PdfFitMode,
  type PdfSpreadMode,
} from './PdfControls'
import { syncPdfViewerLayout } from './pdfViewerLayout'
import './PdfViewer.css'
import type { ViewerFindApi, ViewerProps } from './types'

type PdfViewerStatus =
  | { state: 'loading' }
  | { state: 'ready'; pages: number }
  | { state: 'error'; message: string }

type PdfOutlineItem = Awaited<ReturnType<PDFDocumentProxy['getOutline']>>[number]
type PdfDestination = Parameters<PDFLinkService['goToDestination']>[0]
const ignoreFindResult: NonNullable<ViewerProps['onFindResult']> = () => {}
const WIDE_PDF_VIEW = '(min-width: 900px)'
const PDF_FIND_PENDING = 3

type PdfSearchProgress = {
  pageNumber: number
  phase: 'locating' | 'verifying'
}

function OutlineItems({
  items,
  onSelect,
}: {
  items: PdfOutlineItem[]
  onSelect: (destination: PdfDestination) => void
}) {
  return (
    <ul>
      {items.map((item, index) => (
        <li key={`${item.title}-${index}`}>
          {item.dest ? (
            <button type="button" dir="auto" onClick={() => onSelect(item.dest!)}>
              {item.title}
            </button>
          ) : (
            <span dir="auto">{item.title}</span>
          )}
          {item.items.length > 0 && (
            <OutlineItems
              items={item.items as PdfOutlineItem[]}
              onSelect={onSelect}
            />
          )}
        </li>
      ))}
    </ul>
  )
}

export function PdfViewer({
  url,
  toolbarTarget,
  searchTarget,
  pdfTtsHighlightSpans,
  onBookmarkApiChange,
  onFindApiChange,
  onFindResult,
}: ViewerProps) {
  const { t } = useTranslation()
  const containerRef = useRef<HTMLDivElement>(null)
  const viewerRef = useRef<HTMLDivElement>(null)
  const pdfViewerRef = useRef<PdfJsViewer | null>(null)
  const eventBusRef = useRef<EventBus | null>(null)
  const linkServiceRef = useRef<PDFLinkService | null>(null)
  const findApiRef = useRef<ViewerFindApi | null>(null)
  const spreadModesRef = useRef<{ NONE: number; ODD: number } | null>(null)
  const searchTargetRef = useRef(searchTarget)
  const outlineCloseRef = useRef<HTMLButtonElement>(null)
  const [status, setStatus] = useState<PdfViewerStatus>({ state: 'loading' })
  const [currentPage, setCurrentPage] = useState(1)
  const [zoom, setZoom] = useState(100)
  const [fitMode, setFitMode] = useState<PdfFitMode>('page-width')
  const [outline, setOutline] = useState<PdfOutlineItem[]>([])
  const [outlineOpen, setOutlineOpen] = useState(false)
  const [spreadMode, setSpreadMode] = useState<PdfSpreadMode>('single')
  const [searchProgress, setSearchProgress] = useState<PdfSearchProgress | null>(null)

  useEffect(() => {
    searchTargetRef.current = searchTarget
  }, [searchTarget])

  const applySpreadMode = useCallback((next: PdfSpreadMode) => {
    setSpreadMode(next)
    const pdfViewer = pdfViewerRef.current
    const modes = spreadModesRef.current
    if (pdfViewer && modes) {
      // ODD pairs 1-2, 3-4, and so on; EVEN would leave the cover by itself.
      pdfViewer.spreadMode = next === 'spread' ? modes.ODD : modes.NONE
    }
  }, [])

  useEffect(() => {
    const media = window.matchMedia(WIDE_PDF_VIEW)
    const resetNarrowSpread = () => {
      if (!media.matches) applySpreadMode('single')
    }
    resetNarrowSpread()
    media.addEventListener('change', resetNarrowSpread)
    return () => media.removeEventListener('change', resetNarrowSpread)
  }, [applySpreadMode])

  useEffect(() => {
    let cancelled = false
    let loadingTask: PDFDocumentLoadingTask | undefined
    let pdfViewer: PdfJsViewer | undefined
    let linkService: PDFLinkService | undefined
    let eventBus: EventBus | undefined
    let findController: PDFFindController | undefined
    let findAdapter: ReturnType<typeof createPdfFindAdapter> | undefined
    let ocrFindAdapter: ReturnType<typeof createPdfOcrFindAdapter> | undefined
    let ocrFindActive = false
    let activeOcrFind: {
      match: PdfOcrFindMatch
      query: string
      scroll: boolean
    } | null = null
    let resizeObserver: ResizeObserver | undefined
    let resizeFrame = 0
    let pendingWidthChange = false
    let previousContainerSize: { width: number; height: number } | undefined
    let viewerElement: HTMLDivElement | null = null
    const ocrPageLayers = new Map<number, Promise<PdfPageTextLayer | null>>()

    setCurrentPage(1)
    setZoom(100)
    setFitMode('page-width')
    setOutline([])
    setOutlineOpen(false)
    setSpreadMode('single')
    onBookmarkApiChange?.(null)

    async function openPdf() {
      setStatus({ state: 'loading' })
      const container = containerRef.current
      const viewer = viewerRef.current
      if (!container || !viewer) return
      viewerElement = viewer

      const uploadedPdf = isUploadedPdfDocumentUrl(url)
      const hasOcrText = uploadedPdf
        ? uploadedPdfHasOcrText(url).catch(() => false)
        : Promise.resolve(false)
      const sourceUrl = uploadedPdf
        ? await getUploadedPdfAssetUrl(url)
        : url
      const [pdfjs, viewerModule, , useOcrFind] = await Promise.all([
        loadPdfJs(),
        loadPdfViewer(),
        import('pdfjs-dist/web/pdf_viewer.css'),
        hasOcrText,
      ])
      if (cancelled) return

      spreadModesRef.current = viewerModule.SpreadMode
      eventBus = new viewerModule.EventBus()
      eventBusRef.current = eventBus
      linkService = new viewerModule.PDFLinkService({ eventBus })
      findController = new viewerModule.PDFFindController({
        eventBus,
        linkService,
        delay: 0,
      })
      if (!useOcrFind) {
        findAdapter = createPdfFindAdapter(eventBus, onFindResult ?? ignoreFindResult)
        findApiRef.current = findAdapter.api
        onFindApiChange?.(findAdapter.api)
      }
      linkService.externalLinkEnabled = false
      pdfViewer = new viewerModule.PDFViewer({
        container,
        viewer,
        eventBus,
        linkService,
        findController,
        annotationMode: pdfjs.AnnotationMode.ENABLE,
        enableAutoLinking: false,
        // WebKit may not repaint PDF.js's delayed temporary-canvas copy.
        minDurationToUpdateCanvas: 0,
      })
      pdfViewerRef.current = pdfViewer
      linkServiceRef.current = linkService
      linkService.setViewer(pdfViewer)

      const handlePagesInit = () => {
        if (pdfViewer) {
          pdfViewer.currentScaleValue = window.matchMedia(WIDE_PDF_VIEW).matches
            ? '1'
            : 'page-width'
          const targetPage = pdfSearchTargetPage(
            searchTargetRef.current,
            pdfViewer.pagesCount,
          )
          if (targetPage !== null) pdfViewer.currentPageNumber = targetPage
        }
      }
      const handlePageChange = ({ pageNumber }: { pageNumber: number }) => {
        setCurrentPage(pageNumber)
      }
      const handleScaleChange = ({
        scale,
        presetValue,
      }: {
        scale: number
        presetValue?: string
      }) => {
        const percentage = Math.round(scale * 100)
        setZoom(percentage)
        setFitMode(presetValue === 'page-width' || presetValue === 'page-fit'
          ? presetValue
          : null)
      }

      eventBus.on('pagesinit', handlePagesInit)
      eventBus.on('pagechanging', handlePageChange)
      eventBus.on('scalechanging', handleScaleChange)

      const highlightPendingOcrFind = () => {
        if (!activeOcrFind) return false
        const pageNumber = activeOcrFind.match.pageIndex + 1
        const searchLayer = renderedPdfSearchLayer(viewer, pageNumber)
        if (!searchLayer) return false

        clearSearchTargetHighlight(viewer)
        const range = highlightSearchTarget(
          searchLayer,
          activeOcrFind.query,
          activeOcrFind.match.occurrenceIndex,
        )
        if (!range) return false
        setSearchProgress(null)
        if (activeOcrFind.scroll) {
          range.startContainer.parentElement?.scrollIntoView({
            block: 'center',
            behavior: 'smooth',
          })
          activeOcrFind = { ...activeOcrFind, scroll: false }
        }
        return true
      }
      const navigateOcrFind = (match: PdfOcrFindMatch, query: string) => {
        activeOcrFind = { match, query, scroll: true }
        if (pdfViewer) pdfViewer.currentPageNumber = match.pageIndex + 1
        requestAnimationFrame(highlightPendingOcrFind)
      }
      const activateOcrFind = () => {
        if (ocrFindActive || !isUploadedPdfDocumentUrl(url)) return
        ocrFindActive = true
        findAdapter?.api.clear()
        ocrFindAdapter = createPdfOcrFindAdapter(
          (query) => findUploadedPdfText(url, query),
          navigateOcrFind,
          () => {
            activeOcrFind = null
            clearSearchTargetHighlight(viewer)
          },
          onFindResult ?? ignoreFindResult,
        )
        findApiRef.current = ocrFindAdapter.api
        onFindApiChange?.(ocrFindAdapter.api)
      }
      if (useOcrFind) activateOcrFind()

      const handleTextLayerRendered = ({ pageNumber }: { pageNumber: number }) => {
        if (!isUploadedPdfDocumentUrl(url)) return
        const textLayer = renderedPdfTextLayer(viewer, pageNumber)
        const page = textLayer?.closest<HTMLElement>('.page')
        if (!textLayer || !page) return
        if (hasUsablePdfText(textLayer.textContent ?? '')) {
          page.querySelector(`.${PDF_OCR_TEXT_LAYER_CLASS}`)?.remove()
          if (activeOcrFind?.match.pageIndex === pageNumber - 1) {
            highlightPendingOcrFind()
          }
          return
        }

        const pageIndex = pageNumber - 1
        let request = ocrPageLayers.get(pageIndex)
        if (!request) {
          request = getUploadedPdfPageText(url, pageIndex).catch(() => null)
          ocrPageLayers.set(pageIndex, request)
        }
        void request
          .then((layer) => {
            // Missing or stale derived text must never prevent rendering the source PDF.
            if (!cancelled && page.isConnected && layer) {
              if (textLayer.textContent?.trim() && !hasPdfOcrText(layer)) {
                page.querySelector(`.${PDF_OCR_TEXT_LAYER_CLASS}`)?.remove()
              } else {
                const overlay = renderPdfOcrTextLayer(page, textLayer, layer)
                if (overlay) {
                  activateOcrFind()
                  eventBus?.dispatch('pdfocrtextlayerrendered', { pageNumber })
                }
              }
              if (activeOcrFind?.match.pageIndex === pageIndex) {
                highlightPendingOcrFind()
              }
            }
          })
      }
      eventBus.on('textlayerrendered', handleTextLayerRendered)

      const assetRoot = pdfJsAssetRoot()
      loadingTask = pdfjs.getDocument({
        url: sourceUrl,
        disableStream: true,
        rangeChunkSize: 1_048_576,
        standardFontDataUrl: `${assetRoot}standard_fonts/`,
        wasmUrl: `${assetRoot}wasm/`,
      })
      const pdf = await loadingTask.promise
      if (cancelled) return

      pdfViewer.setDocument(pdf)
      linkService.setDocument(pdf)
      void pdf.getOutline()
        .then((items) => {
          if (!cancelled) setOutline((items ?? []) as PdfOutlineItem[])
        })
        .catch(() => {
          if (!cancelled) setOutline([])
        })
      await pdfViewer.onePageRendered
      if (!cancelled) {
        resizeObserver = new ResizeObserver(([entry]) => {
          if (!entry) return
          const width = Math.round(entry.contentRect.width)
          const height = Math.round(entry.contentRect.height)
          if (width <= 0 || height <= 0) return

          const widthChanged = width !== previousContainerSize?.width
          const heightChanged = height !== previousContainerSize?.height
          if (!widthChanged && !heightChanged) return
          previousContainerSize = { width, height }
          pendingWidthChange ||= widthChanged

          cancelAnimationFrame(resizeFrame)
          resizeFrame = requestAnimationFrame(() => {
            const recomputeWidthFit = pendingWidthChange
            pendingWidthChange = false
            if (!cancelled && pdfViewer?.pagesCount) {
              syncPdfViewerLayout(pdfViewer, recomputeWidthFit)
            }
          })
        })
        resizeObserver.observe(container)
        onBookmarkApiChange?.(createPdfBookmarkApi(pdfViewer, container, eventBus))
        setStatus({ state: 'ready', pages: pdf.numPages })
      }

      return () => {
        eventBus?.off('pagesinit', handlePagesInit)
        eventBus?.off('pagechanging', handlePageChange)
        eventBus?.off('scalechanging', handleScaleChange)
        eventBus?.off('textlayerrendered', handleTextLayerRendered)
      }
    }

    let removeEventListeners: (() => void) | undefined
    void openPdf()
      .then((cleanup) => {
        removeEventListeners = cleanup
      })
      .catch((error: unknown) => {
        if (cancelled) return
        setStatus({
          state: 'error',
          message: pdfLoadErrorMessage(error),
        })
      })

    return () => {
      cancelled = true
      resizeObserver?.disconnect()
      cancelAnimationFrame(resizeFrame)
      removeEventListeners?.()
      findAdapter?.dispose()
      ocrFindAdapter?.dispose()
      findApiRef.current = null
      onFindApiChange?.(null)
      onBookmarkApiChange?.(null)
      if (viewerElement) clearPdfOcrTextLayers(viewerElement)
      linkService?.setDocument(null)
      pdfViewer?.cleanup()
      pdfViewerRef.current = null
      eventBusRef.current = null
      linkServiceRef.current = null
      spreadModesRef.current = null
      void loadingTask?.destroy()
    }
  }, [onBookmarkApiChange, onFindApiChange, onFindResult, url])

  useEffect(() => {
    const pdfViewer = pdfViewerRef.current
    const eventBus = eventBusRef.current
    const doc = viewerRef.current?.ownerDocument
    if (status.state !== 'ready' || !pdfViewer || !eventBus || !doc) return

    clearPdfTtsHighlight(doc)
    if (!pdfTtsHighlightSpans?.length) return

    let scrolled = false
    const renderHighlight = () => {
      const firstRange = applyPdfTtsHighlight(pdfViewer, pdfTtsHighlightSpans, doc)
      if (!firstRange || scrolled) return
      scrolled = true
      firstRange.startContainer.parentElement?.scrollIntoView({
        block: 'center',
        behavior: 'smooth',
      })
    }
    const firstPage = Math.min(
      Math.max(pdfTtsHighlightSpans[0].pageIndex + 1, 1),
      status.pages,
    )
    pdfViewer.currentPageNumber = firstPage
    eventBus.on('textlayerrendered', renderHighlight)
    eventBus.on('updatetextlayermatches', renderHighlight)
    const frame = requestAnimationFrame(renderHighlight)

    return () => {
      cancelAnimationFrame(frame)
      eventBus.off('textlayerrendered', renderHighlight)
      eventBus.off('updatetextlayermatches', renderHighlight)
      clearPdfTtsHighlight(doc)
    }
  }, [pdfTtsHighlightSpans, status])

  useEffect(() => {
    setSearchProgress(null)
    if (status.state !== 'ready' || !searchTarget) return
    const pdfViewer = pdfViewerRef.current
    const eventBus = eventBusRef.current
    const viewer = viewerRef.current
    const findApi = findApiRef.current
    const text = searchTarget.text?.trim()
    if (!pdfViewer || !eventBus || !viewer || !text) return

    clearSearchTargetHighlight(viewer)

    if (searchTarget.pageIndex === undefined) {
      const frame = requestAnimationFrame(() => findApi?.search(text))
      return () => cancelAnimationFrame(frame)
    }

    const pageNumber = pdfSearchTargetPage(searchTarget, status.pages)
    if (pageNumber === null) return
    let complete = false
    let fallbackStarted = false
    setSearchProgress({ pageNumber, phase: 'locating' })

    const finish = () => {
      if (complete) return
      complete = true
      setSearchProgress(null)
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
      setSearchProgress({ pageNumber, phase: 'verifying' })
      findApiRef.current?.search(text)
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
      if (fallbackStarted) findApiRef.current?.clear()
    }
  }, [searchTarget, status])

  useEffect(() => {
    if (!outlineOpen) return
    outlineCloseRef.current?.focus()
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setOutlineOpen(false)
    }
    document.addEventListener('keydown', handleKeyDown)
    return () => document.removeEventListener('keydown', handleKeyDown)
  }, [outlineOpen])

  const pages = status.state === 'ready' ? status.pages : 1
  const ready = status.state === 'ready'
  const requestedSearchPage = pdfSearchTargetPage(
    searchTarget,
    status.state === 'ready' ? status.pages : undefined,
  )

  const selectOutlineItem = (destination: PdfDestination) => {
    setOutlineOpen(false)
    void linkServiceRef.current?.goToDestination(destination)
  }

  const controls = (
    <PdfControls
      currentPage={currentPage}
      fitMode={fitMode}
      hasOutline={outline.length > 0}
      outlineOpen={outlineOpen}
      pages={pages}
      ready={ready}
      spreadMode={spreadMode}
      zoom={zoom}
      onFitChange={(mode) => {
        if (pdfViewerRef.current) pdfViewerRef.current.currentScaleValue = mode
      }}
      onOutlineChange={setOutlineOpen}
      onPageChange={(page) => {
        if (pdfViewerRef.current) pdfViewerRef.current.currentPageNumber = page
      }}
      onPageNext={() => pdfViewerRef.current?.nextPage()}
      onPagePrevious={() => pdfViewerRef.current?.previousPage()}
      onSpreadChange={applySpreadMode}
      onZoomChange={(percentage) => {
        if (pdfViewerRef.current) {
          pdfViewerRef.current.currentScaleValue = String(percentage / 100)
        }
      }}
    />
  )

  return (
    <div className="pdf-viewer">
      {toolbarTarget ? createPortal(controls, toolbarTarget) : controls}

      <div className="pdf-viewer-body">
        {outlineOpen && (
          <nav
            id="pdf-outline"
            className="pdf-outline"
            aria-label={t('reader.pdf.outline')}
          >
            <header>
              <strong>{t('reader.pdf.outline')}</strong>
              <button
                ref={outlineCloseRef}
                type="button"
                aria-label={t('reader.pdf.closeOutline')}
                title={t('reader.pdf.closeOutline')}
                onClick={() => setOutlineOpen(false)}
              >
                &times;
              </button>
            </header>
            <OutlineItems items={outline} onSelect={selectOutlineItem} />
          </nav>
        )}

        {status.state !== 'ready' && (
          <div
            className={`pdf-viewer-status${status.state === 'error' ? ' pdf-viewer-error' : ''}`}
            role={status.state === 'error' ? 'alert' : 'status'}
          >
            {status.state === 'loading' ? (
              <>
                <span className="spinner" aria-hidden="true" />
                {requestedSearchPage === null
                  ? t('reader.pdf.loading')
                  : t('reader.pdf.locatingMatch', { page: requestedSearchPage })}
              </>
            ) : status.message}
          </div>
        )}
        <div className="pdf-viewer-document">
          {status.state === 'ready' && searchProgress !== null && (
            <div className="pdf-search-target-status" role="status" aria-live="polite">
              <span className="spinner" aria-hidden="true" />
              {t(
                searchProgress.phase === 'verifying'
                  ? 'reader.pdf.verifyingMatch'
                  : 'reader.pdf.locatingMatch',
                { page: searchProgress.pageNumber },
              )}
            </div>
          )}
          <div
            ref={containerRef}
            className="pdf-viewer-container"
            aria-busy={status.state === 'loading' || searchProgress !== null}
            aria-label={status.state === 'ready'
              ? t('reader.pdf.documentPages', { count: status.pages })
              : t('reader.pdf.document')}
          >
            <div ref={viewerRef} className="pdfViewer" />
          </div>
        </div>
      </div>
    </div>
  )
}

/**
 * PDF.js appends `.endOfContent` only after a page text layer is complete.
 *
 * Waiting for that sentinel avoids treating a mounted but still-empty layer as
 * a failed indexed-page match and unnecessarily starting a whole-PDF search.
 */
function renderedPdfTextLayer(viewer: HTMLElement, pageNumber: number): HTMLElement | null {
  const textLayer = viewer.querySelector<HTMLElement>(
    `.page[data-page-number="${pageNumber}"] .textLayer`,
  )
  return textLayer?.querySelector('.endOfContent') ? textLayer : null
}

function renderedPdfOcrTextLayer(viewer: HTMLElement, pageNumber: number): HTMLElement | null {
  return viewer.querySelector<HTMLElement>(
    `.page[data-page-number="${pageNumber}"] .${PDF_OCR_TEXT_LAYER_CLASS}`,
  )
}

function renderedPdfSearchLayer(viewer: HTMLElement, pageNumber: number): HTMLElement | null {
  const ocrLayer = renderedPdfOcrTextLayer(viewer, pageNumber)
  if (ocrLayer) return ocrLayer
  const textLayer = renderedPdfTextLayer(viewer, pageNumber)
  return textLayer?.textContent?.trim() ? textLayer : null
}
