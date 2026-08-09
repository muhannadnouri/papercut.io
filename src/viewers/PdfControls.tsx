import { useTranslation } from 'react-i18next'
import { Button, Menu, MenuItem, MenuTrigger, Popover } from 'react-aria-components'
import './PdfControls.css'
import {
  clampPdfPage,
  clampPdfZoom,
  PDF_MAX_ZOOM,
  PDF_MIN_ZOOM,
} from './pdfControlValues'

export type PdfFitMode = 'page-width' | 'page-fit' | null
export type PdfSpreadMode = 'single' | 'spread'

const ZOOM_STEP = 10

type PdfControlsProps = {
  currentPage: number
  fitMode: PdfFitMode
  fullscreen: boolean
  hasOutline: boolean
  outlineOpen: boolean
  pages: number
  ready: boolean
  spreadMode: PdfSpreadMode
  zoom: number
  onFitChange: (mode: Exclude<PdfFitMode, null>) => void
  onFullscreenChange: () => void
  onOutlineChange: (open: boolean) => void
  onPageChange: (page: number) => void
  onPageNext: () => void
  onPagePrevious: () => void
  onSpreadChange: (mode: PdfSpreadMode) => void
  onZoomChange: (percentage: number) => void
}

export function PdfControls({
  currentPage,
  fitMode,
  fullscreen,
  hasOutline,
  outlineOpen,
  pages,
  ready,
  spreadMode,
  zoom,
  onFitChange,
  onFullscreenChange,
  onOutlineChange,
  onPageChange,
  onPageNext,
  onPagePrevious,
  onSpreadChange,
  onZoomChange,
}: PdfControlsProps) {
  const { t } = useTranslation()

  const commitPage = (input: HTMLInputElement) => {
    const next = clampPdfPage(input.value, currentPage, pages)
    input.value = String(next)
    onPageChange(next)
  }

  const commitZoom = (value: string) => {
    const next = clampPdfZoom(value, zoom)
    onZoomChange(next)
    return next
  }

  return (
    <div
      className="pdf-viewer-toolbar"
      role="group"
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
          key={currentPage}
          className="pdf-page-input"
          type="number"
          min="1"
          max={pages}
          inputMode="numeric"
          disabled={!ready}
          defaultValue={currentPage}
          aria-label={t('reader.pdf.pageNumber')}
          onBlur={(event) => commitPage(event.currentTarget)}
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
          disabled={!ready || zoom <= PDF_MIN_ZOOM}
          aria-label={t('reader.pdf.zoomOut')}
          title={t('reader.pdf.zoomOut')}
          onClick={() => commitZoom(String(zoom - ZOOM_STEP))}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
            <circle cx="10.5" cy="10.5" r="6.5" />
            <path d="m15.5 15.5 5 5M7.5 10.5h6" />
          </svg>
        </button>
        <label className="pdf-zoom-input" dir="ltr">
          <input
            key={zoom}
            type="number"
            min={PDF_MIN_ZOOM}
            max={PDF_MAX_ZOOM}
            inputMode="numeric"
            disabled={!ready}
            defaultValue={zoom}
            aria-label={t('reader.pdf.zoomLevel')}
            onBlur={(event) => {
              event.currentTarget.value = String(commitZoom(event.currentTarget.value))
            }}
            onKeyDown={(event) => {
              if (event.key === 'Enter') event.currentTarget.blur()
            }}
          />
        </label>
        <button
          type="button"
          className="pdf-control-button"
          disabled={!ready || zoom >= PDF_MAX_ZOOM}
          aria-label={t('reader.pdf.zoomIn')}
          title={t('reader.pdf.zoomIn')}
          onClick={() => commitZoom(String(zoom + ZOOM_STEP))}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
            <circle cx="10.5" cy="10.5" r="6.5" />
            <path d="m15.5 15.5 5 5M7.5 10.5h6M10.5 7.5v6" />
          </svg>
        </button>
      </div>

      <div className="pdf-control-group pdf-fit-controls" role="group" aria-label={t('reader.pdf.view')}>
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
            <rect x="5" y="3" width="14" height="18" rx="1.5" />
            <path d="M12 6v12M9.5 8.5 12 6l2.5 2.5M9.5 15.5 12 18l2.5-2.5" />
          </svg>
        </button>
        <button
          type="button"
          className={`pdf-control-button pdf-spread-toggle${spreadMode === 'spread' ? ' active' : ''}`}
          disabled={!ready}
          aria-label={t('reader.pdf.twoPageSpread')}
          aria-pressed={spreadMode === 'spread'}
          title={t('reader.pdf.twoPageSpread')}
          onClick={() => onSpreadChange(spreadMode === 'spread' ? 'single' : 'spread')}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
            <rect x="3" y="4" width="7" height="16" rx="1" />
            <rect x="14" y="4" width="7" height="16" rx="1" />
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
        <button
          type="button"
          className={`pdf-control-button${fullscreen ? ' active' : ''}`}
          disabled={!ready}
          aria-label={t(fullscreen ? 'reader.pdf.exitFullscreen' : 'reader.pdf.enterFullscreen')}
          aria-pressed={fullscreen}
          title={t(fullscreen ? 'reader.pdf.exitFullscreen' : 'reader.pdf.enterFullscreen')}
          onClick={onFullscreenChange}
        >
          <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
            {fullscreen ? (
              <path d="M9 4v5H4M15 4v5h5M9 20v-5H4M15 20v-5h5" />
            ) : (
              <path d="M9 4H4v5M15 4h5v5M9 20H4v-5M15 20h5v-5" />
            )}
          </svg>
        </button>
      </div>

      <div className="pdf-control-group pdf-view-options">
        <MenuTrigger>
          <Button
            className="pdf-control-button pdf-view-options-trigger"
            isDisabled={!ready}
            aria-label={t('reader.pdf.view')}
          >
            <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
              <circle cx="12" cy="5" r="1.5" />
              <circle cx="12" cy="12" r="1.5" />
              <circle cx="12" cy="19" r="1.5" />
            </svg>
          </Button>
          <Popover
            className="pdf-view-options-popover"
            placement="bottom end"
            offset={6}
            containerPadding={8}
            shouldFlip
          >
            <Menu className="pdf-view-options-menu" aria-label={t('reader.pdf.view')}>
              <MenuItem
                className="pdf-view-option"
                textValue={t('reader.pdf.fitWidth')}
                onAction={() => onFitChange('page-width')}
              >
                <span className="pdf-view-option-check" aria-hidden="true">
                  {fitMode === 'page-width' ? '✓' : ''}
                </span>
                <span>{t('reader.pdf.fitWidth')}</span>
              </MenuItem>
              <MenuItem
                className="pdf-view-option"
                textValue={t('reader.pdf.fitPage')}
                onAction={() => onFitChange('page-fit')}
              >
                <span className="pdf-view-option-check" aria-hidden="true">
                  {fitMode === 'page-fit' ? '✓' : ''}
                </span>
                <span>{t('reader.pdf.fitPage')}</span>
              </MenuItem>
              {hasOutline && (
                <MenuItem
                  className="pdf-view-option"
                  textValue={t('reader.pdf.outline')}
                  onAction={() => onOutlineChange(!outlineOpen)}
                >
                  <span className="pdf-view-option-check" aria-hidden="true">
                    {outlineOpen ? '✓' : ''}
                  </span>
                  <span>{t('reader.pdf.outline')}</span>
                </MenuItem>
              )}
              <MenuItem
                className="pdf-view-option"
                textValue={t(fullscreen ? 'reader.pdf.exitFullscreen' : 'reader.pdf.enterFullscreen')}
                onAction={onFullscreenChange}
              >
                <span className="pdf-view-option-check" aria-hidden="true">
                  {fullscreen ? '✓' : ''}
                </span>
                <span>{t(fullscreen ? 'reader.pdf.exitFullscreen' : 'reader.pdf.enterFullscreen')}</span>
              </MenuItem>
            </Menu>
          </Popover>
        </MenuTrigger>
      </div>
    </div>
  )
}
