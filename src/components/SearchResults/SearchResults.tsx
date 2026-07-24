import { useTranslation } from 'react-i18next'
import type { TFunction } from 'i18next'
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
  const { t } = useTranslation()
  const filtered = selectedFilters.size > 0
    ? results.filter((r) => selectedFilters.has(r.url))
    : results
  const hasFilters = selectedFilters.size > 0
  const visibleCount = filtered.length

  if (loading) {
    return (
      <div className="results-container" aria-busy="true">
        <div className="search-loading" role="status" aria-live="polite" aria-atomic="true">
          <span className="spinner" aria-hidden="true" />
          <span>{t('search.results.searching')}</span>
        </div>
      </div>
    )
  }

  return (
    <div className="results-container" aria-busy="false">
      {lastSearchInfo && submittedQuery.length > 0 && visibleCount > 0 && (
        <div className="search-info">
          <strong>{t('search.results.resultCount', { count: visibleCount })}</strong>
          {hasFilters && <> · {t('search.results.filtered')}</>}
          {' · '}
          {lastSearchInfo.phrases.length > 0
            ? t(lastSearchInfo.phrases.length === 1 ? 'search.results.exactPhrase' : 'search.results.exactPhrases')
            : t('search.results.matchedTerms')}
          {' '}
          {(lastSearchInfo.phrases.length > 0 ? lastSearchInfo.phrases : [submittedQuery]).map((phrase, index) => (
            <bdi key={index} className="query-tag">&ldquo;{phrase}&rdquo;</bdi>
          ))}
        </div>
      )}

      {submittedQuery.length > 0 && filtered.length === 0 && (
        <p className="no-results">
          <span>{t('search.results.noResults')}</span>
          {hasFilters && <> <span>{t('search.results.filtersApplied')}</span></>}
          {' '}
          <span>
            {lastSearchInfo?.phrases.length
              ? t(lastSearchInfo.phrases.length === 1 ? 'search.results.exactPhrase' : 'search.results.exactPhrases')
              : t('search.results.searchTerms')}
          </span>
          {' '}
          {(lastSearchInfo?.phrases.length ? lastSearchInfo.phrases : [submittedQuery]).map((phrase, index) => (
            <bdi key={index} className="query-tag">&ldquo;{phrase}&rdquo;</bdi>
          ))}
        </p>
      )}

      {filtered.map((result) => {
        const opening = openingDocumentUrl === result.url
        const disabled = openingDisabled || opening
        const exactPhrase = Boolean(lastSearchInfo?.phrases.length)
        const meta = resultMeta(result, exactPhrase, t)
        const excerpt = resultExcerpt(result, exactPhrase)
        return (
          <button
            type="button"
            key={result.id}
            className={'result-card' + (disabled ? ' result-card-disabled' : '')}
            disabled={disabled}
            onClick={() => { if (!disabled) onViewResult(result, searchOpenTargetForResult(result, exactPhrase)) }}
          >
            <span className="result-title">
              <bdi>{result.meta.title}</bdi>
              {opening ? ` (${t('common.opening')})` : ''}
            </span>
            {meta && <span className="result-meta" dir="auto">{meta}</span>}
            {excerpt && (
              <span
                className="result-excerpt"
                dir="auto"
                dangerouslySetInnerHTML={{ __html: excerpt }}
              />
            )}
          </button>
        )
      })}

      {submittedQuery.length === 0 && (
        <div className="welcome">
          <p>{t('search.results.welcome')}</p>
          <p className="welcome-hint">{t('search.results.welcomeHint')}</p>
        </div>
      )}
    </div>
  )
}

function searchOpenTargetForResult(result: SearchResult, exactPhrase: boolean): SearchOpenTarget | undefined {
  const hash = hashFromUrl(result.sub_results?.[0]?.url)
  const text = firstMarkedText(resultExcerpt(result, exactPhrase) ?? '')
  return hash || text ? { hash, text } : undefined
}

// Use the richest safe snippet available, but avoid rendering a body line that
// merely repeats the section label already shown in result metadata.
function resultExcerpt(result: SearchResult, exactPhrase = false): string | null {
  if (!exactPhrase && result.matchScope === 'document') return null

  const sectionTitle = result.sub_results?.[0]?.title
  const excerpt = usefulExcerpt(result.customExcerpt, sectionTitle)
    ?? usefulExcerpt(result.sub_results?.[0]?.excerpt, sectionTitle)
    ?? usefulExcerpt(result.excerpt, sectionTitle)
  if (!exactPhrase && excerpt && !/<mark\b/i.test(excerpt)) return null
  return excerpt
}

// Compare rendered text instead of raw HTML so highlighted snippets and plain
// section titles can be recognized as duplicates.
function usefulExcerpt(excerpt: string | undefined, sectionTitle: string | undefined): string | null {
  if (!excerpt) return null
  if (plainText(excerpt) === plainText(sectionTitle)) return null
  return excerpt
}

function resultMeta(result: SearchResult, exactPhrase: boolean, t: TFunction): string | null {
  if (exactPhrase) {
    const count = result.matchCount
    return count ? t('search.results.exactMatch', { count }) : null
  }

  if (result.matchScope === 'document') {
    return t('search.results.termsSeparateSections')
  }

  const parts = [
    result.sub_results?.[0]?.title
      ? t('search.results.bestSection', { title: result.sub_results[0].title })
      : t('search.results.bestPassage'),
  ]
  const count = result.matchCount ?? result.sub_results?.length
  if (count && count > 1) {
    parts.push(t('search.results.matchingSections', { count }))
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
