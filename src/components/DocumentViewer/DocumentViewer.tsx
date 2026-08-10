import {
  useState,
  useRef,
  useEffect,
  useCallback,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
} from 'react'
import type { TFunction } from 'i18next'
import { useTranslation } from 'react-i18next'
import { resolveViewer } from '../../viewers/registry'
import type { ViewerBookmarkApi, ViewerFindApi } from '../../viewers/types'
import { FindBar } from '../FindBar/FindBar'
import { ScrollTopButton } from '../ScrollTopButton/ScrollTopButton'
import { ReaderSettings } from '../ReaderSettings/ReaderSettings'
import { useReaderSettings } from '../ReaderSettings/useReaderSettings'
import { ExternalLinkPrompt } from '../ExternalLinkPrompt/ExternalLinkPrompt'
import { ReaderBookmarkButton } from './ReaderBookmarkButton'
import { getExternalLinkUrl, getInternalDocumentHash } from './linkUtils'
import { useFindInPage } from '../../hooks/useFindInPage'
import { useReaderBookmark } from '../../hooks/useReaderBookmark'
import { useTtsHighlight } from '../../tts/hooks/useTtsHighlight'
import type { SearchOpenTarget } from '../../types/search'
import type { TtsChunk } from '../../tts/types'
import { openExternalUrl } from '../../utils/openExternalUrl'
import {
  isFullscreenToolbarTap,
  type PointerPosition,
} from './fullscreenToolbar'
import {
  clearSearchTargetHighlight,
  decodeReaderHash,
  highlightFirstSearchTarget,
  readerScrollBehavior,
  scrollToReaderRange,
} from './readerTarget'
import './DocumentViewer.css'

const FLOATING_READER_ACTION_SCROLL_Y = 180
const PDF_FULLSCREEN_TOOLBAR_HIDE_MS = 3000
const PDF_FULLSCREEN_POINTER_THROTTLE_MS = 400

interface TtsHighlightOptions {
  enabled: boolean
  currentChunkIndex: number | null
  chunks: TtsChunk[]
  allowDomFallback?: boolean
  currentChunkTime?: number
  currentChunkDuration?: number
  isPlaying?: boolean
  wordHighlightEnabled?: boolean
}

interface DocumentViewerProps {
  url: string
  format?: string
  content: string
  className?: string
  appControls?: ReactNode
  headerControls?: ReactNode
  beforeDocument?: ReactNode
  ttsHighlight?: TtsHighlightOptions
  searchTarget?: SearchOpenTarget | null
  restoreBookmark?: boolean
  loading?: boolean
  loadError?: string
  onClose: () => void
}

export function DocumentViewer({
  url,
  format,
  content,
  className = '',
  appControls,
  headerControls,
  beforeDocument,
  ttsHighlight,
  searchTarget,
  restoreBookmark = false,
  loading = false,
  loadError,
  onClose,
}: DocumentViewerProps) {
  const { t } = useTranslation()
  const readerShellRef = useRef<HTMLDivElement | null>(null)
  const readerRef = useRef<HTMLElement | null>(null)
  const fullscreenToolbarTimerRef = useRef<number | null>(null)
  const fullscreenPointerActivityRef = useRef(0)
  const fullscreenTouchStartRef = useRef<(PointerPosition & { id: number }) | null>(null)
  const plugin = resolveViewer(url, format)
  const [viewerBookmarkApi, setViewerBookmarkApi] = useState<ViewerBookmarkApi | null>(null)
  const [viewerFindApi, setViewerFindApi] = useState<ViewerFindApi | null>(null)
  const [viewerToolbarTarget, setViewerToolbarTarget] = useState<HTMLDivElement | null>(null)
  const [pdfFullscreen, setPdfFullscreen] = useState(false)
  const [pdfToolbarVisible, setPdfToolbarVisible] = useState(true)
  const [showScrollTop, setShowScrollTop] = useState(false)
  const [pendingExternalUrl, setPendingExternalUrl] = useState<string | null>(null)
  const [externalLinkError, setExternalLinkError] = useState('')
  const { readerSettingsStyle, readerSettingsProps } = useReaderSettings()
  const {
    bookmarkNotice,
    canUndoBookmarkChange,
    dismissBookmarkNotice,
    hasBookmark,
    isAtBookmark,
    moveBookmark,
    removeBookmark,
    restoreBookmark: returnToBookmark,
    saveBookmark,
    undoBookmarkChange,
  } = useReaderBookmark(url, {
    enabled: !loading &&
      !loadError &&
      Boolean(viewerBookmarkApi),
    restoreOnOpen: restoreBookmark,
    viewerApi: viewerBookmarkApi,
  })

  const {
    showFind,
    findQuery,
    findMatchCount,
    findCurrentIndex,
    findInputRef,
    handleFind,
    findNext,
    findPrev,
    closeFind,
    setShowFind,
    handleViewerFindResult,
  } = useFindInPage(readerRef, viewerFindApi, content)

  useTtsHighlight(readerRef, ttsHighlight
    ? { ...ttsHighlight, enabled: ttsHighlight.enabled && plugin.id !== 'pdf' }
    : {
      enabled: false,
      currentChunkIndex: null,
      chunks: [],
    })

  const pdfTtsHighlightSpans = plugin.id === 'pdf' &&
    ttsHighlight?.enabled &&
    ttsHighlight.currentChunkIndex !== null
    ? ttsHighlight.chunks[ttsHighlight.currentChunkIndex]?.pdfSourceSpans
    : undefined

  const togglePdfFullscreen = useCallback(() => {
    const shell = readerShellRef.current
    if (!shell) return
    const doc = shell.ownerDocument
    const next = !pdfFullscreen

    setShowFind(false)
    if (next) setPdfToolbarVisible(true)
    setPdfFullscreen(next)
    if (next && typeof shell.requestFullscreen === 'function') {
      // Keep the CSS reading mode when a mobile WebView rejects native fullscreen.
      void shell.requestFullscreen().catch(() => {})
    } else if (!next && doc.fullscreenElement && typeof doc.exitFullscreen === 'function') {
      void doc.exitFullscreen().catch(() => {})
    }
  }, [pdfFullscreen, setShowFind])

  const clearFullscreenToolbarTimer = useCallback(() => {
    if (fullscreenToolbarTimerRef.current === null) return
    window.clearTimeout(fullscreenToolbarTimerRef.current)
    fullscreenToolbarTimerRef.current = null
  }, [])

  const scheduleFullscreenToolbarHide = useCallback(() => {
    clearFullscreenToolbarTimer()
    if (!pdfFullscreen) return

    const hideWhenIdle = () => {
      const toolbar = viewerToolbarTarget
      const finePointerHover = window.matchMedia('(hover: hover) and (pointer: fine)').matches &&
        toolbar?.matches(':hover')
      const menuOpen = toolbar?.querySelector('[aria-expanded="true"]')
      const activeElement = toolbar?.ownerDocument.activeElement
      const toolbarFocused = Boolean(activeElement && toolbar?.contains(activeElement))
      if (finePointerHover || toolbarFocused || menuOpen) {
        fullscreenToolbarTimerRef.current = window.setTimeout(hideWhenIdle, 1000)
        return
      }
      fullscreenToolbarTimerRef.current = null
      setPdfToolbarVisible(false)
    }

    fullscreenToolbarTimerRef.current = window.setTimeout(
      hideWhenIdle,
      PDF_FULLSCREEN_TOOLBAR_HIDE_MS,
    )
  }, [clearFullscreenToolbarTimer, pdfFullscreen, viewerToolbarTarget])

  const revealFullscreenToolbar = useCallback(() => {
    if (!pdfFullscreen) return
    setPdfToolbarVisible(true)
    scheduleFullscreenToolbarHide()
  }, [pdfFullscreen, scheduleFullscreenToolbarHide])

  useEffect(() => {
    if (pdfFullscreen) scheduleFullscreenToolbarHide()
    else clearFullscreenToolbarTimer()
    return clearFullscreenToolbarTimer
  }, [clearFullscreenToolbarTimer, pdfFullscreen, scheduleFullscreenToolbarHide])

  const handleFullscreenPointerDown = useCallback((event: ReactPointerEvent<HTMLDivElement>) => {
    if (!pdfFullscreen || event.pointerType === 'mouse') return
    fullscreenTouchStartRef.current = {
      id: event.pointerId,
      x: event.clientX,
      y: event.clientY,
    }
  }, [pdfFullscreen])

  const handleFullscreenPointerMove = useCallback((event: ReactPointerEvent<HTMLDivElement>) => {
    if (!pdfFullscreen || event.pointerType !== 'mouse') return
    const now = performance.now()
    if (now - fullscreenPointerActivityRef.current < PDF_FULLSCREEN_POINTER_THROTTLE_MS) return
    fullscreenPointerActivityRef.current = now
    revealFullscreenToolbar()
  }, [pdfFullscreen, revealFullscreenToolbar])

  const handleFullscreenPointerUp = useCallback((event: ReactPointerEvent<HTMLDivElement>) => {
    const start = fullscreenTouchStartRef.current
    fullscreenTouchStartRef.current = null
    if (!pdfFullscreen || event.pointerType === 'mouse' || start?.id !== event.pointerId) return
    if (!isFullscreenToolbarTap(start, { x: event.clientX, y: event.clientY })) return

    const target = event.target
    if (target instanceof Element && target.closest('button, a, input, select, [role="button"], [role="menu"]')) return
    const selection = event.currentTarget.ownerDocument.getSelection()
    if (selection && !selection.isCollapsed) return

    if (pdfToolbarVisible) {
      setPdfToolbarVisible(false)
      clearFullscreenToolbarTimer()
    } else {
      setPdfToolbarVisible(true)
      scheduleFullscreenToolbarHide()
    }
  }, [
    clearFullscreenToolbarTimer,
    pdfFullscreen,
    pdfToolbarVisible,
    scheduleFullscreenToolbarHide,
  ])

  useEffect(() => {
    if (plugin.id !== 'pdf') return
    const shell = readerShellRef.current
    if (!shell) return
    const doc = shell.ownerDocument
    const handleFullscreenChange = () => {
      if (doc.fullscreenElement !== shell) setPdfFullscreen(false)
    }
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !doc.fullscreenElement) setPdfFullscreen(false)
      if (event.key === 'Tab' && pdfFullscreen) revealFullscreenToolbar()
    }

    doc.addEventListener('fullscreenchange', handleFullscreenChange)
    doc.addEventListener('keydown', handleKeyDown)
    return () => {
      doc.removeEventListener('fullscreenchange', handleFullscreenChange)
      doc.removeEventListener('keydown', handleKeyDown)
    }
  }, [pdfFullscreen, plugin.id, revealFullscreenToolbar])

  useEffect(() => {
    if (plugin.id !== 'pdf') return
    const shell = readerShellRef.current
    if (!shell) return
    const doc = shell.ownerDocument
    return () => {
      if (doc.fullscreenElement === shell && typeof doc.exitFullscreen === 'function') {
        void doc.exitFullscreen().catch(() => {})
      }
    }
  }, [plugin.id])

  // Uploaded HTML/EPUB is already sanitized by the backend and rendered in the
  // app DOM. Handle internal anchors here so ToC/footnote clicks do not mutate
  // the app URL and can account for the fixed document header offset.
  const scrollToHash = useCallback((hash: string) => {
    const root = readerRef.current
    if (!root || !hash.startsWith('#')) return

    const id = decodeReaderHash(hash.slice(1))
    const doc = root.ownerDocument
    const idTarget = doc.getElementById(id)
    const namedTarget = Array.from(doc.getElementsByName(id)).find((node) => root.contains(node))
    const target = idTarget && root.contains(idTarget) ? idTarget : namedTarget
    if (!target) return

    const targetTop = window.scrollY + target.getBoundingClientRect().top
    window.scrollTo({ top: Math.max(targetTop - 120, 0), behavior: readerScrollBehavior() })
  }, [])

  useEffect(() => {
    if (plugin.id === 'pdf' || loading || loadError || !content || !searchTarget) return

    let highlightedRoot: HTMLElement | null = null
    const frame = requestAnimationFrame(() => {
      const root = readerRef.current
      if (!root) return
      highlightedRoot = root
      clearSearchTargetHighlight(root)
      if (searchTarget.hash) scrollToHash(searchTarget.hash)
      const target = searchTarget.text ? highlightFirstSearchTarget(root, searchTarget.text) : null
      if (target) scrollToReaderRange(target)
    })

    return () => {
      cancelAnimationFrame(frame)
      if (highlightedRoot) clearSearchTargetHighlight(highlightedRoot)
    }
  }, [content, loadError, loading, plugin.id, scrollToHash, searchTarget, url])

  // Direct rendering makes document links ordinary DOM events again. Internal
  // hash links scroll in-place; everything else asks before leaving the app.
  useEffect(() => {
    const root = readerRef.current
    if (!root) return
    const readerRoot = root

    function handleReaderClick(event: MouseEvent) {
      const target = event.target
      if (!(target instanceof Element)) return
      const link = target.closest('a[href]')
      if (!link || !readerRoot.contains(link)) return

      const href = link.getAttribute('href') ?? ''
      // Imported content never gets browser-default link behavior. Papercut
      // explicitly handles safe hashes and approved external protocols below;
      // malformed or disallowed URLs remain inert even if backend sanitation
      // regresses or an older stored document reaches this reader.
      event.preventDefault()
      const internalHash = getInternalDocumentHash(href)
      if (internalHash) {
        scrollToHash(internalHash)
        return
      }

      const externalUrl = getExternalLinkUrl(href)
      if (!externalUrl) return

      setExternalLinkError('')
      setPendingExternalUrl(externalUrl)
    }

    readerRoot.addEventListener('click', handleReaderClick)
    return () => readerRoot.removeEventListener('click', handleReaderClick)
  }, [content, scrollToHash, url])

  const closeExternalLinkPrompt = useCallback(() => {
    setPendingExternalUrl(null)
    setExternalLinkError('')
  }, [])

  const scrollToTop = useCallback(() => {
    dismissBookmarkNotice()
    if (plugin.id === 'pdf' && viewerBookmarkApi) {
      viewerBookmarkApi.scrollToTop()
      return
    }
    window.scrollTo({ top: 0, behavior: readerScrollBehavior() })
  }, [dismissBookmarkNotice, plugin.id, viewerBookmarkApi])

  const openPendingExternalUrl = useCallback(async () => {
    if (!pendingExternalUrl) return
    try {
      await openExternalUrl(pendingExternalUrl)
      setPendingExternalUrl(null)
      setExternalLinkError('')
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      console.error('Failed to open external link:', err)
      setExternalLinkError(message)
    }
  }, [pendingExternalUrl])

  useEffect(() => {
    if (plugin.id === 'pdf' && viewerBookmarkApi) {
      const update = () => setShowScrollTop(viewerBookmarkApi.isPastStart())
      update()
      return viewerBookmarkApi.subscribe(update)
    }

    function handleScroll() {
      setShowScrollTop(window.scrollY > FLOATING_READER_ACTION_SCROLL_Y)
    }

    handleScroll()
    window.addEventListener('scroll', handleScroll)
    return () => window.removeEventListener('scroll', handleScroll)
  }, [plugin.id, url, viewerBookmarkApi])

  const ViewerComponent = plugin.Component
  const appClassName = [
    'app',
    'app-reader',
    plugin.id === 'pdf' ? 'app-reader-pdf' : '',
    pdfFullscreen ? 'app-reader-pdf-fullscreen' : '',
    pdfFullscreen && !pdfToolbarVisible ? 'app-reader-pdf-toolbar-hidden' : '',
    className,
  ].filter(Boolean).join(' ')

  return (
    <div
      ref={readerShellRef}
      className={appClassName}
      data-reader-theme={plugin.id !== 'pdf' ? readerSettingsProps.settings.pageTheme : undefined}
      onFocusCapture={revealFullscreenToolbar}
      onPointerCancel={() => { fullscreenTouchStartRef.current = null }}
      onPointerDown={handleFullscreenPointerDown}
      onPointerMove={handleFullscreenPointerMove}
      onPointerUp={handleFullscreenPointerUp}
    >
      <header className="header doc-header">
        <div className="header-left">
          <button className="back-button" onClick={onClose}>
            <svg className="back-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
              <path d="M19 12H5" />
              <path d="m12 19-7-7 7-7" />
            </svg>
            {t('reader.back')}
          </button>
        </div>
        <div className="header-right">
          {headerControls && (
            <div className={'header-control-group header-control-group-audio header-controls-slot' + (loading ? ' header-controls-slot-disabled' : '')}>
              {headerControls}
            </div>
          )}
          <div className="header-control-group header-control-group-reader">
            {appControls}
            {plugin.id !== 'pdf' && (
              <ReaderSettings disabled={loading} {...readerSettingsProps} />
            )}
            <button
              className="find-btn"
              disabled={loading || Boolean(loadError)}
              onClick={() => {
                setShowFind(true)
                setTimeout(() => findInputRef.current?.focus(), 0)
              }}
            >
              <svg className="find-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
                <circle cx="11" cy="11" r="7" />
                <path d="m16 16 5 5" />
              </svg>
              {t('reader.find')}
            </button>
          </div>
        </div>
        {plugin.id === 'pdf' && !loading && !loadError && (
          <div
            ref={setViewerToolbarTarget}
            className="viewer-toolbar-slot"
          />
        )}
      </header>

      {showFind && (
        <FindBar
          query={findQuery}
          matchCount={findMatchCount}
          currentIndex={findCurrentIndex}
          inputRef={findInputRef}
          onChange={handleFind}
          onNext={findNext}
          onPrev={findPrev}
          onClose={closeFind}
        />
      )}

      {!pdfFullscreen && beforeDocument}

      <main className="document-view" style={readerSettingsStyle}>
        {loading ? (
          <div className="document-html-surface document-loading-surface" role="status" aria-live="polite">
            <span className="spinner" aria-hidden="true" />
            <span>{t('reader.openingDocument')}</span>
          </div>
        ) : loadError ? (
          <div className="document-html-surface document-loading-surface document-load-error" role="alert">
            <strong>{t('reader.unableToOpen')}</strong>
            <span dir="auto">{loadError}</span>
          </div>
        ) : (
          <ViewerComponent
            url={url}
            format={format}
            content={content}
            contentRef={readerRef}
            toolbarTarget={viewerToolbarTarget}
            searchTarget={searchTarget}
            pdfTtsHighlightSpans={pdfTtsHighlightSpans}
            fullscreen={pdfFullscreen}
            onBookmarkApiChange={setViewerBookmarkApi}
            onFindApiChange={setViewerFindApi}
            onFindResult={handleViewerFindResult}
            onFullscreenChange={togglePdfFullscreen}
          />
        )}
      </main>

      {showScrollTop && (
        <div className="reader-floating-actions">
          <ReaderBookmarkButton
            hasBookmark={hasBookmark}
            isAtBookmark={isAtBookmark}
            onMove={moveBookmark}
            onRemove={removeBookmark}
            onRestore={returnToBookmark}
            onSave={saveBookmark}
          />

          <ScrollTopButton
            visible={showScrollTop}
            onClick={scrollToTop}
          />
        </div>
      )}

      {bookmarkNotice && (
        <div className="reader-bookmark-notice" role="status" aria-live="polite">
          <span>{bookmarkNoticeText(bookmarkNotice, t)}</span>
          {bookmarkNotice === 'restored' && <button type="button" onClick={scrollToTop}>{t('reader.top')}</button>}
          {canUndoBookmarkChange && (
            <button type="button" onClick={undoBookmarkChange}>{t('reader.undoBookmarkChange')}</button>
          )}
          <button
            type="button"
            className="reader-bookmark-dismiss"
            aria-label={t('reader.dismissBookmarkNotice')}
            onClick={dismissBookmarkNotice}
          >
            &times;
          </button>
        </div>
      )}

      {pendingExternalUrl && (
        <ExternalLinkPrompt
          url={pendingExternalUrl}
          error={externalLinkError}
          onCancel={closeExternalLinkPrompt}
          onOpen={openPendingExternalUrl}
        />
      )}
    </div>
  )
}

function bookmarkNoticeText(
  notice: 'restored' | 'saved' | 'updated' | 'removed' | 'changeUndone',
  t: TFunction,
): string {
  if (notice === 'restored') return t('reader.bookmarkRestored')
  if (notice === 'removed') return t('reader.bookmarkRemoved')
  if (notice === 'changeUndone') return t('reader.bookmarkChangeUndone')
  return notice === 'updated' ? t('reader.bookmarkUpdated') : t('reader.bookmarkSaved')
}
