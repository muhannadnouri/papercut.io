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
  exportNativeAudiobook: vi.fn(),
}))
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
beforeEach(() => {
  localStorage.clear()
  vi.clearAllMocks()
  mocks.confirm.mockResolvedValue(true)
  vi.mocked(api.listNativeSavedAudiobooks).mockResolvedValue([])
  vi.mocked(api.exportNativeAudiobook).mockResolvedValue({ path: '/saved.wav' } as api.NativeAudiobookExportResult)
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

async function mount(selectedDoc = '/book.html') {
  let current!: ReturnType<typeof useAudiobookManager>
  const props = {
    allDocuments: [],
    docContent: '<p>Hello world.</p>',
    loadHtmlDocument: async () => '',
    selectedDoc,
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
    expect(api.getNativeAudiobookStatus).toHaveBeenCalledTimes(2)
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
      }), 0, undefined)
  } finally {
    await harness.dispose()
  }
})

/** Both local and bundle-imported PDFs use the same upload URL/status path after indexing. */
it('requires deliberate replacement of an old PDF and becomes playable after the corrected save', async () => {
  const url = '/uploads/ab.pdf'
  const record = {
    ...DEFAULT_AUDIO_PREFERENCES,
    id: createAudiobookId(url, DEFAULT_AUDIO_PREFERENCES),
    documentUrl: url,
    title: 'PDF',
    savedAt: 1,
    chunks: 1,
  }
  vi.mocked(api.listNativeSavedAudiobooks).mockResolvedValue([record])
  vi.mocked(api.getNativeAudiobookStatus).mockResolvedValue({
    cachedChunks: 0,
    totalChunks: 1,
    complete: false,
    requiresRegeneration: true,
    dir: 'saved',
    audioDurationSec: 0,
    wavBytes: 0,
  })
  const harness = await mount(url)
  try {
    expect(harness.current.audioControlsProps.audiobookState.status).toBe('regeneration-required')
    expect(harness.current.audioControlsProps.canPlayAudiobook).toBe(false)
    expect(harness.current.audiobooksPanelProps.savedAudiobooks[0].requiresRegeneration).toBe(true)
    expect(await harness.current.playFromBeginning()).toMatchObject({ reason: 'not-ready' })
    mocks.confirm.mockResolvedValueOnce(false)
    await act(async () => harness.current.audioControlsProps.onSave())
    expect(mocks.confirm).toHaveBeenLastCalledWith(expect.objectContaining({
      description: expect.stringContaining('replaces'),
    }))
    expect(api.saveNativeAudiobook).not.toHaveBeenCalled()
    expect(api.deleteNativeAudiobook).not.toHaveBeenCalled()
    await act(async () => harness.current.audioControlsProps.onSave())
    expect(api.saveNativeAudiobook).toHaveBeenCalledWith(expect.objectContaining({
      chunks: mocks.chunks,
      documentUrl: url,
    }))
    vi.mocked(api.listNativeSavedAudiobooks).mockResolvedValue([{
      ...record,
      savedAt: 2,
    }])
    vi.mocked(api.getNativeAudiobookStatus).mockResolvedValue({
      cachedChunks: 1,
      totalChunks: 1,
      complete: true,
      requiresRegeneration: false,
      dir: 'saved',
      audioDurationSec: 1,
      wavBytes: 100,
    })
    await act(async () => mocks.finishSave())
    expect(harness.current.audioControlsProps.canPlayAudiobook).toBe(true)
    expect(harness.current.audiobooksPanelProps.savedAudiobooks[0].requiresRegeneration).not.toBe(true)
  } finally {
    await harness.dispose()
  }
})

it.each(['bundle', 'wav'] as const)('blocks stale PDF %s export before invoking native export and retains deletion', async format => {
  const record = {
    ...DEFAULT_AUDIO_PREFERENCES,
    id: createAudiobookId('/uploads/ab.pdf', DEFAULT_AUDIO_PREFERENCES),
    documentUrl: '/uploads/ab.pdf',
    title: 'PDF',
    savedAt: 1,
    chunks: 1,
  }
  vi.mocked(api.listNativeSavedAudiobooks).mockResolvedValue([record])
  const harness = await mount()
  try {
    vi.mocked(api.getNativeAudiobookStatus).mockResolvedValueOnce({
      cachedChunks: 0,
      totalChunks: 1,
      complete: false,
      requiresRegeneration: true,
      dir: 'saved',
      audioDurationSec: 0,
      wavBytes: 0,
    })
    await act(async () => harness.current.audiobooksPanelProps.onExportSaved(record, format))
    expect(api.exportNativeAudiobook).not.toHaveBeenCalled()
    expect(harness.current.audiobookNotice?.status).toBe('error')
    expect(harness.current.audiobooksPanelProps.savedAudiobooks[0].requiresRegeneration).toBe(true)
    await act(async () => harness.current.audiobooksPanelProps.onDeleteSaved(record))
    expect(api.deleteNativeAudiobook).toHaveBeenCalledWith(expect.objectContaining({ deleteUserUpload: false }))
  } finally {
    await harness.dispose()
  }
})

/** The registry can finish after the status IPC; the later generation must still receive its notice. */
it('rechecks a PDF when its saved generation arrives after the initial status', async () => {
  const url = '/uploads/ab.pdf'
  const record = {
    ...DEFAULT_AUDIO_PREFERENCES,
    id: createAudiobookId(url, DEFAULT_AUDIO_PREFERENCES),
    documentUrl: url,
    title: 'PDF',
    savedAt: 1,
    chunks: 1,
  }
  let release!: (records: (typeof record)[]) => void
  vi.mocked(api.listNativeSavedAudiobooks).mockReturnValueOnce(new Promise(resolve => {
    release = resolve
  }))
  vi.mocked(api.getNativeAudiobookStatus).mockResolvedValue({
    cachedChunks: 0,
    totalChunks: 1,
    complete: false,
    requiresRegeneration: true,
    dir: 'saved',
    audioDurationSec: 0,
    wavBytes: 0,
  })
  const harness = await mount(url)
  try {
    expect(harness.current.audiobooksPanelProps.savedAudiobooks).toEqual([])
    const checks = vi.mocked(api.getNativeAudiobookStatus).mock.calls.length
    await act(async () => release([record]))
    expect(api.getNativeAudiobookStatus).toHaveBeenCalledTimes(checks + 1)
    expect(harness.current.audiobooksPanelProps.savedAudiobooks[0].requiresRegeneration).toBe(true)
  } finally {
    await harness.dispose()
  }
})

it.each([false, true])('restarts an old interrupted PDF only after confirmation (%s)', async accept => {
  const harness = await mount()
  try {
    vi.mocked(api.getNativeAudiobookStatus).mockResolvedValueOnce({
      cachedChunks: 0,
      totalChunks: 1,
      complete: false,
      requiresRegeneration: true,
      dir: 'saved',
      audioDurationSec: 0,
      wavBytes: 0,
    })
    mocks.confirm.mockResolvedValueOnce(accept)
    await act(async () => harness.current.audiobooksPanelProps.onResumeQueued({
      ...DEFAULT_AUDIO_PREFERENCES,
      id: 'pdf',
      documentUrl: '/uploads/ab.pdf',
      title: 'PDF',
      status: 'paused',
      cachedChunks: 5,
      totalChunks: 9,
      message: '',
      createdAt: 0,
      updatedAt: 0,
      sessionId: null,
    }))
    expect(mocks.confirm).toHaveBeenCalledOnce()
    expect(api.saveNativeAudiobook).toHaveBeenCalledTimes(accept ? 1 : 0)
    expect(api.deleteNativeAudiobook).not.toHaveBeenCalled()
    if (accept) {
      expect(api.saveNativeAudiobook).toHaveBeenCalledWith(expect.objectContaining({ chunks: mocks.chunks }))
    }
  } finally {
    await harness.dispose()
  }
})

it.each(['/book.html', '/uploads/ab.pdf', '/uploads/ab.epub'].flatMap(url => [false, true].map(accept => ({
  url,
  accept,
}))))(
  'requires fresh engine-restart consent for $url (accept=$accept), without persisting it',
  async ({ url, accept }) => {
    const harness = await mount()
    const token = 'retained-generation-token'
    const record = {
      ...DEFAULT_AUDIO_PREFERENCES,
      id: createAudiobookId(url, DEFAULT_AUDIO_PREFERENCES),
      documentUrl: url,
      title: 'Interrupted book',
      status: 'paused' as const,
      cachedChunks: 1,
      totalChunks: 2,
      message: '',
      createdAt: 0,
      updatedAt: 0,
      sessionId: null,
    }
    try {
      vi.mocked(api.getNativeAudiobookStatus).mockResolvedValue({
        cachedChunks: 1,
        totalChunks: 2,
        complete: false,
        requiresRegeneration: true,
        regenerationReason: 'synthesis',
        restartGenerationToken: token,
        dir: 'saved',
        audioDurationSec: 1,
        wavBytes: 100,
      })
      mocks.confirm.mockResolvedValueOnce(accept)
      await act(async () => harness.current.audiobooksPanelProps.onResumeQueued(record))
      expect(mocks.confirm).toHaveBeenLastCalledWith(expect.objectContaining({
        description: expect.stringContaining('deletes its saved progress'),
        confirmLabel: 'Restart from beginning',
      }))
      expect(api.saveNativeAudiobook).toHaveBeenCalledTimes(accept ? 1 : 0)
      expect(api.deleteNativeAudiobook).not.toHaveBeenCalled()
      if (accept) {
        expect(api.saveNativeAudiobook).toHaveBeenCalledWith(expect.objectContaining({
          options: expect.objectContaining({ restartGenerationToken: token }),
        }))
        expect(JSON.stringify(localStorage)).not.toContain(token)
        // A restored queue record is still checked and confirmed on its next attempt.
        mocks.confirm.mockResolvedValueOnce(false)
        await act(async () => harness.current.audiobooksPanelProps.onResumeQueued(record))
        expect(mocks.confirm).toHaveBeenCalledTimes(2)
        expect(api.saveNativeAudiobook).toHaveBeenCalledOnce()
      }
    } finally {
      await harness.dispose()
    }
  },
)

it.each([false, true])('checks the primary Save generation before engine-restart consent (accept=%s)', async accept => {
  const harness = await mount()
  try {
    vi.mocked(api.getNativeAudiobookStatus).mockResolvedValue({
      cachedChunks: 1,
      totalChunks: 2,
      complete: false,
      requiresRegeneration: true,
      regenerationReason: 'synthesis',
      restartGenerationToken: 'primary-save-token',
      dir: 'saved',
      audioDurationSec: 1,
      wavBytes: 100,
    })
    mocks.confirm.mockResolvedValueOnce(accept)
    await act(async () => harness.current.audioControlsProps.onSave())
    expect(api.getNativeAudiobookStatus).toHaveBeenLastCalledWith('/book.html', mocks.chunks,
      expect.objectContaining({ speed: 1 }))
    expect(mocks.confirm).toHaveBeenLastCalledWith(expect.objectContaining({
      description: expect.stringContaining('deletes its saved progress'),
      confirmLabel: 'Restart from beginning',
    }))
    expect(api.saveNativeAudiobook).toHaveBeenCalledTimes(accept ? 1 : 0)
    if (accept) {
      expect(api.saveNativeAudiobook).toHaveBeenCalledWith(expect.objectContaining({
        options: expect.objectContaining({ restartGenerationToken: 'primary-save-token' }),
      }))
    }
  } finally {
    await harness.dispose()
  }
})

it('leaves audio untouched when the primary Save cannot check the retained generation', async () => {
  const harness = await mount()
  try {
    vi.mocked(api.getNativeAudiobookStatus).mockRejectedValueOnce(new Error('Saved audio could not be checked'))
    await act(async () => harness.current.audioControlsProps.onSave())
    expect(mocks.confirm).toHaveBeenLastCalledWith(expect.objectContaining({
      description: 'Saved audio could not be checked',
      cancelLabel: null,
    }))
    expect(api.saveNativeAudiobook).not.toHaveBeenCalled()
    expect(api.deleteNativeAudiobook).not.toHaveBeenCalled()
  } finally {
    await harness.dispose()
  }
})

it('resumes corrected PDF chunks normally and leaves a failed preparation untouched', async () => {
  const harness = await mount()
  const record = {
    ...DEFAULT_AUDIO_PREFERENCES,
    id: 'pdf',
    documentUrl: '/uploads/ab.pdf',
    title: 'PDF',
    status: 'paused' as const,
    cachedChunks: 1,
    totalChunks: 2,
    message: '',
    createdAt: 0,
    updatedAt: 0,
    sessionId: null,
  }
  try {
    await act(async () => harness.current.audiobooksPanelProps.onResumeQueued(record))
    expect(mocks.confirm).not.toHaveBeenCalled()
    expect(api.saveNativeAudiobook).toHaveBeenCalledOnce()
    mocks.getChunks.mockRejectedValueOnce(new Error('PDF sidecar unavailable'))
    await act(async () => harness.current.audiobooksPanelProps.onResumeQueued(record))
    expect(mocks.confirm).toHaveBeenLastCalledWith(expect.objectContaining({ description: 'PDF sidecar unavailable' }))
    expect(api.saveNativeAudiobook).toHaveBeenCalledOnce()
    expect(api.deleteNativeAudiobook).not.toHaveBeenCalled()
  } finally {
    await harness.dispose()
  }
})

it.each(['/uploads/ab.pdf', '/uploads/ab.html', '/user-uploads/ab.html'])('exports matching audio without regeneration for %s', async url => {
  const harness = await mount()
  try {
    await act(async () => harness.current.audiobooksPanelProps.onExportSaved({
      ...DEFAULT_AUDIO_PREFERENCES,
      id: createAudiobookId(url, DEFAULT_AUDIO_PREFERENCES),
      documentUrl: url,
      title: 'Book',
      savedAt: 1,
      chunks: 1,
    }, 'bundle'))
    expect(api.exportNativeAudiobook).toHaveBeenCalledOnce()
    expect(harness.current.audiobookNotice?.status).toBe('success')
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

/** Generated speed belongs to the cache identity; the independent playback rate must survive a new save. */
it.each([0.8, 1.2])('selects the fixed 1x save when regenerating a legacy %sx PDF', async speed => {
  const url = '/uploads/ab.pdf'
  const oldOptions = {
    ...DEFAULT_AUDIO_PREFERENCES,
    speed,
  }
  const record = {
    ...oldOptions,
    id: createAudiobookId(url, oldOptions),
    documentUrl: url,
    title: 'PDF',
    savedAt: 1,
    chunks: 1,
  }
  let generated = false
  vi.mocked(api.listNativeSavedAudiobooks).mockResolvedValue([record])
  vi.mocked(api.getNativeAudiobookStatus).mockImplementation(async (_url, _chunks, options) => ({
    cachedChunks: generated && options.speed === 1 ? 1 : 0,
    totalChunks: 1,
    complete: generated && options.speed === 1,
    requiresRegeneration: options.speed === speed,
    dir: 'saved',
    audioDurationSec: 0,
    wavBytes: 0,
  }))
  const harness = await mount(url)
  try {
    await act(async () => harness.current.audioControlsProps.onSelectSavedAudiobook(record))
    expect(harness.current.audioControlsProps.audiobookState.status).toBe('regeneration-required')
    await act(async () => harness.current.audioControlsProps.onPlaybackRateChange(1.5))
    mocks.confirm.mockResolvedValueOnce(false)
    await act(async () => harness.current.audioControlsProps.onSave())
    expect(harness.current.audioControlsProps.selectedAudiobookId).toBe(record.id)
    expect(mocks.confirm).toHaveBeenLastCalledWith(expect.objectContaining({ description: expect.stringContaining('1×') }))
    expect(api.saveNativeAudiobook).not.toHaveBeenCalled()
    await act(async () => harness.current.audioControlsProps.onSave())
    expect(api.saveNativeAudiobook).toHaveBeenCalledOnce()
    const save = vi.mocked(api.saveNativeAudiobook).mock.calls[0][0]
    expect(save.options.speed).toBe(1)
    expect(harness.current.audioControlsProps.selectedAudiobookId).toBe(createAudiobookId(url, save.options))
    expect(harness.current.audioControlsProps.playbackRate).toBe(1.5)
    generated = true
    vi.mocked(api.listNativeSavedAudiobooks).mockResolvedValue([record, {
      ...record,
      speed: save.options.speed,
      id: createAudiobookId(url, save.options),
      savedAt: 2,
    }])
    await act(async () => mocks.finishSave())
    expect(harness.current.audioControlsProps.canPlayAudiobook).toBe(true)
    expect(harness.current.audioControlsProps.audiobookState.status).toBe('saved')
  } finally {
    await harness.dispose()
  }
})
