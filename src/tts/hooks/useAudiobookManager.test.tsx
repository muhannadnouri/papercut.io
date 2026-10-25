// @vitest-environment jsdom
import {
  act,
  useLayoutEffect,
} from 'react'
import { createRoot } from 'react-dom/client'
import {
  beforeEach,
  expect,
  it,
  vi,
} from 'vitest'
import { useAudiobookManager } from './useAudiobookManager'
import type { AudiobookCacheState } from './useAudiobookCache'
import type { useAudiobookDownloadQueue } from './useAudiobookDownloadQueue'

const mocks = vi.hoisted(() => ({
  complete: false,
  url: '/book.html',
  voice: 'af_heart',
  check: vi.fn(),
  play: vi.fn(() => true),
  resolve: vi.fn(),
  getChunks: vi.fn(),
  noop: () => {},
  chunks: [{ id: 'c0', text: 'Hello world.' }],
  onCompleted: undefined as undefined | (() => void | Promise<void>),
  cache: {
    status: 'partial', complete: false, cachedChunks: 0, totalChunks: 1,
    message: '', audioDurationSec: 0, wavBytes: 0, appliedThreadCount: null,
  } as AudiobookCacheState,
  player: {
    status: 'idle', chunks: [], chunksPlayed: 0, chunksTotal: 0,
    currentChunkIndex: null, currentText: '',
  },
}))
vi.mock('./useTtsPlayer', () => ({
  useTtsPlayer: () => ({
    state: mocks.player, preload: mocks.noop, playFromChunk: mocks.play,
    pause: mocks.noop, resume: mocks.noop, jumpToChunk: mocks.noop,
    skipBackward: mocks.noop, skipForward: mocks.noop, stop: mocks.noop,
  }),
}))
vi.mock('./useTtsModelRuntime', async () => {
  const { DEFAULT_AUDIO_PREFERENCES: preferences } = await import('../storage/audioPreferences')
  const { FALLBACK_TTS_MODELS: models } = await import('../models')
  const runtime = {
    modelId: preferences.modelId, models, threadCount: 1, setModelId: mocks.noop,
    modelStatus: { installed: true, runtimeInstalled: true },
  }
  return { useTtsModelRuntime: () => runtime }
})
vi.mock('./useAudiobookDocument', () => ({
  useAudiobookDocument: () => ({
    chunks: mocks.chunks, canonicalChunks: mocks.chunks,
    getCanonicalChunks: mocks.getChunks, getChunksForDocument: mocks.getChunks,
    getSelectedChunks: mocks.getChunks, getPdfSegments: mocks.getChunks, reset: mocks.noop,
  }),
}))
vi.mock('./useAudiobookCache', () => ({
  useAudiobookCache: () => ({ state: mocks.cache, check: mocks.check, reset: mocks.noop }),
}))
vi.mock('./useSavedAudiobookActions', () => ({ useSavedAudiobookActions: () => ({}) }))
vi.mock('../playAnywhere/usePreparedTextTarget', () => ({ usePreparedTextTarget: () => mocks.resolve }))
vi.mock('../../components/AppDialog/useAppConfirmation', () => ({
  useAppConfirmation: () => ({ confirm: mocks.noop, dialog: null }),
}))
vi.mock('../api/nativeTts', async importOriginal => ({
  ...await importOriginal<typeof import('../api/nativeTts')>(),
  listNativeSavedAudiobooks: vi.fn(async () => []),
}))
vi.mock('./useAudiobookDownloadQueue', async () => {
  const { DEFAULT_AUDIO_PREFERENCES: preferences } = await import('../storage/audioPreferences')
  return {
    useAudiobookDownloadQueue: (options: Parameters<typeof useAudiobookDownloadQueue>[0]) => {
      mocks.onCompleted = options.onCompleted
      return {
        activeDownload: { ...preferences, title: 'Book', url: mocks.url, voice: mocks.voice },
        downloads: [],
        state: {
          ...mocks.cache, cachedChunks: 1,
          status: mocks.complete ? 'saved' : 'saving', complete: mocks.complete,
        },
        start: mocks.noop, cancel: mocks.noop, remove: mocks.noop,
      }
    },
  }
})
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
beforeEach(() => {
  localStorage.clear()
  vi.clearAllMocks()
  mocks.complete = false
  mocks.url = '/book.html'
  mocks.voice = 'af_heart'
  mocks.getChunks.mockResolvedValue(mocks.chunks)
  mocks.resolve.mockResolvedValue({
    status: 'resolved', documentUrl: '/book.html', chunks: mocks.chunks,
    chunkIndex: 0, chunkId: 'c0', isCurrent: () => true,
  })
})

async function mount() {
  let current!: ReturnType<typeof useAudiobookManager>
  const props = {
    allDocuments: [], docContent: '<p>Hello world.</p>',
    loadHtmlDocument: async () => '', selectedDoc: '/book.html',
    uploadedDocuments: [], userUploads: [], onClearDocument: mocks.noop,
    onUserUploadsChanged: mocks.noop, onUploadedDocumentsChanged: mocks.noop,
  }
  function Harness() {
    const result = useAudiobookManager(props)
    useLayoutEffect(() => { current = result }, [result])
    return null
  }
  const element = document.createElement('div')
  document.body.append(element)
  const root = createRoot(element)
  const render = () => act(async () => root.render(<Harness />))
  await render()
  return {
    get current() { return current },
    render,
    async dispose() {
      await act(async () => root.unmount())
      element.remove()
    },
  }
}

it.each(['beginning', 'passage'])('admits %s after the matching save finalizes without waiting for a stale cache recheck', async action => {
  const harness = await mount()
  try {
    expect(harness.current.audioControlsProps.canPlayAudiobook).toBe(false)
    expect(await harness.current.playFromBeginning()).toMatchObject({ reason: 'not-ready' })
    expect(mocks.play).not.toHaveBeenCalled()
    mocks.complete = true
    await harness.render()
    await act(async () => mocks.onCompleted?.())
    expect(mocks.check).toHaveBeenCalledOnce()
    expect(harness.current.audioControlsProps.canPlayAudiobook).toBe(true)
    const result = action === 'beginning' ? await harness.current.playFromBeginning()
      : await harness.current.playFromHere({ captureViewport: async () => ({
        status: 'captured', point: { format: 'html', segmentIndex: 0, offset: 0 },
        segmentTexts: ['Hello world.'], segmentLengths: [12], isCurrent: () => true,
      }) })
    expect(result).toEqual({ status: 'dispatched' })
    expect(mocks.play).toHaveBeenCalledWith(mocks.chunks,
      expect.objectContaining({ documentUrl: '/book.html', voice: 'af_heart' }), 0,
      expect.any(Function), undefined)
  } finally { await harness.dispose() }
})

it.each(['document', 'voice'])('rejects a completed download for a different %s', async mismatch => {
  mocks.complete = true
  if (mismatch === 'document') mocks.url = '/other.html'
  else mocks.voice = 'af_bella'
  const harness = await mount()
  try {
    expect(harness.current.audioControlsProps.canPlayAudiobook).toBe(false)
    expect(await harness.current.playFromBeginning()).toMatchObject({ reason: 'not-ready' })
    expect(mocks.play).not.toHaveBeenCalled()
  } finally { await harness.dispose() }
})
