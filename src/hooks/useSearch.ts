import { useState, useCallback, useRef } from 'react'
import type { PagefindInstance, SearchResult } from '../types/search'
import { searchUploadedDocuments, type UploadedDocumentSearchResult } from '../uploads/DocumentUploads'
import { normalizeForPhraseMatch, escapeHtml } from '../utils/textUtils'
import {
  buildPhraseExcerpt,
  docContainsAllPhrases,
  extractQuotedPhrases,
  stripQuotes,
  type DocumentSourceLoader,
} from '../utils/phraseSearch'

interface LastSearchInfo {
  phrases: string[]
  candidateCount: number
  resultCount: number
}

interface UseSearchOptions {
  loadDocumentSource?: DocumentSourceLoader
}

interface UseSearchReturn {
  query: string
  results: SearchResult[]
  loading: boolean
  submittedQuery: string
  lastSearchInfo: LastSearchInfo | null
  handleSearch: (searchQuery: string) => void
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
  const latestQueryRef = useRef<string>('')

  const performSearch = useCallback(async (rawQuery: string) => {
    const normalized = rawQuery.trim().toLowerCase()
    latestQueryRef.current = normalized
    setSubmittedQuery(normalized)
    if (normalized.length === 0) {
      setResults([])
      setLastSearchInfo(null)
      setLoading(false)
      return
    }

    const phrases = extractQuotedPhrases(normalized)
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
      const uploadPromise = searchUploadedDocuments(searchQuery, 50)
      const [pagefindSearch, uploadedSearch] = await Promise.all([pagefindPromise, uploadPromise])
      if (latestQueryRef.current !== normalized) return

      const pagefindData = await Promise.all(
        pagefindSearch.results.slice(0, 50).map((r) => r.data()),
      )
      const uploadedData = uploadedSearchToResults(uploadedSearch)
      const data = [...pagefindData, ...uploadedData].slice(0, 100)
      if (latestQueryRef.current !== normalized) return

      let filtered = data
      if (phrases.length > 0) {
        const normalizedPhrases = phrases.map(normalizeForPhraseMatch)
        const verdicts = await Promise.all(
          data.map((d) => docContainsAllPhrases(d.url, normalizedPhrases, options.loadDocumentSource)),
        )
        if (latestQueryRef.current !== normalized) return
        filtered = data.filter((_, i) => verdicts[i])
        const excerpts = await Promise.all(
          filtered.map((d) => buildPhraseExcerpt(d.url, normalizedPhrases, options.loadDocumentSource)),
        )
        if (latestQueryRef.current !== normalized) return
        filtered = filtered.map((d, i) =>
          excerpts[i] ? { ...d, customExcerpt: excerpts[i] ?? undefined } : d,
        )
      }

      setResults(filtered)
      setLastSearchInfo({
        phrases,
        candidateCount: data.length,
        resultCount: filtered.length,
      })
    } catch (err) {
      console.error('Search failed:', err)
      if (latestQueryRef.current === normalized) {
        setResults([])
        setLastSearchInfo(null)
      }
    } finally {
      if (latestQueryRef.current === normalized) setLoading(false)
    }
  }, [options.loadDocumentSource, pagefindRef])

  const handleSearch = useCallback((searchQuery: string) => {
    setQuery(searchQuery)
    queryRef.current = searchQuery
    if (searchQuery.trim().length === 0) {
      latestQueryRef.current = ''
      setResults([])
      setSubmittedQuery('')
      setLastSearchInfo(null)
      setLoading(false)
    }
  }, [])

  const submitSearch = useCallback(() => {
    performSearch(queryRef.current)
  }, [performSearch])

  const removeResultsForUrl = useCallback((url: string) => {
    setResults((current) => current.filter((item) => item.url !== url))
  }, [])

  return { query, results, loading, submittedQuery, lastSearchInfo, handleSearch, submitSearch, removeResultsForUrl }
}

function uploadedSearchToResult(result: UploadedDocumentSearchResult, matchCount?: number): SearchResult {
  return {
    id: result.id,
    url: result.url,
    meta: { title: result.title },
    excerpt: sanitizeUploadedExcerpt(result.excerpt),
    matchCount,
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
    matchCounts.set(result.url, (matchCounts.get(result.url) ?? 0) + 1)
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
