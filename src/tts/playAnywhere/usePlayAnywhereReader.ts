import {
  useCallback,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from 'react'
import { flushSync } from 'react-dom'
import type { AudioFollowPolicy, ViewerTextTargetApi, ViewerTextTargetResult } from '../../viewers/types'
import { logTtsDiagnostic } from '../diagnostics/TtsDiagnostics'
import type { PlaybackTargetResult } from './usePlayAnywherePlayback'

interface ReaderPlaybackOptions {
  documentUrl: string | null
  active: boolean
  playFromHere: (api: ViewerTextTargetApi, onStarted?: () => void) => Promise<PlaybackTargetResult>
  playFromBeginning: (onStarted?: () => void) => Promise<PlaybackTargetResult>
  cancelTargetPlayback: () => void
}

/**
 * Browsing suspends reveals without pausing audio; Return restores reveals without seeking.
 * Explicit starts restore following after audio acknowledgement only if newer browsing has not won.
 */
export function usePlayAnywhereReader({
  documentUrl,
  active,
  playFromHere,
  playFromBeginning,
  cancelTargetPlayback,
}: ReaderPlaybackOptions) {
  const api = useRef<ViewerTextTargetApi | null>(null)
  const selectionEpoch = useRef(0)
  const selection = useRef<{ api: ViewerTextTargetApi; read: () => ViewerTextTargetResult } | null>(null)
  const scope = useRef({ alive: true, intent: 0, browse: 0, following: true, active: false })
  const [view, setView] = useState({ documentUrl, active, following: true, revision: 0 })
  const [feedback, setFeedback] = useState<string | null>(null)
  const [pending, setPending] = useState(false)
  const [capturing, setCapturing] = useState(false)
  if (view.documentUrl !== documentUrl || view.active !== active) {
    const reset = view.documentUrl !== documentUrl || !active
    setView({
      documentUrl,
      active,
      following: reset ? true : view.following,
      revision: view.revision + 1,
    })
    if (reset) { setFeedback(view.documentUrl === documentUrl && pending ? 'stale' : null); setPending(false); setCapturing(false) }
  }
  useLayoutEffect(() => {
    const current = { alive: true, intent: 0, browse: 0, following: true, active: false }
    scope.current = current
    return () => { current.alive = false; cancelTargetPlayback() }
  }, [documentUrl, cancelTargetPlayback])
  useLayoutEffect(() => {
    scope.current.active = active
    if (!active) {
      scope.current.following = true
    }
  }, [active, documentUrl])
  const invalidateSelection = useCallback(() => { selectionEpoch.current++; selection.current = null }, [])
  const register = useCallback((value: ViewerTextTargetApi | null) => {
    invalidateSelection(); api.current = value
  }, [invalidateSelection])
  const clearSelection = useCallback(() => { selection.current = null }, [])
  /**
   * Freeze the source-order start before menu focus collapses native selection.
   * The boolean is UI availability, not audio proof; the retained snapshot still checks source/input lifetime.
   */
  const snapshotSelection = useCallback(() => {
    const targetApi = api.current, epoch = selectionEpoch.current
    if (!targetApi) {
      selection.current = null
      return false
    }
    let available = false
    let read: () => ViewerTextTargetResult
    try {
      const capture = targetApi.captureSelection?.() ?? { status: 'unavailable', reason: 'no-selection' } as const
      available = capture.status === 'captured'
      if (capture.status === 'unavailable' && capture.reason === 'no-selection' && selection.current?.api === targetApi) {
        const previous = selection.current.read()
        if (previous.status === 'captured' && previous.isCurrent()) return true
      }
      read = () => capture.status === 'captured' ? { ...capture,
        isCurrent: () => api.current === targetApi && selectionEpoch.current === epoch && capture.isCurrent() } : capture
    } catch (error) { read = () => { throw error } }
    selection.current = { api: targetApi, read }
    return available
  }, [])
  const selectionActions = useMemo(() => ({ snapshot: snapshotSelection, invalidate: invalidateSelection }), [snapshotSelection, invalidateSelection])
  const browse = useCallback(() => {
    const current = scope.current
    current.browse++
    if (!current.active || !current.following) return
    current.following = false
    logTtsDiagnostic('[play-anywhere] follow changed', { following: false })
    setView(previous => ({ ...previous, following: false }))
  }, [])
  const restore = useCallback(() => {
    scope.current.following = true
    logTtsDiagnostic('[play-anywhere] follow changed', { following: true })
    setView(previous => ({ ...previous, following: true, revision: previous.revision + 1 }))
  }, [])
  const cancel = useCallback(() => {
    scope.current.intent++
    invalidateSelection()
    cancelTargetPlayback(); setPending(false); setCapturing(false); setFeedback(null)
  }, [cancelTargetPlayback, invalidateSelection])
  /**
   * Choose selection before menu cleanup can release it, then settle chrome before viewport measurement.
   * Beginning bypasses capture; actual playback and stale-action rejection stay in the coordinator.
   */
  const run = useCallback(async (action: 'viewport' | 'beginning' | 'selection' = 'viewport') => {
    const current = scope.current, intent = ++current.intent, browseVersion = current.browse
    const valid = () => current.alive && current.intent === intent
    if (action === 'selection' && !selection.current) snapshotSelection()
    const chosen = action === 'selection' ? selection.current : null
    if (chosen) selection.current = null
    // Commit chrome changes before viewport capture stamps its unobscured bounds.
    flushSync(() => {
      setPending(true)
      setCapturing(action !== 'beginning')
      setFeedback(null)
    })
    logTtsDiagnostic('[play-anywhere] action requested', { requestId: intent, action })
    const started = () => {
      if (!valid()) return
      setPending(false)
      if (current.browse === browseVersion) restore()
    }
    const targetApi = api.current
    const result = action === 'beginning' ? await playFromBeginning(started)
      : targetApi ? await playFromHere({ captureViewport: async () => {
        const capture = action === 'selection'
          ? chosen?.read() ?? { status: 'unavailable', reason: 'no-selection' } as const
          : await targetApi.captureViewport()
        if (capture.status === 'unavailable') logTtsDiagnostic('[play-anywhere] capture rejected', { requestId: intent, action, reason: capture.reason })
        if (valid()) setCapturing(false)
        return capture
      } }, started)
      : { status: 'unavailable', reason: 'not-ready' } as const
    if (!valid()) return
    setCapturing(false)
    if (result.status !== 'dispatched') setPending(false)
    logTtsDiagnostic('[play-anywhere] action resolved', { requestId: intent, status: result.status, ...(result.status === 'unavailable' ? { reason: result.reason } : {}) })
    if (result.status === 'error') setFeedback('failed')
    else if (result.status === 'unavailable') setFeedback(result.reason)
  }, [playFromBeginning, playFromHere, restore, snapshotSelection])
  const followPolicy: AudioFollowPolicy = useMemo(() => ({
    active: active || pending,
    allowsReveal: () => scope.current.alive && (!scope.current.active || scope.current.following),
    revision: view.revision,
  }), [active, pending, view])
  return {
    register,
    browse,
    selectionActions,
    snapshotSelection,
    clearSelection,
    followPolicy,
    browsing: active && !view.following,
    pending,
    capturing,
    feedback,
    cancel,
    playFromHere: () => { void run() },
    playFromSelection: () => { void run('selection') },
    playFromBeginning: () => { void run('beginning') },
    returnToAudio: restore,
  }
}
