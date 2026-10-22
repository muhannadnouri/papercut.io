// @vitest-environment jsdom
import { beforeEach, expect, it, vi } from 'vitest'
import * as plugin from 'tauri-plugin-native-audio-api'
vi.mock('tauri-plugin-native-audio-api', () => ({ initialize: vi.fn(), setSource: vi.fn(), play: vi.fn(), pause: vi.fn(), seekTo: vi.fn(), setRate: vi.fn(), getState: vi.fn(), dispose: vi.fn() }))
let audio: typeof import('./nativeMobileAudio')
const state = { status: 'idle', currentTime: 0, duration: 10, isPlaying: false, buffering: false, rate: 1 } as plugin.NativeAudioState
function deferred() {
  let resolve!: (value: plugin.NativeAudioState) => void
  const promise = new Promise<plugin.NativeAudioState>(yes => { resolve = yes })
  return { promise, resolve }
}
beforeEach(async () => {
  vi.resetModules(); vi.resetAllMocks()
  for (const method of [plugin.initialize, plugin.setSource, plugin.play, plugin.pause, plugin.seekTo, plugin.setRate, plugin.getState]) vi.mocked(method).mockResolvedValue(state)
  audio = await import('./nativeMobileAudio')
})
it('checks cancellation inside the lifecycle queue and continues after an aborted command', async () => {
  const held = deferred(); vi.mocked(plugin.setRate).mockReturnValueOnce(held.promise)
  const previous = audio.setNativeAudioRate(1)
  await vi.waitFor(() => expect(plugin.setRate).toHaveBeenCalled())
  let current = true
  const next = audio.playNativeAudio(() => current)
  const aborted = expect(next).rejects.toMatchObject({ name: 'AbortError' })
  current = false; held.resolve(state)
  await previous; await aborted
  expect(plugin.play).not.toHaveBeenCalled()
  await audio.playNativeAudio(); expect(plugin.play).toHaveBeenCalledOnce()
})
it('rejects a stale source after shared initialization finishes', async () => {
  const held = deferred(); vi.mocked(plugin.initialize).mockReturnValueOnce(held.promise)
  let current = true
  const source = audio.setNativeAudioSource({ src: 'book.wav' }, () => current)
  const aborted = expect(source).rejects.toMatchObject({ name: 'AbortError' })
  await vi.waitFor(() => expect(plugin.initialize).toHaveBeenCalled())
  current = false; held.resolve(state); await aborted
  expect(plugin.setSource).not.toHaveBeenCalled()
})
it('guards seek, rate and pause mutations without changing unguarded behavior', async () => {
  await expect(audio.seekNativeAudio(5, () => false)).rejects.toMatchObject({ name: 'AbortError' })
  await expect(audio.setNativeAudioRate(1.2, () => false)).rejects.toMatchObject({ name: 'AbortError' })
  await expect(audio.pauseNativeAudio(() => false)).rejects.toMatchObject({ name: 'AbortError' })
  expect(plugin.seekTo).not.toHaveBeenCalled(); expect(plugin.setRate).not.toHaveBeenCalled(); expect(plugin.pause).not.toHaveBeenCalled()
  await audio.seekNativeAudio(5); expect(plugin.seekTo).toHaveBeenCalledWith(5)
})
