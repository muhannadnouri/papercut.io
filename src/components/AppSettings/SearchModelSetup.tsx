import { useCallback, useEffect, useId, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useTranslation } from 'react-i18next'
import { formatStorageSize } from '../../utils/formatUtils'
import { SemanticSearchSetup } from '../../search-semantic/SemanticSearchSetup'
import type { ModelFormat, ProviderStatus, SemanticSearchController } from '../../search-semantic/useSemanticSearch'

interface CacheStatus {
  format: ModelFormat
  filesPresent: boolean
  expectedBytes: number
}
interface DownloadProgress {
  format: ModelFormat
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
  const [localFormat, setLocalFormat] = useState<ModelFormat>('float32')
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
  const [cancelling, setCancelling] = useState(false)
  const refreshPending = useRef<Promise<void> | null>(null)

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
    if (kind === 'remove' && status.activeFormat === format) return
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
  const canChange = ready && status?.compiled && !busy && !semantic?.enabled

  return (
    <section className="app-settings-section search-model-setup" aria-labelledby={`${id}-title`}>
      <h3 id={`${id}-title`}>{t('settings.searchModel.title')}</h3>
      <p>{t('settings.searchModel.description')}</p>
      {!ready && <p role="status">{t('settings.searchModel.loading')}</p>}
      {ready && !status?.compiled && <p>{t('settings.searchModel.unavailable')}</p>}
      {ready && status?.compiled && <>
        <label className="search-model-field">
          <span>{t('settings.searchModel.format')}</span>
          <select value={format} disabled={!canChange} onChange={(event) => {
            const choice = event.target.value as ModelFormat
            if (semantic) semantic.setFormat(choice)
            else setLocalFormat(choice)
            setConsent(false)
            setMessage('')
            setProgress(null)
          }}>
            <option value="float32">{t('settings.searchModel.float32')}</option>
            <option value="dynamic-int8">{t('settings.searchModel.int8')}</option>
          </select>
        </label>
        <p>{t('settings.searchModel.size', { size: formatStorageSize(cache?.expectedBytes) ?? '—' })}</p>
        <p>{t('settings.searchModel.resources')}</p>
        <p>{t('settings.searchModel.platform')}</p>
        {format === 'dynamic-int8' && <p>{t('settings.searchModel.int8Warning')}</p>}
        <p>{t(cache?.filesPresent ? 'settings.searchModel.present' : 'settings.searchModel.absent')}</p>
        <label className="search-model-consent">
          <input type="checkbox" checked={consent} disabled={!canChange} onChange={(event) => setConsent(event.target.checked)} />
          <span>{t('settings.searchModel.consent')}</span>
        </label>
        <div className="search-model-actions">
          <button type="button" className="app-settings-data-action" disabled={!canChange || !cache || !consent}
            onClick={() => { void run('download') }}>
            {t(cache?.filesPresent ? 'settings.searchModel.verify' : 'settings.searchModel.download')}
          </button>
          <button type="button" className="app-settings-data-action" disabled={!canChange || !cache || status.activeFormat === format}
            onClick={() => { void run('remove') }}>
            {t('settings.searchModel.remove')}
          </button>
        </div>
        {status.activeFormat === format && <p>{t('settings.searchModel.inUse')}</p>}
        <p>{t('settings.searchModel.recovery')}</p>
        {busy && <div role="status" aria-live="polite">
          <p>{t(status.loadingFormat ? 'settings.searchModel.activationBusy' : 'settings.searchModel.working')}</p>
          {downloading && <>
            <p>{t(downloading === 'float32' ? 'settings.searchModel.float32' : 'settings.searchModel.int8')}</p>
            <progress aria-label={t('settings.searchModel.progress')} max={100}
              value={matchingProgress && percent !== null ? percent : undefined} />
            {matchingProgress && percent !== null && <span>{percent}%</span>}
          </>}
        </div>}
        {downloading && <button type="button" className="app-settings-data-action" disabled={cancelling}
          onClick={() => { void cancel() }}>{t('common.cancel')}</button>}
        {semantic && <SemanticSearchSetup semantic={semantic} cachePresent={!!cache?.filesPresent} cacheBusy={busy} />}
      </>}
      {message && <p role="status">{t(`settings.searchModel.${message}`)}</p>}
      {cancelling && busy && <p role="status">{t('settings.searchModel.cancelling')}</p>}
      {!busy && progress?.phase === 'cancelled' && !error && <p role="status">{t('settings.searchModel.cancelled')}</p>}
      {!busy && progress?.phase === 'failed' && !error && <p role="alert">{t('settings.searchModel.failed')}</p>}
      {error && <p className="app-dialog-error" role="alert" dir="auto">{error}</p>}
      {error && <button type="button" className="app-settings-data-action" onClick={() => {
        setError('')
        setReady(false)
        setCancelling(false)
        setConnecting((value) => value + 1)
      }}>{t('common.retry')}</button>}
      <p>{t('settings.searchModel.closeHelp')}</p>
    </section>
  )
}
