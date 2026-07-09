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
import type { SearchOpenTarget } from '../../types/search'
import type { TtsChunk } from '../../tts/types'
import { openExternalUrl } from '../../utils/openExternalUrl'
import './DocumentViewer.css'

const SEARCH_TARGET_HIGHLIGHT_NAME = 'search-target'

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
  searchTarget?: SearchOpenTarget | null
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
  loading = false,
  loadError,
  onClose,
}: DocumentViewerProps) {
  const readerRef = useRef<HTMLElement | null>(null)
  const [showScrollTop, setShowScrollTop] = useState(false)
  const [pendingExternalUrl, setPendingExternalUrl] = useState<string | null>(null)
  const [externalLinkError, setExternalLinkError] = useState('')
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
    window.scrollTo({ top: Math.max(targetTop - 120, 0), behavior: readerScrollBehavior() })
  }, [])

  useEffect(() => {
    if (loading || loadError || !content || !searchTarget) return

    let highlightedRoot: HTMLElement | null = null
    const frame = requestAnimationFrame(() => {
      const root = readerRef.current
      if (!root) return
      highlightedRoot = root
      clearSearchTargetHighlight(root)
      if (searchTarget.hash) scrollToHash(searchTarget.hash)
      const target = searchTarget.text ? highlightFirstSearchTarget(root, searchTarget.text) : null
      if (target) scrollToRange(target)
    })

    return () => {
      cancelAnimationFrame(frame)
      if (highlightedRoot) clearSearchTargetHighlight(highlightedRoot)
    }
  }, [content, loadError, loading, scrollToHash, searchTarget, url])

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
          <button className="back-button" onClick={onClose}>
            <svg className="back-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
              <path d="M19 12H5" />
              <path d="m12 19-7-7 7-7" />
            </svg>
            Back
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
            <ReaderSettings disabled={loading} {...readerSettingsProps} />
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
              Find
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
        onClick={() => window.scrollTo({ top: 0, behavior: readerScrollBehavior() })}
      />

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

function decodeHash(value: string): string {
  try {
    return decodeURIComponent(value)
  } catch {
    return value
  }
}

function clearSearchTargetHighlight(root: HTMLElement): void {
  const doc = root.ownerDocument
  clearSearchTargetRegistry(doc)
  root.querySelectorAll('mark[data-search-target]').forEach((mark) => {
    const parent = mark.parentNode
    if (!parent) return
    parent.replaceChild(doc.createTextNode(mark.textContent ?? ''), mark)
    parent.normalize()
  })
}

// Search-result jumps deliberately avoid rewriting large reader DOMs. The old
// <mark> insertion path could destabilize iOS/WebKit after deep EPUB jumps, so
// the normal path is now: find a live Range, register a named CSS Highlight,
// then scroll to the Range rect. The DOM fallback is only for non-iOS WebViews
// that do not expose CSS.highlights.
function highlightFirstSearchTarget(root: HTMLElement, text: string): Range | null {
  const range = findFirstSearchTargetRange(root, text)
  if (!range) return null
  if (setSearchTargetRegistryHighlight(root.ownerDocument, range)) return range

  if (!isIOSWebKit()) {
    const mark = markRangeSearchTarget(range)
    return mark ? rangeForElement(mark) : range
  }

  return range
}

// Finds the first visible text-node occurrence that matches the result snippet.
// This is intentionally a simple first-match locator because search cards are
// document-level summaries; durable section/page locators are the later scalable
// fix for repeated phrases in very large books.
function findFirstSearchTargetRange(root: HTMLElement, text: string): Range | null {
  const query = text.trim().toLowerCase()
  if (!query) return null

  const doc = root.ownerDocument
  const walker = doc.createTreeWalker(root, NodeFilter.SHOW_TEXT)
  let node: Node | null
  while ((node = walker.nextNode())) {
    if (node.parentElement?.closest('script, style, noscript, svg')) continue
    const value = node.textContent ?? ''
    const at = value.toLowerCase().indexOf(query)
    if (at < 0) continue

    const range = doc.createRange()
    range.setStart(node, at)
    range.setEnd(node, at + query.length)
    return range
  }

  return null
}

// Own one CSS Highlight registry entry. Do not clear the whole registry here:
// audiobook playback owns "tts-current", and Find may own its own marks/ranges.
function setSearchTargetRegistryHighlight(doc: Document, range: Range): boolean {
  const view = doc.defaultView
  const registry = view?.CSS?.highlights
  if (!view || !registry) return false

  const highlight = new view.Highlight(range)
  registry.set(SEARCH_TARGET_HIGHLIGHT_NAME, highlight)
  return true
}

function clearSearchTargetRegistry(doc: Document): void {
  const registry = doc.defaultView?.CSS?.highlights
  if (!registry) return
  registry.get(SEARCH_TARGET_HIGHLIGHT_NAME)?.clear()
  registry.delete(SEARCH_TARGET_HIGHLIGHT_NAME)
}

// Last-resort compatibility for older non-iOS WebViews. It only handles the
// single-text-node ranges created above; cross-node wrapping is avoided because
// the search-result target is a convenience jump, not a full document highlighter.
function markRangeSearchTarget(range: Range): HTMLElement | null {
  const doc = range.startContainer.ownerDocument
  if (!doc) return null
  if (range.startContainer !== range.endContainer || range.startContainer.nodeType !== Node.TEXT_NODE) return null

  const text = range.startContainer.textContent ?? ''
  const start = range.startOffset
  const end = range.endOffset
  const mark = doc.createElement('mark')
  mark.dataset.searchTarget = 'true'
  mark.textContent = text.slice(start, end)

  const fragment = doc.createDocumentFragment()
  if (start > 0) fragment.appendChild(doc.createTextNode(text.slice(0, start)))
  fragment.appendChild(mark)
  if (end < text.length) {
    fragment.appendChild(doc.createTextNode(text.slice(end)))
  }
  range.startContainer.parentNode?.replaceChild(fragment, range.startContainer)
  return mark
}

function rangeForElement(element: Element): Range {
  const range = element.ownerDocument.createRange()
  range.selectNodeContents(element)
  return range
}

function scrollToRange(range: Range): void {
  const rect = range.getBoundingClientRect()
  const targetTop = window.scrollY + rect.top
  window.scrollTo({ top: Math.max(targetTop - window.innerHeight / 2, 0), behavior: readerScrollBehavior() })
}

function readerScrollBehavior(): ScrollBehavior {
  return isIOSWebKit() ? 'auto' : 'smooth'
}

function isIOSWebKit(): boolean {
  if (typeof navigator === 'undefined') return false
  return /iP(ad|hone|od)/.test(navigator.userAgent) ||
    (navigator.platform === 'MacIntel' && navigator.maxTouchPoints > 1)
}
