import { useId } from 'react'
import { useTranslation } from 'react-i18next'
import type { SearchOpenTarget } from '../types/search'
import { indexedSearchOpenTarget } from '../utils/searchOpenTarget'
import type { ConceptualState } from './useConceptualSearch'
import '../components/SearchResults/SearchResults.css'
import './ConceptualResults.css'

/** Render authored passage text as text, with no synthetic query marks or literal counts.
 * UTF-8 byte spans stay native metadata; reader navigation uses existing section/page/text targets. */
export function ConceptualResults({ state, openingDisabled = false, openingDocumentUrl, onViewDocument }: {
  state: ConceptualState
  openingDisabled?: boolean
  openingDocumentUrl?: string
  onViewDocument: (url: string, target: SearchOpenTarget) => void
}) {
  const { t } = useTranslation()
  const id = useId()
  if (!state) return null
  const results = state.phase === 'ready' ? state.response?.results ?? [] : []
  return <section className="results-container result-group conceptual-results" aria-labelledby={id}
    aria-busy={state.phase === 'loading'}>
    <div className="result-group-heading">
      <h2 id={id}>{t('search.conceptual.title')}</h2>
      {state.phase === 'ready' && <span>{t('search.conceptual.shown', { count: results.length })}</span>}
    </div>
    <div className="conceptual-results-help">
      <p className="conceptual-results-description">{t('search.conceptual.description')}</p>
      <details className="conceptual-results-details">
        <summary>{t('search.conceptual.matchingTitle')}</summary>
        <p>{t('search.conceptual.matchingDetails')}</p>
      </details>
    </div>
    {state.phase === 'loading' && <p role="status">{t('search.conceptual.loading')}</p>}
    {state.phase === 'fallback' && <p className="search-feedback" role="status">
      {t(`search.conceptual.${state.reason ?? 'failed'}`)}
    </p>}
    {state.response && state.phase === 'fallback' && state.reason === 'partial' && <p className="conceptual-results-coverage">
      {t('search.conceptual.coverage', { ready: state.response.indexedDocuments, total: state.response.eligibleDocuments })}
    </p>}
    {state.phase === 'ready' && results.length === 0 && <p role="status">{t('search.conceptual.empty')}</p>}
    {results.map((result) => {
      const opening = result.url === openingDocumentUrl
      return <article className="result-card" key={result.documentId}>
        <button type="button" className="result-card-primary" disabled={openingDisabled || opening}
          onClick={() => onViewDocument(result.url, indexedSearchOpenTarget(result))}>
          <span className="result-title" dir="auto"><bdi>{result.title}</bdi>{opening ? ` (${t('common.opening')})` : ''}</span>
          {result.sectionTitle && <span className="result-meta" dir="auto"><bdi>{result.sectionTitle}</bdi></span>}
          <span className="result-excerpt" dir="auto">{result.text}</span>
        </button>
      </article>
    })}
  </section>
}
