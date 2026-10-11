import { useCallback, useEffect, useRef, useState } from 'react'
import { listen } from '@tauri-apps/api/event'
import { invoke, isTauri } from '@tauri-apps/api/core'

export type ModelFormat = 'float32' | 'dynamic-int8'
export interface ProviderStatus {
  compiled: boolean
  activeFormat: ModelFormat | null
  loadingFormat: ModelFormat | null
  downloadingFormat: ModelFormat | null
}
interface IndexStatus {
  running: boolean
  cancelRequested: boolean
  indexedDocuments: number
  eligibleDocuments: number
  passages: number
  embeddedPassages: number
  reusedPassages: number
  error: string | null
}
interface Preference { enabled: boolean; format: ModelFormat }
const STORAGE_KEY = 'papercut.semantic-search.v1'

/** Invalid or inaccessible preferences fail closed; cache presence is never consent. */
function readPreference(): Preference {
  try {
    const value = JSON.parse(window.localStorage.getItem(STORAGE_KEY) ?? 'null')
    if (value && typeof value.enabled === 'boolean' &&
      (value.format === 'float32' || value.format === 'dynamic-int8')) return value
  } catch { /* Start disabled when storage is unavailable. */ }
  return { enabled: false, format: 'float32' }
}

/** One App-level owner survives settings/reader navigation. Only saved consent reloads
 * a cached model; indexing is explicit, and querying has its own independent hook. */
export function useSemanticSearch() {
  const [preference, setPreference] = useState(readPreference)
  const [snapshot, setSnapshot] = useState<{ provider: ProviderStatus; index: IndexStatus } | null>(null)
  const [operation, setOperation] = useState<'activate' | 'disable' | 'build' | 'remove' | null>(null)
  const [error, setError] = useState('')
  const [cancelling, setCancelling] = useState(false)
  const mounted = useRef(false)
  const generation = useRef(0)
  const request = useRef(0)
  const action = useRef(false)
  const currentPreference = useRef(preference)
  const invalidate = useCallback(() => {
    mounted.current = false
    generation.current++
    request.current++
  }, [])

  const refresh = useCallback(async () => {
    const ticket = ++request.current
    const version = generation.current
    const [provider, index] = await Promise.all([
      invoke<ProviderStatus>('search_embedding_status'),
      invoke<IndexStatus>('search_semantic_index_status'),
    ])
    const value = { provider, index }
    if (mounted.current && ticket === request.current && version === generation.current) setSnapshot(value)
    return value
  }, [])

  const configure = useCallback(async (format: ModelFormat | null, version: number) => {
    action.current = true
    setOperation(format ? 'activate' : 'disable')
    try {
      await invoke('search_embedding_configure', { format })
    } catch (err) {
      if (mounted.current && version === generation.current) setError(String(err))
    } finally {
      if (mounted.current && version === generation.current) {
        try { await refresh() } catch (err) {
          if (mounted.current && version === generation.current) setError(String(err))
        }
        if (mounted.current && version === generation.current) { action.current = false; setOperation(null) }
      }
    }
  }, [refresh])

  /** Persist before enabling, so a storage failure cannot silently opt a user in. */
  function save(value: Preference): boolean {
    try {
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify(value))
      return true
    } catch {
      setError('Unable to save the semantic-search preference. Check device storage; the previous preference may return after restart.')
      return false
    }
  }

  const provider = snapshot?.provider
  const index = snapshot?.index
  const busy = operation !== null || !!provider?.loadingFormat || !!provider?.downloadingFormat || !!index?.running

  /** Disabling can interrupt activation/indexing; stale completions cannot restore consent. */
  async function setEnabled(enabled: boolean) {
    if (enabled && (action.current || busy || !provider?.compiled)) return
    if (!enabled && operation === 'disable') return
    const value = { ...currentPreference.current, enabled }
    setError('')
    if (!save(value) && enabled) return
    currentPreference.current = value
    setPreference(value)
    const version = ++generation.current
    setCancelling(false)
    await configure(enabled ? value.format : null, version)
  }

  function setFormat(format: ModelFormat) {
    if (currentPreference.current.enabled || action.current || busy) return
    const value = { enabled: false, format }
    setError('')
    if (!save(value)) return
    currentPreference.current = value
    setPreference(value)
  }

  async function run(kind: 'build' | 'remove') {
    if (action.current || busy || !provider?.compiled) return
    if (kind === 'build' && (!currentPreference.current.enabled || provider.activeFormat !== preference.format)) return
    action.current = true
    const version = generation.current
    setOperation(kind)
    setCancelling(false)
    setError('')
    try {
      await invoke(kind === 'build' ? 'search_semantic_index_build' : 'search_semantic_index_remove')
    } catch (err) {
      if (mounted.current && version === generation.current) setError(String(err))
    } finally {
      if (mounted.current && version === generation.current) {
        try { await refresh() } catch (err) {
          if (mounted.current && version === generation.current) setError(String(err))
        }
        if (mounted.current && version === generation.current) { action.current = false; setOperation(null); setCancelling(false) }
      }
    }
  }

  async function cancel() {
    // Wait for native reservation; cancelling before IPC starts the job would be a no-op.
    if (cancelling || !index?.running) return
    setCancelling(true)
    const version = generation.current
    try { await invoke('search_semantic_index_cancel'); await refresh() }
    catch (err) { if (mounted.current && version === generation.current) { setError(String(err)); setCancelling(false) } }
  }

  useEffect(() => {
    if (!isTauri()) return
    mounted.current = true
    const version = generation.current
    void (async () => {
      try {
        const value = await refresh()
        if (!mounted.current || version !== generation.current) return
        // Normal builds retain the preference but never attempt unsupported activation.
        if (currentPreference.current.enabled && value.provider.compiled) {
          await configure(currentPreference.current.format, version)
        }
      } catch (err) {
        if (mounted.current && version === generation.current) { action.current = false; setOperation(null); setError(String(err)) }
      }
    })()
    return invalidate
  }, [refresh, configure, invalidate])

  useEffect(() => {
    if (!isTauri()) return
    let closed = false
    let stop: (() => void) | undefined
    // Downloads outlive Settings; terminal events also catch jobs shorter than one poll.
    void listen<{ phase: string }>('search-embedding-download-progress', ({ payload }) => {
      if (!closed && payload.phase !== 'downloading') void refresh().catch((err: unknown) => {
        if (!closed) setError(String(err))
      })
    }).then((unlisten) => {
      if (closed) unlisten()
      else stop = unlisten
    }).catch((err: unknown) => { if (!closed) setError(String(err)) })
    return () => { closed = true; stop?.() }
  }, [refresh])

  useEffect(() => {
    if (!busy) return
    let polling = false
    // One in-flight status request bounds polling even when a native call is slow.
    const timer = window.setInterval(() => {
      if (polling) return
      polling = true
      const version = generation.current
      void refresh().catch((err: unknown) => {
        if (mounted.current && version === generation.current) setError(String(err))
      })
        .finally(() => { polling = false })
    }, 1000)
    return () => window.clearInterval(timer)
  }, [busy, refresh])

  return { ...preference, provider, index, busy, operation, cancelling, error,
    setEnabled, setFormat, build: () => run('build'), remove: () => run('remove'), cancel,
    refresh: async () => { try { setError(''); await refresh() } catch (err) { setError(String(err)) } },
  }
}

export type SemanticSearchController = ReturnType<typeof useSemanticSearch>
