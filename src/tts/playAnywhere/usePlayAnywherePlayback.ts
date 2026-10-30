import {
  useCallback,
  useLayoutEffect,
  useRef,
} from 'react'
import type {
  ViewerTextTargetApi,
  ViewerTextTargetResult,
} from '../../viewers/types'
import type {
  TtsChunk,
  TtsOptions,
} from '../types'
import type { PlaybackStart } from '../hooks/useTtsPlayer'
import type { PreparedTextTargetResult } from './prepareTextTarget'

interface PlaybackTargetOptions {
  complete: boolean
  options: TtsOptions
  resolve: (url: string, capture: ViewerTextTargetResult, signal?: AbortSignal) => Promise<PreparedTextTargetResult>
  getChunks: () => Promise<readonly TtsChunk[]>
  play: (chunks: readonly TtsChunk[], options: TtsOptions, index: number, withinChunkFraction?: number) => Promise<PlaybackStart> | null
  /** The player's command epoch; any later playback command aborts it. */
  commandSignal: () => AbortSignal
}

export type PlaybackTargetResult = { status: 'dispatched';
  started: Promise<PlaybackStart> }
  | { status: 'cancelled';
    reason: 'superseded' | 'disposed' }
  | { status: 'error';
    error: unknown }
  | Extract<PreparedTextTargetResult, { status: 'unavailable' }>

/**
 * Owns target intent only until dispatch. A run is cancelled by a newer run, by any player command
 * (its epoch) or by disposal; passage validity is checked once more right before the player call.
 * After dispatch the player's own job/navigation fences own the start.
 */
export function createPlayAnywherePlayback(scope: PlaybackTargetOptions) {
  let alive = true
  let preparation: AbortController | undefined
  const unavailable = (reason: 'stale' | 'not-ready' | 'invalid-point') => ({
    status: 'unavailable',
    reason,
  }) as const
  const run = async (api?: ViewerTextTargetApi): Promise<PlaybackTargetResult> => {
    preparation?.abort()
    const url = scope.options.documentUrl
    if (!alive || !scope.complete || !url) {
      return unavailable('not-ready')
    }
    const controller = new AbortController()
    preparation = controller
    const command = scope.commandSignal()
    // Forward rather than AbortSignal.any: the iOS 15.0 minimum lacks it.
    const supersede = () => controller.abort()
    command.addEventListener('abort', supersede, { once: true })
    if (command.aborted) {
      controller.abort()
    }
    const current = () => !controller.signal.aborted
    const cancelled = () => ({
      status: 'cancelled',
      reason: alive ? 'superseded' : 'disposed',
    }) as const
    try {
      let chunks: readonly TtsChunk[]
      let index = 0
      let withinChunkFraction: number | undefined
      let valid = () => true
      if (api) {
        const capture = await api.captureViewport()
        if (!current()) {
          return cancelled()
        }
        if (capture.status === 'unavailable') {
          return capture
        }
        const target = await scope.resolve(url, capture, controller.signal)
        if (!current()) {
          return cancelled()
        }
        if (target.status !== 'resolved') {
          return target
        }
        if (target.documentUrl !== url) {
          return unavailable('stale')
        }
        chunks = target.chunks
        index = target.chunkIndex
        withinChunkFraction = target.withinChunkFraction
        valid = target.isCurrent
      } else {
        chunks = await scope.getChunks()
      }
      if (!current()) {
        return cancelled()
      }
      if (!valid()) {
        return unavailable('stale')
      }
      const started = scope.play(chunks, scope.options, index, withinChunkFraction)
      return started ? {
        status: 'dispatched',
        started,
      } : unavailable('invalid-point')
    } catch (error) {
      return current() ? {
        status: 'error',
        error,
      } : cancelled()
    } finally {
      command.removeEventListener('abort', supersede)
    }
  }
  return {
    playFromHere: (api: ViewerTextTargetApi) => run(api),
    playFromBeginning: () => run(),
    dispose() {
      alive = false
      preparation?.abort()
      preparation = undefined
    },
  }
}

export function usePlayAnywherePlayback(options: PlaybackTargetOptions) {
  const { complete, options: playbackOptions, resolve, getChunks, play, commandSignal } = options
  const active = useRef<ReturnType<typeof createPlayAnywherePlayback> | null>(null)
  useLayoutEffect(() => {
    const scope = createPlayAnywherePlayback({
      complete,
      options: playbackOptions,
      resolve,
      getChunks,
      play,
      commandSignal,
    })
    active.current = scope
    return () => {
      scope.dispose()
      active.current = null
    }
  }, [complete, playbackOptions, resolve, getChunks, play, commandSignal])
  const playFromHere = useCallback((api: ViewerTextTargetApi) => active.current?.playFromHere(api)
    ?? Promise.resolve({
      status: 'unavailable',
      reason: 'not-ready',
    } as const), [])
  const playFromBeginning = useCallback(() => active.current?.playFromBeginning()
    ?? Promise.resolve({
      status: 'unavailable',
      reason: 'not-ready',
    } as const), [])
  return {
    playFromHere,
    playFromBeginning,
  }
}
