// @vitest-environment jsdom
import {
  act,
  StrictMode,
  useLayoutEffect,
  type ReactNode,
} from 'react'
import { createRoot } from 'react-dom/client'
import {
  afterEach,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from 'vitest'
import { useTtsPlayer } from './useTtsPlayer'
import * as api from '../api/nativeTts'
import * as native from '../playback/nativeMobileAudio'
import type {
  TtsChunk,
  TtsOptions,
} from '../types'
import i18n from '../../i18n'
import { AudioControls } from '../components/AudioControls'
import { createPlayAnywherePlayback } from '../playAnywhere/usePlayAnywherePlayback'

vi.mock('../api/nativeTts', async importOriginal => ({
  ...await importOriginal<typeof import('../api/nativeTts')>(),
  getNativeTtsCapabilities: vi.fn(),
  getNativeSavedAudiobookChunk: vi.fn(),
  prepareNativeAudiobookPlayback: vi.fn(),
  resetNativeTtsCapabilities: vi.fn(),
}))
vi.mock('../playback/nativeMobileAudio', () => ({
  initializeNativeAudio: vi.fn(),
  setNativeAudioSource: vi.fn(),
  setNativeAudioRate: vi.fn(),
  seekNativeAudio: vi.fn(),
  playNativeAudio: vi.fn(),
  pauseNativeAudio: vi.fn(),
  getNativeAudioState: vi.fn(),
  stopNativeAudio: vi.fn(),
  disposeNativeAudio: vi.fn(),
}))
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
type SavedChunk = NonNullable<Awaited<ReturnType<typeof api.getNativeSavedAudiobookChunk>>>
const chunks: TtsChunk[] = Array.from({ length: 8 }, (_, i) => ({
  id: 'c' + i,
  text: 'Chunk ' + i,
}))
const options: TtsOptions = {
  documentUrl: '/book.html',
  modelId: 'kokoro',
  voice: 'af_heart',
  speed: 1,
}
const playback: api.NativeAudiobookPlayback = {
  title: 'Book',
  audioUrl: 'track.wav',
  audioDurationSec: 36,
  wavBytes: 100,
  chunks: chunks.map((chunk, index) => ({
    index,
    chunkId: chunk.id,
    startSec: index * (index + 1) / 2,
    durationSec: index + 1,
  })),
}
let audio: FakeAudio
let audios: FakeAudio[]
let player: ReturnType<typeof useTtsPlayer>
let frames: Map<number, FrameRequestCallback>
let unmount: (() => Promise<void>) | undefined
let position: number; let playing: boolean
const nativeState = () => ({
  status: playing ? 'playing' : 'idle',
  currentTime: position,
  duration: 36,
  isPlaying: playing,
  buffering: false,
  rate: 1,
}) as native.NativeAudioState
class FakeAudio extends EventTarget {
  src = ''; currentTime = 0; duration = 5; playbackRate = 1; preload = ''; ended = false
  pause = vi.fn()
  play = vi.fn(async () => {})
  constructor() {
    super(); audios.push(this)
  }
}
type PreparedTarget = Awaited<ReturnType<Parameters<typeof createPlayAnywherePlayback>[0]['resolve']>>
const htmlCapture = {
  status: 'captured',
  point: {
    format: 'html',
    segmentIndex: 0,
    offset: 0,
  },
  segmentTexts: ['Chunk 4'],
  segmentLengths: [7],
  isCurrent: () => true,
} as const
const resolvedTarget = (isCurrent: () => boolean): PreparedTarget => ({
  status: 'resolved',
  documentUrl: options.documentUrl!,
  chunks,
  chunkIndex: 4,
  chunkId: chunks[4].id,
  isCurrent,
})
function deferred<T>() {
  let resolve!: (value: T) => void; let reject!: (error: Error) => void
  const promise = new Promise<T>((yes, no) => {
    resolve = yes; reject = no
  })
  return {
    promise,
    resolve,
    reject,
  }
}
async function mount(platform = 'linux', controls?: (result: ReturnType<typeof useTtsPlayer>) => ReactNode) {
  vi.mocked(api.getNativeTtsCapabilities).mockResolvedValue({ platform } as api.NativeTtsCapabilities)
  function Harness() {
    const result = useTtsPlayer()
    useLayoutEffect(() => {
      player = result
    }, [result])
    return controls?.(result) ?? null
  }
  const element = document.createElement('div'); document.body.append(element)
  const root = createRoot(element)
  await act(async () => root.render(<Harness />))
  audio = audios.at(-1)!
  unmount = async () => {
    await act(async () => root.unmount()); unmount = undefined
  }
}
async function frame() {
  await act(async () => {
    const batch = [...frames.values()]; frames.clear(); batch.forEach(callback => callback(performance.now()))
  })
}
async function start(index?: number) {
  await act(async () => {
    player.speak(chunks, options, index)
  }); await frame()
}
beforeEach(() => {
  vi.resetAllMocks(); audios = []; position = 0; playing = false; frames = new Map(); let id = 0
  vi.stubGlobal('Audio', FakeAudio)
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
    frames.set(++id, callback); return id
  })
  vi.stubGlobal('cancelAnimationFrame', (key: number) => frames.delete(key))
  vi.stubGlobal('URL', class extends URL {
    static createObjectURL = vi.fn(() => 'blob:' + ++id); static revokeObjectURL = vi.fn()
  })
  vi.mocked(api.getNativeSavedAudiobookChunk).mockImplementation(async () => ({ wav: new ArrayBuffer(64) }) as SavedChunk)
  vi.mocked(api.prepareNativeAudiobookPlayback).mockResolvedValue(playback)
  vi.mocked(native.initializeNativeAudio).mockImplementation(async () => nativeState())
  vi.mocked(native.setNativeAudioSource).mockImplementation(async () => {
    position = 0; playing = false; return nativeState()
  })
  vi.mocked(native.setNativeAudioRate).mockImplementation(async () => nativeState())
  vi.mocked(native.seekNativeAudio).mockImplementation(async target => {
    position = target; return nativeState()
  })
  vi.mocked(native.playNativeAudio).mockImplementation(async isCurrent => {
    if (isCurrent && !isCurrent()) {
      throw new DOMException('Superseded playback request', 'AbortError')
    }
    playing = true
    return nativeState()
  })
  vi.mocked(native.pauseNativeAudio).mockImplementation(async () => {
    playing = false; return nativeState()
  })
  vi.mocked(native.getNativeAudioState).mockImplementation(async () => nativeState())
  vi.mocked(native.stopNativeAudio).mockResolvedValue()
  vi.mocked(native.disposeNativeAudio).mockResolvedValue()
})
afterEach(async () => {
  await unmount?.(); vi.restoreAllMocks(); vi.unstubAllGlobals(); document.body.replaceChildren()
})

it.each([
  ['linux', 'source'],
  ['linux', 'play'],
  ['android', 'source'],
  ['android', 'play'],
])('gates startup navigation, then lets unwrapped transport take over on %s during %s', async (platform, phase) => {
  const coordinator = createPlayAnywherePlayback({
    complete: true,
    options,
    getChunks: async () => chunks,
    resolve: async () => ({
      status: 'resolved',
      documentUrl: options.documentUrl!,
      chunks,
      chunkIndex: 4,
      chunkId: chunks[4].id,
      isCurrent: () => true,
    }),
    play: (...args) => player.playFromChunk(...args),
    commandSignal: () => player.commandSignal(),
  })
  const noop = () => {}
  await mount(platform, current => (
    <AudioControls
      ttsState={current.state}
      audiobookState={{
        status: 'saved',
        complete: true,
        cachedChunks: 8,
        totalChunks: 8,
        message: '',
        audioDurationSec: 36,
        wavBytes: 100,
        appliedThreadCount: null,
      }}
      canPlayAudiobook canSkipBackward canSkipForward
      canSaveAudiobook={false} saveInProgress={false} modelSetupLabel={null}
      playbackRate={1} savedAudiobooks={[]} selectedAudiobookId={null} wordHighlightEnabled={false}
      onSkipBackward={current.skipBackward} onSkipForward={current.skipForward}
      onJumpToChunk={index => current.jumpToChunk(index)} onStop={current.stop}
      onPause={current.pause} onResume={current.resume}
      onManageSave={noop} onRead={noop} onSelectSavedAudiobook={noop}
      onPlaybackRateChange={noop} onOpenAudioSetup={noop} onSave={noop}
      onWordHighlightEnabledChange={noop}
    />
  ))
  const demand = deferred<SavedChunk | null>()
  const prepared = deferred<api.NativeAudiobookPlayback>()
  const browserPlay = deferred<void>()
  const nativePlay = deferred<native.NativeAudioState>()
  if (phase === 'source') {
    if (platform === 'linux') {
      vi.mocked(api.getNativeSavedAudiobookChunk).mockReturnValueOnce(demand.promise)
    } else {
      vi.mocked(api.prepareNativeAudiobookPlayback).mockReturnValueOnce(prepared.promise)
    }
  } else if (platform === 'linux') {
    audio.play.mockReturnValueOnce(browserPlay.promise)
  } else {
    vi.mocked(native.playNativeAudio).mockImplementationOnce(async isCurrent => {
      if (isCurrent && !isCurrent()) {
        throw new DOMException('Superseded playback request', 'AbortError')
      }
      return nativePlay.promise
    })
  }

  await act(async () => coordinator.playFromHere({
    captureViewport: async () => ({
      status: 'captured',
      point: {
        format: 'html',
        segmentIndex: 0,
        offset: 0,
      },
      segmentTexts: ['Chunk 4'],
      segmentLengths: [7],
      isCurrent: () => true,
    }),
  }))
  await frame()
  type ControlLabel = 'tts.controls.previousChunk'
    | 'tts.controls.nextChunk'
    | 'tts.controls.stop'
    | 'tts.controls.playbackOptions'
  const button = (key: ControlLabel) => [...document.querySelectorAll('button')]
    .find(element => element.getAttribute('aria-label') === i18n.t(key))!
  expect(player.state.status).toBe('loading')
  expect(player.state.currentChunkIndex).toBeNull()
  for (const key of ['tts.controls.previousChunk', 'tts.controls.nextChunk'] as const) {
    expect(button(key).disabled).toBe(true)
    await act(async () => button(key).click())
  }
  expect(button('tts.controls.stop').disabled).toBe(false)
  await act(async () => button('tts.controls.playbackOptions').click())
  const row = document.querySelector<HTMLButtonElement>('.audio-chunk-item')!
  expect(row.disabled).toBe(true)
  await act(async () => row.click())

  await act(async () => {
    demand.resolve({ wav: new ArrayBuffer(64) } as SavedChunk)
    prepared.resolve(playback)
    browserPlay.resolve()
    playing = true
    nativePlay.resolve(nativeState())
  })
  await frame()
  expect(player.state.status).toBe('playing')
  expect(player.state.currentChunkIndex).toBe(4)
  expect(button('tts.controls.nextChunk').disabled).toBe(false)
  expect(document.querySelector<HTMLButtonElement>('.audio-chunk-item')!.disabled).toBe(false)
  await act(async () => button('tts.controls.nextChunk').click())
  await frame()
  expect(player.state.currentChunkIndex).toBe(5)
  expect(player.state.status).toBe('playing')
  await act(async () => document.querySelector<HTMLButtonElement>('.audio-chunk-item')!.click())
  await frame()
  expect(player.state.currentChunkIndex).toBe(0)
  expect(player.state.status).toBe('playing')
})

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
  it('keeps loading until play acknowledgement while prefetching the next two chunks', async () => {
    await mount()
    const pending = deferred<void>()
    const onStarted = vi.fn()
    audio.play.mockReturnValueOnce(pending.promise)
    await act(async () => {
      void player.playFromChunk(chunks, options, 4)!.then(onStarted)
    })
    await frame()
    expect(player.state.status).toBe('loading')
    expect(player.state.pendingChunkIndex).toBe(4)
    expect(player.state.currentChunkIndex).toBeNull()
    expect(player.state.currentText).toBe('')
    expect(onStarted).not.toHaveBeenCalled()
    expect(vi.mocked(api.getNativeSavedAudiobookChunk).mock.calls.map(call => call[2]))
      .toEqual([4, 5, 6])
    audio.currentTime = 0.25
    await act(async () => audio.dispatchEvent(new Event('timeupdate')))
    expect(player.state.currentChunkTime).toBe(0)
    await act(async () => pending.resolve())
    expect(player.state.status).toBe('playing')
    expect(player.state.pendingChunkIndex).toBeNull()
    expect(player.state.currentChunkIndex).toBe(4)
    expect(player.state.currentChunkTime).toBe(0.25)
    expect(onStarted).toHaveBeenCalledExactlyOnceWith('started')
    expect(vi.mocked(api.getNativeSavedAudiobookChunk).mock.calls.map(call => call[2])).toEqual([4, 5, 6])
  })
  it.each(['stop', 'seek', 'book', 'unmount', 'reject'])('does not commit a superseded play acknowledgement after %s', async action => {
    await mount()
    const pending = deferred<void>()
    const onStarted = vi.fn()
    audio.play.mockReturnValueOnce(pending.promise)
    await act(async () => {
      void player.playFromChunk(chunks, options, 4)!.then(onStarted)
    })
    await frame()
    if (action === 'stop') {
      await act(async () => player.stop())
    }
    if (action === 'seek') {
      await act(async () => {
        player.playFromChunk(chunks, options, 1)
      }); await frame()
    }
    if (action === 'book') {
      await act(async () => player.speak(chunks, {
        ...options,
        documentUrl: '/other.html',
      }, 2)); await frame()
    }
    if (action === 'unmount') {
      await unmount!()
    }
    const readsBeforeAcknowledgement = vi.mocked(api.getNativeSavedAudiobookChunk).mock.calls.length
    if (action === 'reject') {
      await act(async () => pending.reject(new Error('decoder failed')))
    } else {
      await act(async () => pending.resolve())
    }
    expect(onStarted).not.toHaveBeenCalledWith('started')
    expect(api.getNativeSavedAudiobookChunk).toHaveBeenCalledTimes(readsBeforeAcknowledgement)
    if (action === 'stop') {
      expect(player.state.status).toBe('idle')
    }
    if (action === 'seek') {
      expect(player.state.currentChunkIndex).toBe(1)
    }
    if (action === 'book') {
      expect(player.state.currentChunkIndex).toBe(2)
    }
    if (action === 'reject') {
      expect(player.state.status).toBe('error')
    }
  })
  it('supersedes the start on Pause and acknowledges only the resumed attempt at its saved time', async () => {
    await mount()
    const first = deferred<void>(); const resumed = deferred<void>()
    const onStarted = vi.fn()
    audio.play.mockReturnValueOnce(first.promise).mockReturnValueOnce(resumed.promise)
    await act(async () => {
      void player.playFromChunk(chunks, options, 4)!.then(onStarted)
    })
    await frame()
    audio.currentTime = 1.25
    const source = audio.src
    await act(async () => player.pause())
    await act(async () => player.resume())
    expect(player.state.status).toBe('loading')
    expect(audio.currentTime).toBe(1.25)
    expect(audio.src).toBe(source)
    await act(async () => first.resolve())
    expect(player.state.status).toBe('loading')
    await act(async () => resumed.resolve())
    expect(player.state.status).toBe('playing')
    expect(player.state.currentChunkTime).toBe(1.25)
    expect(onStarted).toHaveBeenCalledExactlyOnceWith('superseded')
  })
  it('defaults to zero and rejects malformed indices before touching the current audio', async () => {
    await mount(); await start()
    const calls = audio.pause.mock.calls.length
    for (const index of [-1, NaN, 0.1, Infinity, chunks.length]) {
      expect(player.speak(chunks, options, index)).toBe(false)
      expect(player.playFromChunk(chunks, options, index)).toBeNull()
    }
    expect(audio.pause).toHaveBeenCalledTimes(calls)
    expect(player.state.currentChunkIndex).toBe(0)
  })
  it('coalesces targets and ignores superseded loads, including old failures', async () => {
    await mount()
    const pending = deferred<SavedChunk | null>()
    vi.mocked(api.getNativeSavedAudiobookChunk).mockReturnValueOnce(pending.promise)
    await start(1)
    await act(async () => {
      player.playFromChunk(chunks, options, 3); player.playFromChunk(chunks, options, 6)
    }); await frame()
    await act(async () => pending.reject(new Error('old corrupt chunk'))); await frame()
    expect(player.state.currentChunkIndex).toBe(6)
    expect(player.state.status).toBe('playing')
  })
  it.each(['stop', 'unmount'])('does not play a late chunk after %s', async action => {
    await mount(); const pending = deferred<SavedChunk | null>()
    vi.mocked(api.getNativeSavedAudiobookChunk).mockReturnValueOnce(pending.promise)
    await act(async () => player.speak(chunks, options, 4)); await frame()
    if (action === 'stop') {
      await act(async () => player.stop())
    }
    if (action === 'unmount') {
      await unmount!()
    }
    await act(async () => pending.resolve({ wav: new ArrayBuffer(64) } as SavedChunk))
    expect(audio.play).not.toHaveBeenCalled()
  })
  it('resumes the current desktop time and ignores an old ended event while loading a different target', async () => {
    await mount(); await start(2); audio.currentTime = 1.25
    await act(async () => player.pause()); await act(async () => player.resume())
    expect(audio.currentTime).toBe(1.25)
    const pending = deferred<SavedChunk | null>()
    vi.mocked(api.getNativeSavedAudiobookChunk).mockReturnValueOnce(pending.promise)
    await act(async () => {
      player.playFromChunk(chunks, options, 6)
    }); await frame()
    await act(async () => audio.dispatchEvent(new Event('ended')))
    await act(async () => pending.resolve({ wav: new ArrayBuffer(64) } as SavedChunk)); await frame()
    expect(player.state.currentChunkIndex).toBe(6)
  })
  it('does not let a late capability preload alter a newer playback job', async () => {
    await mount(); const pending = deferred<api.NativeTtsCapabilities>()
    vi.mocked(api.getNativeTtsCapabilities).mockReturnValueOnce(pending.promise)
    await act(async () => player.preload()); await start(4)
    await act(async () => pending.resolve({
      platform: 'android',
      available: false,
      reason: 'old reply',
    } as api.NativeTtsCapabilities))
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
    await act(async () => {
      player.playFromChunk(chunks, options, 5)
    }); await frame()
    expect(position).toBe(15); expect(player.state.status).toBe('playing')
    expect(api.prepareNativeAudiobookPlayback).toHaveBeenCalledOnce()
    expect(native.setNativeAudioSource).toHaveBeenCalledOnce()
  })
  it('starts a new session when canonical hashes or audio options change', async () => {
    await mount('android'); await start(2)
    const changed = chunks.map(chunk => ({
      ...chunk,
      textHash: 'new-' + chunk.id,
    }))
    await act(async () => {
      player.playFromChunk(changed, options, 5)
    })
    expect(api.prepareNativeAudiobookPlayback).toHaveBeenCalledTimes(2)
    await act(async () => {
      player.playFromChunk(changed, {
        ...options,
        voice: 'af_bella',
      }, 6)
    })
    expect(api.prepareNativeAudiobookPlayback).toHaveBeenCalledTimes(3)
  })
  it('keeps only the newest active seek and does not let a late previous-book reply alter the replacement', async () => {
    await mount('android'); await start(1)
    const pending = deferred<native.NativeAudioState>()
    vi.mocked(native.seekNativeAudio).mockReturnValueOnce(pending.promise)
    await act(async () => {
      player.playFromChunk(chunks, options, 2)
    }); await frame()
    await act(async () => {
      player.playFromChunk(chunks, options, 4); player.playFromChunk(chunks, options, 6)
    }); await frame()
    await act(async () => pending.resolve(nativeState())); await frame()
    expect(position).toBe(21); expect(player.state.currentChunkIndex).toBe(6)
    const previous = deferred<native.NativeAudioState>()
    vi.mocked(native.seekNativeAudio).mockReturnValueOnce(previous.promise)
    await act(async () => player.jumpToChunk(3)); await frame()
    await act(async () => player.speak(chunks, {
      ...options,
      documentUrl: '/other.html',
    }, 5))
    await act(async () => previous.resolve({
      ...nativeState(),
      currentTime: 0,
    }))
    expect(player.state.currentChunkIndex).toBe(5)
  })
  it.each(['prepare', 'initialize', 'source', 'rate', 'seek'])('stops during %s without issuing a later play', async phase => {
    await mount('android')
    const pending = deferred<never>()
    const operation = {
      prepare: api.prepareNativeAudiobookPlayback,
      initialize: native.initializeNativeAudio,
      source: native.setNativeAudioSource,
      rate: native.setNativeAudioRate,
      seek: native.seekNativeAudio,
    }[phase]!
    vi.mocked(operation).mockReturnValueOnce(pending.promise)
    await start(4)
    await act(async () => player.stop())
    await act(async () => pending.resolve((phase === 'prepare' ? playback : nativeState()) as never))
    expect(native.playNativeAudio).not.toHaveBeenCalled()
    expect(player.state.status).toBe('idle')
  })
  it.each(['prepare', 'initialize', 'source', 'rate', 'seek', 'play'])('reports a current %s failure', async phase => {
    await mount('android')
    const operation = {
      prepare: api.prepareNativeAudiobookPlayback,
      initialize: native.initializeNativeAudio,
      source: native.setNativeAudioSource,
      rate: native.setNativeAudioRate,
      seek: native.seekNativeAudio,
      play: native.playNativeAudio,
    }[phase]!
    vi.mocked(operation).mockRejectedValueOnce(new Error(phase + ' failed'))
    await start(4); expect(player.state.status).toBe('error')
  })
  it('preserves rapid relative skips while coalescing bridge navigation', async () => {
    await mount('android'); await start(1)
    await act(async () => {
      player.skipForward(); player.skipForward(); player.skipForward()
    }); await frame()
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
    await act(async () => pending.resolve({
      ...nativeState(),
      currentTime: 10,
      isPlaying: true,
      status: 'playing',
    }))
    expect(player.state.status).toBe('idle'); expect(player.state.currentChunkIndex).toBeNull()
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
    await act(async () => {
      player.playFromChunk(chunks, options, 4)
    }); await act(async () => player.stop())
    await act(async () => pending.resolve(nativeState())); await frame()
    expect(native.seekNativeAudio).toHaveBeenCalledTimes(seekCount)
  })
  it('rejects a track for a different canonical sequence before setting its source', async () => {
    await mount('android'); vi.mocked(api.prepareNativeAudiobookPlayback).mockResolvedValue({
      ...playback,
      chunks: playback.chunks.slice(1),
    })
    await start(4); expect(player.state.status).toBe('error'); expect(native.setNativeAudioSource).not.toHaveBeenCalled()
  })
})

describe('command epoch and start tickets', () => {
  type Start = Awaited<NonNullable<ReturnType<typeof player.playFromChunk>>>
  async function dispatch(index: number) {
    let ticket!: ReturnType<typeof player.playFromChunk>
    await act(async () => {
      ticket = player.playFromChunk(chunks, options, index)
    }); await frame()
    const settled = vi.fn<(start: Start) => void>()
    void ticket!.then(settled)
    return settled
  }
  it('replaces the command signal on every public command but not on automatic advance', async () => {
    await mount(); await start(2)
    const signal = player.commandSignal()
    await act(async () => audio.dispatchEvent(new Event('ended'))); await frame()
    expect(player.state.currentChunkIndex).toBe(3)
    expect(signal.aborted).toBe(false)
    for (const command of [player.pause, player.resume, player.skipForward, player.skipBackward, () => player.jumpToChunk(1), player.stop]) {
      const before = player.commandSignal()
      await act(async () => command()); await frame()
      expect(before.aborted).toBe(true)
      expect(player.commandSignal().aborted).toBe(false)
    }
  })
  it('keeps a live command epoch after StrictMode replays effects', async () => {
    vi.mocked(api.getNativeTtsCapabilities).mockResolvedValue({ platform: 'linux' } as api.NativeTtsCapabilities)
    function Harness() {
      const result = useTtsPlayer(); useLayoutEffect(() => {
        player = result
      }, [result]); return null
    }
    const element = document.createElement('div'); document.body.append(element)
    const root = createRoot(element)
    await act(async () => root.render(<StrictMode><Harness /></StrictMode>))
    try {
      expect(player.commandSignal().aborted).toBe(false)
    } finally {
      await act(async () => root.unmount())
    }
  })
  it.each(['linux', 'android'])('settles started only after the play acknowledgement on %s', async platform => {
    await mount(platform)
    const pending = deferred<void>(); const nativePending = deferred<native.NativeAudioState>()
    if (platform === 'linux') {
      audio.play.mockReturnValueOnce(pending.promise)
    } else {
      vi.mocked(native.playNativeAudio).mockReturnValueOnce(nativePending.promise)
    }
    const settled = await dispatch(4)
    expect(settled).not.toHaveBeenCalled()
    await act(async () => {
      pending.resolve(); playing = true; nativePending.resolve(nativeState())
    })
    expect(settled).toHaveBeenCalledExactlyOnceWith('started')
  })
  it.each(['pause', 'stop', 'skip', 'jump', 'target', 'unmount'])('settles superseded when %s precedes the acknowledgement', async action => {
    await mount(); const pending = deferred<void>()
    audio.play.mockReturnValueOnce(pending.promise)
    const settled = await dispatch(4)
    if (action === 'pause') {
      await act(async () => player.pause())
    }
    if (action === 'stop') {
      await act(async () => player.stop())
    }
    if (action === 'skip') {
      await act(async () => player.skipForward())
    }
    if (action === 'jump') {
      await act(async () => player.jumpToChunk(1))
    }
    if (action === 'target') {
      await act(async () => {
        player.playFromChunk(chunks, options, 1)
      })
    }
    if (action === 'unmount') {
      await unmount!()
    }
    await act(async () => pending.resolve())
    expect(settled).toHaveBeenCalledExactlyOnceWith('superseded')
  })
  it.each(['linux', 'android'])('settles failed when the current start fails on %s', async platform => {
    await mount(platform)
    if (platform === 'linux') {
      audio.play.mockRejectedValueOnce(new Error('decoder failed'))
    } else {
      vi.mocked(native.playNativeAudio).mockRejectedValueOnce(new Error('play failed'))
    }
    const settled = await dispatch(4)
    await act(async () => {})
    expect(player.state.status).toBe('error')
    expect(settled).toHaveBeenCalledExactlyOnceWith('failed')
  })
  it.each(['linux', 'android'])('lets any playback command cancel a target still preparing on %s, without a wrapper', async platform => {
    await mount(platform); await start(1)
    const pending = deferred<PreparedTarget>()
    const scope = createPlayAnywherePlayback({
      complete: true,
      options,
      getChunks: async () => chunks,
      resolve: () => pending.promise,
      play: player.playFromChunk,
      commandSignal: player.commandSignal,
    })
    const result = scope.playFromHere({ captureViewport: async () => htmlCapture })
    await act(async () => player.pause())
    await act(async () => pending.resolve(resolvedTarget(() => true)))
    expect(await result).toEqual({
      status: 'cancelled',
      reason: 'superseded',
    })
    expect(player.state.currentChunkIndex).toBe(1)
    expect(player.state.status).toBe('paused')
  })
  it.each(['linux', 'android'])('does not re-check passage validity after dispatch on %s', async platform => {
    await mount(platform)
    let valid = true
    const demand = deferred<SavedChunk | null>(); const prepared = deferred<api.NativeAudiobookPlayback>()
    if (platform === 'linux') {
      vi.mocked(api.getNativeSavedAudiobookChunk).mockReturnValueOnce(demand.promise)
    } else {
      vi.mocked(api.prepareNativeAudiobookPlayback).mockReturnValueOnce(prepared.promise)
    }
    const scope = createPlayAnywherePlayback({
      complete: true,
      options,
      getChunks: async () => chunks,
      resolve: async () => resolvedTarget(() => valid),
      play: player.playFromChunk,
      commandSignal: player.commandSignal,
    })
    let result!: Awaited<ReturnType<typeof scope.playFromHere>>
    await act(async () => {
      result = await scope.playFromHere({ captureViewport: async () => htmlCapture })
    }); await frame()
    if (result.status !== 'dispatched') {
      throw new Error('Expected dispatch')
    }
    // Android's first-play permission dialog hides the page and invalidates the capture here.
    valid = false
    await act(async () => {
      demand.resolve({ wav: new ArrayBuffer(64) } as SavedChunk); prepared.resolve(playback)
    }); await frame()
    expect(player.state.status).toBe('playing')
    expect(player.state.currentChunkIndex).toBe(4)
    expect(await result.started).toBe('started')
  })
  // Visible buttons stay gated during startup; any other path that reaches skip must not idle playback.
  it.each(['linux', 'android'])('keeps playing and navigates when skip reaches a dispatched startup on %s', async platform => {
    await mount(platform)
    const browserPlay = deferred<void>(); const prepared = deferred<api.NativeAudiobookPlayback>()
    if (platform === 'linux') {
      audio.play.mockReturnValueOnce(browserPlay.promise)
    } else {
      vi.mocked(api.prepareNativeAudiobookPlayback).mockReturnValueOnce(prepared.promise)
    }
    const settled = await dispatch(4)
    await act(async () => player.skipForward()); await frame()
    await act(async () => {
      browserPlay.resolve(); prepared.resolve(playback)
    }); await frame()
    await act(async () => player.skipForward()); await frame()
    expect(player.state.status).toBe('playing')
    expect(player.state.currentChunkIndex).toBe(platform === 'linux' ? 6 : 5)
    expect(settled).toHaveBeenCalledExactlyOnceWith('superseded')
  })
})
