import { useState, useRef, useEffect, useCallback, type ReactNode } from 'react'
import { resolveViewer } from '../../viewers/registry'
import { FindBar } from '../FindBar/FindBar'
import { ScrollTopButton } from '../ScrollTopButton/ScrollTopButton'
import { ReaderSettings } from '../ReaderSettings/ReaderSettings'
import { useReaderSettings } from '../ReaderSettings/useReaderSettings'
import { ExternalLinkPrompt } from '../ExternalLinkPrompt/ExternalLinkPrompt'
import { getExternalLinkUrl, getInternalDocumentHash } from './linkUtils'
import { useFindInPage } from '../../hooks/useFindInPage'
import { useTtsHighlight } from '../../tts/hooks/useTtsHighlight'
import type { TtsChunk } from '../../tts/types'

interface TtsHighlightOptions {
  enabled: boolean
  currentChunkIndex: number | null
  chunks: TtsChunk[]
  allowDomFallback?: boolean
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
  loading = false,
  loadError,
  onClose,
}: DocumentViewerProps) {
  const readerRef = useRef<HTMLElement | null>(null)
  const [showScrollTop, setShowScrollTop] = useState(false)
  const [pendingExternalUrl, setPendingExternalUrl] = useState<string | null>(null)
  const { readerSettingsStyle, readerSettingsProps } = useReaderSettings()

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
  } = useFindInPage(readerRef)

  useTtsHighlight(readerRef, ttsHighlight ?? {
    enabled: false,
    currentChunkIndex: null,
    chunks: [],
  })

  // Uploaded HTML/EPUB is already sanitized by the backend and rendered in the
  // app DOM. Handle internal anchors here so ToC/footnote clicks do not mutate
  // the app URL and can account for the fixed document header offset.
  const scrollToHash = useCallback((hash: string) => {
    const root = readerRef.current
    if (!root || !hash.startsWith('#')) return

    const id = decodeHash(hash.slice(1))
    const doc = root.ownerDocument
    const idTarget = doc.getElementById(id)
    const namedTarget = Array.from(doc.getElementsByName(id)).find((node) => root.contains(node))
    const target = idTarget && root.contains(idTarget) ? idTarget : namedTarget
    if (!target) return

    const targetTop = window.scrollY + target.getBoundingClientRect().top
    window.scrollTo({ top: Math.max(targetTop - 120, 0), behavior: 'smooth' })
  }, [])

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
      const internalHash = getInternalDocumentHash(href)
      if (internalHash) {
        event.preventDefault()
        scrollToHash(internalHash)
        return
      }

      const externalUrl = getExternalLinkUrl(href)
      if (!externalUrl) return

      event.preventDefault()
      setPendingExternalUrl(externalUrl)
    }

    readerRoot.addEventListener('click', handleReaderClick)
    return () => readerRoot.removeEventListener('click', handleReaderClick)
  }, [content, scrollToHash, url])

  const closeExternalLinkPrompt = useCallback(() => {
    setPendingExternalUrl(null)
  }, [])

  const openPendingExternalUrl = useCallback(() => {
    if (!pendingExternalUrl) return
    window.open(pendingExternalUrl, '_blank', 'noopener,noreferrer')
    setPendingExternalUrl(null)
  }, [pendingExternalUrl])

  useEffect(() => {
    function handleScroll() {
      setShowScrollTop(window.scrollY > 300)
    }

    handleScroll()
    window.addEventListener('scroll', handleScroll)
    return () => window.removeEventListener('scroll', handleScroll)
  }, [url])

  const plugin = resolveViewer(url, format)
  const ViewerComponent = plugin.Component
  const appClassName = ['app', className].filter(Boolean).join(' ')

  return (
    <div className={appClassName}>
      <header className="header doc-header">
        <div className="header-left">
          <button className="back-button" onClick={onClose}>&larr; Back</button>
        </div>
        <div className="header-right">
          {headerControls && (
            <div className={'header-control-group header-control-group-audio header-controls-slot' + (loading ? ' header-controls-slot-disabled' : '')}>
              {headerControls}
            </div>
          )}
          <div className="header-control-group header-control-group-reader">
            {appControls}
            <ReaderSettings disabled={loading} {...readerSettingsProps} />
            <button
              className="find-btn"
              disabled={loading || Boolean(loadError)}
              onClick={() => {
                setShowFind(true)
                setTimeout(() => findInputRef.current?.focus(), 0)
              }}
            >
              &#128269; Find
            </button>
          </div>
        </div>
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
            <span>Opening Document...</span>
          </div>
        ) : loadError ? (
          <div className="document-html-surface document-loading-surface document-load-error" role="alert">
            <strong>Unable to open document.</strong>
            <span>{loadError}</span>
          </div>
        ) : (
          <ViewerComponent
            url={url}
            format={format}
            content={content}
            contentRef={readerRef}
          />
        )}
      </main>

      <ScrollTopButton
        visible={showScrollTop}
        onClick={() => window.scrollTo({ top: 0, behavior: 'smooth' })}
      />

      {pendingExternalUrl && (
        <ExternalLinkPrompt
          url={pendingExternalUrl}
          onCancel={closeExternalLinkPrompt}
          onOpen={openPendingExternalUrl}
        />
      )}
    </div>
  )
}

function decodeHash(value: string): string {
  try {
    return decodeURIComponent(value)
  } catch {
    return value
  }
}
