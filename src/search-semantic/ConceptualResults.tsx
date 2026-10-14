import { useId, useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { SearchOpenTarget } from '../types/search'
import { indexedSearchOpenTarget } from '../utils/searchOpenTarget'
import { contextPreview } from './contextPreview'
import type { ConceptualResult, ConceptualState } from './useConceptualSearch'
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
    {state.phase === 'loading' && <p role="status">{t('search.conceptual.loading')}</p>}
    {state.phase === 'fallback' && <p className="search-feedback" role="status">
      {t(`search.conceptual.${state.reason ?? 'failed'}`)}
    </p>}
    {state.response && state.phase === 'fallback' && state.reason === 'partial' && <p className="conceptual-results-coverage">
      {t('search.conceptual.coverage', { ready: state.response.indexedDocuments, total: state.response.eligibleDocuments })}
    </p>}
    {state.phase === 'ready' && results.length === 0 && <p role="status">{t('search.conceptual.empty')}</p>}
    {results.map(result => <ConceptualResultCard key={`${result.documentId}:${result.sectionIndex}:${result.byteStart}:${result.byteEnd}`}
      result={result} opening={result.url === openingDocumentUrl} openingDisabled={openingDisabled} onViewDocument={onViewDocument} />)}
  </section>
}

function ConceptualResultCard({ result, opening, openingDisabled, onViewDocument }: {
  result: ConceptualResult
  opening: boolean
  openingDisabled: boolean
  onViewDocument: (url: string, target: SearchOpenTarget) => void
}) {
  const { t } = useTranslation()
  const id = useId()
  const [expanded, setExpanded] = useState(false)
  // Show the match and its nearest source neighbour on each side, not the first three blocks.
  const passages = [result, ...result.context].sort((a, b) =>
    a.sectionIndex - b.sectionIndex || a.byteStart - b.byteStart)
  const match = passages.indexOf(result)
  const initial = passages.slice(Math.max(0, match - 1), match + 2)
  const visible = expanded ? passages : initial
  return <article className="result-card">
    <button type="button" className="result-card-primary" disabled={openingDisabled || opening}
      onClick={() => onViewDocument(result.url, indexedSearchOpenTarget(result))}>
      <span className="result-title" dir="auto"><bdi>{result.title}</bdi>{opening ? ` (${t('common.opening')})` : ''}</span>
      {result.sectionTitle && <span className="result-meta" dir="auto"><bdi>{result.sectionTitle}</bdi></span>}
      <span className="result-excerpt" dir="auto">{contextPreview(result.text, result.format)}</span>
    </button>
    <details className="result-explore conceptual-context">
      <summary>{t('search.conceptual.contextTitle')}</summary>
      <div className="result-evidence-content">
        <p className="conceptual-context-help">{t('search.conceptual.contextHelp')}</p>
        <div id={id} className="conceptual-context-previews">
          {visible.map((passage) => <div className={`conceptual-context-block${passage === result ? ' conceptual-context-block-matched' : ''}`}
            key={`${passage.sectionIndex}:${passage.byteStart}`}>
            <p className="result-meta">{t(passage === result
              ? 'search.conceptual.matchedPassage' : 'search.conceptual.nearbyText')}</p>
            <button type="button" className="result-card-primary" disabled={openingDisabled || opening}
              onClick={() => onViewDocument(result.url, indexedSearchOpenTarget(passage))}>
              {passage.sectionTitle && <span className="result-meta" dir="auto"><bdi>{passage.sectionTitle}</bdi></span>}
              <span className="conceptual-context-text" dir="auto">{passage !== result && passage.byteStart > 0 ? '… ' : ''}{contextPreview(passage.text, result.format)}</span>
              <span className="conceptual-context-open">{t('search.conceptual.openPassage')}</span>
            </button>
          </div>)}
        </div>
        {passages.length > initial.length && <button type="button" className="result-evidence-action"
          aria-expanded={expanded} aria-controls={id} onClick={() => setExpanded(value => !value)}>
          {t(expanded ? 'search.conceptual.showLess' : 'search.conceptual.showMore', { count: passages.length - initial.length })}
        </button>}
      </div>
    </details>
  </article>
}
