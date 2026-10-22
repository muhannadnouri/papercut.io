// @vitest-environment jsdom
import { act, useLayoutEffect } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, expect, it, vi } from 'vitest'
import { usePlayAnywhereReader } from './usePlayAnywhereReader'
import type { PlaybackTargetResult } from './usePlayAnywherePlayback'
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
afterEach(() => document.body.replaceChildren())
async function setup() {
  let reader!: ReturnType<typeof usePlayAnywhereReader>, started: (() => void) | undefined
  const config = { documentUrl: 'one', active: true,
    playFromHere: vi.fn(async (_api: unknown, callback?: () => void): Promise<PlaybackTargetResult> => { started = callback; return { status: 'dispatched' } }),
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
