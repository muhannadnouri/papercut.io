import { useCallback, useEffect, useRef, useState } from 'react'
import type { ViewerBookmarkApi, ViewerBookmarkLocation } from '../viewers/types'

const STORAGE_PREFIX = 'papercut:reader-bookmark:'
const BOOKMARKS_CHANGED_EVENT = 'papercut:reader-bookmarks-changed'
const NOTICE_MS = 6000

type BookmarkNotice = 'restored' | 'saved' | 'updated' | 'removed' | 'changeUndone' | null

export interface ReaderBookmark {
  updatedAtMs: number
  viewerLocation: ViewerBookmarkLocation
}

interface UseReaderBookmarkOptions {
  enabled: boolean
  restoreOnOpen?: boolean
  viewerApi?: ViewerBookmarkApi | null
}

interface UseReaderBookmarkReturn {
  bookmarkNotice: BookmarkNotice
  canUndoBookmarkChange: boolean
  hasBookmark: boolean
  isAtBookmark: boolean
  dismissBookmarkNotice: () => void
  moveBookmark: () => void
  removeBookmark: () => void
  restoreBookmark: () => void
  saveBookmark: () => void
  undoBookmarkChange: () => void
}

type BookmarkStorage = Pick<Storage, 'getItem' | 'key' | 'length'>

/** Read the valid bookmark URLs once for Library status indicators. */
export function readBookmarkedDocumentUrls(storage?: BookmarkStorage): Set<string> {
  const urls = new Set<string>()
  try {
    const source = storage ?? window.localStorage
    for (let index = 0; index < source.length; index += 1) {
      const key = source.key(index)
      if (!key?.startsWith(STORAGE_PREFIX)) continue
      const raw = source.getItem(key)
      if (raw && parseReaderBookmark(raw)) urls.add(key.slice(STORAGE_PREFIX.length))
    }
  } catch {
    // Storage restrictions should leave the Library usable without indicators.
  }
  return urls
}

/** Keep the Library's bookmark snapshot current without reading storage per row. */
export function useBookmarkedDocumentUrls(): ReadonlySet<string> {
  const [urls, setUrls] = useState(readBookmarkedDocumentUrls)

  useEffect(() => {
    const refresh = () => setUrls(readBookmarkedDocumentUrls())
    window.addEventListener(BOOKMARKS_CHANGED_EVENT, refresh)
    return () => window.removeEventListener(BOOKMARKS_CHANGED_EVENT, refresh)
  }, [])

  return urls
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
  const undoBookmarkRef = useRef<ReaderBookmark | null>(null)
  const restoredKeyRef = useRef('')

  const loadBookmark = useCallback((): ReaderBookmark | null => {
    try {
      const raw = window.localStorage.getItem(storageKey(url))
      if (!raw) return null
      const bookmark = parseReaderBookmark(raw)
      if (!bookmark) window.localStorage.removeItem(storageKey(url))
      return bookmark
    } catch {
      return null
    }
  }, [url])

  const dismissBookmarkNotice = useCallback(() => {
    undoBookmarkRef.current = null
    setBookmarkNotice(null)
  }, [])

  const storeCurrentBookmark = useCallback((previous: ReaderBookmark | null) => {
    if (!enabled || !viewerApi) return
    try {
      const viewerLocation = viewerApi.capture()
      if (!viewerLocation) return
      const next: ReaderBookmark = {
        updatedAtMs: Date.now(),
        viewerLocation,
      }

      window.localStorage.setItem(storageKey(url), JSON.stringify(next))
      notifyBookmarksChanged()
      undoBookmarkRef.current = previous
      bookmarkRef.current = next
      setHasBookmark(true)
      setIsAtBookmark(true)
      setBookmarkNotice(previous ? 'updated' : 'saved')
    } catch {
      // Private browsing / storage quota failures should not break reading.
    }
  }, [enabled, url, viewerApi])

  const saveBookmark = useCallback(() => {
    storeCurrentBookmark(null)
  }, [storeCurrentBookmark])

  const moveBookmark = useCallback(() => {
    if (!bookmarkRef.current) return
    storeCurrentBookmark(bookmarkRef.current)
  }, [storeCurrentBookmark])

  const restoreBookmark = useCallback(() => {
    const bookmark = bookmarkRef.current
    if (!enabled || !viewerApi || !bookmark) return
    undoBookmarkRef.current = null
    viewerApi.restore(bookmark.viewerLocation)
    setIsAtBookmark(true)
    setBookmarkNotice('restored')
  }, [enabled, viewerApi])

  const removeBookmark = useCallback(() => {
    const bookmark = bookmarkRef.current
    if (!bookmark) return
    try {
      window.localStorage.removeItem(storageKey(url))
      notifyBookmarksChanged()
      undoBookmarkRef.current = bookmark
      bookmarkRef.current = null
      setHasBookmark(false)
      setIsAtBookmark(false)
      setBookmarkNotice('removed')
    } catch {
      // Private browsing / storage failures should not break reading.
    }
  }, [url])

  const undoBookmarkChange = useCallback(() => {
    const bookmark = undoBookmarkRef.current
    if (!bookmark) return
    try {
      window.localStorage.setItem(storageKey(url), JSON.stringify(bookmark))
      notifyBookmarksChanged()
      undoBookmarkRef.current = null
      bookmarkRef.current = bookmark
      setHasBookmark(true)
      setIsAtBookmark(viewerApi ? viewerApi.isCurrent(bookmark.viewerLocation) : false)
      setBookmarkNotice('changeUndone')
    } catch {
      // Private browsing / storage failures should not break reading.
    }
  }, [url, viewerApi])

  useEffect(() => {
    if (!enabled) return
    const bookmark = loadBookmark()
    undoBookmarkRef.current = null
    bookmarkRef.current = bookmark

    const frame = requestAnimationFrame(() => {
      setHasBookmark(Boolean(bookmark))
      if (
        restoreOnOpen &&
        restoredKeyRef.current !== storageKey(url) &&
        bookmark &&
        viewerApi
      ) {
        restoredKeyRef.current = storageKey(url)
        viewerApi.restore(bookmark.viewerLocation)
        setIsAtBookmark(true)
        setBookmarkNotice('restored')
        return
      }
      setIsAtBookmark(bookmark && viewerApi
        ? viewerApi.isCurrent(bookmark.viewerLocation)
        : false)
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
        setIsAtBookmark(bookmark && viewerApi
          ? viewerApi.isCurrent(bookmark.viewerLocation)
          : false)
      })
    }

    if (!viewerApi) return
    const unsubscribe = viewerApi.subscribe(updateActiveState)
    return () => {
      unsubscribe()
      if (frame) cancelAnimationFrame(frame)
    }
  }, [enabled, viewerApi])

  useEffect(() => {
    if (!bookmarkNotice) return
    const timer = setTimeout(dismissBookmarkNotice, NOTICE_MS)
    return () => clearTimeout(timer)
  }, [bookmarkNotice, dismissBookmarkNotice])

  return {
    bookmarkNotice,
    canUndoBookmarkChange: bookmarkNotice === 'updated' || bookmarkNotice === 'removed',
    dismissBookmarkNotice,
    hasBookmark,
    isAtBookmark,
    moveBookmark,
    removeBookmark,
    restoreBookmark,
    saveBookmark,
    undoBookmarkChange,
  }
}

function storageKey(url: string): string {
  return `${STORAGE_PREFIX}${url}`
}

function notifyBookmarksChanged(): void {
  window.dispatchEvent(new Event(BOOKMARKS_CHANGED_EVENT))
}

/** Accept only current semantic HTML/EPUB or PDF bookmark locations. */
export function parseReaderBookmark(raw: string): ReaderBookmark | null {
  let parsed: unknown
  try {
    parsed = JSON.parse(raw)
  } catch {
    return null
  }
  if (!parsed || typeof parsed !== 'object') return null
  const bookmark = parsed as Partial<ReaderBookmark>
  if (typeof bookmark.updatedAtMs !== 'number' || !Number.isFinite(bookmark.updatedAtMs)) return null
  const location = bookmark.viewerLocation
  if (!location || typeof location !== 'object') return null
  const pdfLocation =
    'pageNumber' in location &&
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
  const htmlLocation =
    'textOffset' in location &&
    Number.isInteger(location.textOffset) &&
    location.textOffset >= 0
    ? { textOffset: location.textOffset }
    : undefined
  const viewerLocation = pdfLocation ?? htmlLocation
  if (!viewerLocation) return null
  return {
    updatedAtMs: bookmark.updatedAtMs,
    viewerLocation,
  }
}
