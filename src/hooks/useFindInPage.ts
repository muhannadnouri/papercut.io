import { useState, useEffect, useCallback, useRef } from 'react'
import { isIOSWebKit } from '../utils/platform'

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
}

export function useFindInPage(
  rootRef: React.RefObject<HTMLElement | null>,
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
    clearFindHighlights()
    const root = rootRef.current
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

    const lowerQuery = searchQuery.toLowerCase()
    const textNodes: Node[] = []
    const treeWalker = doc.createTreeWalker(root, NodeFilter.SHOW_TEXT)
    let node: Node | null
    while ((node = treeWalker.nextNode())) {
      if (node.parentElement?.closest('script, style, noscript, svg')) continue
      textNodes.push(node)
    }

    let count = 0
    for (const textNode of textNodes) {
      const text = textNode.textContent ?? ''
      const lowerText = text.toLowerCase()
      if (!lowerText.includes(lowerQuery)) continue

      const fragment = doc.createDocumentFragment()
      let lastIdx = 0
      let searchIdx = lowerText.indexOf(lowerQuery, lastIdx)
      while (searchIdx !== -1) {
        if (searchIdx > lastIdx) {
          fragment.appendChild(doc.createTextNode(text.slice(lastIdx, searchIdx)))
        }
        const mark = doc.createElement('mark')
        mark.setAttribute('data-find', String(count))
        mark.textContent = text.slice(searchIdx, searchIdx + searchQuery.length)
        fragment.appendChild(mark)
        count++
        lastIdx = searchIdx + searchQuery.length
        searchIdx = lowerText.indexOf(lowerQuery, lastIdx)
      }
      if (lastIdx < text.length) {
        fragment.appendChild(doc.createTextNode(text.slice(lastIdx)))
      }
      textNode.parentNode?.replaceChild(fragment, textNode)
    }
    return count
  }, [rootRef, clearFindHighlights])

  const scrollToMatch = useCallback((index: number) => {
    const root = rootRef.current
    if (!root) return
    const prev = root.querySelector('mark[data-find].current')
    prev?.classList.remove('current')
    const target = root.querySelector(`mark[data-find="${index}"]`)
    if (target) {
      target.classList.add('current')
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
    clearFindHighlights()
  }, [clearFindHighlights, clearPendingFind])

  const handleFind = useCallback((searchQuery: string) => {
    setFindQuery(searchQuery)
    if (searchQuery.trim()) return
    clearPendingFind()
    setFindMatchCount(0)
    setFindCurrentIndex(0)
    clearFindHighlights()
  }, [clearFindHighlights, clearPendingFind])

  useEffect(() => {
    clearPendingFind()
    const searchQuery = findQuery.trim()
    if (!searchQuery) {
      clearFindHighlights()
      return
    }

    findTimerRef.current = setTimeout(() => {
      const count = highlightFindMatches(searchQuery)
      setFindMatchCount(count)
      setFindCurrentIndex(0)
      if (count > 0) scrollToMatch(0)
      findTimerRef.current = null
    }, FIND_DEBOUNCE_MS)

    return clearPendingFind
  }, [clearFindHighlights, clearPendingFind, highlightFindMatches, scrollToMatch, findQuery])

  const findNext = useCallback(() => {
    if (findMatchCount === 0) return
    const next = (findCurrentIndex + 1) % findMatchCount
    setFindCurrentIndex(next)
    scrollToMatch(next)
  }, [findMatchCount, findCurrentIndex, scrollToMatch])

  const findPrev = useCallback(() => {
    if (findMatchCount === 0) return
    const prev = (findCurrentIndex - 1 + findMatchCount) % findMatchCount
    setFindCurrentIndex(prev)
    scrollToMatch(prev)
  }, [findMatchCount, findCurrentIndex, scrollToMatch])

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
  }
}

function findScrollBehavior(): ScrollBehavior {
  return isIOSWebKit() ? 'auto' : 'smooth'
}
