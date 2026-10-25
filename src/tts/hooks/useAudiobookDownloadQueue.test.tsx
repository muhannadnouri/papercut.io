// @vitest-environment jsdom
import { act, useLayoutEffect } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import * as api from '../api/nativeTts'
import { useAudiobookDownloadQueue } from './useAudiobookDownloadQueue'
import { DEFAULT_TTS_MODEL_ID } from '../types'

vi.mock('../api/nativeTts', async importOriginal => ({
  ...await importOriginal<typeof import('../api/nativeTts')>(),
  getNativeTtsCapabilities: vi.fn(), listenNativeAudiobookSaveProgress: vi.fn(),
  saveNativeAudiobook: vi.fn(), cancelNativeAudiobookSave: vi.fn(),
}))
vi.mock('../diagnostics/TtsDiagnostics', () => ({ logTtsDiagnostic: vi.fn(), summarizeTtsCapabilities: vi.fn() }))
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })

let root: Root
let queue: ReturnType<typeof useAudiobookDownloadQueue>
let progress: (event: api.NativeAudiobookSaveProgress) => void
let finish: (result: api.NativeAudiobookSaveResult) => void
let fail: (error: Error) => void
let jobId: string
const onCompleted = vi.fn()
const input = { documentUrl: '/uploads/ab.pdf', title: 'Reader handbook', modelId: DEFAULT_TTS_MODEL_ID,
  textPreprocessor: 'none', voice: 'af_heart', speed: 1, dtype: 'native' as const,
  chunks: [{ id: 'first', text: 'First topic.' }, { id: 'second', text: 'Second topic.' }] }
const result = (): api.NativeAudiobookSaveResult => ({ jobId, cachedChunks: 2, totalChunks: 2,
  generatedChunks: 2, complete: true, dir: 'saved', generateMs: 10, audioDurationSec: 2,
  wavBytes: 100, appliedThreadCount: 1, backend: 'native' })
function emit(status: string) {
  progress({ jobId, status, message: status, cachedChunks: 2, totalChunks: 2, generatedChunks: 2,
    totalAudioDurationSec: 2, totalWavBytes: 100, appliedThreadCount: 1, backend: 'native' })
}

beforeEach(async () => {
  vi.clearAllMocks()
  localStorage.clear()
  vi.mocked(api.getNativeTtsCapabilities).mockResolvedValue({ available: true } as api.NativeTtsCapabilities)
  vi.mocked(api.listenNativeAudiobookSaveProgress).mockImplementation(async handler => {
    progress = handler
    return () => {}
  })
  vi.mocked(api.cancelNativeAudiobookSave).mockResolvedValue()
  vi.mocked(api.saveNativeAudiobook).mockImplementation(input => {
    jobId = input.jobId
    return new Promise((resolve, reject) => { finish = resolve; fail = reject })
  })
  function Harness() {
    const current = useAudiobookDownloadQueue({ threadCount: 1, onCompleted })
    useLayoutEffect(() => { queue = current }, [current])
    return null
  }
  const container = document.createElement('div')
  document.body.append(container)
  root = createRoot(container)
  await act(async () => root.render(<Harness />))
  await act(async () => queue.start(input))
})
afterEach(async () => {
  await act(async () => finish(result()))
  await act(async () => root.unmount())
  document.body.replaceChildren()
  localStorage.clear()
})

it.each(['event', 'response'])('refreshes the registry only after manifest finalization via %s', async completion => {
  await act(async () => emit('saving'))
  expect(queue.state.cachedChunks).toBe(2)
  expect(queue.state.status).toBe('saving')
  expect(queue.state.complete).toBe(false)
  expect(onCompleted).not.toHaveBeenCalled()
  if (completion === 'event') await act(async () => emit('saved'))
  await act(async () => finish(result()))
  expect(queue.state.complete).toBe(true)
  expect(onCompleted).toHaveBeenCalledOnce()
  expect(queue.downloads).toEqual([])
})

it('does not advertise a completed audiobook if finalization fails after the last WAV', async () => {
  await act(async () => emit('saving'))
  await act(async () => fail(new Error('manifest commit failed')))
  expect(queue.state.status).toBe('error')
  expect(queue.state.complete).toBe(false)
  expect(onCompleted).not.toHaveBeenCalled()
  expect(queue.downloads[0]?.status).toBe('error')
})

it('does not infer completion from the chunk count when cancelling before finalization', async () => {
  await act(async () => emit('saving'))
  await act(async () => queue.cancel())
  await act(async () => finish(result()))
  expect(queue.state.status).toBe('partial')
  expect(queue.state.complete).toBe(false)
  expect(onCompleted).not.toHaveBeenCalled()
  expect(api.cancelNativeAudiobookSave).toHaveBeenCalledWith(jobId)
})
