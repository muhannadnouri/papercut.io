import { useCallback, useEffect, useRef, useState } from 'react'

const STORAGE_PREFIX = 'papercut:reader-bookmark:'
const MIN_BOOKMARK_SCROLL_Y = 180
const ACTIVE_BOOKMARK_DISTANCE_PX = 180
const NOTICE_MS = 6000

type BookmarkNotice = 'restored' | 'saved' | 'updated' | 'removed' | null

interface ReaderBookmark {
  scrollRatio: number
  scrollY: number
  updatedAtMs: number
}

interface UseReaderBookmarkOptions {
  enabled: boolean
  restoreOnOpen?: boolean
}

interface UseReaderBookmarkReturn {
  bookmarkNotice: BookmarkNotice
  hasBookmark: boolean
  isAtBookmark: boolean
  dismissBookmarkNotice: () => void
  toggleBookmark: () => void
}

/**
 * Stores one explicit bookmark per document in localStorage.
 *
 * This is intentionally not "last read position" tracking: nothing is saved
 * while the user scrolls. The bookmark changes only when the reader button is
 * pressed, and callers can disable restore for explicit destinations such as
 * search-result jumps and audiobook opens.
 */
export function useReaderBookmark(
  url: string,
  { enabled, restoreOnOpen = true }: UseReaderBookmarkOptions,
): UseReaderBookmarkReturn {
  const [bookmarkNotice, setBookmarkNotice] = useState<BookmarkNotice>(null)
  const [hasBookmark, setHasBookmark] = useState(false)
  const [isAtBookmark, setIsAtBookmark] = useState(false)
  const bookmarkRef = useRef<ReaderBookmark | null>(null)
  const restoredKeyRef = useRef('')

  const loadBookmark = useCallback((): ReaderBookmark | null => {
    try {
      const raw = window.localStorage.getItem(storageKey(url))
      if (!raw) return null
      const parsed = JSON.parse(raw) as Partial<ReaderBookmark>
      if (
        typeof parsed.scrollRatio !== 'number' ||
        typeof parsed.scrollY !== 'number' ||
        typeof parsed.updatedAtMs !== 'number'
      ) {
        return null
      }
      return {
        scrollRatio: clampRatio(parsed.scrollRatio),
        scrollY: Math.max(0, parsed.scrollY),
        updatedAtMs: parsed.updatedAtMs,
      }
    } catch {
      return null
    }
  }, [url])

  const dismissBookmarkNotice = useCallback(() => {
    setBookmarkNotice(null)
  }, [])

  // The floating bookmark button is a toggle at the saved spot, not a separate
  // delete flow. This keeps the reader UI to one bookmark action: save/update
  // when away from the bookmark, remove when the active-state fill is showing.
  const toggleBookmark = useCallback(() => {
    if (!enabled) return
    try {
      if (bookmarkRef.current && isAtBookmark) {
        window.localStorage.removeItem(storageKey(url))
        bookmarkRef.current = null
        setHasBookmark(false)
        setIsAtBookmark(false)
        setBookmarkNotice('removed')
        return
      }

      const maxScrollY = getMaxScrollY()
      if (maxScrollY <= MIN_BOOKMARK_SCROLL_Y || window.scrollY < MIN_BOOKMARK_SCROLL_Y) return

      const wasBookmarked = Boolean(loadBookmark())
      const next: ReaderBookmark = {
        scrollRatio: clampRatio(window.scrollY / maxScrollY),
        scrollY: window.scrollY,
        updatedAtMs: Date.now(),
      }

      window.localStorage.setItem(storageKey(url), JSON.stringify(next))
      bookmarkRef.current = next
      setHasBookmark(true)
      setIsAtBookmark(true)
      setBookmarkNotice(wasBookmarked ? 'updated' : 'saved')
    } catch {
      // Private browsing / storage quota failures should not break reading.
    }
  }, [enabled, isAtBookmark, loadBookmark, url])

  useEffect(() => {
    if (!enabled) return
    const bookmark = loadBookmark()
    bookmarkRef.current = bookmark

    const frame = requestAnimationFrame(() => {
      setHasBookmark(Boolean(bookmark))
      if (restoreOnOpen && restoredKeyRef.current !== storageKey(url) && bookmark && canRestoreBookmark(bookmark)) {
        restoredKeyRef.current = storageKey(url)
        scrollToBookmark(bookmark, 'auto')
        setIsAtBookmark(true)
        setBookmarkNotice('restored')
        return
      }
      setIsAtBookmark(bookmark ? isNearBookmark(bookmark) : false)
      setBookmarkNotice(null)
    })

    return () => cancelAnimationFrame(frame)
  }, [enabled, loadBookmark, restoreOnOpen, url])

  useEffect(() => {
    if (!enabled) return
    let frame = 0

    function updateActiveState() {
      if (frame) cancelAnimationFrame(frame)
      frame = requestAnimationFrame(() => {
        frame = 0
        const bookmark = bookmarkRef.current
        setIsAtBookmark(bookmark ? isNearBookmark(bookmark) : false)
      })
    }

    window.addEventListener('scroll', updateActiveState, { passive: true })
    window.addEventListener('resize', updateActiveState)
    return () => {
      window.removeEventListener('scroll', updateActiveState)
      window.removeEventListener('resize', updateActiveState)
      if (frame) cancelAnimationFrame(frame)
    }
  }, [enabled])

  useEffect(() => {
    if (!bookmarkNotice) return
    const timer = setTimeout(() => setBookmarkNotice(null), NOTICE_MS)
    return () => clearTimeout(timer)
  }, [bookmarkNotice])

  return { bookmarkNotice, dismissBookmarkNotice, hasBookmark, isAtBookmark, toggleBookmark }
}

function storageKey(url: string): string {
  return `${STORAGE_PREFIX}${url}`
}

function getMaxScrollY(): number {
  const doc = document.documentElement
  return Math.max(0, doc.scrollHeight - window.innerHeight)
}

// Store both an absolute scroll offset and a ratio. The absolute offset keeps
// normal reopens precise, while the ratio gives us a reasonable fallback if the
// same EPUB/HTML renders taller or shorter after settings or viewport changes.
function targetScrollY(bookmark: ReaderBookmark): number {
  const maxScrollY = getMaxScrollY()
  const ratioTarget = bookmark.scrollRatio * maxScrollY
  return Math.min(maxScrollY, Math.max(0, Math.max(bookmark.scrollY, ratioTarget)))
}

function scrollToBookmark(bookmark: ReaderBookmark, behavior: ScrollBehavior): void {
  window.scrollTo({ top: targetScrollY(bookmark), behavior })
}

function canRestoreBookmark(bookmark: ReaderBookmark): boolean {
  return targetScrollY(bookmark) >= MIN_BOOKMARK_SCROLL_Y
}

// A bookmark represents "this reading area", not a single exact pixel. The
// tolerance prevents the active fill from flickering after font, viewport, or
// line-height changes move the saved offset by a few lines.
function isNearBookmark(bookmark: ReaderBookmark): boolean {
  return Math.abs(window.scrollY - targetScrollY(bookmark)) <= Math.max(ACTIVE_BOOKMARK_DISTANCE_PX, window.innerHeight * 0.2)
}

function clampRatio(value: number): number {
  if (!Number.isFinite(value)) return 0
  return Math.min(1, Math.max(0, value))
}
