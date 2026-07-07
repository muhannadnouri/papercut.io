import type { SearchOpenTarget, SearchResult } from '../../types/search'

interface LastSearchInfo {
  phrases: string[]
  candidateCount: number
  resultCount: number
}

interface SearchResultsProps {
  results: SearchResult[]
  loading: boolean
  submittedQuery: string
  lastSearchInfo: LastSearchInfo | null
  selectedFilters: Set<string>
  openingDisabled?: boolean
  openingDocumentUrl?: string
  onViewResult: (result: SearchResult, target?: SearchOpenTarget) => void
}

export function SearchResults({
  results,
  loading,
  submittedQuery,
  lastSearchInfo,
  selectedFilters,
  openingDisabled = false,
  openingDocumentUrl,
  onViewResult,
}: SearchResultsProps) {
  const filtered = selectedFilters.size > 0
    ? results.filter((r) => selectedFilters.has(r.meta.title))
    : results

  return (
    <div className="results-container">
      {loading && <div className="search-loading">Searching...</div>}

      {lastSearchInfo && !loading && submittedQuery.length > 0 && (
        <div className="search-info">
          {lastSearchInfo.phrases.length > 0 ? (
            <>
              Exact phrase{lastSearchInfo.phrases.length > 1 ? 's' : ''}:{' '}
              {lastSearchInfo.phrases.map((p, i) => (
                <span key={i} className="phrase-tag">&ldquo;{p}&rdquo;</span>
              ))}{' '}
              &mdash; kept {lastSearchInfo.resultCount} of {lastSearchInfo.candidateCount} candidate
              {lastSearchInfo.candidateCount === 1 ? '' : 's'}.
            </>
          ) : (
            <>{lastSearchInfo.resultCount} result{lastSearchInfo.resultCount === 1 ? '' : 's'}.</>
          )}
        </div>
      )}

      {submittedQuery.length > 0 && filtered.length === 0 && !loading && (
        <p className="no-results">
          No documents found for &ldquo;{submittedQuery}&rdquo;
          {selectedFilters.size > 0 && ' with the selected filters'}
        </p>
      )}

      {filtered.map((result) => {
        const opening = openingDocumentUrl === result.url
        const disabled = openingDisabled || opening
        return (
          <button
            type="button"
            key={result.id}
            className={'result-card' + (disabled ? ' result-card-disabled' : '')}
            disabled={disabled}
            onClick={() => { if (!disabled) onViewResult(result, searchOpenTargetForResult(result)) }}
          >
            <span className="result-title">{result.meta.title}{opening ? ' (Opening...)' : ''}</span>
            <span
              className="result-excerpt"
              dangerouslySetInnerHTML={{ __html: result.customExcerpt ?? result.excerpt }}
            />
          </button>
        )
      })}

      {submittedQuery.length === 0 && (
        <div className="welcome">
          <p>Type a query and press Search (or Enter) to search across all indexed documents.</p>
          <p className="welcome-hint">Wrap a phrase in double quotes for exact-match search.</p>
        </div>
      )}
    </div>
  )
}

function searchOpenTargetForResult(result: SearchResult): SearchOpenTarget | undefined {
  const hash = hashFromUrl(result.sub_results?.[0]?.url)
  const text = firstMarkedText(result.customExcerpt ?? result.sub_results?.[0]?.excerpt ?? result.excerpt)
  return hash || text ? { hash, text } : undefined
}

function hashFromUrl(url?: string): string | undefined {
  const index = url?.indexOf('#') ?? -1
  if (index < 0 || !url) return undefined
  const hash = url.slice(index)
  return hash.length > 1 ? hash : undefined
}

function firstMarkedText(html: string): string | undefined {
  if (typeof DOMParser === 'undefined') return undefined
  const doc = new DOMParser().parseFromString(html, 'text/html')
  const text = doc.querySelector('mark')?.textContent?.trim()
  return text || undefined
}
