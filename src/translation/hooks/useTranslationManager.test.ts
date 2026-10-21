// @vitest-environment jsdom
import { act, createElement, useEffect } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { TranslationError, type TranslationStartRequest, type TranslationJobProgress } from '../api/nativeTranslation'
import { useTranslationManager } from './useTranslationManager'

const mock = vi.hoisted(() => ({ start: vi.fn(), cancel: vi.fn(), list: vi.fn(), libraryChanged: vi.fn(), listen: vi.fn(), capabilities: vi.fn(), status: vi.fn(), install: vi.fn(), remove: vi.fn(), progress: null as null | ((event: TranslationJobProgress) => void) }))
vi.mock('../api/nativeTranslation', async importOriginal => ({
  ...await importOriginal<typeof import('../api/nativeTranslation')>(),
  startTranslationJob: mock.start,
  cancelTranslationJob: mock.cancel,
  getTranslationCapabilities: mock.capabilities,
  getTranslationModelStatus: mock.status,
  installTranslationModel: mock.install,
  removeTranslationModel: mock.remove,
  listTranslatedDocuments: mock.list,
  listenTranslationProgress: mock.listen,
}))
let manager: ReturnType<typeof useTranslationManager>
let root: Root
let container: HTMLDivElement
let rejectJob: (error: unknown) => void
const request: TranslationStartRequest = { documentUrl: '/uploads/example.html', sourceLanguage: 'fr', targetLanguage: 'en', modelId: 'bergamot-fr-en-worker', qualityMode: 'balanced' }
function Harness() {
  const state = useTranslationManager({ enabled: true, onDocumentLibraryChanged: mock.libraryChanged })
  useEffect(() => { manager = state }, [state])
  return null
}
beforeEach(async () => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true)
  vi.clearAllMocks()
  mock.listen.mockImplementation(handler => { mock.progress = handler; return Promise.resolve(() => {}) })
  mock.capabilities.mockResolvedValue({ models: [] })
  mock.status.mockResolvedValue({ installed: false, installing: false })
  mock.cancel.mockResolvedValue(undefined)
  mock.list.mockResolvedValue([])
  mock.libraryChanged.mockResolvedValue(undefined)
  mock.start.mockImplementation((_request, signal?: AbortSignal) => new Promise((_resolve, reject) => {
    rejectJob = reject
    signal?.addEventListener('abort', () => reject(signal.reason), { once: true })
  }))
  container = document.createElement('div')
  root = createRoot(container)
  await act(async () => { root.render(createElement(Harness)) })
  vi.clearAllMocks()
})
afterEach(async () => {
  await act(async () => {
    rejectJob?.(new Error('Test cleanup'))
    await Promise.resolve()
    root.unmount()
  })
  container.remove()
  vi.unstubAllGlobals()
})
async function begin(modelId = request.modelId) {
  let running!: Promise<void>
  await act(async () => { running = manager.onStartTranslationPreflight({ ...request, modelId }) })
  return { running }
}

describe('translation manager cancellation ownership', () => {
  it('aborts the worker job signal on Cancel and gives a retry a fresh owner', async () => {
    const first = await begin()
    const signal = mock.start.mock.calls[0][1] as AbortSignal
    expect(signal).toBeInstanceOf(AbortSignal)
    expect(manager.startState.checking).toBe(true)
    await act(async () => { await manager.onCancelTranslation(); await first.running })
    expect(signal.aborted).toBe(true)
    expect(signal.reason).toMatchObject({ code: 'operation-cancelled' })
    expect(mock.cancel).not.toHaveBeenCalled() // The bridge cancels Rust only after that job starts.
    expect(manager.startState.checking).toBe(false)
    await begin()
    const retry = mock.start.mock.calls[1][1] as AbortSignal
    expect(retry).not.toBe(signal)
    expect(retry.aborted).toBe(false)
    expect(mock.start.mock.calls[1][0].jobId).not.toBe(mock.start.mock.calls[0][0].jobId)
  })

  it('keeps native engine cancellation on the existing Rust command', async () => {
    const native = await begin('opus-mt-fr-en-ctranslate2')
    expect(mock.start.mock.calls[0][1]).toBeUndefined()
    await act(async () => { await manager.onCancelTranslation() })
    expect(mock.cancel).toHaveBeenCalledExactlyOnceWith(mock.start.mock.calls[0][0].jobId)
    await act(async () => { rejectJob(new TranslationError('operation-cancelled', 'Translation cancelled')); await native.running })
    expect(manager.startState.checking).toBe(false)
  })

  it('refreshes the Library on an unknown outcome without claiming cancellation or success', async () => {
    const { running } = await begin()
    const jobId = manager.startState.jobId
    mock.list.mockResolvedValue([{ id: 'saved-despite-lost-reply' }])
    await act(async () => {
      rejectJob(new TranslationError('translation-outcome-unknown', 'Check the Library before trying again'))
      await running
    })
    expect(manager.startState).toMatchObject({ checking: false, cancelling: false, result: null, message: 'Check the Library before trying again' })
    expect(mock.list).toHaveBeenCalledOnce()
    expect(mock.libraryChanged).toHaveBeenCalledOnce()
    expect(manager.translatedDocuments).toEqual([{ id: 'saved-despite-lost-reply' }])
    await act(async () => { mock.progress!({ jobId, status: 'stored', message: 'Late save event' } as TranslationJobProgress) })
    expect(manager.startState.message).toBe('Check the Library before trying again')
    await begin()
    await act(async () => { mock.progress!({ jobId, status: 'stored', message: 'Old save event' } as TranslationJobProgress) })
    expect(manager.startState.message).toBe('Preparing translation job')
  })
})

const pack = 'alignment-awesome-align-int8'
describe('auxiliary setup uses the shared installer lifecycle', () => {
  it('refreshes the separate auxiliary status without inserting it into engine choices or downloading', async () => {
    mock.capabilities.mockResolvedValue({ models: [{ id: request.modelId }], formatting: { modelId: pack, available: false } })
    mock.status.mockImplementation(async modelId => ({ modelId, installed: modelId === pack, installing: false }))
    await act(async () => { await manager.refresh() })
    expect(mock.status.mock.calls.map(c => c[0])).toEqual([request.modelId, pack])
    expect(manager.modelStatuses[pack].installed).toBe(true)
    expect(manager.capabilities!.models).toHaveLength(1)
    expect(mock.install).not.toHaveBeenCalled()
  })

  it('keeps failed installs/removals actionable and re-verifies status after a successful retry', async () => {
    mock.capabilities.mockResolvedValue({ models: [], formatting: { modelId: pack } })
    mock.status.mockResolvedValue({ modelId: pack, installed: false, installing: false })
    await act(async () => { await manager.refresh() })
    mock.install.mockRejectedValueOnce(new Error('Pack download failed'))
    await act(async () => { await manager.onInstallTranslationModel(pack) })
    expect(manager.modelInstallState).toMatchObject({ installingModelId: '', result: null, message: 'Pack download failed' })
    expect(manager.modelStatuses[pack].installed).toBe(false)
    mock.install.mockResolvedValue({ modelId: pack, bytes: 123 })
    mock.status.mockResolvedValue({ modelId: pack, installed: true, installing: false })
    await act(async () => { await manager.onInstallTranslationModel(pack) })
    expect(manager.modelStatuses[pack].installed).toBe(true)
    mock.remove.mockRejectedValueOnce(new Error('Pack is in use'))
    await act(async () => { await manager.onRemoveTranslationModel(pack) })
    expect(manager.modelRemoveState).toMatchObject({ removingModelId: '', result: null, message: 'Pack is in use' })
    expect(manager.modelStatuses[pack].installed).toBe(true)
    mock.remove.mockResolvedValue({ modelId: pack, removed: true, bytesFreed: 123 })
    mock.status.mockResolvedValue({ modelId: pack, installed: false, installing: false })
    await act(async () => { await manager.onRemoveTranslationModel(pack) })
    expect(manager.modelStatuses[pack].installed).toBe(false)
  })

  it('retains strict intent across native cancellation and retry, ignoring late alignment progress', async () => {
    let first!: Promise<void>
    const strict = { ...request, modelId: 'opus-mt-fr-en-ctranslate2', preserveFormatting: true }
    await act(async () => { first = manager.onStartTranslationPreflight(strict) })
    const jobId = manager.startState.jobId
    await act(async () => { mock.progress!({ jobId, status: 'aligning-batch', message: 'Aligning' } as TranslationJobProgress) })
    expect(manager.startState.progress?.status).toBe('aligning-batch')
    await act(async () => { await manager.onCancelTranslation(); rejectJob(new TranslationError('operation-cancelled', 'Translation cancelled')); await first })
    await act(async () => { mock.progress!({ jobId, status: 'batch-complete', message: 'Late' } as TranslationJobProgress) })
    expect(manager.startState.checking).toBe(false)
    expect(manager.startState.message).not.toBe('Late')
    await act(async () => { void manager.onStartTranslationPreflight(strict) })
    expect(mock.start.mock.calls[1][0]).toMatchObject({ preserveFormatting: true, modelId: strict.modelId })
    expect(manager.startState.jobId).not.toBe(jobId)
  })
})
