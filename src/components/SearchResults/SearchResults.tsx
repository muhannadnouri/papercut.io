import type { SearchOpenTarget, SearchResult } from '../../types/search'
import './SearchResults.css'

interface LastSearchInfo {
  phrases: string[]
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
    ? results.filter((r) => selectedFilters.has(r.url))
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
        const excerpt = resultExcerpt(result)
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
            {excerpt && (
              <span
                className="result-excerpt"
                dangerouslySetInnerHTML={{ __html: excerpt }}
              />
            )}
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
  const text = firstMarkedText(resultExcerpt(result) ?? '')
  return hash || text ? { hash, text } : undefined
}

// Use the richest safe snippet available, but avoid rendering a body line that
// merely repeats the section label already shown in result metadata.
function resultExcerpt(result: SearchResult): string | null {
  const sectionTitle = result.sub_results?.[0]?.title
  return usefulExcerpt(result.customExcerpt, sectionTitle)
    ?? usefulExcerpt(result.sub_results?.[0]?.excerpt, sectionTitle)
    ?? usefulExcerpt(result.excerpt, sectionTitle)
}

// Compare rendered text instead of raw HTML so highlighted snippets and plain
// section titles can be recognized as duplicates.
function usefulExcerpt(excerpt: string | undefined, sectionTitle: string | undefined): string | null {
  if (!excerpt) return null
  if (plainText(excerpt) === plainText(sectionTitle)) return null
  return excerpt
}

function resultMeta(result: SearchResult, exactPhrase: boolean): string | null {
  if (exactPhrase) {
    const count = result.matchCount
    return count ? count + ' exact match' + (count === 1 ? '' : 'es') : null
  }

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

// Converts snippet HTML into comparable visible text without letting entities,
// tags, or <mark> wrappers change duplicate detection.
function plainText(html?: string): string {
  if (!html) return ''
  if (typeof DOMParser === 'undefined') return html.replace(/<[^>]+>/g, ' ').replace(/\s+/g, ' ').trim()
  const doc = new DOMParser().parseFromString(html, 'text/html')
  return (doc.body.textContent ?? '').replace(/\s+/g, ' ').trim()
}
