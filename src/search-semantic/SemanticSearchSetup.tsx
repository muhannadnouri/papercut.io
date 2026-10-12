import { useId, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import type { SemanticSearchController } from './useSemanticSearch'
import { semanticSearchStatus } from './semanticSearchStatus'

/** Explicit opt-in shares the cache installer; no download or indexing starts from a search. */
export function SemanticSearchSetup({ semantic, cachePresent, cacheBusy, children, indexRemoval }: {
  semantic: SemanticSearchController; cachePresent: boolean; cacheBusy: boolean; children?: ReactNode; indexRemoval?: ReactNode
}) {
  const { t } = useTranslation()
  const id = useId()
  const { enabled, provider, index, operation, error } = semantic
  const busy = semantic.busy || cacheBusy
  const active = enabled && provider?.activeFormat === semantic.format
  const indexing = index?.running || operation === 'build'
  const status = semanticSearchStatus(semantic)
  const current = status === 'ready'
  const showIndex = enabled || index?.hasStoredIndex || indexing || operation === 'remove' || !!index?.error
  const expandIndex = enabled || indexing || operation === 'remove' || !!index?.error
  return <>
    <section className="search-model-panel search-semantic-setup" aria-labelledby={`${id}-semantic-title`}>
      <h3 id={`${id}-semantic-title`}>{t('settings.semantic.controlsTitle')}</h3>
      <label className="app-setting app-setting-toggle">
        <span id={`${id}-enabled-label`}>{t('settings.semantic.enabled')}</span>
        <span className="app-settings-switch"><input type="checkbox" role="switch" checked={enabled}
          aria-labelledby={`${id}-semantic-title ${id}-enabled-label`}
          aria-describedby={`${id}-status${!cachePresent && !enabled ? ` ${id}-prerequisite` : ''}`}
          disabled={operation === 'disable' || (!enabled && (busy || !cachePresent || !provider?.compiled))}
          onChange={(event) => { void semantic.setEnabled(event.target.checked) }} /><span aria-hidden="true" /></span>
      </label>
      <p>{t('settings.searchModel.resourceSummary')}</p>
      <p id={`${id}-status`} role="status" aria-live="polite">{t(`settings.semantic.summary.${status}`)}</p>
      {enabled && !active && <button type="button" className="app-settings-data-action" disabled={busy || !cachePresent || !provider?.compiled}
        onClick={() => { void semantic.setEnabled(true) }}>{t('settings.semantic.retry')}</button>}
      {error && <p className="app-dialog-error" role="alert" dir="auto">{error}</p>}
      {!cachePresent && !enabled && <p id={`${id}-prerequisite`}>{t('settings.semantic.downloadFirst')}</p>}
    </section>
    {children}
    {showIndex && <details className="search-model-panel search-library-index" open={expandIndex}>
      <summary>{t('settings.semantic.libraryTitle')}
        {!enabled && !indexing && index?.hasStoredIndex && <span className="search-model-summary-status">{t('settings.semantic.savedIndex')}</span>}
      </summary>
      <div className="search-index-status">
        <p role="status">{t(!enabled && !indexing ? 'settings.semantic.enableFirst' : indexing ? 'settings.semantic.indexing' : !index ? 'settings.semantic.checkingIndex' : index.eligibleDocuments === 0
          ? 'settings.semantic.emptyLibrary' : !active ? 'settings.semantic.modelRequired'
          : current ? 'settings.semantic.indexReady' : 'settings.semantic.needsIndex')}</p>
        {enabled && index && index.eligibleDocuments > 0 && <p>{t('settings.semantic.documentsReady', { ready: index.indexedDocuments,
          total: index.eligibleDocuments })}</p>}
      </div>
      {indexing && index && <p role="status">{t('settings.semantic.progress', {
        embedded: index.embeddedPassages, reused: index.reusedPassages })}</p>}
      {(enabled || indexing) && !current && <div className="search-model-actions">
        <button type="button" className="app-settings-data-action search-action-primary" disabled={busy || !active || !index || index.eligibleDocuments === 0}
          onClick={() => { void semantic.build() }}>{t(index && index.eligibleDocuments > 0 && (index.indexedDocuments > 0 || index.passages > 0)
            ? 'settings.semantic.update' : 'settings.semantic.build')}</button>
        {indexing && <button type="button" className="app-settings-data-action"
          disabled={!index?.running || semantic.cancelling || index.cancelRequested}
          onClick={() => { void semantic.cancel() }}>{t('common.cancel')}</button>}
      </div>}
      {enabled && !current && index && index.eligibleDocuments > 0 && <p>{t('settings.semantic.indexHint')}</p>}
      <details className="search-index-details">
        <summary>{t('settings.semantic.details')}</summary>
        <div className="search-index-details-content">
          {active && index && <p>{t('settings.semantic.passages', { count: index.passages })}</p>}
          <button type="button" className="app-settings-data-action" disabled={busy}
            onClick={() => { void semantic.refresh() }}>{t('settings.semantic.refresh')}</button>
        </div>
      </details>
      {(semantic.cancelling || index?.cancelRequested) && busy && <p role="status">{t('settings.semantic.cancelling')}</p>}
      {index?.error && index.error !== error && <p className="app-dialog-error" role="alert" dir="auto">{index.error}</p>}
      {indexing && <p>{t('settings.semantic.closeHelp')}</p>}
      {index && !index.hasStoredIndex && !indexing && <p>{t('settings.semantic.noStoredIndex')}</p>}
      {indexRemoval}
    </details>}
  </>
}
