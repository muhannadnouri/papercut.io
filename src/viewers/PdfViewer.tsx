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
import { loadPdfJs, loadPdfViewer, pdfJsAssetRoot } from '../pdf/pdfJs'
import {
  getUploadedPdfAssetUrl,
  isUploadedPdfDocumentUrl,
} from '../uploads/DocumentUploads'
import {
  applyPdfTtsHighlight,
  clearPdfTtsHighlight,
} from '../pdf/pdfTtsHighlight'
import {
  clearSearchTargetHighlight,
  highlightFirstSearchTarget,
} from '../components/DocumentViewer/readerTarget'
import { createPdfFindAdapter, pdfSearchTargetPage } from './pdfFind'
import { createPdfBookmarkApi } from './pdfBookmark'
import {
  PdfControls,
  type PdfFitMode,
  type PdfSpreadMode,
} from './PdfControls'
import './PdfViewer.css'
import type { ViewerProps } from './types'

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
  const findAdapterRef = useRef<ReturnType<typeof createPdfFindAdapter> | null>(null)
  const spreadModesRef = useRef<{ NONE: number; ODD: number } | null>(null)
  const searchTargetRef = useRef(searchTarget)
  searchTargetRef.current = searchTarget
  const outlineCloseRef = useRef<HTMLButtonElement>(null)
  const [status, setStatus] = useState<PdfViewerStatus>({ state: 'loading' })
  const [currentPage, setCurrentPage] = useState(1)
  const [zoom, setZoom] = useState(100)
  const [fitMode, setFitMode] = useState<PdfFitMode>('page-width')
  const [outline, setOutline] = useState<PdfOutlineItem[]>([])
  const [outlineOpen, setOutlineOpen] = useState(false)
  const [spreadMode, setSpreadMode] = useState<PdfSpreadMode>('single')
  const [searchProgress, setSearchProgress] = useState<PdfSearchProgress | null>(null)

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

      const sourceUrl = isUploadedPdfDocumentUrl(url)
        ? await getUploadedPdfAssetUrl(url)
        : url
      const [pdfjs, viewerModule] = await Promise.all([
        loadPdfJs(),
        loadPdfViewer(),
        import('pdfjs-dist/web/pdf_viewer.css'),
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
      findAdapter = createPdfFindAdapter(eventBus, onFindResult ?? ignoreFindResult)
      findAdapterRef.current = findAdapter
      onFindApiChange?.(findAdapter.api)
      linkService.externalLinkEnabled = false
      pdfViewer = new viewerModule.PDFViewer({
        container,
        viewer,
        eventBus,
        linkService,
        findController,
        annotationMode: pdfjs.AnnotationMode.ENABLE,
        enableAutoLinking: false,
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

      const assetRoot = pdfJsAssetRoot()
      loadingTask = pdfjs.getDocument({
        url: sourceUrl,
        disableAutoFetch: true,
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
        onBookmarkApiChange?.(createPdfBookmarkApi(pdfViewer, container, eventBus))
        setStatus({ state: 'ready', pages: pdf.numPages })
      }

      return () => {
        eventBus?.off('pagesinit', handlePagesInit)
        eventBus?.off('pagechanging', handlePageChange)
        eventBus?.off('scalechanging', handleScaleChange)
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
          message: error instanceof Error ? error.message : String(error),
        })
      })

    return () => {
      cancelled = true
      removeEventListeners?.()
      findAdapter?.dispose()
      if (findAdapterRef.current === findAdapter) findAdapterRef.current = null
      onFindApiChange?.(null)
      onBookmarkApiChange?.(null)
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
    const findAdapter = findAdapterRef.current
    const text = searchTarget.text?.trim()
    if (!pdfViewer || !eventBus || !viewer || !text) return

    clearSearchTargetHighlight(viewer)

    if (searchTarget.pageIndex === undefined) {
      const frame = requestAnimationFrame(() => findAdapter?.api.search(text))
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
      const textLayer = renderedPdfTextLayer(viewer, pageNumber)
      if (!textLayer) return false
      const range = highlightFirstSearchTarget(textLayer, text)
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
      findAdapter?.api.search(text)
    }
    const handleTextLayerRendered = ({ pageNumber: renderedPage }: { pageNumber: number }) => {
      if (renderedPage !== pageNumber) return
      if (!highlightTargetPage()) fallbackToDocumentFind()
    }
    const handleFindSettled = ({ state }: { state: number }) => {
      if (fallbackStarted && state !== PDF_FIND_PENDING) finish()
    }

    eventBus.on('textlayerrendered', handleTextLayerRendered)
    eventBus.on('updatefindcontrolstate', handleFindSettled)
    pdfViewer.currentPageNumber = pageNumber
    const frame = requestAnimationFrame(() => {
      if (highlightTargetPage()) return
      if (renderedPdfTextLayer(viewer, pageNumber)) fallbackToDocumentFind()
    })

    return () => {
      complete = true
      cancelAnimationFrame(frame)
      eventBus.off('textlayerrendered', handleTextLayerRendered)
      eventBus.off('updatefindcontrolstate', handleFindSettled)
      clearSearchTargetHighlight(viewer)
      if (fallbackStarted) findAdapter?.api.clear()
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
