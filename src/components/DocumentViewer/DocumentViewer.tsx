import { useState, useRef, useEffect, useCallback, type ReactNode } from 'react'
import type { TFunction } from 'i18next'
import { useTranslation } from 'react-i18next'
import { resolveViewer } from '../../viewers/registry'
import type { ViewerFindApi } from '../../viewers/types'
import { FindBar } from '../FindBar/FindBar'
import { ScrollTopButton } from '../ScrollTopButton/ScrollTopButton'
import { ReaderSettings } from '../ReaderSettings/ReaderSettings'
import { useReaderSettings } from '../ReaderSettings/useReaderSettings'
import { ExternalLinkPrompt } from '../ExternalLinkPrompt/ExternalLinkPrompt'
import { getExternalLinkUrl, getInternalDocumentHash } from './linkUtils'
import { useFindInPage } from '../../hooks/useFindInPage'
import { useReaderBookmark } from '../../hooks/useReaderBookmark'
import { useTtsHighlight } from '../../tts/hooks/useTtsHighlight'
import type { SearchOpenTarget } from '../../types/search'
import type { TtsChunk } from '../../tts/types'
import { openExternalUrl } from '../../utils/openExternalUrl'
import {
  clearSearchTargetHighlight,
  decodeReaderHash,
  highlightFirstSearchTarget,
  readerScrollBehavior,
  scrollToReaderRange,
} from './readerTarget'
import './DocumentViewer.css'

const FLOATING_READER_ACTION_SCROLL_Y = 180

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
  const readerRef = useRef<HTMLElement | null>(null)
  const plugin = resolveViewer(url, format)
  const [viewerFindApi, setViewerFindApi] = useState<ViewerFindApi | null>(null)
  const [viewerToolbarTarget, setViewerToolbarTarget] = useState<HTMLDivElement | null>(null)
  const [showScrollTop, setShowScrollTop] = useState(false)
  const [pendingExternalUrl, setPendingExternalUrl] = useState<string | null>(null)
  const [externalLinkError, setExternalLinkError] = useState('')
  const { readerSettingsStyle, readerSettingsProps } = useReaderSettings()
  const {
    bookmarkNotice,
    dismissBookmarkNotice,
    hasBookmark,
    isAtBookmark,
    toggleBookmark,
  } = useReaderBookmark(url, {
    enabled: !loading && !loadError && Boolean(content),
    restoreOnOpen: restoreBookmark,
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
  } = useFindInPage(readerRef, viewerFindApi)

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
    window.scrollTo({ top: 0, behavior: readerScrollBehavior() })
  }, [dismissBookmarkNotice])

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
    function handleScroll() {
      setShowScrollTop(window.scrollY > FLOATING_READER_ACTION_SCROLL_Y)
    }

    handleScroll()
    window.addEventListener('scroll', handleScroll)
    return () => window.removeEventListener('scroll', handleScroll)
  }, [url])

  const ViewerComponent = plugin.Component
  const appClassName = [
    'app',
    'app-reader',
    plugin.id === 'pdf' ? 'app-reader-pdf' : '',
    className,
  ].filter(Boolean).join(' ')

  return (
    <div className={appClassName}>
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
            <ReaderSettings disabled={loading || plugin.id === 'pdf'} {...readerSettingsProps} />
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

      {beforeDocument}

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
            onFindApiChange={setViewerFindApi}
            onFindResult={handleViewerFindResult}
          />
        )}
      </main>

      {showScrollTop && (
        <div className="reader-floating-actions">
          <button
            type="button"
            className={'reader-bookmark-btn' + (isAtBookmark ? ' reader-bookmark-btn-active' : '')}
            aria-label={bookmarkActionLabel(hasBookmark, isAtBookmark, t)}
            title={bookmarkActionLabel(hasBookmark, isAtBookmark, t)}
            onClick={toggleBookmark}
          >
            <svg className="reader-bookmark-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
              <path d="M6 4.75A2.75 2.75 0 0 1 8.75 2h6.5A2.75 2.75 0 0 1 18 4.75V21l-6-3.5L6 21z" />
              {hasBookmark && <path d="m9 10.8 2 2 4-4" />}
            </svg>
          </button>

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
  notice: 'restored' | 'saved' | 'updated' | 'removed',
  t: TFunction,
): string {
  if (notice === 'restored') return t('reader.bookmarkRestored')
  if (notice === 'removed') return t('reader.bookmarkRemoved')
  return notice === 'updated' ? t('reader.bookmarkUpdated') : t('reader.bookmarkSaved')
}

function bookmarkActionLabel(
  hasBookmark: boolean,
  isAtBookmark: boolean,
  t: TFunction,
): string {
  if (isAtBookmark) return t('reader.removeBookmark')
  return hasBookmark ? t('reader.updateBookmark') : t('reader.saveBookmark')
}
