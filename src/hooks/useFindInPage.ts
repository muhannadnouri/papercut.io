import { useState, useEffect, useCallback, useRef } from 'react'
import {
  createReaderTextIndex,
  findReaderTextIndexMatches,
  rangeForReaderTextIndexMatch,
  type ReaderTextIndex,
  type ReaderTextIndexMatch,
} from '../components/DocumentViewer/readerTextRanges'
import { clearSearchTargetHighlight } from '../components/DocumentViewer/readerTarget'
import { isIOSWebKit } from '../utils/platform'
import type { ViewerFindApi, ViewerFindResult } from '../viewers/types'

const FIND_DEBOUNCE_MS = 180
const FIND_HIGHLIGHT_NAME = 'find-match'
const FIND_CURRENT_HIGHLIGHT_NAME = 'find-current'
const MAX_FIND_HIGHLIGHTS = 500

interface UseFindInPageReturn {
  showFind: boolean
  findQuery: string
  findMatchCount: number
  findCurrentIndex: number
  findInputRef: React.RefObject<HTMLInputElement | null>
  handleFind: (searchQuery: string) => void
  findNext: () => void
  findPrev: () => void
  closeFind: () => void
  setShowFind: React.Dispatch<React.SetStateAction<boolean>>
  handleViewerFindResult: (result: ViewerFindResult) => void
}

export function useFindInPage(
  rootRef: React.RefObject<HTMLElement | null>,
  viewerFindApi?: ViewerFindApi | null,
  searchContextKey?: unknown,
): UseFindInPageReturn {
  const [showFind, setShowFind] = useState(false)
  const [findQuery, setFindQuery] = useState('')
  const [findMatchCount, setFindMatchCount] = useState(0)
  const [findCurrentIndex, setFindCurrentIndex] = useState(0)
  const findInputRef = useRef<HTMLInputElement | null>(null)
  const findTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const readerIndexRef = useRef<{
    root: HTMLElement
    key: unknown
    index: ReaderTextIndex
  } | null>(null)
  const findMatchesRef = useRef<ReaderTextIndexMatch[]>([])
  const fallbackSelectionActiveRef = useRef(false)

  const clearPendingFind = useCallback(() => {
    if (findTimerRef.current === null) return
    clearTimeout(findTimerRef.current)
    findTimerRef.current = null
  }, [])

  const clearFindHighlights = useCallback(() => {
    const root = rootRef.current
    if (!root) return
    const doc = root.ownerDocument
    clearFindRegistryHighlights(doc)
    if (fallbackSelectionActiveRef.current) {
      doc.getSelection()?.removeAllRanges()
      fallbackSelectionActiveRef.current = false
    }

    // Remove marks left by older app versions or a hot reload. New Find
    // highlights never rewrite the reader DOM.
    const marks = root.querySelectorAll('mark[data-find]')
    marks.forEach((mark) => {
      const parent = mark.parentNode
      if (parent) {
        parent.replaceChild(doc.createTextNode(mark.textContent ?? ''), mark)
        parent.normalize()
      }
    })
  }, [rootRef])

  const getReaderIndex = useCallback((root: HTMLElement): ReaderTextIndex => {
    const cached = readerIndexRef.current
    if (cached?.root === root && cached.key === searchContextKey) return cached.index

    const index = createReaderTextIndex(root)
    readerIndexRef.current = { root, key: searchContextKey, index }
    return index
  }, [searchContextKey])

  const highlightFindMatches = useCallback((searchQuery: string): number => {
    const root = rootRef.current
    if (root) clearSearchTargetHighlight(root)
    clearFindHighlights()
    if (!root || !searchQuery.trim()) return 0
    const doc = root.ownerDocument

    const index = getReaderIndex(root)
    const matches = findReaderTextIndexMatches(index, searchQuery)
    findMatchesRef.current = matches
    if (matches.length <= MAX_FIND_HIGHLIGHTS) {
      setFindRegistryHighlight(
        doc,
        FIND_HIGHLIGHT_NAME,
        matches.map((match) => rangeForReaderTextIndexMatch(index, match)),
      )
    }
    return matches.length
  }, [rootRef, clearFindHighlights, getReaderIndex])

  const scrollToMatch = useCallback((index: number) => {
    const root = rootRef.current
    if (!root) return
    const readerIndex = readerIndexRef.current?.index
    const match = findMatchesRef.current[index]
    if (!readerIndex || !match) return

    const doc = root.ownerDocument
    clearCurrentFindHighlight(doc)
    if (fallbackSelectionActiveRef.current) doc.getSelection()?.removeAllRanges()

    const range = rangeForReaderTextIndexMatch(readerIndex, match)
    fallbackSelectionActiveRef.current = !setFindRegistryHighlight(
      doc,
      FIND_CURRENT_HIGHLIGHT_NAME,
      [range],
    )
    if (fallbackSelectionActiveRef.current) {
      const selection = doc.getSelection()
      selection?.removeAllRanges()
      selection?.addRange(range.cloneRange())
    }

    const absoluteTop = window.scrollY + range.getBoundingClientRect().top
    window.scrollTo({ top: absoluteTop - window.innerHeight / 2, behavior: findScrollBehavior() })
  }, [rootRef])

  const closeFind = useCallback(() => {
    clearPendingFind()
    setShowFind(false)
    setFindQuery('')
    setFindMatchCount(0)
    setFindCurrentIndex(0)
    findMatchesRef.current = []
    viewerFindApi?.clear()
    clearFindHighlights()
  }, [clearFindHighlights, clearPendingFind, viewerFindApi])

  const handleFind = useCallback((searchQuery: string) => {
    setFindQuery(searchQuery)
    setFindMatchCount(0)
    setFindCurrentIndex(0)
    findMatchesRef.current = []
    if (searchQuery.trim()) return
    clearPendingFind()
    viewerFindApi?.clear()
    clearFindHighlights()
  }, [clearFindHighlights, clearPendingFind, viewerFindApi])

  useEffect(() => {
    clearPendingFind()
    const searchQuery = findQuery.trim()
    if (!searchQuery) {
      clearFindHighlights()
      return
    }

    findTimerRef.current = setTimeout(() => {
      if (viewerFindApi) {
        viewerFindApi.search(searchQuery)
        findTimerRef.current = null
        return
      }
      const count = highlightFindMatches(searchQuery)
      setFindMatchCount(count)
      setFindCurrentIndex(0)
      if (count > 0) scrollToMatch(0)
      findTimerRef.current = null
    }, FIND_DEBOUNCE_MS)

    return clearPendingFind
  }, [clearFindHighlights, clearPendingFind, highlightFindMatches, scrollToMatch, findQuery, viewerFindApi])

  const findNext = useCallback(() => {
    if (findMatchCount === 0) return
    if (viewerFindApi) {
      viewerFindApi.next()
      return
    }
    const next = (findCurrentIndex + 1) % findMatchCount
    setFindCurrentIndex(next)
    scrollToMatch(next)
  }, [findMatchCount, findCurrentIndex, scrollToMatch, viewerFindApi])

  const findPrev = useCallback(() => {
    if (findMatchCount === 0) return
    if (viewerFindApi) {
      viewerFindApi.previous()
      return
    }
    const prev = (findCurrentIndex - 1 + findMatchCount) % findMatchCount
    setFindCurrentIndex(prev)
    scrollToMatch(prev)
  }, [findMatchCount, findCurrentIndex, scrollToMatch, viewerFindApi])

  const handleViewerFindResult = useCallback((result: ViewerFindResult) => {
    setFindMatchCount(result.matchCount)
    setFindCurrentIndex(result.currentIndex)
  }, [])

  useEffect(() => {
    function handleKeyDown(e: KeyboardEvent) {
      if ((e.ctrlKey || e.metaKey) && e.key === 'f') {
        e.preventDefault()
        setShowFind(true)
        setTimeout(() => findInputRef.current?.focus(), 0)
      }
      if (e.key === 'Escape') closeFind()
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [closeFind])

  useEffect(() => () => {
    clearPendingFind()
    clearFindHighlights()
    readerIndexRef.current = null
    findMatchesRef.current = []
  }, [clearFindHighlights, clearPendingFind])

  return {
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
  }
}

function setFindRegistryHighlight(doc: Document, name: string, ranges: Range[]): boolean {
  const view = doc.defaultView
  const registry = view?.CSS?.highlights
  if (!view || !registry) return false
  registry.set(name, new view.Highlight(...ranges))
  return true
}

function clearCurrentFindHighlight(doc: Document): void {
  const registry = doc.defaultView?.CSS.highlights
  if (!registry) return
  registry.get(FIND_CURRENT_HIGHLIGHT_NAME)?.clear()
  registry.delete(FIND_CURRENT_HIGHLIGHT_NAME)
}

function clearFindRegistryHighlights(doc: Document): void {
  const registry = doc.defaultView?.CSS.highlights
  if (!registry) return
  registry.get(FIND_HIGHLIGHT_NAME)?.clear()
  registry.delete(FIND_HIGHLIGHT_NAME)
  clearCurrentFindHighlight(doc)
}

function findScrollBehavior(): ScrollBehavior {
  return isIOSWebKit() ? 'auto' : 'smooth'
}
