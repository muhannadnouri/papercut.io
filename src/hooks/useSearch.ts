import { useState, useCallback, useRef } from 'react'
import type { PagefindInstance, SearchResult } from '../types/search'
import {
  searchUploadedDocuments,
  type UploadedDocumentSearchResult,
  type UploadedDocumentSearchStage,
} from '../uploads/DocumentUploads'
import { normalizeForPhraseMatch, sanitizeMarkedExcerpt } from '../utils/textUtils'
import {
  buildPhraseExcerpt,
  countPhraseOccurrences,
  docContainsAllPhrases,
  parseSearchQuery,
  type DocumentSourceLoader,
} from '../utils/phraseSearch'

// Keep source reads bounded: provider snippets cover most results, and fallback
// excerpts are mainly for the first screenful when Pagefind only returns a heading.
const FUZZY_FALLBACK_EXCERPT_LIMIT = 12

export type SearchPhase = 'indexes' | 'candidates' | 'phrases' | 'evidence' | 'results' | 'excerpts'

const UPLOADED_SEARCH_PHASES: Record<UploadedDocumentSearchStage, SearchPhase> = {
  findingCandidates: 'candidates',
  verifyingPhrases: 'phrases',
  buildingResults: 'evidence',
}

export interface LastSearchInfo {
  phrases: string[]
  unquotedText: string
  uploadedDocuments: number
  uploadedMatchingSections: number
  starterDocuments: number
}

export type SearchQueryError = 'unmatchedQuote'

interface PagefindResultSet {
  results: SearchResult[]
  totalDocuments: number
}

interface UseSearchOptions {
  loadDocumentSource?: DocumentSourceLoader
  scopeUrls?: Set<string>
  scopeActive?: boolean
}

interface UseSearchReturn {
  query: string
  results: SearchResult[]
  loading: boolean
  queryError: SearchQueryError | null
  searchFailed: boolean
  searchPhase: SearchPhase | null
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
  const [queryError, setQueryError] = useState<SearchQueryError | null>(null)
  const [searchFailed, setSearchFailed] = useState(false)
  const [searchPhase, setSearchPhase] = useState<SearchPhase | null>(null)
  const [submittedQuery, setSubmittedQuery] = useState('')
  const [lastSearchInfo, setLastSearchInfo] = useState<LastSearchInfo | null>(null)

  const queryRef = useRef(query)
  queryRef.current = query
  const submittedQueryRef = useRef('')
  const latestSearchRequestRef = useRef(0)
  const activeSearchKeyRef = useRef<string>('')

  const performSearch = useCallback(async (rawQuery: string) => {
    const displayQuery = rawQuery.trim()
    const normalized = displayQuery.toLowerCase()
    const scopeUrls = options.scopeUrls
    const hasScope = options.scopeActive ?? Boolean(scopeUrls?.size)
    const scopeList = hasScope ? Array.from(scopeUrls ?? []).sort() : undefined
    const searchKey = normalized + '\0' + (hasScope ? `scope\0${scopeList?.join('\0') ?? ''}` : 'all')
    if (normalized.length > 0 && activeSearchKeyRef.current === searchKey) return

    // Query text is not a unique identity: A → B → A can leave the first
    // A running. A monotonic id keeps that stale completion from winning.
    const requestId = latestSearchRequestRef.current + 1
    latestSearchRequestRef.current = requestId
    if (normalized.length === 0) {
      activeSearchKeyRef.current = ''
      setResults([])
      setQueryError(null)
      setSearchFailed(false)
      setLastSearchInfo(null)
      setSearchPhase(null)
      return
    }

    const parsedQuery = parseSearchQuery(displayQuery)
    if (parsedQuery.unmatchedQuote) {
      activeSearchKeyRef.current = ''
      submittedQueryRef.current = ''
      setSubmittedQuery('')
      setResults([])
      setQueryError('unmatchedQuote')
      setSearchFailed(false)
      setLastSearchInfo(null)
      setSearchPhase(null)
      return
    }

    submittedQueryRef.current = displayQuery
    setSubmittedQuery(displayQuery)
    setQueryError(null)
    const displayPhrases = parsedQuery.exactPhrases
    const phrases = displayPhrases.map(normalizeForPhraseMatch)
    const searchQuery = parsedQuery.providerQuery.toLowerCase()
    const uploadedQuery = parsedQuery.unquotedText.toLowerCase()
    if (searchQuery.length === 0) {
      activeSearchKeyRef.current = ''
      setResults([])
      setSearchFailed(false)
      setLastSearchInfo(null)
      setSearchPhase(null)
      return
    }

    if (hasScope && scopeList?.length === 0) {
      activeSearchKeyRef.current = ''
      setResults([])
      setSearchFailed(false)
      setLastSearchInfo({
        phrases: displayPhrases,
        unquotedText: parsedQuery.unquotedText,
        uploadedDocuments: 0,
        uploadedMatchingSections: 0,
        starterDocuments: 0,
      })
      setSearchPhase(null)
      return
    }
    activeSearchKeyRef.current = searchKey
    setSearchFailed(false)
    setSearchPhase('indexes')
    try {
      const pagefindPromise = pagefindRef.current
        ? pagefindRef.current.search(searchQuery)
        : Promise.resolve({ results: [] })
      const uploadPromise = searchUploadedDocuments(
        uploadedQuery,
        hasScope ? 100 : 50,
        scopeList,
        phrases.length > 0 ? phrases : undefined,
        (stage) => {
          if (latestSearchRequestRef.current === requestId) {
            setSearchPhase(UPLOADED_SEARCH_PHASES[stage])
          }
        },
      )
      const [pagefindSearch, uploadedSearch] = await Promise.all([pagefindPromise, uploadPromise])
      if (latestSearchRequestRef.current !== requestId) return

      setSearchPhase('results')
      const pagefind = await pagefindResultsInScope(pagefindSearch.results, scopeUrls, hasScope)
      let pagefindData = pagefind.results.map((result) => ({ ...result, source: 'starter' as const }))
      const uploadedData = uploadedSearchToResults(uploadedSearch.results, phrases.length === 0)
      if (phrases.length > 0) {
        setSearchPhase('phrases')
        const verdicts = await Promise.all(
          pagefindData.map((result) => (
            docContainsAllPhrases(result.url, phrases, options.loadDocumentSource)
          )),
        )
        if (latestSearchRequestRef.current !== requestId) return
        pagefindData = pagefindData.filter((_, index) => verdicts[index])
        if (pagefindData.length > 0) {
          setSearchPhase('excerpts')
          const [excerpts, matchCounts] = await Promise.all([
            Promise.all(pagefindData.map((result) => (
              buildPhraseExcerpt(result.url, phrases, options.loadDocumentSource)
            ))),
            Promise.all(pagefindData.map((result) => (
              countPhraseOccurrences(result.url, phrases, options.loadDocumentSource)
            ))),
          ])
          if (latestSearchRequestRef.current !== requestId) return
          pagefindData = pagefindData.map((result, index) => (
            excerpts[index]
              ? { ...result, customExcerpt: excerpts[index] ?? undefined, matchCount: matchCounts[index] }
              : { ...result, matchCount: matchCounts[index] }
          ))
        }
      }

      const data = [...uploadedData, ...pagefindData]
        .filter((result) => !hasScope || scopeUrls?.has(result.url))
      if (latestSearchRequestRef.current !== requestId) return

      let filtered = data
      if (phrases.length === 0 && options.loadDocumentSource) {
        const terms = searchQuery.split(/\s+/).filter(Boolean)
        const fallbackTargets = filtered
          .slice(0, FUZZY_FALLBACK_EXCERPT_LIMIT)
          .map((result, index) => ({ result, index }))
          .filter(({ result }) => !hasUsefulProviderExcerpt(result))
        if (fallbackTargets.length > 0) {
          setSearchPhase('excerpts')
          const excerpts = await Promise.all(
            fallbackTargets.map(({ result }) => buildPhraseExcerpt(result.url, terms, options.loadDocumentSource)),
          )
          if (latestSearchRequestRef.current !== requestId) return
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
        unquotedText: parsedQuery.unquotedText,
        uploadedDocuments: uploadedSearch.totalDocuments,
        uploadedMatchingSections: uploadedSearch.totalMatchingSections,
        starterDocuments: phrases.length > 0
          ? filtered.filter((result) => result.source === 'starter').length
          : pagefind.totalDocuments,
      })
    } catch (err) {
      console.error('Search failed:', err)
      if (latestSearchRequestRef.current === requestId) {
        setResults([])
        setSearchFailed(true)
        setLastSearchInfo(null)
      }
    } finally {
      if (latestSearchRequestRef.current === requestId) {
        activeSearchKeyRef.current = ''
        setSearchPhase(null)
      }
    }
  }, [options.loadDocumentSource, options.scopeActive, options.scopeUrls, pagefindRef])

  const handleSearch = useCallback((searchQuery: string) => {
    setQuery(searchQuery)
    queryRef.current = searchQuery
    setQueryError(null)
    if (searchQuery.trim().length === 0) {
      latestSearchRequestRef.current += 1
      activeSearchKeyRef.current = ''
      submittedQueryRef.current = ''
      setResults([])
      setSubmittedQuery('')
      setSearchFailed(false)
      setLastSearchInfo(null)
      setSearchPhase(null)
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

  return {
    query,
    results,
    loading: searchPhase !== null,
    queryError,
    searchFailed,
    searchPhase,
    submittedQuery,
    lastSearchInfo,
    handleSearch,
    rerunSearch,
    submitSearch,
    removeResultsForUrl,
  }
}

export async function pagefindResultsInScope(
  results: { id: string; data: () => Promise<SearchResult> }[],
  scopeUrls?: Set<string>,
  scopeActive = Boolean(scopeUrls?.size),
): Promise<PagefindResultSet> {
  if (!scopeActive) {
    return {
      results: await Promise.all(results.slice(0, 50).map((r) => r.data())),
      totalDocuments: results.length,
    }
  }
  if (!scopeUrls?.size) return { results: [], totalDocuments: 0 }

  const scoped: SearchResult[] = []
  let totalDocuments = 0
  for (const result of results) {
    const data = await result.data()
    if (!scopeUrls.has(data.url)) continue
    totalDocuments += 1
    if (scoped.length < 50) scoped.push(data)
  }
  return { results: scoped, totalDocuments }
}

function uploadedSearchToResult(
  result: UploadedDocumentSearchResult,
  includeSectionCount = true,
): SearchResult {
  return {
    id: result.id,
    url: result.url,
    meta: { title: result.title },
    excerpt: sanitizeUploadedExcerpt(result.excerpt),
    sectionIndex: result.sectionIndex,
    pageIndex: result.pageIndex,
    matchCount: result.matchCount ?? (includeSectionCount ? result.matchingSections : undefined),
    matchScope: result.matchScope,
    matchingSections: result.matchingSections,
    passages: result.passages.map((passage) => ({
      ...passage,
      excerpt: sanitizeUploadedExcerpt(passage.excerpt),
    })),
    matchLocations: result.matchLocations,
    termMatches: result.termMatches,
    source: 'upload',
    sub_results: result.sectionTitle
      ? [{ url: result.url, title: result.sectionTitle }]
      : undefined,
  }
}

function uploadedSearchToResults(
  results: UploadedDocumentSearchResult[],
  includeSectionCount = true,
): SearchResult[] {
  return results.map((result) => uploadedSearchToResult(result, includeSectionCount))
}

function sanitizeUploadedExcerpt(excerpt: string): string {
  return sanitizeMarkedExcerpt(excerpt)
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
