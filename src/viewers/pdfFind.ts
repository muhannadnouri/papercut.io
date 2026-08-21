import type { ViewerFindApi, ViewerFindResult } from './types'
import type { SearchOpenTarget } from '../types/search'
import {
  normalizeSearchPunctuation,
  SEARCH_DASH_CHARACTERS,
} from '../utils/textUtils'

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

/** Let PDF.js independently match compact, line-wrapped, or joined spellings
 * for a few compounds without allowing pasted input to expand without bound. */
function pdfFindQuery(input: string): PdfJsQuery {
  const original = input.trim()
  if (!original) return ''

  const normalized = normalizeSearchPunctuation(original)
  const compact = normalized.replace(/(\p{L})-\s+(?=\p{L})/gu, '$1-')
  const characters = Array.from(compact)
  const positions = characters
    .map((character, index) => (
      character === '-'
      && index > 0
      && index + 1 < characters.length
      && /\p{L}/u.test(characters[index - 1])
      && /\p{L}/u.test(characters[index + 1])
        ? index
        : -1
    ))
    .filter((index) => index >= 0)
    .slice(0, 3)
  const aliases = new Set([original, normalized])
  const combinations = 3 ** positions.length
  for (let combination = 0; combination < combinations; combination += 1) {
    let state = combination
    const choices = new Map<number, number>()
    positions.forEach((position) => {
      choices.set(position, state % 3)
      state = Math.floor(state / 3)
    })
    aliases.add(characters.map((character, index) => {
      const choice = choices.get(index)
      if (choice === 1) return '- '
      if (choice === 2) return ''
      return character
    }).join(''))
  }
  aliases.add(compact.replace(/(\p{L})-(?=\p{L})/gu, '$1'))
  for (const dash of SEARCH_DASH_CHARACTERS.slice(1)) {
    aliases.add(compact.replaceAll('-', dash))
  }

  const values = [...aliases]
  return values.length === 1 ? original : values
}
