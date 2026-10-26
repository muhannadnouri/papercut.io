import {
  useId,
  useState,
} from 'react'
import { useTranslation } from 'react-i18next'
import { SettingsIcon } from '../components/SettingsIcon/SettingsIcon'
import { WarningIcon } from '../components/WarningIcon/WarningIcon'
import type { semanticSearchStatus } from './semanticSearchStatus'
import type { SearchOpenTarget } from '../types/search'
import { indexedSearchOpenTarget } from '../utils/searchOpenTarget'
import { contextPreview } from './contextPreview'
import type {
  ConceptualResult,
  ConceptualState,
} from './useConceptualSearch'
import '../components/SearchResults/SearchResults.css'
import './ConceptualResults.css'

/** Render authored passage text as text, with no synthetic query marks or literal counts.
 * UTF-8 byte spans stay native metadata; reader navigation uses existing section/page/text targets. */
export function ConceptualResults({ visibleLimit = Infinity, onShowMore, state, setupStatus, hasStoredIndex = false, onOpenSearchSettings, openingDisabled = false, openingDocumentUrl, onViewDocument }: {
  visibleLimit?: number
  onShowMore?: () => void
  state: ConceptualState
  setupStatus?: ReturnType<typeof semanticSearchStatus> | 'downloading'
  hasStoredIndex?: boolean
  onOpenSearchSettings?: () => void
  openingDisabled?: boolean
  openingDocumentUrl?: string
  onViewDocument: (url: string, target: SearchOpenTarget) => void
}) {
  const { t } = useTranslation()
  const id = useId()
  if (!state) {
    if (!setupStatus) {
      return null
    }
    const needsIndex = setupStatus === 'needsIndex'
    const preparing = ['loading', 'downloading', 'indexing', 'cancelling', 'checking', 'disabling'].includes(setupStatus)
    const message = ({
      ready: 'search.conceptual.welcome',
      needsIndex: hasStoredIndex ? 'search.conceptual.indexUpdateRequired' : 'settings.semantic.needsIndex',
      empty: 'search.conceptual.emptyLibrary',
      loading: 'settings.searchModel.loadingModel',
      downloading: 'settings.semantic.downloading',
      indexing: 'settings.semantic.indexing',
      cancelling: 'settings.semantic.cancelling',
      checking: 'settings.semantic.checkingIndex',
      disabling: 'settings.semantic.disabling',
      statusUnavailable: 'settings.semantic.statusUnavailable',
      fallback: 'search.conceptual.setupUnavailable',
      off: 'search.conceptual.setupUnavailable',
    } as const)[setupStatus]
    return <div className="welcome">
      <p role="status" className={needsIndex ? 'conceptual-index-warning' : preparing ? 'conceptual-setup-phase' : undefined}>
        {needsIndex && <WarningIcon />}{preparing && <span className="spinner" aria-hidden="true" />}{t(message)}
      </p>
      {onOpenSearchSettings && (needsIndex || setupStatus === 'fallback' || setupStatus === 'statusUnavailable') &&
        <button type="button" className="result-evidence-action conceptual-settings-action" aria-haspopup="dialog" onClick={onOpenSearchSettings}><SettingsIcon />{t('search.conceptual.openSettings')}</button>}
    </div>
  }
  const indexWarning = state.phase === 'fallback' && ['missing-index', 'incompatible-index', 'partial'].includes(state.reason ?? '')
  const results = state.phase === 'ready' ? state.response?.results ?? [] : []
  return <section className="results-container result-group conceptual-results" aria-labelledby={id}
    aria-busy={state.phase === 'loading'}>
    <div className="result-group-heading">
      <h2 id={id}>{t('search.conceptual.title')}</h2>
      {state.phase === 'ready' && <span>{t('search.conceptual.shown', { count: results.length })}</span>}
    </div>
    {state.phase === 'loading' && <p role="status">{t('search.conceptual.loading')}</p>}
    {state.phase === 'fallback' && <p className={`search-feedback${indexWarning ? ' conceptual-index-warning' : ''}`} role="status">
      {indexWarning && <WarningIcon />}{t(`search.conceptual.${state.reason ?? 'failed'}`)}
    </p>}
    {indexWarning && onOpenSearchSettings && <button type="button" className="result-evidence-action conceptual-settings-action" aria-haspopup="dialog"
      onClick={onOpenSearchSettings}><SettingsIcon />{t('search.conceptual.openSettings')}</button>}
    {state.response && state.phase === 'fallback' && state.reason === 'partial' && <p className="conceptual-results-coverage">
      {t('search.conceptual.coverage', {
        ready: state.response.indexedDocuments,
        total: state.response.eligibleDocuments,
      })}
    </p>}
    {state.phase === 'ready' && results.length === 0 && <p role="status">{t('search.conceptual.empty')}</p>}
    {results.slice(0, visibleLimit).map(result => <ConceptualResultCard key={`${result.documentId}:${result.sectionIndex}:${result.byteStart}:${result.byteEnd}`}
      result={result} opening={result.url === openingDocumentUrl} openingDisabled={openingDisabled} onViewDocument={onViewDocument} />)}
    {onShowMore && results.length > 10 && <div className="result-list-more">
      <span role="status">{t('search.results.documentsShown', {
        shown: Math.min(visibleLimit, results.length),
        total: results.length,
      })}</span>
      <button type="button" className="result-evidence-action conceptual-more-action" aria-disabled={visibleLimit >= results.length}
        onClick={() => {
          if (visibleLimit < results.length) {
            onShowMore()
          }
        }}>
        <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false"><path d="m6 9 6 6 6-6" /></svg>
        {t('search.results.showMore')}
      </button>
    </div>}
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
            <h3 className="conceptual-context-label">{t(passage === result
              ? 'search.conceptual.matchedPassage' : 'search.conceptual.nearbyText')}</h3>
            <button type="button" className="result-card-primary" disabled={openingDisabled || opening}
              onClick={() => onViewDocument(result.url, indexedSearchOpenTarget(passage))}>
              {passage.sectionTitle && <span className="result-meta" dir="auto"><bdi>{passage.sectionTitle}</bdi></span>}
              <span className="conceptual-context-text" dir="auto">{passage !== result && passage.byteStart > 0 ? '… ' : ''}{contextPreview(passage.text, result.format)}</span>
              <span className="conceptual-context-open">{t('search.conceptual.openPassage')}</span>
            </button>
          </div>)}
        </div>
        {passages.length > initial.length && <button type="button" className="result-evidence-action conceptual-more-action"
          aria-expanded={expanded} aria-controls={id} onClick={() => setExpanded(value => !value)}>
          <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
            <path d={expanded ? 'm6 15 6-6 6 6' : 'm6 9 6 6 6-6'} />
          </svg>
          {t(expanded ? 'search.conceptual.showLess' : 'search.conceptual.showMore', { count: passages.length - initial.length })}
        </button>}
      </div>
    </details>
  </article>
}
