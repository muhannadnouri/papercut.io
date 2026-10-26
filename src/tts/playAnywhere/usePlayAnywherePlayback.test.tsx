// @vitest-environment jsdom
import { act, useLayoutEffect } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, expect, it, vi } from 'vitest'
import { createPlayAnywherePlayback, usePlayAnywherePlayback } from './usePlayAnywherePlayback'
import type { PreparedTextTargetResult } from './prepareTextTarget'
import type { ViewerTextTargetApi, ViewerTextTargetResult } from '../../viewers/types'
import type { TtsOptions } from '../types'
import { createTextTargetPreparation } from './prepareTextTarget'
import { chunkAudiobookSaveHtmlWithSpans } from '../utils/text'

Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
afterEach(() => document.body.replaceChildren())
const options: TtsOptions = { documentUrl: '/book.html', modelId: 'kokoro', voice: 'af_heart', speed: 1 }
const chunks = [{ id: 'a', text: 'A' }, { id: 'b', text: 'B' }]
const capture: ViewerTextTargetResult = { status: 'captured', point: { format: 'html', segmentIndex: 0, offset: 0 }, segmentTexts: ['A B'], segmentLengths: [3], isCurrent: () => true }
const target: PreparedTextTargetResult = { status: 'resolved', documentUrl: '/book.html', chunks, chunkIndex: 1, chunkId: 'b', isCurrent: () => true }
const setup = () => ({ complete: true, options, resolve: vi.fn<Parameters<typeof createPlayAnywherePlayback>[0]['resolve']>(async () => target), getChunks: vi.fn(async () => chunks),
  play: vi.fn<Parameters<typeof createPlayAnywherePlayback>[0]['play']>(() => Promise.resolve('started')), commandSignal: () => new AbortController().signal })
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>(yes => { resolve = yes }); return { promise, resolve } }
it('dispatches verified chunk boundaries with the original full sequence; beginning uses zero', async () => {
  const config = setup(), scope = createPlayAnywherePlayback(config)
  const api = { captureViewport: vi.fn(async () => capture) }
  expect(await scope.playFromHere(api)).toMatchObject({ status: 'dispatched' })
  expect(config.play).toHaveBeenCalledWith(chunks, options, 1)
  await scope.playFromBeginning(); expect(config.play).toHaveBeenLastCalledWith(chunks, options, 0)
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

it('targets imported saved chunks after playing from the beginning despite clause spacing elsewhere in the book', async () => {
  const text = 'A narrated clause with repeated words '.repeat(4).trim() + ',“Another clause follows without a source space”, ' +
    'A final long clause keeps the sentence above the splitting limit '.repeat(8).trim() + '.'
  const docContent = `<p>${text}</p><p>Later viewport passage.</p>`
  const saved = chunkAudiobookSaveHtmlWithSpans(docContent).map(({ text }, i) => ({ id: 'saved-' + i, text, textHash: 'keep-' + i }))
  const original = structuredClone(saved), documentUrl = '/user-uploads/ab.html'
  const preparation = createTextTargetPreparation({ documentUrl, docContent, chunks: saved, getChunks: async () => saved,
    getPdfSegments: async () => [], getPdfPageText: async () => { throw new Error('HTML must not read PDF') } })
  const config = { ...setup(), options: { ...options, documentUrl }, resolve: preparation.resolve, getChunks: async () => saved }
  const scope = createPlayAnywherePlayback(config)
  expect(await scope.playFromBeginning()).toMatchObject({ status: 'dispatched' })
  expect(config.play).toHaveBeenLastCalledWith(saved, config.options, 0)
  const visible: ViewerTextTargetResult = { status: 'captured', point: { format: 'html', segmentIndex: 1, offset: 2 },
    segmentTexts: [text, 'Later viewport passage.'], segmentLengths: [text.length, 23], isCurrent: () => true }
  expect(await scope.playFromHere({ captureViewport: async () => visible })).toMatchObject({ status: 'dispatched' })
  expect(config.play).toHaveBeenLastCalledWith(saved, config.options, saved.length - 1)
  expect(saved).toEqual(original)
  scope.dispose(); preparation.dispose()
})
it.each(['capture', 'resolve'])('keeps only the newest action during %s', async phase => {
  const config = setup(), scope = createPlayAnywherePlayback(config)
  const pendingCapture = deferred<ViewerTextTargetResult>(), pendingTarget = deferred<PreparedTextTargetResult>()
  const api: ViewerTextTargetApi = { captureViewport: async () => phase === 'capture' ? pendingCapture.promise : capture }
  if (phase === 'resolve') config.resolve.mockReturnValueOnce(pendingTarget.promise)
  const old = scope.playFromHere(api)
  await Promise.resolve()
  await scope.playFromBeginning()
  pendingCapture.resolve(capture); pendingTarget.resolve(target)
  expect(await old).toEqual({ status: 'cancelled', reason: 'superseded' })
  expect(config.play).toHaveBeenCalledOnce()
})
it.each(['command', 'dispose', 'newer'] as const)('reports a %s before dispatch as cancelled, never as a changed passage', async action => {
  const command = new AbortController()
  const config = { ...setup(), commandSignal: () => command.signal }, scope = createPlayAnywherePlayback(config)
  const pending = deferred<PreparedTextTargetResult>()
  config.resolve.mockReturnValueOnce(pending.promise)
  const old = scope.playFromHere({ captureViewport: async () => capture })
  await Promise.resolve()
  const signal = config.resolve.mock.calls[0]![2]!
  if (action === 'command') command.abort()
  else if (action === 'dispose') scope.dispose()
  else void scope.playFromBeginning()
  expect(signal.aborted).toBe(true)
  pending.resolve(target)
  expect(await old).toEqual({ status: 'cancelled', reason: action === 'dispose' ? 'disposed' : 'superseded' })
  expect(config.play).toHaveBeenCalledTimes(action === 'newer' ? 1 : 0)
})
it('checks passage validity once before dispatch and returns the player start ticket', async () => {
  const started = Promise.resolve('started' as const)
  const config = { ...setup(), commandSignal: () => new AbortController().signal }, scope = createPlayAnywherePlayback(config)
  config.play.mockReturnValue(started)
  let valid = false
  config.resolve.mockResolvedValue({ ...target, isCurrent: () => valid })
  expect(await scope.playFromHere({ captureViewport: async () => capture })).toEqual({ status: 'unavailable', reason: 'stale' })
  expect(config.play).not.toHaveBeenCalled()
  valid = true
  const result = await scope.playFromHere({ captureViewport: async () => capture })
  expect(result).toEqual({ status: 'dispatched', started })
  expect(config.play).toHaveBeenCalledWith(chunks, options, 1)
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
      pending.resolve(capture); expect(await old).toEqual({ status: 'cancelled', reason: 'disposed' })
      await act(async () => app.render(<Harness {...config} />))
    }
  } finally { await act(async () => app.unmount()) }
  expect(await controls.playFromBeginning()).toMatchObject({ reason: 'not-ready' })
  expect(config.play).not.toHaveBeenCalled()
})
