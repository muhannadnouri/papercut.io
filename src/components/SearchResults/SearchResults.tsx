import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { TFunction } from 'i18next'
import type { LastSearchInfo, SearchPhase } from '../../hooks/useSearch'
import type { SearchOpenTarget, SearchPassage, SearchResult } from '../../types/search'
import {
  findUploadedDocumentOccurrences,
  type UploadedDocumentConcordanceEntry,
} from '../../uploads/DocumentUploads'
import { indexedSearchOpenTarget } from '../../utils/searchOpenTarget'
import { sanitizeMarkedExcerpt } from '../../utils/textUtils'
import './SearchResults.css'

const OCCURRENCE_MAP_BINS = 12
const SEARCH_PHASE_KEYS = {
  indexes: 'search.results.searchingDocuments',
  candidates: 'search.results.findingCandidates',
  evidence: 'search.results.buildingEvidence',
  results: 'search.results.loadingRankedResults',
  phrases: 'search.results.verifyingExactPhrases',
  excerpts: 'search.results.preparingExcerpts',
} as const

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
  const [concordance, setConcordance] = useState<{
    key: string
    entries: UploadedDocumentConcordanceEntry[]
    totalMatches: number
    nextOffset?: number | null
    loading: boolean
    failed: boolean
  } | null>(null)
  const filtered = scopeActive
    ? results.filter((r) => scopeUrls.has(r.url))
    : results
  const hasFilters = scopeActive
  const visibleCount = filtered.length
  const loadConcordance = async (result: SearchResult, term: string, offset = 0) => {
    const key = `${result.url}\0${term}`
    setConcordance((current) => ({
      key,
      entries: offset > 0 && current?.key === key ? current.entries : [],
      totalMatches: current?.key === key ? current.totalMatches : 0,
      nextOffset: current?.key === key ? current.nextOffset : undefined,
      loading: true,
      failed: false,
    }))
    try {
      const response = await findUploadedDocumentOccurrences(result.url, term, offset)
      setConcordance((current) => current?.key === key ? {
        key,
        entries: [
          ...(offset > 0 ? current.entries : []),
          ...response.entries.map((entry) => ({
            ...entry,
            excerpt: sanitizeMarkedExcerpt(entry.excerpt),
          })),
        ],
        totalMatches: response.totalMatches,
        nextOffset: response.nextOffset,
        loading: false,
        failed: false,
      } : current)
    } catch {
      setConcordance((current) => current?.key === key
        ? { ...current, loading: false, failed: true }
        : current)
    }
  }
  const resultGroups = [
    {
      key: 'upload',
      title: t('search.results.yourLibrary'),
      results: filtered.filter((result) => result.source === 'upload'),
      total: lastSearchInfo?.uploadedDocuments ?? 0,
    },
    {
      key: 'starter',
      title: t('search.results.starterDocuments'),
      results: filtered.filter((result) => result.source !== 'upload'),
      total: lastSearchInfo?.starterDocuments ?? 0,
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
        <p className="no-results search-feedback search-feedback-error" role="alert">
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
          <QuerySummary info={lastSearchInfo} t={t} />
        </div>
      )}

      {!searchFailed && submittedQuery.length > 0 && filtered.length === 0 && (
        <p className="no-results search-feedback" role="status">
          <span>{t('search.results.noResults')}</span>
          {hasFilters && <> <span>{t('search.results.filtersApplied')}</span></>}
          {lastSearchInfo && <> {' '}<QuerySummary info={lastSearchInfo} t={t} /></>}
          {' '}
          <span>{t('search.results.noResultsHint')}</span>
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
            const hasEvidence = !exactPhrase
              && result.source === 'upload'
              && result.matchingSections !== undefined
              && result.matchingSections > 1
              && (additionalPassages.length > 0 || locations.length > 1)
            const terms = concordanceTerms(result, lastSearchInfo)
            const activeTerm = terms.find((term) => concordance?.key === `${result.url}\0${term}`)
            const activeConcordance = activeTerm ? concordance : null
            const hasConcordance = result.source === 'upload'
              && terms.length > 0
              && (!exactPhrase || terms.length > 1 || result.matchCount !== 1)
            const hasExplore = hasEvidence || hasConcordance
            return (
              <article
                key={result.id}
                className={'result-card'
                  + (disabled ? ' result-card-disabled' : '')}
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
                  {(result.termMatches?.length ?? 0) > 1 && (
                    <span className="result-term-coverage" dir="auto">
                      <span>{t('search.results.sectionsByTerm')}</span>
                      {result.termMatches?.map((match) => (
                        <span className="result-term-count" key={match.term}>
                          <bdi>{match.term}</bdi> {match.matchingSections}
                        </span>
                      ))}
                    </span>
                  )}
                  {excerpt && (
                    <span
                      className="result-excerpt"
                      dir="auto"
                      dangerouslySetInnerHTML={{ __html: excerpt }}
                    />
                  )}
                </button>
                {hasExplore && (
                  <details
                    className="result-explore"
                    onToggle={(event) => {
                      if (
                        event.currentTarget.open
                        && exactPhrase
                        && terms.length === 1
                        && !activeConcordance
                      ) {
                        void loadConcordance(result, terms[0])
                      }
                    }}
                  >
                    <summary>
                      {t('search.results.exploreMatches')}
                    </summary>
                    <div className="result-evidence-content">
                      {hasEvidence && (
                        <>
                          <span className="result-evidence-title">
                            {supportingSectionsLabel(result, result.matchingSections ?? 0, t)}
                          </span>
                          <div
                            className="result-occurrence-map"
                            role="group"
                            aria-label={supportingSectionsLabel(result, result.matchingSections ?? 0, t)}
                          >
                            {evidenceCells.map(({ index, location }) => location ? (
                              <button
                                type="button"
                                className="result-occurrence-marker"
                                key={index}
                                disabled={disabled}
                                aria-label={t('search.results.sectionMapPosition', {
                                  position: index + 1,
                                  total: OCCURRENCE_MAP_BINS,
                                  count: location.matchCount,
                                })}
                                title={t('search.results.sectionMapPosition', {
                                  position: index + 1,
                                  total: OCCURRENCE_MAP_BINS,
                                  count: location.matchCount,
                                })}
                                onClick={() => onViewResult(result, indexedSearchOpenTarget(location))}
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
                        </>
                      )}
                      {hasConcordance && (terms.length > 1 || !exactPhrase) && (
                        <div className="result-concordance-terms">
                          {terms.map((term) => (
                            <button
                              type="button"
                              className="result-concordance-term"
                              aria-pressed={activeTerm === term}
                              key={term}
                              onClick={() => void loadConcordance(result, term)}
                            >
                              {t('search.results.occurrencesFor', { term })}
                            </button>
                          ))}
                        </div>
                      )}
                      {activeConcordance && activeTerm && (
                        <>
                          {activeConcordance.loading && activeConcordance.entries.length === 0 && (
                            <span className="result-evidence-feedback" role="status">
                              {t('search.results.loadingOccurrences')}
                            </span>
                          )}
                          {activeConcordance.failed && (
                            <div
                              className="result-evidence-feedback result-evidence-feedback-error"
                              role="alert"
                            >
                              <span>
                                {t('search.results.occurrencesFailed')}
                              </span>
                              <button
                                type="button"
                                className="result-evidence-action"
                                onClick={() => void loadConcordance(
                                  result,
                                  activeTerm,
                                  activeConcordance.nextOffset ?? 0,
                                )}
                              >
                                {t('search.results.retryOccurrences')}
                              </button>
                            </div>
                          )}
                          {!activeConcordance.failed && activeConcordance.totalMatches > 0 && (
                            <span className="result-evidence-title">
                              {t('search.results.occurrenceCount', {
                                shown: activeConcordance.entries.length,
                                total: activeConcordance.totalMatches,
                              })}
                            </span>
                          )}
                          {!activeConcordance.loading
                            && !activeConcordance.failed
                            && activeConcordance.totalMatches === 0 && (
                            <span className="result-evidence-feedback" role="status">
                              {t('search.results.noLiteralOccurrences')}
                            </span>
                          )}
                          {activeConcordance.entries.length > 0 && (
                            <div className="result-evidence-passages">
                              {activeConcordance.entries.map((entry) => (
                                <button
                                  type="button"
                                  className="result-evidence-passage"
                                  key={entry.occurrenceIndex}
                                  disabled={disabled}
                                  onClick={() => onViewResult(result, indexedSearchOpenTarget({
                                    ...entry,
                                    occurrenceIndex: entry.sectionOccurrenceIndex,
                                  }, firstMarkedText(entry.excerpt) ?? activeTerm))}
                                >
                                  {entry.sectionTitle && (
                                    <span className="result-evidence-title" dir="auto">
                                      <bdi>{entry.sectionTitle}</bdi>
                                    </span>
                                  )}
                                  <span
                                    className="result-evidence-excerpt"
                                    dir="auto"
                                    dangerouslySetInnerHTML={{ __html: entry.excerpt }}
                                  />
                                </button>
                              ))}
                            </div>
                          )}
                          {activeConcordance.nextOffset !== null
                            && activeConcordance.nextOffset !== undefined && (
                            <button
                              type="button"
                              className="result-evidence-action"
                              disabled={activeConcordance.loading}
                              onClick={() => void loadConcordance(
                                result,
                                activeTerm,
                                activeConcordance.nextOffset ?? 0,
                              )}
                            >
                              {activeConcordance.loading
                                ? t('search.results.loadingOccurrences')
                                : t('search.results.showMoreOccurrences')}
                            </button>
                          )}
                        </>
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

function QuerySummary({ info, t }: { info: LastSearchInfo; t: TFunction }) {
  return (
    <>
      {info.unquotedText && (
        <>
          {t('search.results.allWords')} {' '}
          <bdi className="query-tag">{info.unquotedText}</bdi>
        </>
      )}
      {info.unquotedText && info.phrases.length > 0 && <> · </>}
      {info.phrases.length > 0 && (
        <>
          {t(info.phrases.length === 1 ? 'search.results.exactPhrase' : 'search.results.exactPhrases')}
          {' '}
          {info.phrases.map((phrase, index) => (
            <bdi key={index} className="query-tag">&ldquo;{phrase}&rdquo;</bdi>
          ))}
        </>
      )}
    </>
  )
}

function searchOpenTargetForPassage(passage: SearchPassage): SearchOpenTarget {
  return {
    text: firstMarkedText(passage.excerpt),
    sectionIndex: passage.sectionIndex,
    pageIndex: passage.pageIndex ?? undefined,
  }
}

function concordanceTerms(result: SearchResult, info: LastSearchInfo | null): string[] {
  const phrases = info?.phrases.map((phrase) => phrase.trim()).filter(Boolean) ?? []
  if (phrases.length > 0) return [...new Set(phrases)]
  const terms = result.termMatches
    ?.map((match) => match.text?.trim() || match.term.trim())
    .filter(Boolean) ?? []
  const fallback = firstMarkedText(result.excerpt)
  return terms.length > 0 ? [...new Set(terms)] : fallback ? [fallback] : []
}

function supportingSectionsLabel(result: SearchResult, count: number, t: TFunction): string {
  return t(
    result.matchScope === 'document'
      ? 'search.results.sectionsWithAnyTerm'
      : 'search.results.matchingSections',
    { count },
  )
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
