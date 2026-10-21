// @vitest-environment jsdom
import { act, createElement, useEffect } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { TranslationError, type TranslationStartRequest, type TranslationJobProgress, type TranslationModelInstallProgress, type TranslationModelInfo, type TranslationModelStatus } from '../api/nativeTranslation'
import { useTranslationManager } from './useTranslationManager'
import { TranslationPanel } from '../components/TranslationPanel'

vi.mock('react-i18next', async original => ({ ...await original<typeof import('react-i18next')>(), useTranslation: () => ({ t: (key: string) => key, i18n: { language: 'en' } }) }))

const mock = vi.hoisted(() => ({ start: vi.fn(), cancel: vi.fn(), list: vi.fn(), libraryChanged: vi.fn(), listen: vi.fn(), listenInstall: vi.fn(), installProgress: null as null | ((event: TranslationModelInstallProgress) => void), capabilities: vi.fn(), status: vi.fn(), install: vi.fn(), remove: vi.fn(), progress: null as null | ((event: TranslationJobProgress) => void) }))
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
  listenTranslationModelInstallProgress: mock.listenInstall,
}))
let manager: ReturnType<typeof useTranslationManager>
let root: Root
let container: HTMLDivElement
let rejectJob: (error: unknown) => void
const request: TranslationStartRequest = { documentUrl: '/uploads/example.html', sourceLanguage: 'fr', targetLanguage: 'en', modelId: 'bergamot-fr-en-worker', qualityMode: 'balanced' }
const savedDocument = {
  id: 'saved-despite-lost-reply', title: 'Saved', documentUrl: '/uploads/translated.html',
  sourceDocumentUrl: request.documentUrl, sourceLanguage: 'fr', targetLanguage: 'en',
  modelId: request.modelId, status: 'stored', createdAtMs: 1, updatedAtMs: 1,
}
/** Keep the hook and real controls connected so disabled buttons cannot hide a broken retry. */
function Harness() {
  const state = useTranslationManager({ enabled: true, onDocumentLibraryChanged: mock.libraryChanged })
  useEffect(() => { manager = state }, [state])
  return createElement(TranslationPanel, {
    ...state,
    selectedDocument: { title: 'Synthetic book', url: request.documentUrl, format: 'html' },
    onOpenTranslatedDocument: () => {},
  })
}
beforeEach(async () => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true)
  vi.clearAllMocks()
  mock.listenInstall.mockImplementation(handler => { mock.installProgress = handler; return Promise.resolve(() => {}) })
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
    mock.list.mockResolvedValue([savedDocument])
    await act(async () => {
      rejectJob(new TranslationError('translation-outcome-unknown', 'Check the Library before trying again'))
      await running
    })
    expect(manager.startState).toMatchObject({ checking: false, cancelling: false, result: null, message: 'Check the Library before trying again' })
    expect(mock.list).toHaveBeenCalledOnce()
    expect(mock.libraryChanged).toHaveBeenCalledOnce()
    expect(manager.translatedDocuments).toEqual([savedDocument])
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
    mock.capabilities.mockResolvedValue({ models: [nativeModel], formatting: { modelId: pack, available: false } })
    mock.status.mockImplementation(async modelId => ({ modelId, installed: modelId === pack, installing: false }))
    await act(async () => { await manager.refresh() })
    expect(mock.status.mock.calls.map(c => c[0])).toEqual([nativeModel.id, pack])
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

const nativeModel: TranslationModelInfo = {
  id: 'opus-mt-fr-en-ctranslate2', name: 'OPUS French', engine: 'ctranslate2', available: true,
  tier: 'fast', manifestState: 'pinned-file-manifest', sourceLanguages: ['fr'], targetLanguages: ['en'],
  defaultQualityMode: 'balanced', recommendedPlatforms: ['desktop'], licenseNotes: '', sizeNotes: '', notes: '',
}
const absent: TranslationModelStatus = {
  modelId: pack, installed: false, installing: false, sourceUrl: '', sourceLabel: '',
  archiveBytes: 123, installedBytes: 0, sha256: '', message: '',
}

/** Ready capability is a fixture only; real native release admission stays disabled. */
async function workbench(modelToInstall = pack) {
  mock.capabilities.mockResolvedValue({
    available: true, backend: 'test', platform: 'linux', reason: '', defaultQualityMode: 'balanced', hardwareAcceleration: null,
    models: [nativeModel], formatting: { available: true, reason: 'available', modelId: pack, archiveBytes: 123 },
  })
  mock.status.mockImplementation(async modelId => ({ ...absent, modelId, installed: modelId === nativeModel.id && modelToInstall !== nativeModel.id }))
  await act(async () => { await manager.refresh() })
  if (modelToInstall === nativeModel.id) {
    await act(async () => { [...container.querySelectorAll('button')].find(b => b.textContent?.includes('translation.models.title'))!.click() })
  }
}
function installButton(modelId = pack) {
  return container.querySelector<HTMLButtonElement>(modelId === pack
    ? 'section[aria-label="translation.formatting.label"] .translation-model-install-btn'
    : '.translation-model-item .translation-model-install-btn')!
}
function installProgress(modelId: string, status = 'downloading'): TranslationModelInstallProgress {
  return { modelId, status, message: 'Synthetic download progress', downloadedBytes: 30, totalBytes: 123, percent: 24 }
}

/** Drive real event ordering and buttons, rather than invoking a disabled retry handler directly. */
describe('download failure recovery through the workbench', () => {
  it.each([pack, nativeModel.id])('retries %s after progress and failure, ignoring finished-attempt events', async modelId => {
    await workbench(modelId)
    let fail!: (reason: Error) => void
    mock.install.mockImplementationOnce(() => new Promise((_resolve, reject) => { fail = reject }))
    await act(async () => { installButton(modelId).click() })
    await act(async () => { mock.installProgress!(installProgress(modelId, 'starting')); mock.installProgress!(installProgress(modelId)) })
    expect(manager.modelStatuses[modelId].installing).toBe(true)
    expect(installButton(modelId).disabled).toBe(true)
    await act(async () => { fail(new Error('Download interrupted')) })
    expect(manager.modelInstallState).toMatchObject({ installingModelId: '', progress: null, result: null, message: 'Download interrupted' })
    expect(manager.modelStatuses[modelId]).toMatchObject({ installed: false, installing: false })
    expect(installButton(modelId).disabled).toBe(false)
    // A queued event from the failed attempt cannot relock the retry control.
    await act(async () => { mock.installProgress!(installProgress(modelId)) })
    expect(installButton(modelId).disabled).toBe(false)
    expect(manager.modelInstallState.message).toBe('Download interrupted')
    let succeed!: (value: { modelId: string; bytes: number }) => void
    mock.install.mockImplementationOnce(() => new Promise(resolve => { succeed = resolve }))
    await act(async () => { installButton(modelId).click() })
    await act(async () => { mock.installProgress!({ ...installProgress(modelId), percent: 54, downloadedBytes: 66 }) })
    expect(manager.modelInstallState.progress?.percent).toBe(54)
    expect(installButton(modelId).disabled).toBe(true)
    mock.status.mockImplementation(async id => ({ ...absent, modelId: id, installed: true }))
    await act(async () => { succeed({ modelId, bytes: 123 }) })
    expect(mock.install).toHaveBeenCalledTimes(2)
    expect(manager.modelStatuses[modelId]).toMatchObject({ installed: true, installing: false })
    expect(manager.modelInstallState.result).toMatchObject({ modelId })
  })

  it('releases retry but never advertises verified assets if the failure status check also fails', async () => {
    await workbench()
    let fail!: (reason: Error) => void
    mock.install.mockImplementationOnce(() => new Promise((_resolve, reject) => { fail = reject }))
    await act(async () => { installButton().click(); mock.installProgress!(installProgress(pack)) })
    mock.status.mockRejectedValueOnce(new Error('Status unavailable'))
    await act(async () => { fail(new Error('Download interrupted')) })
    expect(installButton().disabled).toBe(false)
    expect(manager.modelInstallState.message).toBe('Download interrupted')
    expect(manager.modelStatuses[pack]).toMatchObject({ installed: false, installing: false })
    expect(container.querySelector<HTMLInputElement>('section input[type="checkbox"]')!.disabled).toBe(true)
    expect(manager.modelStatuses[nativeModel.id].installed).toBe(true)
  })

  it('still observes a native install already running when the workbench refreshes', async () => {
    await workbench()
    mock.status.mockResolvedValueOnce({ ...absent, installing: true })
    mock.status.mockResolvedValueOnce({ ...absent, installing: true })
    await act(async () => { await manager.refresh() })
    await act(async () => { mock.installProgress!(installProgress(pack)) })
    expect(manager.modelInstallState.installingModelId).toBe(pack)
    await act(async () => { mock.installProgress!(installProgress(pack, 'installed')) })
    expect(manager.modelStatuses[pack]).toMatchObject({ installed: true, installing: false })
    await act(async () => { mock.installProgress!(installProgress(pack)) })
    expect(manager.modelStatuses[pack]).toMatchObject({ installed: true, installing: false })
    expect(mock.install).not.toHaveBeenCalled()
  })

  it('keeps a restored valid pack verified after an unsuccessful repair', async () => {
    await workbench()
    mock.install.mockRejectedValueOnce(new Error('Repair interrupted'))
    mock.status.mockResolvedValueOnce({ ...absent, installed: true, installedBytes: 123 })
    await act(async () => { installButton().click() })
    expect(manager.modelStatuses[pack]).toMatchObject({ installed: true, installing: false })
    expect(manager.modelInstallState.message).toBe('Repair interrupted')
    expect(container.querySelector<HTMLInputElement>('section input[type="checkbox"]')!.disabled).toBe(false)
  })
})

describe('command rejection finalizes visible progress', () => {
  it.each([
    ['loading-model', 'translation-failed', 'failed', 'translation.status.translationFailed'],
    ['loading-alignment', 'alignment-assets-invalid', 'failed', 'translation.status.translationFailed'],
    ['loading-model', 'operation-cancelled', 'cancelled', 'translation.progress.status.cancelled'],
    ['loading-alignment', 'operation-cancelled', 'cancelled', 'translation.progress.status.cancelled'],
  ])('%s ends as %s without a native terminal event', async (phase, code, status, label) => {
    await workbench()
    const { running } = await begin(nativeModel.id)
    const jobId = manager.startState.jobId
    const progress = { jobId, status: phase, message: 'Loading', totalSegments: 10, completedSegments: 0, totalBatches: 2, completedBatches: 0, percent: 0, elapsedMs: 10 } as TranslationJobProgress
    await act(async () => { mock.progress!(progress) })
    expect(container.querySelector('.translation-progress-meter.is-indeterminate')).not.toBeNull()
    if (code === 'operation-cancelled') await act(async () => { await manager.onCancelTranslation() })
    await act(async () => { rejectJob(new TranslationError(code, 'Synthetic load ended')); await running })
    expect(manager.startState).toMatchObject({ checking: false, cancelling: false, result: null, progress: { ...progress, status } })
    expect(container.querySelector('.translation-progress-meter.is-indeterminate')).toBeNull()
    expect(container.querySelector('[role="progressbar"]')?.getAttribute('aria-valuetext')).toBe(label)
    // A late native notification cannot restart the ended animation.
    await act(async () => { mock.progress!({ ...progress, message: 'Late loading' }) })
    expect(manager.startState.progress?.status).toBe(status)
    const retry = await begin(nativeModel.id)
    expect(manager.startState.jobId).not.toBe(jobId)
    expect(manager.startState.progress).toBeNull()
    await act(async () => { rejectJob(new TranslationError('operation-cancelled', 'Cleanup')); await retry.running })
  })

  it('retains partial counts and the actionable error after a rejection', async () => {
    await workbench()
    const { running } = await begin(nativeModel.id)
    const progress = { jobId: manager.startState.jobId, status: 'aligning-batch', message: 'Aligning', totalSegments: 10, completedSegments: 4, totalBatches: 3, completedBatches: 1, cachedSegments: 2, percent: 40, elapsedMs: 10 } as TranslationJobProgress
    await act(async () => { mock.progress!(progress) })
    await act(async () => { rejectJob(new TranslationError('alignment-mapping-failed', 'Synthetic mapping rejected')); await running })
    expect(manager.startState.progress).toMatchObject({ ...progress, status: 'failed' })
    expect(container.querySelector('[role="progressbar"]')?.getAttribute('aria-valuenow')).toBe('40')
    expect(container.querySelector('[role="progressbar"]')?.getAttribute('aria-valuetext')).toBe('translation.status.translationFailed')
    expect(manager.startState.message).not.toBe('Synthetic mapping rejected')
    expect(manager.startState.message).not.toBe('Aligning')
  })
})
