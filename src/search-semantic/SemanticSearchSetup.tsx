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
  return <>
    <section className="search-model-panel search-semantic-setup" aria-labelledby={`${id}-semantic-title`}>
      <h3 id={`${id}-semantic-title`}>{t('settings.semantic.enableTitle')}</h3>
      <label className="app-setting app-setting-toggle">
        <span>{t('settings.semantic.toggle')}</span>
        <span className="app-settings-switch"><input type="checkbox" role="switch" checked={enabled}
          disabled={operation === 'disable' || (!enabled && (busy || !cachePresent || !provider?.compiled))}
          onChange={(event) => { void semantic.setEnabled(event.target.checked) }} /><span aria-hidden="true" /></span>
      </label>
      <p role="status" aria-live="polite">{t(operation === 'disable' ? 'settings.semantic.disabling'
        : operation === 'activate' || provider?.loadingFormat ? 'settings.semantic.loading'
        : active ? 'settings.semantic.active' : enabled ? 'settings.semantic.fallback' : 'settings.semantic.off')}</p>
      {enabled && !active && <button type="button" className="app-settings-data-action" disabled={busy || !cachePresent || !provider?.compiled}
        onClick={() => { void semantic.setEnabled(true) }}>{t('settings.semantic.retry')}</button>}
      {error && <p className="app-dialog-error" role="alert" dir="auto">{error}</p>}
      {!cachePresent && !enabled && <p>{t('settings.semantic.downloadFirst')}</p>}
    </section>
    <section className="search-model-panel" aria-labelledby={`${id}-index-title`}>
      <h3 id={`${id}-index-title`}>{t('settings.semantic.indexTitle')}</h3>
      <p role="status">{t(index?.running || operation === 'build' ? 'settings.semantic.indexing' : !index ? 'settings.semantic.checkingIndex' : index.eligibleDocuments === 0
        ? 'settings.semantic.emptyLibrary' : index.indexedDocuments === index.eligibleDocuments
          ? 'settings.semantic.indexReady' : 'settings.semantic.needsIndex')}</p>
      {index && index.eligibleDocuments > 0 && <p>{t('settings.semantic.coverage', { ready: index.indexedDocuments,
        total: index.eligibleDocuments, passages: index.passages })}</p>}
      {(index?.running || operation === 'build') && index && <p role="status">{t('settings.semantic.progress', {
        embedded: index.embeddedPassages, reused: index.reusedPassages })}</p>}
      <div className="search-model-actions">
        <button type="button" className="app-settings-data-action" disabled={busy || !active || !index || index.eligibleDocuments === 0}
          onClick={() => { void semantic.build() }}>{t('settings.semantic.build')}</button>
        {(index?.running || operation === 'build') && <button type="button" className="app-settings-data-action"
          disabled={!index?.running || semantic.cancelling || index.cancelRequested}
          onClick={() => { void semantic.cancel() }}>{t('common.cancel')}</button>}
        <button type="button" className="app-settings-data-action" disabled={busy}
          onClick={() => { void semantic.refresh() }}>{t('settings.semantic.refresh')}</button>
      </div>
      {(semantic.cancelling || index?.cancelRequested) && busy && <p role="status">{t('settings.semantic.cancelling')}</p>}
      {enabled && index?.error && index.error !== error && <p className="app-dialog-error" role="alert" dir="auto">{index.error}</p>}
      {!active && cachePresent && index && index.eligibleDocuments > 0 && <p>{t('settings.semantic.enableFirst')}</p>}
    </section>
  </>
}
