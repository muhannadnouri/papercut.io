import { useEffect, useRef, useState } from 'react'
import type {
  PDFDocumentLoadingTask,
  PDFPageProxy,
  RenderTask,
  TextLayer,
} from 'pdfjs-dist/legacy/build/pdf.mjs'
import pdfWorkerUrl from 'pdfjs-dist/legacy/build/pdf.worker.min.mjs?url'
import './PdfViewer.css'
import type { ViewerProps } from './types'

let pdfJsPromise: Promise<typeof import('pdfjs-dist/legacy/build/pdf.mjs')> | undefined

// Keep the large renderer and text-layer stylesheet out of normal app startup.
function loadPdfJs() {
  pdfJsPromise ??= Promise.all([
    import('pdfjs-dist/legacy/build/pdf.mjs'),
    import('pdfjs-dist/web/pdf_viewer.css'),
  ]).then(([pdfjs]) => {
    pdfjs.GlobalWorkerOptions.workerSrc = pdfWorkerUrl
    return pdfjs
  })
  return pdfJsPromise
}

type PdfViewerStatus =
  | { state: 'loading' }
  | { state: 'ready'; pages: number }
  | { state: 'error'; message: string }

export function PdfViewer({ url }: ViewerProps) {
  const pageRef = useRef<HTMLDivElement>(null)
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const textLayerRef = useRef<HTMLDivElement>(null)
  const [status, setStatus] = useState<PdfViewerStatus>({ state: 'loading' })

  useEffect(() => {
    let loadingTask: PDFDocumentLoadingTask | undefined
    let cancelled = false
    let page: PDFPageProxy | undefined
    let renderTask: RenderTask | undefined
    let textLayer: TextLayer | undefined

    async function renderFirstPage() {
      setStatus({ state: 'loading' })
      const pdfjs = await loadPdfJs()
      if (cancelled) return
      loadingTask = pdfjs.getDocument({ url })
      const pdf = await loadingTask.promise
      page = await pdf.getPage(1)
      if (cancelled) return

      const baseViewport = page.getViewport({ scale: 1 })
      const availableWidth = pageRef.current?.parentElement?.clientWidth ?? baseViewport.width
      const scale = Math.min(1.5, availableWidth / baseViewport.width)
      const viewport = page.getViewport({ scale })
      const outputScale = Math.min(window.devicePixelRatio || 1, 2)
      const canvas = canvasRef.current
      const pageElement = pageRef.current
      const textElement = textLayerRef.current
      if (!canvas || !pageElement || !textElement) return

      pageElement.style.width = `${viewport.width}px`
      pageElement.style.height = `${viewport.height}px`
      canvas.width = Math.floor(viewport.width * outputScale)
      canvas.height = Math.floor(viewport.height * outputScale)
      canvas.style.width = `${viewport.width}px`
      canvas.style.height = `${viewport.height}px`

      renderTask = page.render({
        canvas,
        viewport,
        transform: outputScale === 1
          ? undefined
          : [outputScale, 0, 0, outputScale, 0, 0],
      })
      textLayer = new pdfjs.TextLayer({
        container: textElement,
        textContentSource: page.streamTextContent({
          includeMarkedContent: true,
          disableNormalization: true,
        }),
        viewport,
      })

      await Promise.all([renderTask.promise, textLayer.render()])
      if (!cancelled) setStatus({ state: 'ready', pages: pdf.numPages })
    }

    void renderFirstPage().catch((error: unknown) => {
      if (cancelled) return
      setStatus({
        state: 'error',
        message: error instanceof Error ? error.message : String(error),
      })
    })

    return () => {
      cancelled = true
      renderTask?.cancel()
      textLayer?.cancel()
      page?.cleanup()
      void loadingTask?.destroy()
    }
  }, [url])

  return (
    <div className="pdf-viewer">
      {status.state === 'loading' && (
        <div className="pdf-viewer-status" role="status">
          <span className="spinner" aria-hidden="true" />
          Loading PDF...
        </div>
      )}
      {status.state === 'error' && (
        <p className="pdf-viewer-error" role="alert">{status.message}</p>
      )}
      <div
        ref={pageRef}
        className="pdf-viewer-page"
        aria-label={status.state === 'ready'
          ? `Page 1 of ${status.pages}`
          : 'PDF page 1'}
      >
        <canvas ref={canvasRef} aria-hidden="true" />
        <div ref={textLayerRef} className="textLayer" />
      </div>
    </div>
  )
}
