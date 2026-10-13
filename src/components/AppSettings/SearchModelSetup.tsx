import { useCallback, useEffect, useId, useRef, useState, type ReactNode } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useTranslation } from 'react-i18next'
import { formatStorageSize } from '../../utils/formatUtils'
import { SemanticSearchSetup } from '../../search-semantic/SemanticSearchSetup'
import type { ModelSelection, ProviderStatus, SemanticSearchController } from '../../search-semantic/useSemanticSearch'
import './SearchModelSetup.css'

interface CacheStatus {
  format: ModelSelection
  filesPresent: boolean
  hasStoredFiles: boolean
  expectedBytes: number
}
interface DownloadProgress {
  format: ModelSelection
  phase: 'starting' | 'downloading' | 'installed' | 'cancelled' | 'failed'
  downloadedBytes: number
  totalBytes: number
}

/** Cache preparation never activates a model; the separate opt-in controls own activation. */
export function SearchModelSetup({ semantic }: { semantic?: SemanticSearchController } = {}) {
  const { t } = useTranslation()
  const id = useId()
  const [snapshot, setSnapshot] = useState<{ status: ProviderStatus; caches: CacheStatus[] } | null>(null)
  const [ready, setReady] = useState(false)
  const [localFormat, setLocalFormat] = useState<ModelSelection>('float32')
  const format = semantic?.format ?? localFormat
  const [consent, setConsent] = useState(false)
  const [operation, setOperation] = useState<'download' | 'remove' | null>(null)
  const [progress, setProgress] = useState<DownloadProgress | null>(null)
  const [error, setError] = useState('')
  const [message, setMessage] = useState<'' | 'verified' | 'removed'>('')
  const [connecting, setConnecting] = useState(0)
  const mounted = useRef(false)
  const request = useRef(0)
  const action = useRef(false)
  const [removal, setRemoval] = useState<'model' | 'index' | null>(null)
  const [cancelling, setCancelling] = useState(false)
  const refreshPending = useRef<Promise<void> | null>(null)
  const setup = useRef<HTMLElement>(null)

  const refresh = useCallback(async () => {
    if (refreshPending.current) return refreshPending.current
    const pending = (async () => {
      const current = ++request.current
      const status = await invoke<ProviderStatus>('search_embedding_status')
      const caches = status.compiled
        ? await invoke<CacheStatus[]>('search_embedding_cache_status') : []
      if (mounted.current && current === request.current) setSnapshot({ status, caches })
    })()
    refreshPending.current = pending
    try { await pending } finally { refreshPending.current = null }
  }, [])

  useEffect(() => {
    mounted.current = true
    let closed = false
    let unlisten: (() => void) | undefined
    // Subscribe before reading status so opening settings cannot miss completion.
    void listen<DownloadProgress>('search-embedding-download-progress', ({ payload }) => {
      if (closed) return
      setProgress(payload)
      if (payload.phase !== 'downloading') void refresh().catch((err: unknown) => {
        if (!closed) setError(String(err))
      })
    }).then(async (stop) => {
      if (closed) { stop(); return }
      unlisten = stop
      await refresh()
      if (!closed) setReady(true)
    }).catch((err: unknown) => {
      if (!closed) setError(String(err))
    })
    return () => {
      closed = true
      mounted.current = false
      unlisten?.()
    }
  }, [connecting, refresh])

  const status = snapshot?.status
  const busy = operation !== null || !!status?.downloadingFormat || !!status?.loadingFormat || !!semantic?.busy
  const cache = snapshot?.caches.find((item) => item.format === format)
  const semanticActiveFormat = semantic?.provider?.activeFormat
  const semanticBusy = semantic?.busy

  useEffect(() => {
    if (!ready || semanticBusy === undefined || semanticBusy) return
    // Fast native operations can finish before the poll fires; refresh cache controls too.
    void refresh().catch((err: unknown) => { if (mounted.current) setError(String(err)) })
  }, [ready, semanticBusy, semanticActiveFormat, refresh])

  useEffect(() => {
    if (!busy || !ready) return
    // ponytail: poll only while work is pending; a job history is unnecessary
    // until cache operations must survive more than one app process.
    const timer = window.setInterval(() => {
      void refresh().catch((err: unknown) => {
        if (mounted.current) setError(String(err))
      })
    }, 1000)
    return () => window.clearInterval(timer)
  }, [busy, ready, refresh])

  async function run(kind: 'download' | 'remove') {
    if (action.current || !ready || !status?.compiled || busy || !cache || semantic?.enabled) return
    if (kind === 'download' && !consent) return
    if (kind === 'remove' && (!cache.hasStoredFiles || status.activeFormat === format)) return
    action.current = true
    setOperation(kind)
    setProgress(null)
    setMessage('')
    setError('')
    setCancelling(false)
    try {
      await invoke(kind === 'download' ? 'search_embedding_download' : 'search_embedding_remove', { format })
      if (mounted.current) setMessage(kind === 'download' ? 'verified' : 'removed')
    } catch (err) {
      if (mounted.current) setError(String(err))
    } finally {
      try { await refresh() } catch (err) {
        if (mounted.current) setError(String(err))
      }
      action.current = false
      if (mounted.current) {
        setOperation(null)
        setCancelling(false)
        setConsent(false)
      }
    }
  }

  async function cancel() {
    if (cancelling || !status?.downloadingFormat) return
    setCancelling(true)
    setError('')
    try {
      await invoke('search_embedding_cancel_download')
      await refresh()
    } catch (err) {
      if (mounted.current) { setError(String(err)); setCancelling(false) }
    }
  }

  const percent = progress && progress.totalBytes > 0
    ? Math.min(100, Math.max(0, Math.floor(progress.downloadedBytes / progress.totalBytes * 100))) : null
  const downloading = status?.downloadingFormat
  const matchingProgress = progress?.format === downloading ? progress : null
  const preparingFiles = operation === 'download' || !!downloading
  // Byte events do not report native verification separately; retain an honest pending label.
  const workLabel = operation === 'remove' ? 'removing'
    : matchingProgress?.phase === 'downloading' && percent !== null && percent < 100 ? 'transferring'
    : (!downloading || downloading === format) && cache?.filesPresent && matchingProgress?.phase !== 'downloading' ? 'verifying' : 'working'
  // Active provider identity is verified during native loading; file presence alone is not.
  const loaded = (semantic?.provider ?? status)?.activeFormat === format
  const canChange = ready && status?.compiled && !busy && !semantic?.enabled && !loaded
  const small = format === 'float32' || format === 'dynamic-int8'
  const formatLabel = (choice: ModelSelection) => `multilingual-e5-${choice === 'base-float32' ? 'base' : choice === 'large-float32' ? 'large' : 'small'} · ${t(choice === 'dynamic-int8' ? 'settings.searchModel.int8' : 'settings.searchModel.float32')}`

  /** A different model or precision needs its own download consent; changing it starts no work. */
  function selectFormat(choice: ModelSelection) {
    if (!canChange) return
    if (semantic) semantic.setFormat(choice)
    else setLocalFormat(choice)
    setConsent(false)
    setRemoval(null)
    setError('')
    setMessage('')
    setProgress(null)
  }

  const canRemoveModel = !!canChange && !!cache?.hasStoredFiles && status?.activeFormat !== format
  const canRemoveIndex = !busy && !!semantic?.provider?.compiled && !!semantic.index?.hasStoredIndex

  /** Share the action row, but keep confirmation outside it and removal apart from preparation. */
  function removalControls(kind: 'model' | 'index', preparation?: ReactNode) {
    const allowed = kind === 'model' ? canRemoveModel : canRemoveIndex
    const label = t(kind === 'model' ? 'settings.searchModel.remove' : 'settings.semantic.remove')
    return <div className="search-removal-controls">
      <div className="search-model-actions search-removal-actions">
        {preparation}
        <button type="button" className="app-settings-data-action search-remove-confirm" disabled={!allowed}
          onClick={() => { if (allowed) setRemoval(kind) }}>{label}</button>
      </div>
      {removal === kind && <div className="search-removal-confirmation" aria-live="polite" role="group" aria-label={label}>
        <p>{t(kind === 'model' ? 'settings.searchModel.confirmRemove' : 'settings.semantic.confirmRemove')}</p>
        <div className="search-model-actions">
          <button type="button" className="app-settings-data-action" onClick={(event) => {
            event.currentTarget.closest('.search-removal-controls')?.querySelector<HTMLButtonElement>('.search-remove-confirm')?.focus()
            setRemoval(null)
          }}>{t('common.cancel')}</button>
          <button type="button" className="app-settings-data-action search-remove-confirm" disabled={!allowed}
            onClick={() => {
              // Keep confirmation in its owning section, without a nested modal/focus trap.
              if (!allowed) return
              // Focus stays on a surviving control when confirmation or an empty off-index disappears.
              setup.current?.querySelector<HTMLElement>(kind === 'index'
                ? 'input[role="switch"]:not(:disabled), .search-model-management > summary'
                : '.search-model-management > summary')?.focus()
              if (kind === 'model') void run('remove')
              else void semantic?.remove()
              setRemoval(null)
            }}>{label}</button>
        </div>
      </div>}
    </div>
  }

  // Model preparation sits between the opt-in and index controls without owning their state.
  const modelControls = <>
    {ready && status?.compiled && <details className="search-model-panel search-model-management" open={(!loaded && !cache?.filesPresent) || operation !== null || !!downloading || !!status.loadingFormat || !!error}>
      <summary>{t(loaded || cache?.hasStoredFiles ? 'settings.searchModel.manage' : 'settings.searchModel.title')}
        {(loaded || cache?.filesPresent) && <span className="search-model-summary-status" role="status">{formatLabel(format)} · {t(loaded ? 'settings.searchModel.loaded' : 'settings.searchModel.stored')}</span>}
      </summary>
      <div className="search-model-selection">
        <label className="search-model-field">
          <span>{t('settings.searchModel.model')}</span>
          <select aria-label={t('settings.searchModel.model')} value={small ? 'float32' : format} disabled={!canChange}
            onChange={(event) => selectFormat(event.target.value as ModelSelection)}>
            <option value="float32">multilingual-e5-small</option>
            <option value="base-float32">multilingual-e5-base</option>
            <option value="large-float32">multilingual-e5-large</option>
          </select>
        </label>
        <label className="search-model-field">
          <span>{t('settings.searchModel.format')}</span>
          <select aria-label={t('settings.searchModel.format')} value={format} disabled={!canChange || !small}
            onChange={(event) => selectFormat(event.target.value as ModelSelection)}>
            <option value={small ? 'float32' : format}>{t('settings.searchModel.float32')}</option>
            {small && <option value="dynamic-int8">{t('settings.searchModel.int8')}</option>}
          </select>
        </label>
      </div>
      <dl className="search-model-metadata">
        <dt>{t('settings.searchModel.downloadSize')}</dt><dd>{formatStorageSize(cache?.expectedBytes) ?? '—'}</dd>
      </dl>
      {!small && <p>{t('settings.searchModel.largerWarning')}</p>}
      <p>{t('settings.searchModel.switchHint')}</p>
      {format === 'dynamic-int8' && <p>{t('settings.searchModel.int8Warning')}</p>}
      {!loaded && !preparingFiles && operation !== 'remove' && <p role="status">{t(cache?.filesPresent
        ? message === 'verified' ? 'settings.searchModel.verified' : 'settings.searchModel.present'
        : cache?.hasStoredFiles ? 'settings.searchModel.incomplete' : 'settings.searchModel.absent')}</p>}
      {(!semantic?.enabled && !loaded) && <>
        <p>{t('settings.searchModel.reserve')}</p>
        <label className="search-model-consent">
          <input type="checkbox" checked={consent} disabled={!canChange} onChange={(event) => setConsent(event.target.checked)} />
          <span>{t('settings.searchModel.consent')}</span>
        </label>
      </>}
      {(semantic?.enabled || loaded) && <p>{t('settings.searchModel.inUse')}</p>}
      {removalControls('model', !semantic?.enabled && !loaded &&
        <button type="button" className="app-settings-data-action search-action-primary" disabled={!canChange || !cache || !consent}
          onClick={() => { void run('download') }}>
          {t(cache?.filesPresent ? 'settings.searchModel.verify' : 'settings.searchModel.download')}
        </button>)}

      {(operation !== null || downloading) && <div className="search-download-progress">
        <p role="status" aria-live="polite">{t(`settings.searchModel.${cancelling ? 'cancelling' : workLabel}`)}</p>
        {downloading && <>
          <p>{formatLabel(downloading)}</p>
          <progress aria-label={t('settings.searchModel.progress')} max={100}
            value={matchingProgress && percent !== null ? percent : undefined} />
          {matchingProgress && percent !== null && <span>{percent}%</span>}
          <div className="search-model-actions">
            <button type="button" className="app-settings-data-action" disabled={cancelling}
              onClick={() => { void cancel() }}>{t('settings.searchModel.cancel')}</button>
          </div>
        </>}
      </div>}
      {message === 'removed' && <p role="status">{t('settings.searchModel.removed')}</p>}
      {!busy && progress?.phase === 'cancelled' && !error && <p role="status">{t('settings.searchModel.cancelled')}</p>}
      {!busy && progress?.phase === 'failed' && !error && <p role="alert">{t('settings.searchModel.failed')}</p>}
    </details>}
    {error && <p className="app-dialog-error" role="alert" dir="auto">{error}</p>}
    {error && <button type="button" className="app-settings-data-action" onClick={() => {
      setError('')
      setReady(false)
      setCancelling(false)
      setConnecting((value) => value + 1)
    }}>{t('common.retry')}</button>}
  </>

  return (
    <section ref={setup} className="app-settings-section search-model-setup" aria-labelledby={`${id}-title`}>
      <p id={`${id}-title`}>{t('settings.semantic.description')}</p>
      {!ready && <p role="status">{t('settings.searchModel.loading')}</p>}
      {ready && !status?.compiled && <p>{t('settings.searchModel.unavailable')}</p>}
      {ready && semantic
        ? <SemanticSearchSetup semantic={semantic} cachePresent={!!cache?.filesPresent} cacheBusy={busy} indexMaintenance={removalControls('index',
          <button type="button" className="app-settings-data-action" disabled={busy}
            onClick={() => { void semantic.refresh() }}>{t('settings.semantic.refresh')}</button>)}>
          {modelControls}
        </SemanticSearchSetup>
        : modelControls}
      {ready && status?.compiled && <details className="search-model-panel">
        <summary>{t('settings.searchModel.details')}</summary>
        <p>{t('settings.searchModel.resources')}</p>
        <p>{t('settings.semantic.recovery')}</p>
        <p>{t('settings.searchModel.recovery')}</p>
      </details>}
    </section>
  )
}
