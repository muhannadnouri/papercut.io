import { useCallback, useEffect, useRef, useState } from 'react'
import type { SearchOpenTarget } from '../types/search'

type DocumentLoadState =
  | { status: 'idle' }
  | { status: 'loading'; url: string; message: string }
  | { status: 'error'; url: string; message: string }

type BrowseScrollSnapshot = {
  activeTab: string
  windowX: number
  windowY: number
  panelScrollTop: number | null
}

interface UseDocumentViewerStateOptions {
  activeTab: string
  documentsLoading: boolean
  loadHtmlDocument: (url: string) => Promise<string>
}

function getDocumentBrowserPanelBody(tab: string): HTMLElement | null {
  const panel = document.querySelector(`.tab-panel[data-tab="${tab}"] .document-browser-panel .panel-body`)
  return panel instanceof HTMLElement ? panel : null
}

/**
 * Owns the reader transition state that is shared by Library/Search/Audiobooks:
 * capture browse scroll, open one document at a time, ignore stale loads, and
 * restore the list position after the reader closes.
 */
export function useDocumentViewerState({
  activeTab,
  documentsLoading,
  loadHtmlDocument,
}: UseDocumentViewerStateOptions) {
  const [selectedDoc, setSelectedDoc] = useState<string | null>(null)
  const [searchOpenTarget, setSearchOpenTarget] = useState<SearchOpenTarget | null>(null)
  const [docContent, setDocContent] = useState('')
  const [documentLoad, setDocumentLoad] = useState<DocumentLoadState>({ status: 'idle' })
  const openDocumentRequestRef = useRef(0)
  const documentOpeningRef = useRef(false)
  const browseScrollRef = useRef<BrowseScrollSnapshot | null>(null)

  const clearSelectedDocument = useCallback((options?: { restoreBrowseScroll?: boolean }) => {
    if (options?.restoreBrowseScroll === false) browseScrollRef.current = null
    openDocumentRequestRef.current += 1
    documentOpeningRef.current = false
    setSelectedDoc(null)
    setSearchOpenTarget(null)
    setDocContent('')
    setDocumentLoad({ status: 'idle' })
  }, [])

  useEffect(() => {
    if (selectedDoc) return
    const snapshot = browseScrollRef.current
    if (!snapshot || snapshot.activeTab !== activeTab) return
    if (documentsLoading) return

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
    prepareOpen?: () => void,
  ) => {
    if (documentOpeningRef.current) return
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
    setSelectedDoc(url)
    setDocContent('')
    setDocumentLoad({ status: 'loading', url, message: 'Opening Document...' })
    window.scrollTo({ top: 0 })

    try {
      const html = await loadHtmlDocument(url)
      if (openDocumentRequestRef.current !== requestId) return
      documentOpeningRef.current = false
      setDocContent(html)
      setDocumentLoad({ status: 'idle' })
    } catch (err) {
      if (openDocumentRequestRef.current !== requestId) return
      const message = err instanceof Error ? err.message : String(err)
      documentOpeningRef.current = false
      setDocContent('')
      setDocumentLoad({ status: 'error', url, message })
      console.error('Failed to load document:', err)
    }
  }, [activeTab, loadHtmlDocument])

  return {
    clearSelectedDocument,
    docContent,
    documentLoad,
    documentOpening: documentLoad.status === 'loading',
    openDocument,
    searchOpenTarget,
    selectedDoc,
  }
}
