import { useCallback, useEffect, useRef, useState } from 'react'
import { listen } from '@tauri-apps/api/event'
import { invoke, isTauri } from '@tauri-apps/api/core'
import type { UploadedDocument } from '../uploads/DocumentUploads'

// Existing Small preference keys remain valid; larger selections are FP32 only.
const MODEL_SELECTIONS = ['float32', 'dynamic-int8', 'base-float32', 'large-float32'] as const
export type ModelSelection = typeof MODEL_SELECTIONS[number]
export interface ProviderStatus {
  compiled: boolean
  activeFormat: ModelSelection | null
  loadingFormat: ModelSelection | null
  downloadingFormat: ModelSelection | null
}
interface IndexStatus {
  running: boolean
  cancelRequested: boolean
  indexedDocuments: number
  eligibleDocuments: number
  passages: number
  hasStoredIndex: boolean
  embeddedPassages: number
  reusedPassages: number
  error: string | null
}
interface Preference { enabled: boolean; format: ModelSelection }
type ActivityNotice = 'complete' | 'cancelled' | 'failed' | 'statusFailed'
const STORAGE_KEY = 'papercut.semantic-search.v1'

/** Invalid or inaccessible preferences fail closed; cache presence is never consent. */
function readPreference(): Preference {
  try {
    const value = JSON.parse(window.localStorage.getItem(STORAGE_KEY) ?? 'null')
    if (value && typeof value.enabled === 'boolean' &&
      MODEL_SELECTIONS.includes(value.format)) return value
  } catch { /* Start disabled when storage is unavailable. */ }
  return { enabled: false, format: 'float32' }
}

/** One App-level owner survives settings/reader navigation. Only saved consent reloads
 * a cached model; indexing is explicit, and querying has its own independent hook. */
export function useSemanticSearch(library?: readonly UploadedDocument[]) {
  const [preference, setPreference] = useState(readPreference)
  const [snapshot, setSnapshot] = useState<{ provider: ProviderStatus; index: IndexStatus } | null>(null)
  const [operation, setOperation] = useState<'activate' | 'disable' | 'build' | 'remove' | null>(null)
  const [error, setError] = useState('')
  // Retrying a status read must not erase a failed indexing/activation diagnostic.
  const [statusError, setStatusError] = useState('')
  const [cancelling, setCancelling] = useState(false)
  // A fresh identity resets expiry even when two fast jobs finish with the same outcome.
  const [activityNotice, setActivityNotice] = useState<{ kind: ActivityNotice } | null>(null)
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
    try {
      const [provider, index] = await Promise.all([
        invoke<ProviderStatus>('search_embedding_status'),
        invoke<IndexStatus>('search_semantic_index_status'),
      ])
      const value = { provider, index }
      if (mounted.current && ticket === request.current && version === generation.current) {
        setSnapshot(value)
        setStatusError('')
      }
      return value
    } catch (err) {
      if (mounted.current && ticket === request.current && version === generation.current) setStatusError(String(err))
      throw err
    }
  }, [])

  const configure = useCallback(async (format: ModelSelection | null, version: number) => {
    action.current = true
    setOperation(format ? 'activate' : 'disable')
    try {
      await invoke('search_embedding_configure', { format })
    } catch (err) {
      if (mounted.current && version === generation.current) setError(String(err))
    } finally {
      if (mounted.current && version === generation.current) {
        await refresh().catch(() => {})
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
    setActivityNotice(null)
    if (!save(value) && enabled) return
    currentPreference.current = value
    setPreference(value)
    const version = ++generation.current
    setCancelling(false)
    await configure(enabled ? value.format : null, version)
  }

  function setFormat(format: ModelSelection) {
    if (currentPreference.current.enabled || action.current || busy) return
    const value = { enabled: false, format }
    setError('')
    setActivityNotice(null)
    if (!save(value)) return
    currentPreference.current = value
    setPreference(value)
  }

  async function run(kind: 'build' | 'remove') {
    if (action.current || busy || !provider?.compiled) return
    if (kind === 'remove' && !index?.hasStoredIndex) return
    if (kind === 'build' && (!currentPreference.current.enabled || provider.activeFormat !== preference.format)) return
    action.current = true
    const version = generation.current
    setOperation(kind)
    setCancelling(false)
    setError('')
    setActivityNotice(null)
    let notice: ActivityNotice | null = null
    try {
      await invoke(kind === 'build' ? 'search_semantic_index_build' : 'search_semantic_index_remove')
      if (kind === 'build') notice = 'complete'
    } catch (err) {
      // A requested cancel can race a real failure; only the native cancellation result is cancellation.
      if (kind === 'build') notice = String(err) === 'Semantic indexing cancelled' ? 'cancelled' : 'failed'
      if (mounted.current && version === generation.current) setError(String(err))
    } finally {
      if (mounted.current && version === generation.current) {
        try { await refresh() } catch {
          if (notice === 'complete') notice = 'statusFailed'
        }
        if (mounted.current && version === generation.current) {
          action.current = false; setOperation(null); setCancelling(false)
          setActivityNotice(notice ? { kind: notice } : null)
        }
      }
    }
  }

  async function cancel() {
    // Wait for native reservation; cancelling before IPC starts the job would be a no-op.
    if (cancelling || !index?.running) return
    setCancelling(true)
    const version = generation.current
    try { await invoke('search_semantic_index_cancel') }
    catch (err) {
      if (mounted.current && version === generation.current) { setError(String(err)); setCancelling(false) }
      return
    }
    await refresh().catch(() => {})
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
      } catch {
        if (mounted.current && version === generation.current) { action.current = false; setOperation(null) }
      }
    })()
    return invalidate
  }, [refresh, configure, invalidate])

  useEffect(() => {
    if (isTauri() && library) void refresh().catch(() => {})
  }, [library, refresh])

  useEffect(() => {
    if (!isTauri()) return
    let closed = false
    let stop: (() => void) | undefined
    // Downloads outlive Settings; terminal events also catch jobs shorter than one poll.
    void listen<{ phase: string }>('search-embedding-download-progress', ({ payload }) => {
      if (!closed && payload.phase !== 'downloading') void refresh().catch(() => {})
    }).then((unlisten) => {
      if (closed) unlisten()
      else stop = unlisten
    }).catch((err: unknown) => { if (!closed) setError(String(err)) })
    return () => { closed = true; stop?.() }
  }, [refresh])

  useEffect(() => {
    // App-owned notices survive reader navigation; errors stay until reviewed or a new action starts.
    if (activityNotice?.kind !== 'complete' && activityNotice?.kind !== 'cancelled') return
    const timer = setTimeout(() => setActivityNotice(null), 8000)
    return () => clearTimeout(timer)
  }, [activityNotice])

  useEffect(() => {
    if (!busy) return
    let polling = false
    // One in-flight status request bounds polling even when a native call is slow.
    const timer = window.setInterval(() => {
      if (polling) return
      polling = true
      void refresh().catch(() => {})
        .finally(() => { polling = false })
    }, 1000)
    return () => window.clearInterval(timer)
  }, [busy, refresh])

  const refreshStatus = useCallback(async () => {
    if (isTauri()) await refresh().catch(() => {})
  }, [refresh])

  return { ...preference, provider, index, busy, operation, cancelling, error, statusError, activityNotice: activityNotice?.kind ?? null,
    dismissActivity: () => setActivityNotice(null),
    setEnabled, setFormat, build: () => run('build'), remove: () => run('remove'), cancel,
    refresh: refreshStatus,
  }
}

export type SemanticSearchController = ReturnType<typeof useSemanticSearch>
