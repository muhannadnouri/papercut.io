// @vitest-environment jsdom
import { act, useLayoutEffect } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { useTtsPlayer } from './useTtsPlayer'
import * as api from '../api/nativeTts'
import * as native from '../playback/nativeMobileAudio'
import type { TtsChunk, TtsOptions } from '../types'

vi.mock('../api/nativeTts', async importOriginal => ({ ...await importOriginal<typeof import('../api/nativeTts')>(), getNativeTtsCapabilities: vi.fn(), getNativeSavedAudiobookChunk: vi.fn(), prepareNativeAudiobookPlayback: vi.fn(), resetNativeTtsCapabilities: vi.fn() }))
vi.mock('../playback/nativeMobileAudio', () => ({ initializeNativeAudio: vi.fn(), setNativeAudioSource: vi.fn(), setNativeAudioRate: vi.fn(), seekNativeAudio: vi.fn(), playNativeAudio: vi.fn(), pauseNativeAudio: vi.fn(), getNativeAudioState: vi.fn(), stopNativeAudio: vi.fn(), disposeNativeAudio: vi.fn() }))
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
type SavedChunk = NonNullable<Awaited<ReturnType<typeof api.getNativeSavedAudiobookChunk>>>
const chunks: TtsChunk[] = Array.from({ length: 8 }, (_, i) => ({ id: 'c' + i, text: 'Chunk ' + i }))
const options: TtsOptions = { documentUrl: '/book.html', modelId: 'kokoro', voice: 'af_heart', speed: 1 }
const playback: api.NativeAudiobookPlayback = { title: 'Book', audioUrl: 'track.wav', audioDurationSec: 36, wavBytes: 100,
  chunks: chunks.map((chunk, index) => ({ index, chunkId: chunk.id, startSec: index * (index + 1) / 2, durationSec: index + 1 })) }
let audio: FakeAudio
let audios: FakeAudio[]
let player: ReturnType<typeof useTtsPlayer>
let frames: Map<number, FrameRequestCallback>
let unmount: (() => Promise<void>) | undefined
let position: number, playing: boolean
const nativeState = () => ({ status: playing ? 'playing' : 'idle', currentTime: position, duration: 36, isPlaying: playing, buffering: false, rate: 1 }) as native.NativeAudioState
class FakeAudio extends EventTarget {
  src = ''; currentTime = 0; duration = 5; playbackRate = 1; preload = ''; ended = false
  pause = vi.fn()
  play = vi.fn(async () => {})
  constructor() { super(); audios.push(this) }
}
function deferred<T>() {
  let resolve!: (value: T) => void, reject!: (error: Error) => void
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no })
  return { promise, resolve, reject }
}
async function mount(platform = 'linux') {
  vi.mocked(api.getNativeTtsCapabilities).mockResolvedValue({ platform } as api.NativeTtsCapabilities)
  function Harness() { const result = useTtsPlayer(); useLayoutEffect(() => { player = result }, [result]); return null }
  const element = document.createElement('div'); document.body.append(element)
  const root = createRoot(element)
  await act(async () => root.render(<Harness />))
  audio = audios.at(-1)!
  unmount = async () => { await act(async () => root.unmount()); unmount = undefined }
}
async function frame() {
  await act(async () => { const batch = [...frames.values()]; frames.clear(); batch.forEach(callback => callback(performance.now())) })
}
async function start(index?: number) { await act(async () => { player.speak(chunks, options, index) }); await frame() }
beforeEach(() => {
  vi.resetAllMocks(); audios = []; position = 0; playing = false; frames = new Map(); let id = 0
  vi.stubGlobal('Audio', FakeAudio)
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => { frames.set(++id, callback); return id })
  vi.stubGlobal('cancelAnimationFrame', (key: number) => frames.delete(key))
  vi.stubGlobal('URL', class extends URL { static createObjectURL = vi.fn(() => 'blob:' + ++id); static revokeObjectURL = vi.fn() })
  vi.mocked(api.getNativeSavedAudiobookChunk).mockImplementation(async () => ({ wav: new ArrayBuffer(64) }) as SavedChunk)
  vi.mocked(api.prepareNativeAudiobookPlayback).mockResolvedValue(playback)
  vi.mocked(native.initializeNativeAudio).mockImplementation(async () => nativeState())
  vi.mocked(native.setNativeAudioSource).mockImplementation(async () => { position = 0; playing = false; return nativeState() })
  vi.mocked(native.setNativeAudioRate).mockImplementation(async () => nativeState())
  vi.mocked(native.seekNativeAudio).mockImplementation(async target => { position = target; return nativeState() })
  vi.mocked(native.playNativeAudio).mockImplementation(async () => { playing = true; return nativeState() })
  vi.mocked(native.pauseNativeAudio).mockImplementation(async () => { playing = false; return nativeState() })
  vi.mocked(native.getNativeAudioState).mockImplementation(async () => nativeState())
  vi.mocked(native.stopNativeAudio).mockResolvedValue()
  vi.mocked(native.disposeNativeAudio).mockResolvedValue()
})
afterEach(async () => { await unmount?.(); vi.restoreAllMocks(); vi.unstubAllGlobals(); document.body.replaceChildren() })

describe('desktop target playback', () => {
  it('starts the canonical target first, keeps the complete sequence and prefetches only the next two chunks', async () => {
    await mount(); await start(4)
    expect(vi.mocked(api.getNativeSavedAudiobookChunk).mock.calls.map(call => call[2])).toEqual([4, 5, 6])
    expect(player.state.currentChunkIndex).toBe(4)
    expect(player.state.chunks).toEqual(chunks)
    expect(audio.currentTime).toBe(0)
    expect(audio.play).toHaveBeenCalledOnce()
    await act(async () => audio.dispatchEvent(new Event('ended'))); await frame()
    expect(player.state.currentChunkIndex).toBe(5)
  })
  it('defaults to zero and rejects malformed indices before touching the current audio', async () => {
    await mount(); await start()
    const calls = audio.pause.mock.calls.length
    for (const index of [-1, NaN, 0.1, Infinity, chunks.length]) {
      expect(player.speak(chunks, options, index)).toBe(false)
      expect(player.playFromChunk(chunks, options, index)).toBe(false)
    }
    expect(audio.pause).toHaveBeenCalledTimes(calls)
    expect(player.state.currentChunkIndex).toBe(0)
  })
  it('coalesces targets and ignores superseded loads, including old failures', async () => {
    await mount()
    const pending = deferred<SavedChunk | null>()
    vi.mocked(api.getNativeSavedAudiobookChunk).mockReturnValueOnce(pending.promise)
    await start(1)
    await act(async () => { player.playFromChunk(chunks, options, 3); player.playFromChunk(chunks, options, 6) }); await frame()
    await act(async () => pending.reject(new Error('old corrupt chunk'))); await frame()
    expect(player.state.currentChunkIndex).toBe(6)
    expect(player.state.status).toBe('playing')
  })
  it.each(['stop', 'stale', 'unmount'])('does not play a late chunk after %s', async action => {
    await mount(); const pending = deferred<SavedChunk | null>()
    vi.mocked(api.getNativeSavedAudiobookChunk).mockReturnValueOnce(pending.promise)
    let current = true
    await act(async () => player.speak(chunks, options, 4, () => current)); await frame()
    if (action === 'stop') await act(async () => player.stop())
    if (action === 'stale') current = false
    if (action === 'unmount') await unmount!()
    await act(async () => pending.resolve({ wav: new ArrayBuffer(64) } as SavedChunk))
    expect(audio.play).not.toHaveBeenCalled()
  })
  it('resumes the current desktop time and ignores an old ended event while loading a different target', async () => {
    await mount(); await start(2); audio.currentTime = 1.25
    await act(async () => player.pause()); await act(async () => player.resume())
    expect(audio.currentTime).toBe(1.25)
    const pending = deferred<SavedChunk | null>()
    vi.mocked(api.getNativeSavedAudiobookChunk).mockReturnValueOnce(pending.promise)
    await act(async () => player.playFromChunk(chunks, options, 6)); await frame()
    await act(async () => audio.dispatchEvent(new Event('ended')))
    await act(async () => pending.resolve({ wav: new ArrayBuffer(64) } as SavedChunk)); await frame()
    expect(player.state.currentChunkIndex).toBe(6)
  })
  it('does not let a late capability preload alter a newer playback job', async () => {
    await mount(); const pending = deferred<api.NativeTtsCapabilities>()
    vi.mocked(api.getNativeTtsCapabilities).mockReturnValueOnce(pending.promise)
    await act(async () => player.preload()); await start(4)
    await act(async () => pending.resolve({ platform: 'android', available: false, reason: 'old reply' } as api.NativeTtsCapabilities))
    expect(player.state.status).toBe('playing'); expect(player.state.currentChunkIndex).toBe(4)
  })
  it('surfaces missing chunks and decoder rejection without falling back to zero', async () => {
    await mount(); vi.mocked(api.getNativeSavedAudiobookChunk).mockResolvedValueOnce(null)
    await start(4); expect(player.state.status).toBe('error'); expect(audio.play).not.toHaveBeenCalled()
    audio.play.mockRejectedValueOnce(new Error('corrupt WAV'))
    await start(4); expect(player.state.status).toBe('error')
    expect(vi.mocked(api.getNativeSavedAudiobookChunk).mock.calls.every(call => call[2] >= 4)).toBe(true)
  })
})

describe('native target playback', () => {
  it('sets source/rate and seeks the real target start before playing, with all canonical chunks', async () => {
    await mount('android'); await start(4)
    expect(api.prepareNativeAudiobookPlayback).toHaveBeenCalledWith(options.documentUrl, chunks, options)
    expect(native.seekNativeAudio).toHaveBeenCalledWith(10, expect.any(Function))
    expect(vi.mocked(native.setNativeAudioSource).mock.invocationCallOrder[0]).toBeLessThan(vi.mocked(native.setNativeAudioRate).mock.invocationCallOrder[0])
    expect(vi.mocked(native.seekNativeAudio).mock.invocationCallOrder[0]).toBeLessThan(vi.mocked(native.playNativeAudio).mock.invocationCallOrder[0])
    expect(player.state.currentChunkIndex).toBe(4)
    expect(player.state.currentChunkTime).toBe(0)
  })
  it('reuses an active paused session for explicit seek-and-play while Resume preserves time and ordinary jump stays paused', async () => {
    await mount('ios'); await start(2)
    await act(async () => player.pause()); position = 4.25
    await act(async () => player.resume())
    expect(position).toBe(4.25)
    await act(async () => player.pause())
    const calls = vi.mocked(native.playNativeAudio).mock.calls.length
    await act(async () => player.jumpToChunk(3)); await frame()
    expect(native.playNativeAudio).toHaveBeenCalledTimes(calls)
    expect(player.state.status).toBe('paused')
    await act(async () => player.playFromChunk(chunks, options, 5)); await frame()
    expect(position).toBe(15); expect(player.state.status).toBe('playing')
    expect(api.prepareNativeAudiobookPlayback).toHaveBeenCalledOnce()
    expect(native.setNativeAudioSource).toHaveBeenCalledOnce()
  })
  it('starts a new session when canonical hashes or audio options change', async () => {
    await mount('android'); await start(2)
    const changed = chunks.map(chunk => ({ ...chunk, textHash: 'new-' + chunk.id }))
    await act(async () => player.playFromChunk(changed, options, 5))
    expect(api.prepareNativeAudiobookPlayback).toHaveBeenCalledTimes(2)
    await act(async () => player.playFromChunk(changed, { ...options, voice: 'af_bella' }, 6))
    expect(api.prepareNativeAudiobookPlayback).toHaveBeenCalledTimes(3)
  })
  it('keeps only the newest active seek and does not let a late previous-book reply alter the replacement', async () => {
    await mount('android'); await start(1)
    const pending = deferred<native.NativeAudioState>()
    vi.mocked(native.seekNativeAudio).mockReturnValueOnce(pending.promise)
    await act(async () => player.playFromChunk(chunks, options, 2)); await frame()
    await act(async () => { player.playFromChunk(chunks, options, 4); player.playFromChunk(chunks, options, 6) }); await frame()
    await act(async () => pending.resolve(nativeState())); await frame()
    expect(position).toBe(21); expect(player.state.currentChunkIndex).toBe(6)
    const previous = deferred<native.NativeAudioState>()
    vi.mocked(native.seekNativeAudio).mockReturnValueOnce(previous.promise)
    await act(async () => player.jumpToChunk(3)); await frame()
    await act(async () => player.speak(chunks, { ...options, documentUrl: '/other.html' }, 5))
    await act(async () => previous.resolve({ ...nativeState(), currentTime: 0 }))
    expect(player.state.currentChunkIndex).toBe(5)
  })
  it.each(['prepare', 'initialize', 'source', 'rate', 'seek'])('stops during %s without issuing a later play', async phase => {
    await mount('android')
    const pending = deferred<never>()
    const operation = { prepare: api.prepareNativeAudiobookPlayback, initialize: native.initializeNativeAudio, source: native.setNativeAudioSource, rate: native.setNativeAudioRate, seek: native.seekNativeAudio }[phase]!
    vi.mocked(operation).mockReturnValueOnce(pending.promise)
    await start(4)
    await act(async () => player.stop())
    await act(async () => pending.resolve((phase === 'prepare' ? playback : nativeState()) as never))
    expect(native.playNativeAudio).not.toHaveBeenCalled()
    expect(player.state.status).toBe('idle')
  })
  it.each(['prepare', 'initialize', 'source', 'rate', 'seek', 'play'])('reports a current %s failure', async phase => {
    await mount('android')
    const operation = { prepare: api.prepareNativeAudiobookPlayback, initialize: native.initializeNativeAudio, source: native.setNativeAudioSource, rate: native.setNativeAudioRate, seek: native.seekNativeAudio, play: native.playNativeAudio }[phase]!
    vi.mocked(operation).mockRejectedValueOnce(new Error(phase + ' failed'))
    await start(4); expect(player.state.status).toBe('error')
  })
  it('preserves rapid relative skips while coalescing bridge navigation', async () => {
    await mount('android'); await start(1)
    await act(async () => { player.skipForward(); player.skipForward(); player.skipForward() }); await frame()
    expect(player.state.currentChunkIndex).toBe(4)
  })
  it('does not play before initial preparation finishes when Pause/Resume occurs during loading', async () => {
    await mount('android'); const pending = deferred<api.NativeAudiobookPlayback>()
    vi.mocked(api.prepareNativeAudiobookPlayback).mockReturnValueOnce(pending.promise)
    await start(4); await act(async () => player.pause()); await act(async () => player.resume())
    expect(native.playNativeAudio).not.toHaveBeenCalled()
    await act(async () => pending.resolve(playback))
    expect(native.seekNativeAudio).toHaveBeenLastCalledWith(10, expect.any(Function))
    expect(player.state.currentChunkIndex).toBe(4)
    expect(player.state.status).toBe('playing')
  })
  it('ignores late play replies after Stop', async () => {
    await mount('android'); const pending = deferred<native.NativeAudioState>()
    vi.mocked(native.playNativeAudio).mockReturnValueOnce(pending.promise)
    await start(4); await act(async () => player.stop())
    await act(async () => pending.resolve({ ...nativeState(), currentTime: 10, isPlaying: true, status: 'playing' }))
    expect(player.state.status).toBe('idle'); expect(player.state.currentChunkIndex).toBeNull()
  })
  it('rejects a target that becomes stale during native preparation', async () => {
    await mount('android'); const pending = deferred<api.NativeAudiobookPlayback>(); let current = true
    vi.mocked(api.prepareNativeAudiobookPlayback).mockReturnValueOnce(pending.promise)
    await act(async () => player.speak(chunks, options, 4, () => current)); current = false
    await act(async () => pending.resolve(playback))
    expect(native.setNativeAudioSource).not.toHaveBeenCalled(); expect(native.playNativeAudio).not.toHaveBeenCalled()
    expect(player.state.status).toBe('idle')
  })
  it('does not enqueue a seek after Stop during foreground synchronization', async () => {
    await mount('android'); await start(1)
    const visibility = vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('hidden')
    await act(async () => document.dispatchEvent(new Event('visibilitychange')))
    const pending = deferred<native.NativeAudioState>()
    vi.mocked(native.getNativeAudioState).mockReturnValueOnce(pending.promise)
    visibility.mockReturnValue('visible')
    await act(async () => document.dispatchEvent(new Event('visibilitychange')))
    const seekCount = vi.mocked(native.seekNativeAudio).mock.calls.length
    await act(async () => player.playFromChunk(chunks, options, 4)); await act(async () => player.stop())
    await act(async () => pending.resolve(nativeState())); await frame()
    expect(native.seekNativeAudio).toHaveBeenCalledTimes(seekCount)
  })
  it('rejects a track for a different canonical sequence before setting its source', async () => {
    await mount('android'); vi.mocked(api.prepareNativeAudiobookPlayback).mockResolvedValue({ ...playback, chunks: playback.chunks.slice(1) })
    await start(4); expect(player.state.status).toBe('error'); expect(native.setNativeAudioSource).not.toHaveBeenCalled()
  })
})

it.each(['linux', 'android'])('acknowledges a target only after play succeeds on %s and never after cancellation', async platform => {
  await mount(platform)
  const onStarted = vi.fn(), pending = deferred<void>(), nativePending = deferred<native.NativeAudioState>()
  if (platform === 'linux') audio.play.mockReturnValueOnce(pending.promise)
  else vi.mocked(native.playNativeAudio).mockReturnValueOnce(nativePending.promise)
  await act(async () => player.playFromChunk(chunks, options, 4, () => true, onStarted)); await frame()
  expect(onStarted).not.toHaveBeenCalled()
  await act(async () => { pending.resolve(); playing = true; nativePending.resolve(nativeState()) })
  expect(onStarted).toHaveBeenCalledOnce()
  onStarted.mockClear()
  const old = deferred<void>(), oldNative = deferred<native.NativeAudioState>()
  if (platform === 'linux') audio.play.mockReturnValueOnce(old.promise)
  else vi.mocked(native.playNativeAudio).mockReturnValueOnce(oldNative.promise)
  await act(async () => player.playFromChunk(chunks, options, 2, () => true, onStarted)); await frame()
  await act(async () => player.stop())
  await act(async () => { old.resolve(); oldNative.resolve(nativeState()) })
  expect(onStarted).not.toHaveBeenCalled()
})
