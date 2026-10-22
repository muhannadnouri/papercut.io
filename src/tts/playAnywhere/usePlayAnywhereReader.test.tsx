// @vitest-environment jsdom
import { act, useLayoutEffect } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, expect, it, vi } from 'vitest'
import { createPlayAnywherePlayback } from './usePlayAnywherePlayback'
import type { PreparedTextTargetResult } from './prepareTextTarget'
import { usePlayAnywhereReader } from './usePlayAnywhereReader'
import type { ViewerTextTargetApi, ViewerTextTargetResult } from '../../viewers/types'
import type { PlaybackTargetResult } from './usePlayAnywherePlayback'
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
afterEach(() => document.body.replaceChildren())
async function setup() {
  let reader!: ReturnType<typeof usePlayAnywhereReader>, started: (() => void) | undefined
  const config = { documentUrl: 'one', active: true,
    playFromHere: vi.fn(async (_api: ViewerTextTargetApi, callback?: () => void): Promise<PlaybackTargetResult> => { started = callback; return { status: 'dispatched' } }),
    playFromBeginning: vi.fn(async (callback?: () => void): Promise<PlaybackTargetResult> => { started = callback; return { status: 'dispatched' } }), cancelTargetPlayback: vi.fn() }
  function Harness(props: typeof config) { const result = usePlayAnywhereReader(props); useLayoutEffect(() => { reader = result }); return null }
  const element = document.createElement('div'); document.body.append(element); const app = createRoot(element)
  const render = async (overrides: Partial<typeof config> = {}) => { await act(async () => app.render(<Harness {...config} {...overrides} />)) }
  await render()
  reader.register({ captureViewport: async () => ({ status: 'unavailable', reason: 'not-ready' }) })
  return { config, render, get reader() { return reader }, started: () => started?.(), unmount: () => act(async () => app.unmount()) }
}
it('keeps browsing independent of playback and Return reveals without dispatching audio', async () => {
  const app = await setup()
  try {
    const policy = app.reader.followPolicy
    await act(async () => app.reader.browse())
    expect(policy.allowsReveal()).toBe(false)
    expect(app.reader.browsing).toBe(true)
    await act(async () => app.reader.returnToAudio())
    expect(app.reader.followPolicy.allowsReveal()).toBe(true)
    expect(app.reader.followPolicy.revision).toBeGreaterThan(policy.revision)
    expect(app.config.playFromHere).not.toHaveBeenCalled(); expect(app.config.playFromBeginning).not.toHaveBeenCalled()
  } finally { await app.unmount() }
})
it('restores follow only after play succeeds and lets later browse input win', async () => {
  const app = await setup()
  try {
    await act(async () => app.reader.browse()); await act(async () => app.reader.playFromHere())
    expect(app.reader.browsing).toBe(true); expect(app.reader.pending).toBe(true)
    await act(async () => app.started()); expect(app.reader.browsing).toBe(false); expect(app.reader.pending).toBe(false)
    await act(async () => app.reader.browse()); await act(async () => app.reader.playFromHere())
    await act(async () => app.reader.browse()); await act(async () => app.started())
    expect(app.reader.browsing).toBe(true)
    await act(async () => app.reader.playFromBeginning()); await act(async () => app.started())
    expect(app.reader.browsing).toBe(false)
  } finally { await app.unmount() }
})
it('reports unavailable without fallback, clears sessions on stop and ignores old document acknowledgements', async () => {
  const app = await setup()
  try {
    app.config.playFromHere.mockResolvedValueOnce({ status: 'unavailable', reason: 'unmapped' })
    await act(async () => app.reader.playFromHere())
    expect(app.reader.feedback).toBe('unmapped'); expect(app.config.playFromBeginning).not.toHaveBeenCalled()
    await act(async () => app.reader.playFromHere()); await act(async () => app.reader.browse())
    await app.render({ documentUrl: 'two' }); await act(async () => app.reader.browse()); await act(async () => app.started())
    expect(app.reader.browsing).toBe(true)
    await app.render({ documentUrl: 'two', active: false }); expect(app.reader.browsing).toBe(false)
    expect(app.reader.feedback).toBeNull(); expect(app.reader.followPolicy.active).toBe(false)
  } finally { await app.unmount() }
})
it('cancels UI work and pending completion when transport takes over', async () => {
  const app = await setup()
  try {
    await act(async () => app.reader.browse()); await act(async () => app.reader.playFromHere())
    await act(async () => app.reader.cancel()); await act(async () => app.started())
    expect(app.reader.browsing).toBe(true); expect(app.config.cancelTargetPlayback).toHaveBeenCalled()
  } finally { await app.unmount() }
})

it('snapshots selection before focus loss, keeps viewport independent and invalidates a pending selection on reader input', async () => {
  const app = await setup()
  const captured: ViewerTextTargetResult = { status: 'captured', point: { format: 'html', segmentIndex: 3, offset: 7 }, segmentTexts: ['A'], segmentLengths: [1], isCurrent: () => true }
  const captureSelection = vi.fn((): ViewerTextTargetResult => captured), captureViewport = vi.fn(async () => ({ status: 'unavailable', reason: 'no-visible-text' } as const))
  const seen: ViewerTextTargetResult[] = []
  app.config.playFromHere.mockImplementation(async api => {
    seen.push(await api.captureViewport()); return { status: 'dispatched' }
  })
  try {
    app.reader.register({ captureViewport, captureSelection })
    app.reader.snapshotSelection()
    captureSelection.mockReturnValue({ status: 'unavailable', reason: 'no-selection' })
    app.reader.snapshotSelection()
    await act(async () => app.reader.playFromSelection())
    expect(seen[0]).toMatchObject({ status: 'captured', point: { segmentIndex: 3, offset: 7 } })
    expect(captureViewport).not.toHaveBeenCalled()
    if (seen[0].status !== 'captured') throw new Error('Expected snapshot')
    expect(seen[0].isCurrent()).toBe(true)
    await act(async () => app.reader.selectionActions.invalidate())
    expect(seen[0].isCurrent()).toBe(false)
    await act(async () => app.reader.playFromHere()); expect(captureViewport).toHaveBeenCalledOnce()
    await act(async () => app.reader.playFromSelection()); expect(seen.at(-1)).toMatchObject({ reason: 'no-selection' })
  } finally { await app.unmount() }
})

it('does not dispatch a selected target invalidated while verified preparation is pending', async () => {
  const app = await setup(), play = vi.fn(() => true), chunks = [{ id: 'saved', text: 'Selected passage' }]
  let finish!: (value: PreparedTextTargetResult) => void
  let captured!: Extract<ViewerTextTargetResult, { status: 'captured' }>
  const preparation = new Promise<PreparedTextTargetResult>(resolve => { finish = resolve })
  const playback = createPlayAnywherePlayback({ complete: true, options: { documentUrl: 'one', modelId: 'kokoro', voice: 'af_heart', speed: 1 }, play,
    resolve: async (_url, capture) => { if (capture.status !== 'captured') throw new Error('Missing selection'); captured = capture; return preparation }, getChunks: async () => chunks })
  app.config.playFromHere.mockImplementation(playback.playFromHere)
  app.reader.register({ captureViewport: async () => ({ status: 'unavailable', reason: 'not-ready' }), captureSelection: () => ({ status: 'captured', point: { format: 'html', segmentIndex: 0, offset: 3 }, segmentTexts: ['Selected passage'], segmentLengths: [16], isCurrent: () => true }) })
  try {
    await act(async () => app.reader.playFromSelection())
    expect(captured).toBeDefined(); app.reader.clearSelection()
    expect(captured.isCurrent()).toBe(true)
    app.reader.selectionActions.invalidate()
    await act(async () => finish({ status: 'resolved', documentUrl: 'one', chunks, chunkIndex: 0, chunkId: 'saved', isCurrent: captured.isCurrent }))
    expect(play).not.toHaveBeenCalled(); expect(app.reader.feedback).toBe('stale')
  } finally { playback.dispose(); await app.unmount() }
})
