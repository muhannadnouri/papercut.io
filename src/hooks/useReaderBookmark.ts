import { useCallback, useEffect, useRef, useState } from 'react'
import type { ViewerBookmarkApi, ViewerBookmarkLocation } from '../viewers/types'

const STORAGE_PREFIX = 'papercut:reader-bookmark:'
const MIN_BOOKMARK_SCROLL_Y = 180
const ACTIVE_BOOKMARK_DISTANCE_PX = 180
const NOTICE_MS = 6000

type BookmarkNotice = 'restored' | 'saved' | 'updated' | 'removed' | null

export interface ReaderBookmark {
  scrollRatio: number
  scrollY: number
  updatedAtMs: number
  viewerLocation?: ViewerBookmarkLocation
}

interface UseReaderBookmarkOptions {
  enabled: boolean
  restoreOnOpen?: boolean
  viewerApi?: ViewerBookmarkApi | null
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
  { enabled, restoreOnOpen = true, viewerApi }: UseReaderBookmarkOptions,
): UseReaderBookmarkReturn {
  const [bookmarkNotice, setBookmarkNotice] = useState<BookmarkNotice>(null)
  const [hasBookmark, setHasBookmark] = useState(false)
  const [isAtBookmark, setIsAtBookmark] = useState(false)
  const bookmarkRef = useRef<ReaderBookmark | null>(null)
  const restoredKeyRef = useRef('')

  const loadBookmark = useCallback((): ReaderBookmark | null => {
    try {
      const raw = window.localStorage.getItem(storageKey(url))
      return raw ? parseReaderBookmark(raw) : null
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

      const maxScrollY = viewerApi ? 0 : getMaxScrollY()
      if (!viewerApi && (maxScrollY <= MIN_BOOKMARK_SCROLL_Y || window.scrollY < MIN_BOOKMARK_SCROLL_Y)) return

      const wasBookmarked = Boolean(loadBookmark())
      const next: ReaderBookmark = {
        scrollRatio: viewerApi ? 0 : clampRatio(window.scrollY / maxScrollY),
        scrollY: viewerApi ? 0 : window.scrollY,
        updatedAtMs: Date.now(),
        viewerLocation: viewerApi?.capture(),
      }

      window.localStorage.setItem(storageKey(url), JSON.stringify(next))
      bookmarkRef.current = next
      setHasBookmark(true)
      setIsAtBookmark(true)
      setBookmarkNotice(wasBookmarked ? 'updated' : 'saved')
    } catch {
      // Private browsing / storage quota failures should not break reading.
    }
  }, [enabled, isAtBookmark, loadBookmark, url, viewerApi])

  useEffect(() => {
    if (!enabled) return
    const bookmark = loadBookmark()
    bookmarkRef.current = bookmark

    const frame = requestAnimationFrame(() => {
      setHasBookmark(Boolean(bookmark))
      if (
        restoreOnOpen &&
        restoredKeyRef.current !== storageKey(url) &&
        bookmark &&
        canRestoreBookmark(bookmark, viewerApi)
      ) {
        restoredKeyRef.current = storageKey(url)
        scrollToBookmark(bookmark, 'auto', viewerApi)
        setIsAtBookmark(true)
        setBookmarkNotice('restored')
        return
      }
      setIsAtBookmark(bookmark ? isNearBookmark(bookmark, viewerApi) : false)
      setBookmarkNotice(null)
    })

    return () => cancelAnimationFrame(frame)
  }, [enabled, loadBookmark, restoreOnOpen, url, viewerApi])

  useEffect(() => {
    if (!enabled) return
    let frame = 0

    function updateActiveState() {
      if (frame) cancelAnimationFrame(frame)
      frame = requestAnimationFrame(() => {
        frame = 0
        const bookmark = bookmarkRef.current
        setIsAtBookmark(bookmark ? isNearBookmark(bookmark, viewerApi) : false)
      })
    }

    if (viewerApi) {
      const unsubscribe = viewerApi.subscribe(updateActiveState)
      return () => {
        unsubscribe()
        if (frame) cancelAnimationFrame(frame)
      }
    }

    window.addEventListener('scroll', updateActiveState, { passive: true })
    window.addEventListener('resize', updateActiveState)
    return () => {
      window.removeEventListener('scroll', updateActiveState)
      window.removeEventListener('resize', updateActiveState)
      if (frame) cancelAnimationFrame(frame)
    }
  }, [enabled, viewerApi])

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

function scrollToBookmark(
  bookmark: ReaderBookmark,
  behavior: ScrollBehavior,
  viewerApi?: ViewerBookmarkApi | null,
): void {
  if (viewerApi && bookmark.viewerLocation) {
    viewerApi.restore(bookmark.viewerLocation)
    return
  }
  window.scrollTo({ top: targetScrollY(bookmark), behavior })
}

function canRestoreBookmark(
  bookmark: ReaderBookmark,
  viewerApi?: ViewerBookmarkApi | null,
): boolean {
  if (viewerApi) return Boolean(bookmark.viewerLocation)
  return targetScrollY(bookmark) >= MIN_BOOKMARK_SCROLL_Y
}

// A bookmark represents "this reading area", not a single exact pixel. The
// tolerance prevents the active fill from flickering after font, viewport, or
// line-height changes move the saved offset by a few lines.
function isNearBookmark(
  bookmark: ReaderBookmark,
  viewerApi?: ViewerBookmarkApi | null,
): boolean {
  if (viewerApi) {
    return bookmark.viewerLocation ? viewerApi.isCurrent(bookmark.viewerLocation) : false
  }
  return Math.abs(window.scrollY - targetScrollY(bookmark)) <= Math.max(ACTIVE_BOOKMARK_DISTANCE_PX, window.innerHeight * 0.2)
}

function clampRatio(value: number): number {
  if (!Number.isFinite(value)) return 0
  return Math.min(1, Math.max(0, value))
}

/** Parse window bookmarks and PDF.js-owned page coordinates from storage. */
export function parseReaderBookmark(raw: string): ReaderBookmark | null {
  const parsed = JSON.parse(raw) as Partial<ReaderBookmark>
  if (
    typeof parsed.scrollRatio !== 'number' ||
    typeof parsed.scrollY !== 'number' ||
    typeof parsed.updatedAtMs !== 'number'
  ) {
    return null
  }
  const location = parsed.viewerLocation
  const viewerLocation = location &&
    Number.isInteger(location.pageNumber) &&
    location.pageNumber > 0 &&
    typeof location.left === 'number' &&
    Number.isFinite(location.left) &&
    typeof location.top === 'number' &&
    Number.isFinite(location.top)
    ? {
        pageNumber: location.pageNumber,
        left: location.left,
        top: location.top,
      }
    : undefined
  if (location && !viewerLocation) return null
  return {
    scrollRatio: clampRatio(parsed.scrollRatio),
    scrollY: Math.max(0, parsed.scrollY),
    updatedAtMs: parsed.updatedAtMs,
    ...(viewerLocation ? { viewerLocation } : {}),
  }
}
