import { useId, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import type { SemanticSearchController } from './useSemanticSearch'
import { WarningIcon } from '../components/WarningIcon/WarningIcon'
import { semanticSearchStatus } from './semanticSearchStatus'
import { isIndexingCancelled } from './indexingOutcome'

/** Explicit opt-in shares the cache installer; no download or indexing starts from a search. */
export function SemanticSearchSetup({ semantic, cachePresent, cacheBusy, children, indexMaintenance }: {
  semantic: SemanticSearchController; cachePresent: boolean; cacheBusy: boolean; children?: ReactNode; indexMaintenance?: (actions: ReactNode) => ReactNode
}) {
  const { t } = useTranslation()
  const id = useId()
  const { enabled, provider, index, operation, error, statusError } = semantic
  const actionError = isIndexingCancelled(error) ? '' : error
  const indexError = isIndexingCancelled(index?.error) ? null : index?.error
  const cancelled = isIndexingCancelled(error) || isIndexingCancelled(index?.error)
  const busy = semantic.busy || cacheBusy
  const active = enabled && provider?.activeFormat === semantic.format
  const indexing = index?.running || operation === 'build'
  const status = semanticSearchStatus(semantic)
  const current = status === 'ready'
  const cancelling = indexing && (semantic.cancelling || index?.cancelRequested)
  const needsIndex = active && !busy && !!index && status === 'needsIndex'
  const description = [`${id}-status`, !cachePresent && !enabled && `${id}-prerequisite`].filter(Boolean).join(' ')
  const showIndex = enabled || index?.hasStoredIndex || indexing || operation === 'remove' || !!index?.error
  const expandIndex = enabled || indexing || operation === 'remove' || !!index?.error
  const indexActions = ((enabled && !statusError) || indexing) && !current && <>
    {!indexing && <button type="button" className="app-settings-data-action search-action-primary" disabled={busy || !active || !index || index.eligibleDocuments === 0}
      onClick={() => { void semantic.build() }}>{t(index && index.eligibleDocuments > 0 && (index.indexedDocuments > 0 || index.passages > 0)
        ? 'settings.semantic.update' : 'settings.semantic.build')}</button>}
    {indexing && <button type="button" className="app-settings-data-action"
      disabled={!index?.running || semantic.cancelling || index.cancelRequested}
      onClick={() => { void semantic.cancel() }}>{t('settings.semantic.stop')}</button>}
  </>
  return <>
    <section className="search-model-panel search-semantic-setup" aria-labelledby={`${id}-semantic-title`}>
      <h3 id={`${id}-semantic-title`}>{t('settings.semantic.controlsTitle')}</h3>
      <label className="app-setting app-setting-toggle">
        <span id={`${id}-status`} role="status" aria-live="polite">{t(`settings.semantic.summary.${status}`)}</span>
        <span className="app-switch"><input type="checkbox" role="switch" checked={enabled}
          aria-labelledby={`${id}-semantic-title`}
          aria-describedby={description}
          disabled={operation === 'disable' || (!enabled && (busy || !!statusError || !cachePresent || !provider?.compiled))}
          onChange={(event) => { void semantic.setEnabled(event.target.checked) }} /><span aria-hidden="true" /></span>
      </label>
      {enabled && !active && !statusError && <button type="button" className="app-settings-data-action" disabled={busy || !cachePresent || !provider?.compiled}
        onClick={() => { void semantic.setEnabled(true) }}>{t('settings.semantic.retry')}</button>}
      {actionError && <p className="app-dialog-error" role="alert" dir="auto">{actionError}</p>}
      {statusError && <div className="search-status-recovery">
        <p className="app-dialog-error" role="alert" dir="auto">{statusError}</p>
        <button type="button" className="app-settings-data-action" onClick={async (event) => {
          const button = event.currentTarget
          const toggle = button.closest('.search-semantic-setup')?.querySelector<HTMLInputElement>('input[role="switch"]')
          await semantic.refresh()
          requestAnimationFrame(() => {
            if (!button.isConnected && button.ownerDocument.activeElement === button.ownerDocument.body && toggle?.isConnected && !toggle.disabled) toggle.focus()
          })
        }}>{t('settings.semantic.retryStatus')}</button>
      </div>}
      {!cachePresent && !enabled && <p id={`${id}-prerequisite`} className="search-model-attention"><WarningIcon />{t('settings.semantic.downloadFirst')}</p>}
    </section>
    {children}
    {showIndex && <details className="search-model-panel search-library-index" open={expandIndex}>
      <summary>
        {t('settings.semantic.libraryTitle')}
        {indexing && <span className="search-index-phase" role="status" aria-live="polite">
          <span className="spinner search-index-spinner" aria-hidden="true" />
          {t(cancelling ? 'settings.semantic.cancelling' : 'settings.semantic.indexing')}
        </span>}
        {!enabled && !indexing && index?.hasStoredIndex && <span className="search-model-summary-status">{t('settings.semantic.savedIndex')}</span>}
      </summary>
      <div className="search-index-status">
        {indexing && !statusError && index && <p>{t('settings.semantic.progress', { embedded: index.embeddedPassages })}
          {index.reusedPassages > 0 && <> · {t('settings.semantic.reusedProgress', { reused: index.reusedPassages })}</>}</p>}
        {!indexing && <p role="status" className={needsIndex ? 'search-index-attention' : undefined}>
          {needsIndex && <WarningIcon />}
          {t(statusError ? 'settings.semantic.statusUnavailable' : !enabled ? 'settings.semantic.enableFirst' : !index ? 'settings.semantic.checkingIndex' : index.eligibleDocuments === 0
          ? 'settings.semantic.emptyLibrary' : !active ? 'settings.semantic.modelRequired'
          : current ? 'settings.semantic.indexReady' : 'settings.semantic.needsIndex')}</p>}
        {enabled && !statusError && index && index.eligibleDocuments > 0 && <p>{t('settings.semantic.documentsIndexed', { ready: index.indexedDocuments,
          total: index.eligibleDocuments })}</p>}
      </div>
      {!indexing && cancelled && !actionError && !indexError && !statusError && <p role="status">{t('settings.semantic.activity.cancelled')}</p>}
      {needsIndex && <p className="search-index-help">{t(index.indexedDocuments > 0 ? 'settings.semantic.updateHelp' : 'settings.semantic.buildHelp')}</p>}
      {needsIndex && (index.indexedDocuments > 0 || index.passages > 0) && <p className="search-index-help">{t('settings.semantic.indexHint')}</p>}
      {!indexing && !statusError && active && index && index.passages > 0 && <p className="search-index-passages">{t('settings.semantic.passages', { count: index.passages })}</p>}
      {indexError && indexError !== actionError && <p className="app-dialog-error" role="alert" dir="auto">{indexError}</p>}
      {indexMaintenance ? indexMaintenance(indexActions) : <div className="search-model-actions">{indexActions}</div>}
    </details>}
  </>
}
