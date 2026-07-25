import { useState, useEffect, useCallback, useRef } from 'react'
import {
  findReaderTextMatches,
  type ReaderTextMatch,
} from '../components/DocumentViewer/readerTextRanges'
import { clearSearchTargetHighlight } from '../components/DocumentViewer/readerTarget'
import { isIOSWebKit } from '../utils/platform'
import type { ViewerFindApi, ViewerFindResult } from '../viewers/types'

const FIND_DEBOUNCE_MS = 180

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
): UseFindInPageReturn {
  const [showFind, setShowFind] = useState(false)
  const [findQuery, setFindQuery] = useState('')
  const [findMatchCount, setFindMatchCount] = useState(0)
  const [findCurrentIndex, setFindCurrentIndex] = useState(0)
  const findInputRef = useRef<HTMLInputElement | null>(null)
  const findTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const clearPendingFind = useCallback(() => {
    if (findTimerRef.current === null) return
    clearTimeout(findTimerRef.current)
    findTimerRef.current = null
  }, [])

  const clearFindHighlights = useCallback(() => {
    const root = rootRef.current
    if (!root) return
    const doc = root.ownerDocument
    const marks = root.querySelectorAll('mark[data-find]')
    marks.forEach((mark) => {
      const parent = mark.parentNode
      if (parent) {
        parent.replaceChild(doc.createTextNode(mark.textContent ?? ''), mark)
        parent.normalize()
      }
    })
  }, [rootRef])

  const highlightFindMatches = useCallback((searchQuery: string): number => {
    const root = rootRef.current
    if (root) clearSearchTargetHighlight(root)
    clearFindHighlights()
    if (!root || !searchQuery.trim()) return 0
    const doc = root.ownerDocument

    if (!doc.getElementById('find-styles')) {
      const style = doc.createElement('style')
      style.id = 'find-styles'
      style.textContent =
        'mark[data-find] { background: var(--highlight-find, #fef08a); color: inherit; padding: 0; border-radius: 2px; }' +
        'mark[data-find].current { background: var(--highlight-current, #f97316); color: var(--highlight-current-text, #fff); }'
      doc.head.appendChild(style)
    }

    const matches = findReaderTextMatches(root, searchQuery)
    markFindMatches(doc, matches)
    return matches.length
  }, [rootRef, clearFindHighlights])

  const scrollToMatch = useCallback((index: number) => {
    const root = rootRef.current
    if (!root) return
    root.querySelectorAll('mark[data-find].current').forEach((mark) => mark.classList.remove('current'))
    const targets = root.querySelectorAll<HTMLElement>(`mark[data-find="${index}"]`)
    const target = targets[0]
    if (target) {
      targets.forEach((mark) => mark.classList.add('current'))
      const absoluteTop = window.scrollY + target.getBoundingClientRect().top
      window.scrollTo({ top: absoluteTop - window.innerHeight / 2, behavior: findScrollBehavior() })
    }
  }, [rootRef])

  const closeFind = useCallback(() => {
    clearPendingFind()
    setShowFind(false)
    setFindQuery('')
    setFindMatchCount(0)
    setFindCurrentIndex(0)
    viewerFindApi?.clear()
    clearFindHighlights()
  }, [clearFindHighlights, clearPendingFind, viewerFindApi])

  const handleFind = useCallback((searchQuery: string) => {
    setFindQuery(searchQuery)
    if (searchQuery.trim()) return
    clearPendingFind()
    setFindMatchCount(0)
    setFindCurrentIndex(0)
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

// Rewrite each affected Text node once. A logical match may produce several
// marks when inline elements split it, but every piece keeps one match index.
function markFindMatches(doc: Document, matches: ReaderTextMatch[]): void {
  const partsByNode = new Map<Text, Array<{ index: number; startOffset: number; endOffset: number }>>()
  matches.forEach((match, index) => {
    match.parts.forEach((part) => {
      const parts = partsByNode.get(part.node) ?? []
      parts.push({ index, startOffset: part.startOffset, endOffset: part.endOffset })
      partsByNode.set(part.node, parts)
    })
  })

  partsByNode.forEach((parts, textNode) => {
    const parent = textNode.parentNode
    if (!parent) return
    const text = textNode.data
    const fragment = doc.createDocumentFragment()
    let offset = 0

    parts.sort((left, right) => left.startOffset - right.startOffset)
    parts.forEach((part) => {
      if (part.startOffset > offset) {
        fragment.appendChild(doc.createTextNode(text.slice(offset, part.startOffset)))
      }
      const mark = doc.createElement('mark')
      mark.dataset.find = String(part.index)
      mark.textContent = text.slice(part.startOffset, part.endOffset)
      fragment.appendChild(mark)
      offset = part.endOffset
    })
    if (offset < text.length) fragment.appendChild(doc.createTextNode(text.slice(offset)))
    parent.replaceChild(fragment, textNode)
  })
}

function findScrollBehavior(): ScrollBehavior {
  return isIOSWebKit() ? 'auto' : 'smooth'
}
