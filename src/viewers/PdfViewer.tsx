import { useCallback, useEffect, useRef, useState } from 'react'
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
import { createPdfFindAdapter } from './pdfFind'
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
  searchTarget,
  onFindApiChange,
  onFindResult,
}: ViewerProps) {
  const { t } = useTranslation()
  const containerRef = useRef<HTMLDivElement>(null)
  const viewerRef = useRef<HTMLDivElement>(null)
  const pdfViewerRef = useRef<PdfJsViewer | null>(null)
  const linkServiceRef = useRef<PDFLinkService | null>(null)
  const findAdapterRef = useRef<ReturnType<typeof createPdfFindAdapter> | null>(null)
  const spreadModesRef = useRef<{ NONE: number; EVEN: number } | null>(null)
  const outlineCloseRef = useRef<HTMLButtonElement>(null)
  const [status, setStatus] = useState<PdfViewerStatus>({ state: 'loading' })
  const [currentPage, setCurrentPage] = useState(1)
  const [zoom, setZoom] = useState(100)
  const [fitMode, setFitMode] = useState<PdfFitMode>('page-width')
  const [outline, setOutline] = useState<PdfOutlineItem[]>([])
  const [outlineOpen, setOutlineOpen] = useState(false)
  const [spreadMode, setSpreadMode] = useState<PdfSpreadMode>('single')

  const applySpreadMode = useCallback((next: PdfSpreadMode) => {
    setSpreadMode(next)
    const pdfViewer = pdfViewerRef.current
    const modes = spreadModesRef.current
    if (pdfViewer && modes) {
      // EVEN keeps the cover alone, then pairs pages 2-3, 4-5, and so on.
      pdfViewer.spreadMode = next === 'spread' ? modes.EVEN : modes.NONE
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
        if (pdfViewer) pdfViewer.currentScaleValue = 'page-width'
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
      if (!cancelled) setStatus({ state: 'ready', pages: pdf.numPages })

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
      linkService?.setDocument(null)
      pdfViewer?.cleanup()
      pdfViewerRef.current = null
      linkServiceRef.current = null
      spreadModesRef.current = null
      void loadingTask?.destroy()
    }
  }, [onFindApiChange, onFindResult, url])

  useEffect(() => {
    if (status.state !== 'ready' || !searchTarget) return
    const pdfViewer = pdfViewerRef.current
    if (!pdfViewer) return

    if (searchTarget.pageIndex !== undefined) {
      pdfViewer.currentPageNumber = Math.min(
        Math.max(searchTarget.pageIndex + 1, 1),
        status.pages,
      )
    }
    if (!searchTarget.text?.trim()) return

    const frame = requestAnimationFrame(() => {
      findAdapterRef.current?.api.search(searchTarget.text!)
    })
    return () => cancelAnimationFrame(frame)
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

  const selectOutlineItem = (destination: PdfDestination) => {
    setOutlineOpen(false)
    void linkServiceRef.current?.goToDestination(destination)
  }

  return (
    <div className="pdf-viewer">
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
                {t('reader.pdf.loading')}
              </>
            ) : status.message}
          </div>
        )}
        <div className="pdf-viewer-document">
          <div
            ref={containerRef}
            className="pdf-viewer-container"
            aria-busy={status.state === 'loading'}
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
