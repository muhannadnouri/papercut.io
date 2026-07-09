import { useState, useCallback, useRef } from 'react'
import type { PagefindInstance, SearchResult } from '../types/search'
import { searchUploadedDocuments, type UploadedDocumentSearchResult } from '../uploads/DocumentUploads'
import { normalizeForPhraseMatch, escapeHtml } from '../utils/textUtils'
import {
  buildPhraseExcerpt,
  countPhraseOccurrences,
  docContainsAllPhrases,
  extractQuotedPhrases,
  stripQuotes,
  type DocumentSourceLoader,
} from '../utils/phraseSearch'

// Keep source reads bounded: provider snippets cover most results, and fallback
// excerpts are mainly for the first screenful when Pagefind only returns a heading.
const FUZZY_FALLBACK_EXCERPT_LIMIT = 12

interface LastSearchInfo {
  phrases: string[]
}

interface UseSearchOptions {
  loadDocumentSource?: DocumentSourceLoader
  scopeUrls?: Set<string>
}

interface UseSearchReturn {
  query: string
  results: SearchResult[]
  loading: boolean
  submittedQuery: string
  lastSearchInfo: LastSearchInfo | null
  handleSearch: (searchQuery: string) => void
  rerunSearch: () => void
  submitSearch: () => void
  removeResultsForUrl: (url: string) => void
}

export function useSearch(
  pagefindRef: React.MutableRefObject<PagefindInstance | null>,
  options: UseSearchOptions = {},
): UseSearchReturn {
  const [query, setQuery] = useState('')
  const [results, setResults] = useState<SearchResult[]>([])
  const [loading, setLoading] = useState(false)
  const [submittedQuery, setSubmittedQuery] = useState('')
  const [lastSearchInfo, setLastSearchInfo] = useState<LastSearchInfo | null>(null)

  const queryRef = useRef(query)
  queryRef.current = query
  const submittedQueryRef = useRef('')
  const latestSearchKeyRef = useRef<string>('')

  const performSearch = useCallback(async (rawQuery: string) => {
    const displayQuery = rawQuery.trim()
    const normalized = displayQuery.toLowerCase()
    const scopeUrls = options.scopeUrls
    const hasScope = Boolean(scopeUrls?.size)
    const scopeList = hasScope ? Array.from(scopeUrls ?? []).sort() : undefined
    const searchKey = normalized + '\0' + (scopeList?.join('\0') ?? '')
    latestSearchKeyRef.current = searchKey
    submittedQueryRef.current = displayQuery
    setSubmittedQuery(displayQuery)
    if (normalized.length === 0) {
      setResults([])
      setLastSearchInfo(null)
      setLoading(false)
      return
    }

    const displayPhrases = extractQuotedPhrases(displayQuery)
    const phrases = displayPhrases.map(normalizeForPhraseMatch)
    const searchQuery = phrases.length > 0 ? stripQuotes(normalized) : normalized
    if (searchQuery.length === 0) {
      setResults([])
      setLastSearchInfo(null)
      setLoading(false)
      return
    }

    setLoading(true)
    try {
      const pagefindPromise = pagefindRef.current
        ? pagefindRef.current.search(searchQuery)
        : Promise.resolve({ results: [] })
      const uploadPromise = searchUploadedDocuments(searchQuery, hasScope ? 100 : 50, scopeList)
      const [pagefindSearch, uploadedSearch] = await Promise.all([pagefindPromise, uploadPromise])
      if (latestSearchKeyRef.current !== searchKey) return

      const pagefindData = await pagefindResultsInScope(pagefindSearch.results, scopeUrls)
      const uploadedData = uploadedSearchToResults(uploadedSearch)
      const data = [...pagefindData, ...uploadedData]
        .filter((result) => !scopeUrls?.size || scopeUrls.has(result.url))
        .slice(0, 100)
      if (latestSearchKeyRef.current !== searchKey) return

      let filtered = data
      if (phrases.length > 0) {
        const verdicts = await Promise.all(
          data.map((d) => docContainsAllPhrases(d.url, phrases, options.loadDocumentSource)),
        )
        if (latestSearchKeyRef.current !== searchKey) return
        filtered = data.filter((_, i) => verdicts[i])
        const [excerpts, matchCounts] = await Promise.all([
          Promise.all(filtered.map((d) => buildPhraseExcerpt(d.url, phrases, options.loadDocumentSource))),
          Promise.all(filtered.map((d) => countPhraseOccurrences(d.url, phrases, options.loadDocumentSource))),
        ])
        if (latestSearchKeyRef.current !== searchKey) return
        filtered = filtered.map((d, i) =>
          excerpts[i]
            ? { ...d, customExcerpt: excerpts[i] ?? undefined, matchCount: matchCounts[i] }
            : { ...d, matchCount: matchCounts[i] },
        )
      } else if (options.loadDocumentSource) {
        const terms = searchQuery.split(/\s+/).filter(Boolean)
        const fallbackTargets = filtered
          .slice(0, FUZZY_FALLBACK_EXCERPT_LIMIT)
          .map((result, index) => ({ result, index }))
          .filter(({ result }) => !hasUsefulProviderExcerpt(result))
        if (fallbackTargets.length > 0) {
          const excerpts = await Promise.all(
            fallbackTargets.map(({ result }) => buildPhraseExcerpt(result.url, terms, options.loadDocumentSource)),
          )
          if (latestSearchKeyRef.current !== searchKey) return
          filtered = filtered.map((result, index) => {
            const targetIndex = fallbackTargets.findIndex((target) => target.index === index)
            const excerpt = targetIndex === -1 ? null : excerpts[targetIndex]
            return excerpt ? { ...result, customExcerpt: excerpt } : result
          })
        }
      }

      setResults(filtered)
      setLastSearchInfo({
        phrases: displayPhrases,
      })
    } catch (err) {
      console.error('Search failed:', err)
      if (latestSearchKeyRef.current === searchKey) {
        setResults([])
        setLastSearchInfo(null)
      }
    } finally {
      if (latestSearchKeyRef.current === searchKey) setLoading(false)
    }
  }, [options.loadDocumentSource, options.scopeUrls, pagefindRef])

  const handleSearch = useCallback((searchQuery: string) => {
    setQuery(searchQuery)
    queryRef.current = searchQuery
    if (searchQuery.trim().length === 0) {
      latestSearchKeyRef.current = ''
      submittedQueryRef.current = ''
      setResults([])
      setSubmittedQuery('')
      setLastSearchInfo(null)
      setLoading(false)
    }
  }, [])

  const submitSearch = useCallback(() => {
    performSearch(queryRef.current)
  }, [performSearch])

  const rerunSearch = useCallback(() => {
    if (submittedQueryRef.current.trim().length > 0) {
      performSearch(submittedQueryRef.current)
    }
  }, [performSearch])

  const removeResultsForUrl = useCallback((url: string) => {
    setResults((current) => current.filter((item) => item.url !== url))
  }, [])

  return { query, results, loading, submittedQuery, lastSearchInfo, handleSearch, rerunSearch, submitSearch, removeResultsForUrl }
}

async function pagefindResultsInScope(
  results: { id: string; data: () => Promise<SearchResult> }[],
  scopeUrls?: Set<string>,
): Promise<SearchResult[]> {
  if (!scopeUrls?.size) {
    return Promise.all(results.slice(0, 50).map((r) => r.data()))
  }

  const scoped: SearchResult[] = []
  for (const result of results) {
    const data = await result.data()
    if (!scopeUrls.has(data.url)) continue
    scoped.push(data)
    if (scoped.length >= Math.min(scopeUrls.size, 100)) break
  }
  return scoped
}

function uploadedSearchToResult(result: UploadedDocumentSearchResult, matchCount?: number): SearchResult {
  return {
    id: result.id,
    url: result.url,
    meta: { title: result.title },
    excerpt: sanitizeUploadedExcerpt(result.excerpt),
    matchCount,
    matchScope: result.matchScope,
    sub_results: result.sectionTitle
      ? [{ url: result.url, title: result.sectionTitle }]
      : undefined,
  }
}

// SQLite returns section-level hits, while the UI intentionally shows one card
// per document. Preserve the section count for broad searches, but keep the
// first ranked hit as the document's representative snippet.
function uploadedSearchToResults(results: UploadedDocumentSearchResult[]): SearchResult[] {
  const matchCounts = new Map<string, number>()
  for (const result of results) {
    if (result.matchScope === 'document') {
      matchCounts.set(result.url, Math.max(matchCounts.get(result.url) ?? 0, 1))
    } else {
      matchCounts.set(result.url, (matchCounts.get(result.url) ?? 0) + 1)
    }
  }

  const seen = new Set<string>()
  const deduped: SearchResult[] = []
  for (const result of results) {
    // SQLite returns the best matching sections first, so the first result per uploaded
    // document is the snippet we want to show on the single document-level card.
    if (seen.has(result.url)) continue
    seen.add(result.url)
    deduped.push(uploadedSearchToResult(result, matchCounts.get(result.url)))
  }
  return deduped
}

function sanitizeUploadedExcerpt(excerpt: string): string {
  return escapeHtml(excerpt)
    .replace(/&lt;mark&gt;/g, '<mark>')
    .replace(/&lt;\/mark&gt;/g, '</mark>')
}

// Provider snippets can collapse to the section heading for generated EPUB
// pages. Treat those as missing so we only fetch source text when the existing
// snippet would not help the user decide whether to open the result.
function hasUsefulProviderExcerpt(result: SearchResult): boolean {
  const sectionTitle = result.sub_results?.[0]?.title
  return Boolean(
    usefulExcerpt(result.sub_results?.[0]?.excerpt, sectionTitle)
      ?? usefulExcerpt(result.excerpt, sectionTitle),
  )
}

// Compares rendered text, not raw markup, because snippets may wrap matches in
// <mark> while section titles are plain strings.
function usefulExcerpt(excerpt: string | undefined, sectionTitle: string | undefined): string | null {
  if (!excerpt) return null
  if (plainText(excerpt) === plainText(sectionTitle)) return null
  return excerpt
}

// Small DOM-based text extraction keeps HTML entities and <mark> tags from
// affecting duplicate-snippet checks.
function plainText(html?: string): string {
  if (!html) return ''
  if (typeof DOMParser === 'undefined') return html.replace(/<[^>]+>/g, ' ').replace(/\s+/g, ' ').trim()
  const doc = new DOMParser().parseFromString(html, 'text/html')
  return (doc.body.textContent ?? '').replace(/\s+/g, ' ').trim()
}
