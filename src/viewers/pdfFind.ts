import type { ViewerFindApi, ViewerFindResult } from './types'

interface PdfFindEventBus {
  on: (name: string, listener: (event: PdfFindResultEvent) => void) => void
  off: (name: string, listener: (event: PdfFindResultEvent) => void) => void
  dispatch: (name: string, event: object) => void
}

interface PdfFindResultEvent {
  matchesCount?: {
    current: number
    total: number
  }
}

interface PdfFindAdapter {
  api: ViewerFindApi
  dispose: () => void
}

/** Translate Papercut's existing Find controls into PDF.js's public event
 * contract so PDF.js retains responsibility for extraction, highlighting,
 * page rendering, and match navigation. */
export function createPdfFindAdapter(
  eventBus: PdfFindEventBus,
  onResult: (result: ViewerFindResult) => void,
): PdfFindAdapter {
  let query = ''

  const dispatchFind = (type: '' | 'again', findPrevious = false) => {
    if (!query) return
    eventBus.dispatch('find', {
      source: api,
      type,
      query,
      caseSensitive: false,
      entireWord: false,
      highlightAll: true,
      findPrevious,
      matchDiacritics: false,
    })
  }

  const api: ViewerFindApi = {
    search(nextQuery) {
      query = nextQuery.trim()
      if (!query) {
        api.clear()
        return
      }
      dispatchFind('')
    },
    next() {
      dispatchFind('again')
    },
    previous() {
      dispatchFind('again', true)
    },
    clear() {
      query = ''
      eventBus.dispatch('findbarclose', { source: api })
      onResult({ currentIndex: 0, matchCount: 0 })
    },
  }

  const handleResult = ({ matchesCount }: PdfFindResultEvent) => {
    if (!matchesCount) return
    onResult({
      currentIndex: Math.max(matchesCount.current - 1, 0),
      matchCount: matchesCount.total,
    })
  }

  eventBus.on('updatefindmatchescount', handleResult)
  eventBus.on('updatefindcontrolstate', handleResult)

  return {
    api,
    dispose() {
      eventBus.off('updatefindmatchescount', handleResult)
      eventBus.off('updatefindcontrolstate', handleResult)
    },
  }
}
