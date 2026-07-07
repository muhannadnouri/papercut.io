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
  const hasFilters = selectedFilters.size > 0
  const visibleCount = filtered.length

  return (
    <div className="results-container">
      {loading && <div className="search-loading">Searching...</div>}

      {lastSearchInfo && !loading && submittedQuery.length > 0 && visibleCount > 0 && (
        <div className="search-info">
          {lastSearchInfo.phrases.length > 0 ? (
            <>
              {visibleCount} {hasFilters ? 'filtered ' : ''}document{visibleCount === 1 ? '' : 's'} contain{visibleCount === 1 ? 's' : ''}{' '}
              exact phrase{lastSearchInfo.phrases.length === 1 ? '' : 's'}{' '}
              {lastSearchInfo.phrases.map((p, i) => (
                <span key={i} className="query-tag">&ldquo;{p}&rdquo;</span>
              ))}
              .
            </>
          ) : (
            <>
              {visibleCount} {hasFilters ? 'filtered ' : ''}document{visibleCount === 1 ? '' : 's'} matched terms{' '}
              <span className="query-tag">&ldquo;{submittedQuery}&rdquo;</span>.
            </>
          )}
        </div>
      )}

      {submittedQuery.length > 0 && filtered.length === 0 && !loading && (
        <p className="no-results">
          {lastSearchInfo?.phrases.length ? (
            <>
              No {hasFilters ? 'filtered ' : ''}documents contain exact phrase{lastSearchInfo.phrases.length === 1 ? '' : 's'}{' '}
              {lastSearchInfo.phrases.map((p, i) => (
                <span key={i} className="query-tag">&ldquo;{p}&rdquo;</span>
              ))}
              .
            </>
          ) : (
            <>
              No {hasFilters ? 'filtered ' : ''}documents matched terms{' '}
              <span className="query-tag">&ldquo;{submittedQuery}&rdquo;</span>.
            </>
          )}
        </p>
      )}

      {filtered.map((result) => {
        const opening = openingDocumentUrl === result.url
        const disabled = openingDisabled || opening
        const meta = resultMeta(result, Boolean(lastSearchInfo?.phrases.length))
        return (
          <button
            type="button"
            key={result.id}
            className={'result-card' + (disabled ? ' result-card-disabled' : '')}
            disabled={disabled}
            onClick={() => { if (!disabled) onViewResult(result, searchOpenTargetForResult(result)) }}
          >
            <span className="result-title">{result.meta.title}{opening ? ' (Opening...)' : ''}</span>
            {meta && <span className="result-meta">{meta}</span>}
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

// Quoted searches use Pagefind/SQLite as broad candidate finders, then verify
// the exact phrase against source text. Provider section counts are therefore
// useful for unquoted searches, but misleading as "exact" counts.
function resultMeta(result: SearchResult, exactPhrase: boolean): string | null {
  if (exactPhrase) return null

  const parts = [
    result.sub_results?.[0]?.title
      ? 'Section: ' + result.sub_results[0].title
      : 'Best matching passage',
  ]
  const count = result.matchCount ?? result.sub_results?.length
  if (count && count > 1) {
    parts.push(count + ' matching sections')
  }
  return parts.join(' · ')
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
