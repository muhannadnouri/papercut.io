import { useEffect, useRef, useState } from 'react'
import type { PDFDocumentLoadingTask } from 'pdfjs-dist/legacy/build/pdf.mjs'
import type {
  EventBus,
  PDFLinkService,
  PDFViewer as PdfJsViewer,
} from 'pdfjs-dist/legacy/web/pdf_viewer.mjs'
import { loadPdfJs, loadPdfViewer, pdfJsAssetRoot } from '../pdf/pdfJs'
import {
  getUploadedPdfAssetUrl,
  isUploadedPdfDocumentUrl,
} from '../uploads/DocumentUploads'
import './PdfViewer.css'
import type { ViewerProps } from './types'

type PdfViewerStatus =
  | { state: 'loading' }
  | { state: 'ready'; pages: number }
  | { state: 'error'; message: string }

export function PdfViewer({ url }: ViewerProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const viewerRef = useRef<HTMLDivElement>(null)
  const [status, setStatus] = useState<PdfViewerStatus>({ state: 'loading' })

  useEffect(() => {
    let cancelled = false
    let loadingTask: PDFDocumentLoadingTask | undefined
    let pdfViewer: PdfJsViewer | undefined
    let linkService: PDFLinkService | undefined
    let eventBus: EventBus | undefined

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

      eventBus = new viewerModule.EventBus()
      linkService = new viewerModule.PDFLinkService({ eventBus })
      linkService.externalLinkEnabled = false
      pdfViewer = new viewerModule.PDFViewer({
        container,
        viewer,
        eventBus,
        linkService,
        annotationMode: pdfjs.AnnotationMode.ENABLE,
        enableAutoLinking: false,
      })
      linkService.setViewer(pdfViewer)

      eventBus.on('pagesinit', () => {
        if (pdfViewer) pdfViewer.currentScaleValue = 'page-width'
      })

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
      await pdfViewer.onePageRendered
      if (!cancelled) setStatus({ state: 'ready', pages: pdf.numPages })
    }

    void openPdf().catch((error: unknown) => {
      if (cancelled) return
      setStatus({
        state: 'error',
        message: error instanceof Error ? error.message : String(error),
      })
    })

    return () => {
      cancelled = true
      linkService?.setDocument(null)
      pdfViewer?.cleanup()
      void loadingTask?.destroy()
    }
  }, [url])

  return (
    <div className="pdf-viewer">
      {status.state !== 'ready' && (
        <div
          className={`pdf-viewer-status${status.state === 'error' ? ' pdf-viewer-error' : ''}`}
          role={status.state === 'error' ? 'alert' : 'status'}
        >
          {status.state === 'loading' ? (
            <>
              <span className="spinner" aria-hidden="true" />
              Loading PDF...
            </>
          ) : status.message}
        </div>
      )}
      <div
        ref={containerRef}
        className="pdf-viewer-container"
        aria-busy={status.state === 'loading'}
        aria-label={status.state === 'ready'
          ? `PDF document, ${status.pages} pages`
          : 'PDF document'}
      >
        <div ref={viewerRef} className="pdfViewer" />
      </div>
    </div>
  )
}
