import {
  useCallback,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from 'react'
import { flushSync } from 'react-dom'
import type {
  AudioFollowPolicy,
  ViewerTextTargetApi,
  ViewerTextTargetResult,
} from '../../viewers/types'
import { logTtsDiagnostic } from '../diagnostics/TtsDiagnostics'
import type { PlaybackTargetResult } from './usePlayAnywherePlayback'

interface ReaderPlaybackOptions {
  documentUrl: string | null
  active: boolean
  playFromHere: (api: ViewerTextTargetApi) => Promise<PlaybackTargetResult>
  playFromBeginning: () => Promise<PlaybackTargetResult>
  /** The player's command epoch; the next playback command clears this run's feedback. */
  commandSignal: () => AbortSignal
}

/**
 * Browsing suspends reveals without pausing audio; Return restores reveals without seeking.
 * Viewport and selection starts always restore following after audio acknowledgement, so the
 * reader shows where playback began even when browsing landed during preparation; beginning
 * keeps the browse-won guard. This hook presents results only: cancellation belongs to the
 * playback scope and the player.
 */
export function usePlayAnywhereReader({
  documentUrl,
  active,
  playFromHere,
  playFromBeginning,
  commandSignal,
}: ReaderPlaybackOptions) {
  const api = useRef<ViewerTextTargetApi | null>(null)
  const selectionEpoch = useRef(0)
  const selection = useRef<{ api: ViewerTextTargetApi;
    read: () => ViewerTextTargetResult } | null>(null)
  const scope = useRef({
    alive: true,
    intent: 0,
    browse: 0,
    following: true,
    active: false,
  })
  const [view, setView] = useState({
    documentUrl,
    active,
    following: true,
    revision: 0,
  })
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
    if (reset) {
      setFeedback(view.documentUrl === documentUrl && pending ? 'stale' : null)
      setPending(false)
      setCapturing(false)
    }
  }
  useLayoutEffect(() => {
    const current = {
      alive: true,
      intent: 0,
      browse: 0,
      following: true,
      active: false,
    }
    scope.current = current
    return () => {
      current.alive = false
    }
  }, [documentUrl])
  useLayoutEffect(() => {
    scope.current.active = active
    if (!active) {
      scope.current.following = true
    }
  }, [active, documentUrl])
  const invalidateSelection = useCallback(() => {
    selectionEpoch.current++
    selection.current = null
  }, [])
  const register = useCallback((value: ViewerTextTargetApi | null) => {
    invalidateSelection()
    api.current = value
  }, [invalidateSelection])
  const clearSelection = useCallback(() => {
    selection.current = null
  }, [])
  /**
   * Freeze the source-order start before menu focus collapses native selection.
   * The boolean is UI availability, not audio proof; the retained snapshot still checks source/input lifetime.
   */
  const snapshotSelection = useCallback(() => {
    const targetApi = api.current
    const epoch = selectionEpoch.current
    if (!targetApi) {
      selection.current = null
      return false
    }
    let available = false
    let read: () => ViewerTextTargetResult
    try {
      const capture = targetApi.captureSelection?.() ?? {
        status: 'unavailable',
        reason: 'no-selection',
      } as const
      available = capture.status === 'captured'
      if (capture.status === 'unavailable' && capture.reason === 'no-selection' && selection.current?.api === targetApi) {
        const previous = selection.current.read()
        if (previous.status === 'captured' && previous.isCurrent()) {
          return true
        }
      }
      read = () => capture.status === 'captured' ? {
        ...capture,
        isCurrent: () => api.current === targetApi && selectionEpoch.current === epoch && capture.isCurrent(),
      } : capture
    } catch (error) {
      read = () => {
        throw error
      }
    }
    selection.current = {
      api: targetApi,
      read,
    }
    return available
  }, [])
  const selectionActions = useMemo(() => ({
    snapshot: snapshotSelection,
    invalidate: invalidateSelection,
  }), [snapshotSelection, invalidateSelection])
  const browse = useCallback(() => {
    const current = scope.current
    current.browse++
    if (!current.active || !current.following) {
      return
    }
    current.following = false
    logTtsDiagnostic('[play-anywhere] follow changed', { following: false })
    setView(previous => ({
      ...previous,
      following: false,
    }))
  }, [])
  const restore = useCallback(() => {
    scope.current.following = true
    logTtsDiagnostic('[play-anywhere] follow changed', { following: true })
    setView(previous => ({
      ...previous,
      following: true,
      revision: previous.revision + 1,
    }))
  }, [])
  /**
   * Choose selection before menu cleanup can release it, then settle chrome before viewport measurement.
   * Beginning bypasses capture; actual playback and stale-action rejection stay in the coordinator.
   */
  const run = useCallback(async (action: 'viewport' | 'beginning' | 'selection' = 'viewport') => {
    const current = scope.current
    const intent = ++current.intent
    const browseVersion = current.browse
    const valid = () => current.alive && current.intent === intent
    if (action === 'selection' && !selection.current) {
      snapshotSelection()
    }
    const chosen = action === 'selection' ? selection.current : null
    if (chosen) {
      selection.current = null
    }
    // Commit chrome changes before viewport capture stamps its unobscured bounds.
    flushSync(() => {
      setPending(true)
      setCapturing(action !== 'beginning')
      setFeedback(null)
    })
    logTtsDiagnostic('[play-anywhere] action requested', {
      requestId: intent,
      action,
    })
    const targetApi = api.current
    const result = action === 'beginning' ? await playFromBeginning()
      : targetApi ? await playFromHere({
        captureViewport: async () => {
          const capture = action === 'selection'
            ? chosen?.read() ?? {
              status: 'unavailable',
              reason: 'no-selection',
            } as const
            : await targetApi.captureViewport()
          if (capture.status === 'unavailable') {
            logTtsDiagnostic('[play-anywhere] capture rejected', {
              requestId: intent,
              action,
              reason: capture.reason,
            })
          }
          if (valid()) {
            setCapturing(false)
          }
          return capture
        },
      })
        : {
          status: 'unavailable',
          reason: 'not-ready',
        } as const
    if (!valid()) {
      return
    }
    setCapturing(false)
    logTtsDiagnostic('[play-anywhere] action resolved', {
      requestId: intent,
      status: result.status,
      ...('reason' in result ? { reason: result.reason } : {}),
    })
    if (result.status === 'dispatched') {
      const start = await result.started
      if (!valid()) {
        return
      }
      setPending(false)
      // An explicit start means "play from here", so the reader always shows
      // where "here" resolved to — chunk starts sit above the viewport and on
      // mobile nothing else exposes the playing position. Beginning has no
      // resolved position, so it keeps the browse-won guard.
      if (start === 'started' && (action !== 'beginning' || current.browse === browseVersion)) {
        restore()
      }
      return
    }
    setPending(false)
    // Cancellation is the user's own newer action, never a passage problem worth reporting.
    if (result.status === 'cancelled') {
      return
    }
    setFeedback(result.status === 'error' ? 'failed' : result.reason)
    commandSignal().addEventListener('abort', () => {
      if (valid()) {
        setFeedback(null)
      }
    }, { once: true })
  }, [commandSignal, playFromBeginning, playFromHere, restore, snapshotSelection])
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
    playFromHere: () => {
      void run()
    },
    playFromSelection: () => {
      void run('selection')
    },
    playFromBeginning: () => {
      void run('beginning')
    },
    returnToAudio: restore,
  }
}
