import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import './PdfControls.css'

export type PdfFitMode = 'page-width' | 'page-fit' | null

const MIN_ZOOM = 25
const MAX_ZOOM = 400
const ZOOM_STEP = 10

export function clampPdfPage(input: string, currentPage: number, pages: number) {
  const parsed = Number.parseInt(input, 10)
  const value = Number.isNaN(parsed) ? currentPage : parsed
  return Math.min(pages, Math.max(1, value))
}

export function clampPdfZoom(input: string, currentZoom: number) {
  const parsed = Number.parseInt(input, 10)
  const value = Number.isNaN(parsed) ? currentZoom : parsed
  return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, Math.round(value)))
}

type PdfControlsProps = {
  currentPage: number
  fitMode: PdfFitMode
  hasOutline: boolean
  outlineOpen: boolean
  pages: number
  ready: boolean
  zoom: number
  onFitChange: (mode: Exclude<PdfFitMode, null>) => void
  onOutlineChange: (open: boolean) => void
  onPageChange: (page: number) => void
  onPageNext: () => void
  onPagePrevious: () => void
  onZoomChange: (percentage: number) => void
}

export function PdfControls({
  currentPage,
  fitMode,
  hasOutline,
  outlineOpen,
  pages,
  ready,
  zoom,
  onFitChange,
  onOutlineChange,
  onPageChange,
  onPageNext,
  onPagePrevious,
  onZoomChange,
}: PdfControlsProps) {
  const { t } = useTranslation()
  const [pageInput, setPageInput] = useState(String(currentPage))
  const [zoomInput, setZoomInput] = useState(String(zoom))

  useEffect(() => setPageInput(String(currentPage)), [currentPage])
  useEffect(() => setZoomInput(String(zoom)), [zoom])

  const commitPage = () => {
    const next = clampPdfPage(pageInput, currentPage, pages)
    setPageInput(String(next))
    onPageChange(next)
  }

  const setZoom = (value: string) => {
    const next = clampPdfZoom(value, zoom)
    setZoomInput(String(next))
    onZoomChange(next)
  }

  return (
    <div
      className="pdf-viewer-toolbar"
      role="toolbar"
      aria-label={t('reader.pdf.toolbar')}
    >
      <div className="pdf-control-group pdf-page-controls" role="group" aria-label={t('reader.pdf.pages')}>
        <button
          type="button"
          className="pdf-control-button pdf-page-previous"
          disabled={!ready || currentPage <= 1}
          aria-label={t('reader.pdf.previousPage')}
          title={t('reader.pdf.previousPage')}
          onClick={onPagePrevious}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
            <path d="M19 12H5M11 18l-6-6 6-6" />
          </svg>
        </button>
        <input
          className="pdf-page-input"
          type="number"
          min="1"
          max={pages}
          inputMode="numeric"
          disabled={!ready}
          value={pageInput}
          aria-label={t('reader.pdf.pageNumber')}
          onChange={(event) => setPageInput(event.target.value)}
          onBlur={commitPage}
          onKeyDown={(event) => {
            if (event.key === 'Enter') event.currentTarget.blur()
          }}
        />
        <span
          className="pdf-page-total"
          dir="ltr"
        >
          <span aria-hidden="true">/ {pages}</span>
          <span className="pdf-control-sr-only">
            {t('reader.pdf.pageTotal', { total: pages })}
          </span>
        </span>
        <button
          type="button"
          className="pdf-control-button pdf-page-next"
          disabled={!ready || currentPage >= pages}
          aria-label={t('reader.pdf.nextPage')}
          title={t('reader.pdf.nextPage')}
          onClick={onPageNext}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
            <path d="M5 12h14M13 6l6 6-6 6" />
          </svg>
        </button>
      </div>

      <div className="pdf-control-group pdf-zoom-controls" role="group" aria-label={t('reader.pdf.zoom')}>
        <button
          type="button"
          className="pdf-control-button"
          disabled={!ready || zoom <= MIN_ZOOM}
          aria-label={t('reader.pdf.zoomOut')}
          title={t('reader.pdf.zoomOut')}
          onClick={() => setZoom(String(zoom - ZOOM_STEP))}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
            <circle cx="10.5" cy="10.5" r="6.5" />
            <path d="m15.5 15.5 5 5M7.5 10.5h6" />
          </svg>
        </button>
        <label className="pdf-zoom-input" dir="ltr">
          <input
            type="number"
            min={MIN_ZOOM}
            max={MAX_ZOOM}
            inputMode="numeric"
            disabled={!ready}
            value={zoomInput}
            aria-label={t('reader.pdf.zoomLevel')}
            onChange={(event) => setZoomInput(event.target.value)}
            onBlur={() => setZoom(zoomInput)}
            onKeyDown={(event) => {
              if (event.key === 'Enter') event.currentTarget.blur()
            }}
          />
        </label>
        <button
          type="button"
          className="pdf-control-button"
          disabled={!ready || zoom >= MAX_ZOOM}
          aria-label={t('reader.pdf.zoomIn')}
          title={t('reader.pdf.zoomIn')}
          onClick={() => setZoom(String(zoom + ZOOM_STEP))}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
            <circle cx="10.5" cy="10.5" r="6.5" />
            <path d="m15.5 15.5 5 5M7.5 10.5h6M10.5 7.5v6" />
          </svg>
        </button>
      </div>

      <div className="pdf-control-group pdf-fit-controls" role="group" aria-label={t('reader.pdf.fit')}>
        <button
          type="button"
          className={`pdf-control-button${fitMode === 'page-width' ? ' active' : ''}`}
          disabled={!ready}
          aria-label={t('reader.pdf.fitWidth')}
          aria-pressed={fitMode === 'page-width'}
          title={t('reader.pdf.fitWidth')}
          onClick={() => onFitChange('page-width')}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
            <rect x="3" y="5" width="18" height="14" rx="1.5" />
            <path d="M7 12h10M7 12l3-3M7 12l3 3M17 12l-3-3M17 12l-3 3" />
          </svg>
        </button>
        <button
          type="button"
          className={`pdf-control-button${fitMode === 'page-fit' ? ' active' : ''}`}
          disabled={!ready}
          aria-label={t('reader.pdf.fitPage')}
          aria-pressed={fitMode === 'page-fit'}
          title={t('reader.pdf.fitPage')}
          onClick={() => onFitChange('page-fit')}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
            <path d="M8 3H3v5M16 3h5v5M8 21H3v-5M16 21h5v-5" />
          </svg>
        </button>
        {hasOutline && (
          <button
            type="button"
            className={`pdf-control-button${outlineOpen ? ' active' : ''}`}
            aria-label={t('reader.pdf.outline')}
            aria-expanded={outlineOpen}
            aria-controls="pdf-outline"
            title={t('reader.pdf.outline')}
            onClick={() => onOutlineChange(!outlineOpen)}
          >
            <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
              <path d="M9 6h11M9 12h11M9 18h11" />
              <circle cx="4" cy="6" r="1" />
              <circle cx="4" cy="12" r="1" />
              <circle cx="4" cy="18" r="1" />
            </svg>
          </button>
        )}
      </div>
    </div>
  )
}
