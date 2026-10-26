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
import * as api from '../api/nativeTts'
import { createAudiobookId } from '../storage/AudiobookLibrary'
import { DEFAULT_AUDIO_PREFERENCES } from '../storage/audioPreferences'

const mocks = vi.hoisted(() => ({
  confirm: vi.fn(async () => true),
  finishSave: () => {},
  play: vi.fn(() => Promise.resolve('started')),
  commandSignal: () => new AbortController().signal,
  resolve: vi.fn(),
  getChunks: vi.fn(),
  noop: () => {},
  chunks: [{
    id: 'c0',
    text: 'Hello world.',
  }],
  player: {
    status: 'idle',
    chunks: [],
    chunksPlayed: 0,
    chunksTotal: 0,
    currentChunkIndex: null,
    currentText: '',
  },
}))
vi.mock('./useTtsPlayer', () => ({
  useTtsPlayer: () => ({
    state: mocks.player,
    preload: mocks.noop,
    playFromChunk: mocks.play,
    pause: mocks.noop,
    resume: mocks.noop,
    jumpToChunk: mocks.noop,
    skipBackward: mocks.noop,
    skipForward: mocks.noop,
    stop: mocks.noop,
    commandSignal: mocks.commandSignal,
  }),
}))
vi.mock('./useTtsModelRuntime', async () => {
  const { DEFAULT_AUDIO_PREFERENCES: preferences } = await import('../storage/audioPreferences')
  const { FALLBACK_TTS_MODELS: models } = await import('../models')
  const runtime = {
    modelId: preferences.modelId,
    models,
    threadCount: 1,
    setModelId: mocks.noop,
    modelStatus: {
      installed: true,
      runtimeInstalled: true,
    },
  }
  return { useTtsModelRuntime: () => runtime }
})
vi.mock('./useAudiobookDocument', () => ({
  useAudiobookDocument: () => ({
    chunks: mocks.chunks,
    canonicalChunks: mocks.chunks,
    getCanonicalChunks: mocks.getChunks,
    getChunksForDocument: mocks.getChunks,
    getSelectedChunks: mocks.getChunks,
    getPdfSegments: mocks.getChunks,
    reset: mocks.noop,
  }),
}))
vi.mock('../playAnywhere/usePreparedTextTarget', () => ({ usePreparedTextTarget: () => mocks.resolve }))
vi.mock('../../components/AppDialog/useAppConfirmation', () => ({
  useAppConfirmation: () => ({
    confirm: mocks.confirm,
    dialog: null,
  }),
}))
vi.mock('../api/nativeTts', async importOriginal => ({
  ...await importOriginal<typeof import('../api/nativeTts')>(),
  listNativeSavedAudiobooks: vi.fn(async () => []),
  getNativeAudiobookStatus: vi.fn(),
  getNativeTtsCapabilities: vi.fn(),
  listenNativeAudiobookSaveProgress: vi.fn(async () => () => {}),
  saveNativeAudiobook: vi.fn(),
  deleteNativeAudiobook: vi.fn(),
}))
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
beforeEach(() => {
  localStorage.clear()
  vi.clearAllMocks()
  mocks.confirm.mockResolvedValue(true)
  vi.mocked(api.getNativeTtsCapabilities).mockResolvedValue({ available: true } as api.NativeTtsCapabilities)
  vi.mocked(api.getNativeAudiobookStatus).mockResolvedValue({
    cachedChunks: 0,
    totalChunks: 1,
    complete: false,
    dir: 'saved',
    audioDurationSec: 0,
    wavBytes: 0,
  })
  vi.mocked(api.deleteNativeAudiobook).mockResolvedValue({
    deletedAudio: true,
    deletedUserUpload: false,
    bytesFreed: 100,
  })
  vi.mocked(api.saveNativeAudiobook).mockImplementation(input => new Promise(resolve => {
    mocks.finishSave = () => resolve({
      jobId: input.jobId,
      cachedChunks: 1,
      totalChunks: 1,
      generatedChunks: 1,
      complete: true,
      dir: 'saved',
      generateMs: 1,
      audioDurationSec: 1,
      wavBytes: 100,
      appliedThreadCount: 1,
      backend: 'native',
    })
  }))
  mocks.getChunks.mockResolvedValue(mocks.chunks)
  mocks.resolve.mockResolvedValue({
    status: 'resolved',
    documentUrl: '/book.html',
    chunks: mocks.chunks,
    chunkIndex: 0,
    chunkId: 'c0',
    isCurrent: () => true,
  })
})

async function mount() {
  let current!: ReturnType<typeof useAudiobookManager>
  const props = {
    allDocuments: [],
    docContent: '<p>Hello world.</p>',
    loadHtmlDocument: async () => '',
    selectedDoc: '/book.html',
    uploadedDocuments: [],
    userUploads: [],
    onClearDocument: mocks.noop,
    onUserUploadsChanged: mocks.noop,
    onUploadedDocumentsChanged: mocks.noop,
  }
  function Harness() {
    const result = useAudiobookManager(props)
    useLayoutEffect(() => {
      current = result
    }, [result])
    return null
  }
  const element = document.createElement('div')
  document.body.append(element)
  const root = createRoot(element)
  await act(async () => root.render(<Harness />))
  return {
    get current() {
      return current
    },
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
    await act(async () => harness.current.audioControlsProps.onSave())
    expect(harness.current.audioControlsProps.canPlayAudiobook).toBe(false)
    await act(async () => mocks.finishSave())
    expect(api.getNativeAudiobookStatus).toHaveBeenCalledOnce()
    expect(harness.current.audioControlsProps.canPlayAudiobook).toBe(true)
    const result = action === 'beginning' ? await harness.current.playFromBeginning()
      : await harness.current.playFromHere({
        captureViewport: async () => ({
          status: 'captured',
          point: {
            format: 'html',
            segmentIndex: 0,
            offset: 0,
          },
          segmentTexts: ['Hello world.'],
          segmentLengths: [12],
          isCurrent: () => true,
        }),
      })
    expect(result).toMatchObject({ status: 'dispatched' })
    expect(mocks.play).toHaveBeenCalledWith(mocks.chunks,
      expect.objectContaining({
        documentUrl: '/book.html',
        voice: 'af_heart',
      }), 0)
  } finally {
    await harness.dispose()
  }
})

it.each(['document', 'voice'])('rejects a completed download for a different %s', async mismatch => {
  const harness = await mount()
  try {
    await act(async () => harness.current.audioControlsProps.onSave())
    await act(async () => mocks.finishSave())
    if (mismatch === 'document') {
      await act(async () => harness.current.audiobooksPanelProps.onResumeQueued({
        ...DEFAULT_AUDIO_PREFERENCES,
        id: 'other',
        documentUrl: '/other.html',
        title: 'Other',
        status: 'paused',
        cachedChunks: 0,
        totalChunks: 1,
        message: '',
        createdAt: 0,
        updatedAt: 0,
        sessionId: null,
      }))
      await act(async () => mocks.finishSave())
    } else {
      await act(async () => harness.current.audioSetupProps.onVoiceChange('af_bella'))
    }
    expect(harness.current.audioControlsProps.canPlayAudiobook).toBe(false)
    expect(await harness.current.playFromBeginning()).toMatchObject({ reason: 'not-ready' })
    expect(mocks.play).not.toHaveBeenCalled()
  } finally {
    await harness.dispose()
  }
})

it.each(['success', 'failure', 'cancel'])('updates playback readiness only after successful deletion (%s)', async outcome => {
  const harness = await mount()
  try {
    await act(async () => harness.current.audioControlsProps.onSave())
    await act(async () => mocks.finishSave())
    const record = {
      ...DEFAULT_AUDIO_PREFERENCES,
      id: createAudiobookId('/book.html', DEFAULT_AUDIO_PREFERENCES),
      documentUrl: '/book.html',
      title: 'Book',
      savedAt: 1,
      chunks: 1,
    }
    expect(harness.current.audioControlsProps.canPlayAudiobook).toBe(true)
    if (outcome === 'failure') {
      vi.mocked(api.deleteNativeAudiobook).mockRejectedValueOnce(new Error('delete failed'))
    }
    if (outcome === 'cancel') {
      mocks.confirm.mockResolvedValueOnce(false)
    }
    await act(async () => harness.current.audiobooksPanelProps.onDeleteSaved(record))
    if (outcome === 'success') {
      expect(harness.current.audiobooksPanelProps.activeDownload).toBeNull()
      expect(harness.current.audioControlsProps.canPlayAudiobook).toBe(false)
      expect(await harness.current.playFromBeginning()).toMatchObject({ reason: 'not-ready' })
      expect(mocks.play).not.toHaveBeenCalled()
    } else {
      expect(harness.current.audioControlsProps.canPlayAudiobook).toBe(true)
      expect(await harness.current.playFromBeginning()).toMatchObject({ status: 'dispatched' })
    }
    expect(api.deleteNativeAudiobook).toHaveBeenCalledTimes(outcome === 'cancel' ? 0 : 1)
  } finally {
    await harness.dispose()
  }
})

it('keeps a replacement save ready when an older deletion finishes later', async () => {
  const harness = await mount()
  try {
    await act(async () => harness.current.audioControlsProps.onSave())
    await act(async () => mocks.finishSave())
    let finishDelete!: (result: api.NativeAudiobookDeleteResult) => void
    vi.mocked(api.deleteNativeAudiobook).mockImplementationOnce(() => new Promise(resolve => {
      finishDelete = resolve
    }))
    const record = {
      ...DEFAULT_AUDIO_PREFERENCES,
      id: createAudiobookId('/book.html', DEFAULT_AUDIO_PREFERENCES),
      documentUrl: '/book.html',
      title: 'Book',
      savedAt: 1,
      chunks: 1,
    }
    let deletion!: Promise<void>
    await act(async () => {
      deletion = harness.current.audiobooksPanelProps.onDeleteSaved(record)
    })
    await act(async () => harness.current.audioControlsProps.onSave())
    await act(async () => mocks.finishSave())
    const replacement = harness.current.audiobooksPanelProps.activeDownload
    await act(async () => {
      finishDelete({
        deletedAudio: true,
        deletedUserUpload: false,
        bytesFreed: 100,
      })
      await deletion
    })
    expect(harness.current.audiobooksPanelProps.activeDownload).toBe(replacement)
    expect(harness.current.audioControlsProps.canPlayAudiobook).toBe(true)
  } finally {
    await harness.dispose()
  }
})
