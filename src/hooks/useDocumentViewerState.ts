import {
  useCallback,
  useEffect,
  useRef,
  useState,
} from 'react'
import type { ReaderDocument } from '../viewers/types'
import type { SearchOpenTarget } from '../types/search'
import { readReaderBookmark } from './useReaderBookmark'

type DocumentLoadState =
  | { status: 'idle' }
  | { status: 'loading';
    url: string;
    message: string }
  | { status: 'restoring';
    url: string;
    message: string }
  | { status: 'error';
    url: string;
    message: string }

type BrowseScrollSnapshot = {
  activeTab: string
  windowX: number
  windowY: number
  panelScrollTop: number | null
}

interface UseDocumentViewerStateOptions {
  activeTab: string
  documentsLoading: boolean
  loadReaderDocument: (url: string) => Promise<ReaderDocument>
}

interface OpenDocumentOptions {
  restoreBookmark?: boolean
}

function getDocumentBrowserPanelBody(tab: string): HTMLElement | null {
  const panel = document.querySelector(`.tab-panel[data-tab="${tab}"] .document-browser-panel .panel-body`)
  return panel instanceof HTMLElement ? panel : null
}

/**
 * Owns the reader transition state that is shared by Library/Search/Audiobooks:
 * capture browse scroll, open one document at a time, ignore stale loads, and
 * restore the list position after the reader closes. HTML and outline share a
 * request ID so a late response cannot pair one book with another book’s destinations.
 */
export function useDocumentViewerState({
  activeTab,
  documentsLoading,
  loadReaderDocument,
}: UseDocumentViewerStateOptions) {
  const [selectedDoc, setSelectedDoc] = useState<string | null>(null)
  const [searchOpenTarget, setSearchOpenTarget] = useState<SearchOpenTarget | null>(null)
  const [restoreBookmark, setRestoreBookmark] = useState(false)
  const [readerDocument, setReaderDocument] = useState<ReaderDocument | null>(null)
  const [documentLoad, setDocumentLoad] = useState<DocumentLoadState>({ status: 'idle' })
  const openDocumentRequestRef = useRef(0)
  const documentOpeningRef = useRef(false)
  const browseScrollRef = useRef<BrowseScrollSnapshot | null>(null)

  useEffect(() => {
    if (documentLoad.status !== 'restoring') {
      return
    }
    const timer = window.setTimeout(() => {
      setDocumentLoad(current => current === documentLoad ? { status: 'idle' } : current)
    }, 20000)
    return () => window.clearTimeout(timer)
  }, [documentLoad])

  const clearSelectedDocument = useCallback((options?: { restoreBrowseScroll?: boolean }) => {
    if (options?.restoreBrowseScroll === false) {
      browseScrollRef.current = null
    }
    openDocumentRequestRef.current += 1
    documentOpeningRef.current = false
    setSelectedDoc(null)
    setSearchOpenTarget(null)
    setRestoreBookmark(false)
    setReaderDocument(null)
    setDocumentLoad({ status: 'idle' })
  }, [])

  useEffect(() => {
    if (selectedDoc) {
      return
    }
    const snapshot = browseScrollRef.current
    if (!snapshot || snapshot.activeTab !== activeTab) {
      return
    }
    if (documentsLoading) {
      return
    }

    browseScrollRef.current = null
    let innerFrame = 0
    const frame = requestAnimationFrame(() => {
      innerFrame = requestAnimationFrame(() => {
        window.scrollTo(snapshot.windowX, snapshot.windowY)
        const panelBody = getDocumentBrowserPanelBody(snapshot.activeTab)
        if (panelBody && snapshot.panelScrollTop !== null) {
          panelBody.scrollTop = snapshot.panelScrollTop
        }
      })
    })

    return () => {
      cancelAnimationFrame(frame)
      cancelAnimationFrame(innerFrame)
    }
  }, [activeTab, documentsLoading, selectedDoc])

  const openDocument = useCallback(async (
    url: string,
    target?: SearchOpenTarget,
    options?: OpenDocumentOptions,
    prepareOpen?: () => void,
  ) => {
    if (documentOpeningRef.current) {
      return
    }
    documentOpeningRef.current = true
    const requestId = openDocumentRequestRef.current + 1
    openDocumentRequestRef.current = requestId
    const panelBody = getDocumentBrowserPanelBody(activeTab)
    browseScrollRef.current = {
      activeTab,
      windowX: window.scrollX,
      windowY: window.scrollY,
      panelScrollTop: panelBody?.scrollTop ?? null,
    }
    prepareOpen?.()
    setSearchOpenTarget(target ?? null)
    setRestoreBookmark(Boolean(options?.restoreBookmark))
    setSelectedDoc(url)
    setReaderDocument(null)
    setDocumentLoad({
      status: 'loading',
      url,
      message: 'Opening Document...',
    })
    window.scrollTo({ top: 0 })

    try {
      const source = await loadReaderDocument(url)
      if (openDocumentRequestRef.current !== requestId) {
        return
      }
      documentOpeningRef.current = false
      setReaderDocument(source)
      if (options?.restoreBookmark && readReaderBookmark(url)) {
        // Paint the destination, not the top: the viewer mounts hidden while
        // the bookmark anchor index builds and the seek completes, then the
        // content reveals already scrolled. The failsafe unwedges a viewer
        // that never registers its bookmark API.
        setDocumentLoad({
          status: 'restoring',
          url,
          message: 'Restoring bookmark...',
        })
      } else {
        setDocumentLoad({ status: 'idle' })
      }
    } catch (err) {
      if (openDocumentRequestRef.current !== requestId) {
        return
      }
      const message = err instanceof Error ? err.message : String(err)
      documentOpeningRef.current = false
      setReaderDocument(null)
      setDocumentLoad({
        status: 'error',
        url,
        message,
      })
      console.error('Failed to load document:', err)
    }
  }, [activeTab, loadReaderDocument])

  const completeRestoringDocument = useCallback(() => {
    setDocumentLoad((prev) => prev === documentLoad && prev.status === 'restoring' ? { status: 'idle' } : prev)
  }, [documentLoad])

  return {
    clearSelectedDocument,
    completeRestoringDocument,
    docContent: readerDocument?.html ?? '',
    docOutline: readerDocument?.outline ?? [],
    documentLoad,
    documentOpening: documentLoad.status === 'loading',
    openDocument,
    restoreBookmark,
    searchOpenTarget,
    selectedDoc,
  }
}
