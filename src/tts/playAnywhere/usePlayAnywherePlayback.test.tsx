// @vitest-environment jsdom
import { act, useLayoutEffect } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, expect, it, vi } from 'vitest'
import { createPlayAnywherePlayback, usePlayAnywherePlayback } from './usePlayAnywherePlayback'
import type { PreparedTextTargetResult } from './prepareTextTarget'
import type { ViewerTextTargetApi, ViewerTextTargetResult } from '../../viewers/types'
import type { TtsOptions } from '../types'

Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
afterEach(() => document.body.replaceChildren())
const options: TtsOptions = { documentUrl: '/book.html', modelId: 'kokoro', voice: 'af_heart', speed: 1 }
const chunks = [{ id: 'a', text: 'A' }, { id: 'b', text: 'B' }]
const capture: ViewerTextTargetResult = { status: 'captured', point: { format: 'html', segmentIndex: 0, offset: 0 }, segmentTexts: ['A B'], segmentLengths: [3], isCurrent: () => true }
const target: PreparedTextTargetResult = { status: 'resolved', documentUrl: '/book.html', chunks, chunkIndex: 1, chunkId: 'b', isCurrent: () => true }
const setup = () => ({ complete: true, options, resolve: vi.fn<Parameters<typeof createPlayAnywherePlayback>[0]['resolve']>(async () => target), getChunks: vi.fn(async () => chunks), play: vi.fn<Parameters<typeof createPlayAnywherePlayback>[0]['play']>(() => true) })
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(yes => { resolve = yes }); return { promise, resolve } }
it('dispatches verified chunk boundaries with the original full sequence and a live guard; beginning uses zero', async () => {
  const config = setup(), scope = createPlayAnywherePlayback(config)
  const api = { captureViewport: vi.fn(async () => capture) }
  expect(await scope.playFromHere(api)).toEqual({ status: 'dispatched' })
  expect(config.play).toHaveBeenCalledWith(chunks, options, 1, expect.any(Function))
  const guard = config.play.mock.calls[0]![3]
  expect(guard()).toBe(true); scope.cancel(); expect(guard()).toBe(false)
  await scope.playFromBeginning(); expect(config.play).toHaveBeenLastCalledWith(chunks, options, 0, expect.any(Function))
})
it('requires completed saved audio and never changes playback when capture or resolution fails', async () => {
  const config = setup(), scope = createPlayAnywherePlayback(config)
  expect(await scope.playFromHere({ captureViewport: async () => ({ status: 'unavailable', reason: 'no-visible-text' }) })).toMatchObject({ reason: 'no-visible-text' })
  expect(config.play).not.toHaveBeenCalled()
  config.play.mockClear(); config.resolve.mockResolvedValue({ status: 'unavailable', reason: 'unmapped' })
  expect(await scope.playFromHere({ captureViewport: async () => capture })).toMatchObject({ reason: 'unmapped' })
  expect(config.play).not.toHaveBeenCalled()
  const disabled = createPlayAnywherePlayback({ ...config, complete: false })
  expect(await disabled.playFromBeginning()).toMatchObject({ reason: 'not-ready' })
})
it.each(['capture', 'resolve'])('keeps only the newest action during %s and honors cancellation', async phase => {
  const config = setup(), scope = createPlayAnywherePlayback(config)
  const pendingCapture = deferred<ViewerTextTargetResult>(), pendingTarget = deferred<PreparedTextTargetResult>()
  const api: ViewerTextTargetApi = { captureViewport: async () => phase === 'capture' ? pendingCapture.promise : capture }
  if (phase === 'resolve') config.resolve.mockReturnValueOnce(pendingTarget.promise)
  const old = scope.playFromHere(api)
  await Promise.resolve()
  await scope.playFromBeginning()
  pendingCapture.resolve(capture); pendingTarget.resolve(target)
  expect(await old).toMatchObject({ reason: 'stale' })
  expect(config.play).toHaveBeenCalledOnce()
  const other = scope.playFromHere({ captureViewport: async () => capture }); scope.cancel()
  expect(await other).toMatchObject({ reason: 'stale' })
})
it('contains errors and rejects stale or cross-document resolved targets', async () => {
  const config = setup(), scope = createPlayAnywherePlayback(config)
  expect(await scope.playFromHere({ captureViewport: async () => { throw new Error('capture error') } })).toMatchObject({ status: 'error' })
  config.resolve.mockResolvedValue({ ...target, documentUrl: '/other.html' })
  expect(await scope.playFromHere({ captureViewport: async () => capture })).toMatchObject({ reason: 'stale' })
  expect(config.play).not.toHaveBeenCalled()
})
it('disposes action scopes on document/options/source changes and unmount before pending results can dispatch', async () => {
  let controls!: ReturnType<typeof usePlayAnywherePlayback>
  const config = setup()
  function Harness(props: Parameters<typeof usePlayAnywherePlayback>[0]) {
    const result = usePlayAnywherePlayback(props); useLayoutEffect(() => { controls = result }, [result]); return null
  }
  const element = document.createElement('div'); document.body.append(element); const app = createRoot(element)
  try {
    await act(async () => app.render(<Harness {...config} />))
    for (const overrides of [{ options: { ...options, voice: 'af_bella' as const } }, { complete: false }, { options: { ...options, documentUrl: '/other.html' } }]) {
      const pending = deferred<ViewerTextTargetResult>()
      const old = controls.playFromHere({ captureViewport: () => pending.promise })
      await act(async () => app.render(<Harness {...config} {...overrides} />))
      pending.resolve(capture); expect(await old).toMatchObject({ reason: 'stale' })
      await act(async () => app.render(<Harness {...config} />))
    }
  } finally { await act(async () => app.unmount()) }
  expect(await controls.playFromBeginning()).toMatchObject({ reason: 'not-ready' })
  expect(config.play).not.toHaveBeenCalled()
})
