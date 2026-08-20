import { useTranslation } from 'react-i18next'
import type { TFunction } from 'i18next'
import type { SearchPhase } from '../../hooks/useSearch'
import type { SearchOpenTarget, SearchPassage, SearchResult } from '../../types/search'
import './SearchResults.css'

const OCCURRENCE_MAP_BINS = 12
const SEARCH_PHASE_KEYS = {
  indexes: 'search.results.searchingDocuments',
  results: 'search.results.loadingRankedResults',
  phrases: 'search.results.verifyingExactPhrases',
  excerpts: 'search.results.preparingExcerpts',
} as const

interface LastSearchInfo {
  phrases: string[]
  uploadedDocuments: number
  uploadedMatchingSections: number
  starterDocuments: number
}

interface SearchResultsProps {
  results: SearchResult[]
  loading: boolean
  searchFailed: boolean
  searchableDocumentCount: number
  searchPhase: SearchPhase | null
  submittedQuery: string
  lastSearchInfo: LastSearchInfo | null
  scopeUrls: Set<string>
  scopeActive: boolean
  openingDisabled?: boolean
  openingDocumentUrl?: string
  onViewResult: (result: SearchResult, target?: SearchOpenTarget) => void
}

export function SearchResults({
  results,
  loading,
  searchFailed,
  searchableDocumentCount,
  searchPhase,
  submittedQuery,
  lastSearchInfo,
  scopeUrls,
  scopeActive,
  openingDisabled = false,
  openingDocumentUrl,
  onViewResult,
}: SearchResultsProps) {
  const { t } = useTranslation()
  const filtered = scopeActive
    ? results.filter((r) => scopeUrls.has(r.url))
    : results
  const hasFilters = scopeActive
  const visibleCount = filtered.length
  const resultGroups = [
    {
      key: 'upload',
      title: t('search.results.yourLibrary'),
      results: filtered.filter((result) => result.source === 'upload'),
      total: lastSearchInfo?.uploadedDocuments ?? 0,
      matchingSections: lastSearchInfo?.phrases.length === 0
        ? lastSearchInfo.uploadedMatchingSections
        : 0,
    },
    {
      key: 'starter',
      title: t('search.results.starterDocuments'),
      results: filtered.filter((result) => result.source !== 'upload'),
      total: lastSearchInfo?.starterDocuments ?? 0,
      matchingSections: 0,
    },
  ].filter((group) => group.results.length > 0)

  if (loading) {
    return (
      <div className="results-container" aria-busy="true">
        <div className="search-loading" role="status" aria-live="polite" aria-atomic="true">
          <span className="spinner" aria-hidden="true" />
          <span className="search-loading-message">
            <span>{t(
              searchPhase ? SEARCH_PHASE_KEYS[searchPhase] : 'search.results.searching',
              { count: searchableDocumentCount },
            )}</span>
            <span className="search-loading-detail">
              {t('search.results.slowSearch')}
            </span>
          </span>
        </div>
      </div>
    )
  }

  return (
    <div className="results-container" aria-busy="false">
      {searchFailed && (
        <p className="no-results search-error" role="alert">
          {t('search.results.searchFailed')}
        </p>
      )}
      {lastSearchInfo && submittedQuery.length > 0 && visibleCount > 0 && (
        <div className="search-info">
          <strong>{t('search.results.resultCount', {
            count: lastSearchInfo.uploadedDocuments + lastSearchInfo.starterDocuments,
          })}</strong>
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

      {!searchFailed && submittedQuery.length > 0 && filtered.length === 0 && (
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

      {resultGroups.map((group) => (
        <section className="result-group" aria-labelledby={`search-result-group-${group.key}`} key={group.key}>
          <div className="result-group-heading">
            <h2 id={`search-result-group-${group.key}`}>{group.title}</h2>
            <span>
              {group.results.length < group.total
                ? t('search.results.showingCount', { shown: group.results.length, total: group.total })
                : t('search.results.resultCount', { count: group.total })}
              {group.matchingSections > 0 && (
                <> · {t('search.results.matchingSections', { count: group.matchingSections })}</>
              )}
            </span>
          </div>
          {group.results.map((result) => {
            const opening = openingDocumentUrl === result.url
            const disabled = openingDisabled || opening
            const exactPhrase = Boolean(lastSearchInfo?.phrases.length)
            const meta = resultMeta(result, exactPhrase, t)
            const excerpt = resultExcerpt(result, exactPhrase)
            const additionalPassages = (result.passages ?? [])
              .filter((passage) => passage.sectionIndex !== result.sectionIndex)
              .slice(0, 2)
            const locations = result.matchLocations ?? []
            const evidenceCells = Array.from({ length: OCCURRENCE_MAP_BINS }, (_, index) => ({
              index,
              location: locations.find((location) => location.binIndex === index),
            }))
            const hasEvidence = result.source === 'upload'
              && result.matchingSections !== undefined
              && result.matchingSections > 1
              && (additionalPassages.length > 0 || locations.length > 1)
            return (
              <article
                key={result.id}
                className={'result-card' + (disabled ? ' result-card-disabled' : '')}
              >
                <button
                  type="button"
                  className="result-card-primary"
                  disabled={disabled}
                  onClick={() => onViewResult(result, searchOpenTargetForResult(
                    result,
                    exactPhrase ? lastSearchInfo?.phrases[0] : undefined,
                  ))}
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
                {hasEvidence && (
                  <details className="result-evidence">
                    <summary>
                      {t('search.results.matchingSections', { count: result.matchingSections })}
                    </summary>
                    <div className="result-evidence-content">
                      <div
                        className="result-occurrence-map"
                        role="group"
                        aria-label={t('search.results.matchingSections', { count: result.matchingSections })}
                      >
                        {evidenceCells.map(({ index, location }) => location ? (
                          <button
                            type="button"
                            className="result-occurrence-marker"
                            key={index}
                            disabled={disabled}
                            aria-label={t('search.results.matchingSections', { count: location.matchCount })}
                            aria-posinset={index + 1}
                            aria-setsize={OCCURRENCE_MAP_BINS}
                            title={t('search.results.matchingSections', { count: location.matchCount })}
                            onClick={() => onViewResult(result, {
                              sectionIndex: location.sectionIndex,
                              pageIndex: location.pageIndex ?? undefined,
                            })}
                          >
                            {location.matchCount > 1 ? location.matchCount : ''}
                          </button>
                        ) : <span className="result-occurrence-empty" aria-hidden="true" key={index} />)}
                      </div>
                      {additionalPassages.length > 0 && (
                        <div className="result-evidence-passages">
                          {additionalPassages.map((passage) => {
                            const passageExcerpt = usefulExcerpt(
                              passage.excerpt,
                              passage.sectionTitle ?? undefined,
                            )
                            return (
                              <button
                                type="button"
                                className="result-evidence-passage"
                                key={passage.sectionIndex}
                                disabled={disabled}
                                onClick={() => onViewResult(result, searchOpenTargetForPassage(passage))}
                              >
                                {passage.sectionTitle && (
                                  <span className="result-evidence-title" dir="auto">
                                    <bdi>{passage.sectionTitle}</bdi>
                                  </span>
                                )}
                                {passageExcerpt && (
                                  <span
                                    className="result-evidence-excerpt"
                                    dir="auto"
                                    dangerouslySetInnerHTML={{ __html: passageExcerpt }}
                                  />
                                )}
                              </button>
                            )
                          })}
                        </div>
                      )}
                    </div>
                  </details>
                )}
              </article>
            )
          })}
        </section>
      ))}

      {submittedQuery.length === 0 && (
        <div className="welcome">
          <p>{t('search.results.welcome')}</p>
          <p className="welcome-hint">{t('search.results.welcomeHint')}</p>
        </div>
      )}
    </div>
  )
}

function searchOpenTargetForPassage(passage: SearchPassage): SearchOpenTarget {
  return {
    text: firstMarkedText(passage.excerpt),
    sectionIndex: passage.sectionIndex,
    pageIndex: passage.pageIndex ?? undefined,
  }
}

function searchOpenTargetForResult(result: SearchResult, exactPhrase?: string): SearchOpenTarget | undefined {
  const hash = hashFromUrl(result.sub_results?.[0]?.url)
  const text = exactPhrase?.trim() || firstMarkedText(resultExcerpt(result, Boolean(exactPhrase)) ?? '')
  const sectionIndex = result.sectionIndex
  const pageIndex = result.pageIndex ?? undefined
  return hash || text || sectionIndex !== undefined || pageIndex !== undefined
    ? { hash, text, sectionIndex, pageIndex }
    : undefined
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
  if (count && count > 1 && !result.matchLocations?.length) {
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
