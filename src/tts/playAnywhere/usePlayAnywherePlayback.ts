import { useCallback, useLayoutEffect, useRef } from 'react'
import type { ViewerTextTargetApi, ViewerTextTargetResult } from '../../viewers/types'
import type { TtsChunk, TtsOptions } from '../types'
import type { PreparedTextTargetResult } from './prepareTextTarget'

interface PlaybackTargetOptions {
  complete: boolean
  options: TtsOptions
  resolve: (url: string, capture: ViewerTextTargetResult, signal?: AbortSignal) => Promise<PreparedTextTargetResult>
  getChunks: () => Promise<readonly TtsChunk[]>
  play: (chunks: readonly TtsChunk[], options: TtsOptions, index: number, isCurrent: () => boolean, onStarted?: () => void) => boolean
}

export type PlaybackTargetResult = { status: 'dispatched' }
  | { status: 'error'; error: unknown }
  | Extract<PreparedTextTargetResult, { status: 'unavailable' }>

/** Only this boundary owns target intent; the existing player still owns transport queues. */
export function createPlayAnywherePlayback(scope: PlaybackTargetOptions) {
  let alive = true, intent = 0
  let preparation: AbortController | undefined
  const unavailable = (reason: 'stale' | 'not-ready' | 'invalid-point') => ({ status: 'unavailable', reason }) as const
  const run = async (api?: ViewerTextTargetApi, onStarted?: () => void): Promise<PlaybackTargetResult> => {
    preparation?.abort()
    const request = ++intent
    const controller = new AbortController(); preparation = controller
    const current = () => alive && intent === request
    const url = scope.options.documentUrl
    if (!current() || !scope.complete || !url) return unavailable('not-ready')
    try {
      let chunks: readonly TtsChunk[], index = 0
      let accepts = current
      if (api) {
        const capture = await api.captureViewport()
        if (!current()) return unavailable('stale')
        if (capture.status === 'unavailable') return capture
        const target = await scope.resolve(url, capture, controller.signal)
        if (!current()) return unavailable('stale')
        if (target.status !== 'resolved') return target
        if (target.documentUrl !== url) return unavailable('stale')
        chunks = target.chunks; index = target.chunkIndex
        accepts = () => current() && target.isCurrent()
      } else {
        chunks = await scope.getChunks()
      }
      if (!accepts()) return unavailable('stale')
      return scope.play(chunks, scope.options, index, accepts, onStarted ? () => { if (current()) onStarted() } : undefined) ? { status: 'dispatched' } : unavailable('invalid-point')
    } catch (error) { return current() ? { status: 'error', error } : unavailable('stale') }
  }
  return { playFromHere: (api: ViewerTextTargetApi, onStarted?: () => void) => run(api, onStarted), playFromBeginning: (onStarted?: () => void) => run(undefined, onStarted),
    cancel() { intent++; preparation?.abort(); preparation = undefined }, dispose() { alive = false; intent++; preparation?.abort(); preparation = undefined } }
}

export function usePlayAnywherePlayback(options: PlaybackTargetOptions) {
  const { complete, options: playbackOptions, resolve, getChunks, play } = options
  const active = useRef<ReturnType<typeof createPlayAnywherePlayback> | null>(null)
  useLayoutEffect(() => {
    const scope = createPlayAnywherePlayback({ complete, options: playbackOptions, resolve, getChunks, play })
    active.current = scope
    return () => { scope.dispose(); active.current = null }
  }, [complete, playbackOptions, resolve, getChunks, play])
  const cancel = useCallback(() => active.current?.cancel(), [])
  const playFromHere = useCallback((api: ViewerTextTargetApi, onStarted?: () => void) => active.current?.playFromHere(api, onStarted)
    ?? Promise.resolve({ status: 'unavailable', reason: 'not-ready' } as const), [])
  const playFromBeginning = useCallback((onStarted?: () => void) => active.current?.playFromBeginning(onStarted)
    ?? Promise.resolve({ status: 'unavailable', reason: 'not-ready' } as const), [])
  return { cancel, playFromHere, playFromBeginning }
}
