import { useId } from 'react'
import { useTranslation } from 'react-i18next'
import type { SemanticSearchController } from './useSemanticSearch'

/** Explicit opt-in shares the cache installer; no download or indexing starts from a search. */
export function SemanticSearchSetup({ semantic, cachePresent, cacheBusy }: {
  semantic: SemanticSearchController; cachePresent: boolean; cacheBusy: boolean
}) {
  const { t } = useTranslation()
  const id = useId()
  const { enabled, provider, index, operation, error } = semantic
  const busy = semantic.busy || cacheBusy
  const active = enabled && provider?.activeFormat === semantic.format
  const indexing = index?.running || operation === 'build'
  // Coverage belongs to the selected active encoder; old or failed jobs are not readiness.
  const current = active && !!index && index.eligibleDocuments > 0 &&
    index.indexedDocuments === index.eligibleDocuments && !indexing && !index.error && !error
  return <>
    <section className="search-model-panel search-semantic-setup" aria-labelledby={`${id}-semantic-title`}>
      <h3 id={`${id}-semantic-title`}>{t(active ? 'settings.semantic.controlsTitle' : 'settings.semantic.enableTitle')}</h3>
      <label className="app-setting app-setting-toggle">
        <span id={`${id}-enabled-label`}>{t('settings.semantic.enabled')}</span>
        <span className="app-settings-switch"><input type="checkbox" role="switch" checked={enabled}
          aria-labelledby={`${id}-semantic-title ${id}-enabled-label`}
          disabled={operation === 'disable' || (!enabled && (busy || !cachePresent || !provider?.compiled))}
          onChange={(event) => { void semantic.setEnabled(event.target.checked) }} /><span aria-hidden="true" /></span>
      </label>
      <p>{t('settings.searchModel.resourceSummary')}</p>
      <p role="status" aria-live="polite">{t(operation === 'disable' ? 'settings.semantic.disabling'
        : operation === 'activate' || provider?.loadingFormat ? 'settings.semantic.loading'
        : current ? 'settings.semantic.ready' : active ? 'settings.semantic.active' : enabled ? 'settings.semantic.fallback' : 'settings.semantic.off')}</p>
      {enabled && !active && <button type="button" className="app-settings-data-action" disabled={busy || !cachePresent || !provider?.compiled}
        onClick={() => { void semantic.setEnabled(true) }}>{t('settings.semantic.retry')}</button>}
      {error && <p className="app-dialog-error" role="alert" dir="auto">{error}</p>}
      {!cachePresent && !enabled && <p>{t('settings.semantic.downloadFirst')}</p>}
    </section>
    <section className="search-model-panel" aria-labelledby={`${id}-index-title`}>
      <h3 id={`${id}-index-title`}>{t(active ? 'settings.semantic.libraryTitle' : 'settings.semantic.indexTitle')}</h3>
      <p role="status">{t(indexing ? 'settings.semantic.indexing' : !index ? 'settings.semantic.checkingIndex' : index.eligibleDocuments === 0
        ? 'settings.semantic.emptyLibrary' : current ? 'settings.semantic.indexReady' : 'settings.semantic.needsIndex')}</p>
      {index && index.eligibleDocuments > 0 && <p>{t('settings.semantic.documentsReady', { ready: index.indexedDocuments,
        total: index.eligibleDocuments })}</p>}
      {indexing && index && <p role="status">{t('settings.semantic.progress', {
        embedded: index.embeddedPassages, reused: index.reusedPassages })}</p>}
      {!current && <div className="search-model-actions">
        <button type="button" className="app-settings-data-action search-action-primary" disabled={busy || !active || !index || index.eligibleDocuments === 0}
          onClick={() => { void semantic.build() }}>{t(index && index.eligibleDocuments > 0 && (index.indexedDocuments > 0 || index.passages > 0)
            ? 'settings.semantic.update' : 'settings.semantic.build')}</button>
        {indexing && <button type="button" className="app-settings-data-action"
          disabled={!index?.running || semantic.cancelling || index.cancelRequested}
          onClick={() => { void semantic.cancel() }}>{t('common.cancel')}</button>}
      </div>}
      {!current && index && index.eligibleDocuments > 0 && <p>{t('settings.semantic.indexHint')}</p>}
      <details className="search-index-details">
        <summary>{t('settings.semantic.details')}</summary>
        {index && <p>{t('settings.semantic.passages', { count: index.passages })}</p>}
        <button type="button" className="app-settings-data-action" disabled={busy}
          onClick={() => { void semantic.refresh() }}>{t('settings.semantic.refresh')}</button>
      </details>
      {(semantic.cancelling || index?.cancelRequested) && busy && <p role="status">{t('settings.semantic.cancelling')}</p>}
      {enabled && index?.error && index.error !== error && <p className="app-dialog-error" role="alert" dir="auto">{index.error}</p>}
      {!active && cachePresent && index && index.eligibleDocuments > 0 && <p>{t('settings.semantic.enableFirst')}</p>}
    </section>
  </>
}
