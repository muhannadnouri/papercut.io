import type { PdfFindResult } from '../../uploads/DocumentUploads'
import type { ViewerFindApi, ViewerFindResult } from '../../viewers/types'

export interface PdfOcrFindMatch {
  pageIndex: number
  occurrenceIndex: number
}

/** Adapt compact per-page OCR match counts to the shared Find controls. */
export function createPdfOcrFindAdapter(
  find: (query: string) => Promise<PdfFindResult>,
  navigate: (match: PdfOcrFindMatch, query: string) => void,
  clearHighlight: () => void,
  onResult: (result: ViewerFindResult) => void,
) {
  let requestId = 0
  let query = ''
  let result: PdfFindResult = { matchCount: 0, pages: [] }
  let currentIndex = 0

  const showCurrent = () => {
    if (result.matchCount === 0) return
    let offset = currentIndex
    for (const page of result.pages) {
      if (offset < page.matchCount) {
        navigate({ pageIndex: page.pageIndex, occurrenceIndex: offset }, query)
        onResult({ currentIndex, matchCount: result.matchCount })
        return
      }
      offset -= page.matchCount
    }
  }

  const clear = () => {
    requestId += 1
    query = ''
    result = { matchCount: 0, pages: [] }
    currentIndex = 0
    clearHighlight()
    onResult({ currentIndex: 0, matchCount: 0 })
  }

  const api: ViewerFindApi = {
    search(nextQuery) {
      const trimmed = nextQuery.trim()
      if (!trimmed) {
        clear()
        return
      }
      query = trimmed
      const currentRequest = ++requestId
      void find(trimmed).then((nextResult) => {
        if (currentRequest !== requestId) return
        result = nextResult
        currentIndex = 0
        onResult({ currentIndex: 0, matchCount: result.matchCount })
        showCurrent()
      }).catch(() => {
        if (currentRequest !== requestId) return
        result = { matchCount: 0, pages: [] }
        currentIndex = 0
        onResult({ currentIndex: 0, matchCount: 0 })
      })
    },
    next() {
      if (result.matchCount === 0) return
      currentIndex = (currentIndex + 1) % result.matchCount
      showCurrent()
    },
    previous() {
      if (result.matchCount === 0) return
      currentIndex = (currentIndex - 1 + result.matchCount) % result.matchCount
      showCurrent()
    },
    clear,
  }

  return {
    api,
    dispose() {
      requestId += 1
    },
  }
}
