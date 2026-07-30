import type { ViewerFindApi, ViewerFindResult } from './types'
import type { SearchOpenTarget } from '../types/search'

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

type PdfJsQuery = string | string[]

/** Map the indexed zero-based PDF page to PDF.js's one-based viewer page. */
export function pdfSearchTargetPage(
  target: SearchOpenTarget | null | undefined,
  pageCount?: number,
): number | null {
  if (!target?.text?.trim() || target.pageIndex === undefined) return null
  const page = Math.max(target.pageIndex + 1, 1)
  return pageCount === undefined ? page : Math.min(page, Math.max(pageCount, 1))
}

/** Translate Papercut's existing Find controls into PDF.js's public event
 * contract so PDF.js retains responsibility for extraction, highlighting,
 * page rendering, and match navigation. */
export function createPdfFindAdapter(
  eventBus: PdfFindEventBus,
  onResult: (result: ViewerFindResult) => void,
): PdfFindAdapter {
  let query: PdfJsQuery = ''

  const dispatchFind = (type: '' | 'again', findPrevious = false) => {
    if (query.length === 0) return
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
      query = pdfFindQuery(nextQuery)
      if (query.length === 0) {
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

/** Let PDF.js match a typed compound, its spaced extraction form, or the
 * canonical word without replacing PDF.js's own text/offset normalization. */
function pdfFindQuery(input: string): PdfJsQuery {
  const original = input.trim()
  if (!original) return ''

  const compact = original.replace(/(\p{L})-\s+(?=\p{L})/gu, '$1-')
  const spaced = compact.replace(/(\p{L})-(?=\p{L})/gu, '$1- ')
  const joined = compact.replace(/(\p{L})-(?=\p{L})/gu, '$1')
  const aliases = [...new Set([original, compact, spaced, joined])]
  return aliases.length === 1 ? original : aliases
}
