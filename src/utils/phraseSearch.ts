import { normalizeForDisplay, normalizeSearchPunctuation, escapeHtml, escapeRegex } from './textUtils'

interface DocText {
  raw: string
  lower: string
}

export type DocumentSourceLoader = (url: string) => Promise<string>

const phraseFetchCache = new Map<string, DocText>()
const EMPTY_DOC_TEXT: DocText = { raw: '', lower: '' }

export function clearPhraseFetchCache(url?: string): void {
  if (url) phraseFetchCache.delete(url)
  else phraseFetchCache.clear()
}

async function fetchDocText(url: string, loadSource?: DocumentSourceLoader): Promise<DocText> {
  const cached = phraseFetchCache.get(url)
  if (cached !== undefined) return cached
  try {
    const html = loadSource ? await loadSource(url) : await fetchSource(url)
    const raw = normalizeForDisplay(htmlToSearchText(html))
    const entry: DocText = { raw, lower: raw.toLowerCase() }
    phraseFetchCache.set(url, entry)
    return entry
  } catch {
    phraseFetchCache.set(url, EMPTY_DOC_TEXT)
    return EMPTY_DOC_TEXT
  }
}

// Exact phrase search needs decoded text, not raw HTML. DOMParser turns
// entities like &nbsp; into whitespace before normalizeForDisplay collapses it.
function htmlToSearchText(html: string): string {
  if (typeof DOMParser === 'undefined') return html.replace(/<[^>]+>/g, ' ')

  const doc = new DOMParser().parseFromString(html, 'text/html')
  doc.querySelectorAll('script, style, noscript').forEach((node) => node.remove())
  return doc.body.textContent ?? ''
}

async function fetchSource(url: string): Promise<string> {
  const res = await fetch(url)
  if (!res.ok) throw new Error('Document source unavailable')
  return res.text()
}

export interface ParsedSearchQuery {
  exactPhrases: string[]
  providerQuery: string
  unquotedText: string
  unmatchedQuote: boolean
}

/** Keep provider input and the user-visible query clauses derived from the same
 * quote scan so mixed searches cannot be presented as phrase-only searches. */
export function parseSearchQuery(q: string): ParsedSearchQuery {
  const exactPhrases: string[] = []
  let providerQuery = ''
  let unquotedText = ''
  let phrase = ''
  const closingQuotes: string[] = []
  let mismatchedQuote = false

  for (const character of q) {
    if (character === '"' || character === '“' || character === '”') {
      // Keep inner quotation marks in source verification. Their delimiter
      // type must not close an outer phrase of a different type.
      if (character === closingQuotes.at(-1)) {
        closingQuotes.pop()
        if (closingQuotes.length > 0) {
          phrase += '"'
        } else {
          const value = phrase.trim()
          if (value) exactPhrases.push(value)
          phrase = ''
        }
      } else if (character === '”') {
        mismatchedQuote = true
      } else {
        if (closingQuotes.length > 0) phrase += '"'
        closingQuotes.push(character === '“' ? '”' : '"')
      }
      providerQuery += ' '
      unquotedText += ' '
    } else if (closingQuotes.length > 0) {
      phrase += character
      providerQuery += character
    } else {
      providerQuery += character
      unquotedText += character
    }
  }

  return {
    exactPhrases,
    providerQuery: providerQuery.replace(/\s+/g, ' ').trim(),
    unquotedText: unquotedText.replace(/\s+/g, ' ').trim(),
    unmatchedQuote: mismatchedQuote || closingQuotes.length > 0,
  }
}

export async function docContainsAllPhrases(
  url: string,
  phrases: string[],
  loadSource?: DocumentSourceLoader,
): Promise<boolean> {
  const { lower } = await fetchDocText(url, loadSource)
  if (lower.length === 0) return false
  return phrases.every((p) => lower.includes(p))
}

// Counts verified source-text occurrences after HTML/entity normalization, so
// exact search counts match what the reader can find in rendered documents.
export async function countPhraseOccurrences(
  url: string,
  phrases: string[],
  loadSource?: DocumentSourceLoader,
): Promise<number> {
  const { lower } = await fetchDocText(url, loadSource)
  if (lower.length === 0) return 0

  let count = 0
  for (const phrase of phrases) {
    if (phrase.length === 0) continue
    let index = lower.indexOf(phrase)
    while (index !== -1) {
      count += 1
      index = lower.indexOf(phrase, index + phrase.length)
    }
  }
  return count
}

export async function buildPhraseExcerpt(
  url: string,
  phrases: string[],
  loadSource?: DocumentSourceLoader,
): Promise<string | null> {
  const { raw, lower } = await fetchDocText(url, loadSource)
  if (lower.length === 0) return null

  let earliest = Infinity
  for (const p of phrases) {
    const idx = lower.indexOf(p)
    if (idx !== -1 && idx < earliest) earliest = idx
  }
  if (earliest === Infinity) return null

  const WINDOW = 120
  const start = Math.max(0, earliest - WINDOW)
  const end = Math.min(raw.length, earliest + phrases[0].length + WINDOW)
  let snippet = raw.slice(start, end)
  if (start > 0) snippet = '… ' + snippet
  if (end < raw.length) snippet = snippet + ' …'

  let html = escapeHtml(snippet)
  for (const p of phrases) {
    const re = new RegExp(escapeRegex(p), 'gi')
    html = html.replace(re, (m) => '<mark>' + m + '</mark>')
  }
  return html
}

/** Match the native query budget before either provider starts; IPC still validates independently. */
export function searchQueryWithinLimits(query: string, phrases: readonly string[]): boolean {
  const parts = [query, ...phrases]
  if (parts.reduce((count, part) => count + Array.from(part).length, 0) > 4096) return false
  let terms = 0
  for (const part of parts) {
    const tokens = normalizeSearchPunctuation(part)
      .replace(/(\p{Alphabetic})-\p{White_Space}+(?=\p{Alphabetic})/gu, '$1-')
      .split(/\p{White_Space}+/u)
    for (const token of tokens) {
      if (token.replace(/^[^\p{Alphabetic}\p{N}_-]+|[^\p{Alphabetic}\p{N}_-]+$/gu, '')) terms++
      if (terms > 128) return false
    }
  }
  return true
}
