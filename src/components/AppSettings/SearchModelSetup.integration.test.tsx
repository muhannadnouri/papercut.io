// @vitest-environment jsdom
import { act, useEffect } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { SearchModelSetup } from './SearchModelSetup'
import { useSemanticSearch, type ProviderStatus, type SemanticSearchController, type DownloadProgress } from '../../search-semantic/useSemanticSearch'
import { semanticSearchStatus } from '../../search-semantic/semanticSearchStatus'
import { invoke } from '@tauri-apps/api/core'
import { listen, type Event } from '@tauri-apps/api/event'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: () => true }))
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }))
vi.mock('react-i18next', async (original) => ({
  ...await original<typeof import('react-i18next')>(),
  useTranslation: () => ({ t: (key: string) => key }),
}))

let root: Root
let container: HTMLDivElement
let semantic: SemanticSearchController
let provider: ProviderStatus
let resolveActivation: () => void
let rejectActivation: (error: Error) => void
let filesPresent: boolean
let storedFiles: boolean
let resolveDownload: () => void
let rejectDownload: (error: string) => void
let downloadJob: number
const downloadListeners = new Set<(event: Event<DownloadProgress>) => void>()
function downloadEvent(phase: DownloadProgress['phase'], error: string | null = null) {
  for (const listener of downloadListeners) listener({ event: 'search-embedding-download-progress', id: 0,
    payload: { jobId: `download-${downloadJob}`, format: 'float32', phase, downloadedBytes: 50, totalBytes: 100, error } })
}

function Harness() {
  const controller = useSemanticSearch()
  useEffect(() => { semantic = controller }, [controller])
  return <SearchModelSetup semantic={controller} />
}

beforeEach(async () => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true)
  localStorage.clear()
  container = document.createElement('div')
  document.body.append(container)
  root = createRoot(container)
  filesPresent = true
  storedFiles = true
  downloadJob = 0
  downloadListeners.clear()
  vi.mocked(listen).mockImplementation(async (_, callback) => {
    const listener = callback as (event: Event<DownloadProgress>) => void
    downloadListeners.add(listener)
    return () => { downloadListeners.delete(listener) }
  })
  provider = { compiled: true, activeFormat: null, loadingFormat: null, downloadingFormat: null }
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === 'search_embedding_status') return { ...provider }
    if (command === 'search_embedding_cache_status') return [{ format: 'float32', filesPresent, hasStoredFiles: storedFiles, expectedBytes: 465000000 }]
    if (command === 'search_semantic_index_status') return { running: false, cancelRequested: false, indexedDocuments: 2,
      eligibleDocuments: 2, passages: 100, hasStoredIndex: true, embeddedPassages: 0, reusedPassages: 0, error: null }
    if (command === 'search_embedding_download') {
      downloadJob++
      provider = { ...provider, downloadingFormat: 'float32' }
      downloadEvent('starting')
      return new Promise<void>((resolve, reject) => { resolveDownload = resolve; rejectDownload = reject })
    }
    if (command === 'search_embedding_cancel_download') return { ...provider }
    if (command === 'search_embedding_configure') {
      if ((args as { format: string | null }).format === null) {
        provider = { ...provider, activeFormat: null, loadingFormat: null }
        return
      }
      return new Promise<void>((resolve, reject) => { resolveActivation = resolve; rejectActivation = reject })
    }
    throw new Error(`Unexpected IPC: ${command}`)
  })
  await act(() => root.render(<Harness />))
})

afterEach(async () => {
  await act(() => root.unmount())
  container.remove()
  vi.useRealTimers()
  vi.clearAllMocks()
  vi.unstubAllGlobals()
})

function toggle() { return container.querySelector<HTMLInputElement>('[role="switch"]')! }
function phase() { return container.querySelector('.search-model-management > summary .search-model-phase') }
function assertDescriptionsResolve() {
  for (const id of toggle().getAttribute('aria-describedby')?.split(' ') ?? []) expect(document.getElementById(id)).not.toBeNull()
}
function assertToggleRow(state: string) {
  const heading = container.querySelector<HTMLElement>('.search-semantic-setup h3')!
  expect(heading.textContent).toBe('settings.semantic.controlsTitle')
  expect(toggle().getAttribute('aria-labelledby')).toBe(heading.id)
  const status = container.querySelector<HTMLElement>('.search-semantic-setup label [role="status"]')
  expect(status).not.toBeNull()
  expect(status!.textContent).toBe(`settings.semantic.summary.${state}`)
  expect(status!.getAttribute('aria-live')).toBe('polite')
  expect(toggle().getAttribute('aria-describedby')?.split(' ')).toContain(status!.id)
  expect(container.querySelector('.search-semantic-setup > [role="status"]')).toBeNull()
  assertDescriptionsResolve()
}
async function enable() { await act(() => toggle().click()) }

it('announces activation once, keeps Off usable, then clears loading and exposes ready index controls', async () => {
  expect(toggle().disabled).toBe(false)
  expect(toggle().checked).toBe(false)
  assertDescriptionsResolve()
  await enable()
  expect(toggle().checked).toBe(true)
  expect(toggle().disabled).toBe(false)
  expect(phase()?.getAttribute('role')).toBe('status')
  expect(phase()?.getAttribute('aria-live')).toBe('polite')
  expect(phase()?.querySelector('.spinner')?.getAttribute('aria-hidden')).toBe('true')
  expect(container.textContent?.match(/settings.searchModel.loadingModel/g)).toHaveLength(1)
  assertToggleRow('loading')
  await act(() => {
    provider = { ...provider, activeFormat: 'float32' }
    resolveActivation()
  })
  expect(phase()).toBeNull()
  expect(container.querySelector('.search-model-management > summary')?.textContent).toContain('settings.searchModel.loaded')
  assertToggleRow('ready')
  expect(container.querySelector('.search-library-index')?.textContent).toContain('settings.semantic.indexReady')
  // Settings without the switch still need their compact readiness summary.
  expect(semanticSearchStatus(semantic)).toBe('ready')
})

it('replaces loading with the retained activation error and an enabled Retry action', async () => {
  await enable()
  await act(() => rejectActivation(new Error('Invalid model file')))
  expect(phase()).toBeNull()
  assertToggleRow('fallback')
  expect(container.querySelector('[role="alert"]')?.textContent).toContain('Invalid model file')
  const retry = [...container.querySelectorAll<HTMLButtonElement>('button')].find(button => button.textContent === 'settings.semantic.retry')!
  expect(retry.disabled).toBe(false)
  expect(toggle().checked).toBe(true)
  assertDescriptionsResolve()
  await act(() => retry.click())
  expect(phase()).not.toBeNull()
  expect(container.querySelector('[role="alert"]')).toBeNull()
  await act(() => resolveActivation())
})

it('clears loading on disable and ignores the late activation completion', async () => {
  await enable()
  await act(() => toggle().click())
  expect(toggle().checked).toBe(false)
  expect(phase()).toBeNull()
  expect(container.querySelector('.search-semantic-setup [role="status"]')?.textContent).toBe('settings.semantic.summary.off')
  assertDescriptionsResolve()
  await act(() => resolveActivation())
  expect(toggle().checked).toBe(false)
  expect(phase()).toBeNull()
  expect(semanticSearchStatus(semantic)).toBe('off')
})

it('keeps native loading feedback after activation IPC settles, while download work keeps its separate status', async () => {
  vi.useFakeTimers()
  await enable()
  await act(() => {
    provider = { ...provider, loadingFormat: 'float32' }
    resolveActivation()
  })
  expect(phase()).not.toBeNull()
  expect(toggle().disabled).toBe(false)
  await act(() => toggle().click())
  provider = { ...provider, loadingFormat: null, downloadingFormat: 'float32' }
  await act(() => semantic.refresh())
  expect(phase()).toBeNull()
  expect(toggle().disabled).toBe(true)
  await act(() => vi.advanceTimersByTimeAsync(1000))
  expect(container.querySelector('.search-download-progress')?.textContent).toContain('settings.searchModel.verifying')
  assertDescriptionsResolve()
})


function button(key: string) {
  return [...container.querySelectorAll<HTMLButtonElement>('button')].find(item => item.textContent === key)
}
async function reopen() {
  await act(() => root.unmount())
  root = createRoot(container)
  await act(() => root.render(<Harness />))
}
async function startDownload() {
  filesPresent = false; storedFiles = false
  await reopen()
  expect(button('settings.searchModel.download')?.disabled).toBe(true)
  await act(() => container.querySelector<HTMLInputElement>('.search-model-consent input')!.click())
  await act(() => button('settings.searchModel.download')!.click())
}
function downloadCalls() { return vi.mocked(invoke).mock.calls.filter(([command]) => command === 'search_embedding_download') }

it.each([true, false])('keeps cancellation local and guarded through cleanup (terminal event: %s)', async (withEvent) => {
  vi.useFakeTimers()
  await startDownload()
  await act(() => button('settings.searchModel.cancel')!.click())
  expect(button('settings.searchModel.cancel')?.disabled).toBe(true)
  expect(toggle().disabled).toBe(true)
  expect(container.querySelector('.search-model-management')?.textContent).not.toContain('settings.searchModel.cancelled')
  expect(downloadCalls()).toHaveLength(1)
  await act(() => {
    provider = { ...provider, downloadingFormat: null }
    storedFiles = true
    if (withEvent) downloadEvent('cancelled', 'Embedding download cancelled')
    rejectDownload('Embedding download cancelled')
  })
  // Native polling reconciles the controller even when its terminal event is missed.
  await act(() => vi.advanceTimersByTimeAsync(1000))
  const panel = container.querySelector<HTMLDetailsElement>('.search-model-management')!
  expect(panel.open).toBe(true)
  expect([...panel.querySelectorAll('[role="status"]')].filter(item => item.textContent === 'settings.searchModel.cancelled')).toHaveLength(1)
  expect(container.textContent).not.toContain('Embedding download cancelled')
  expect(container.querySelector('[role="alert"]')).toBeNull()
  expect(button('common.retry')).toBeUndefined()
  expect(panel.querySelector<HTMLInputElement>('.search-model-consent input')?.checked).toBe(false)
  expect(button('settings.searchModel.download')?.disabled).toBe(true)
  expect(button('settings.searchModel.remove')?.disabled).toBe(false)
  expect(downloadCalls()).toHaveLength(1)
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'search_embedding_configure')).toBe(false)
})

it('keeps real download failure and Retry in the model panel, even after a cancellation request', async () => {
  await startDownload()
  await act(() => button('settings.searchModel.cancel')!.click())
  const diagnostic = 'Embedding download unavailable: connection cancelled by proxy'
  await act(() => {
    provider = { ...provider, downloadingFormat: null }
    downloadEvent('failed', diagnostic)
    rejectDownload(diagnostic)
  })
  const panel = container.querySelector<HTMLDetailsElement>('.search-model-management')!
  expect(panel.open).toBe(true)
  expect(panel.querySelector('[role="alert"]')?.textContent).toBe(diagnostic)
  expect(container.querySelectorAll('[role="alert"]')).toHaveLength(1)
  expect(panel.contains(button('common.retry')!)).toBe(true)
  expect(panel.textContent).not.toContain('settings.searchModel.cancelled')
  await act(() => button('common.retry')!.click())
  expect(container.querySelector('.search-model-management [role="alert"]')?.textContent).toBe(diagnostic)
  expect(button('settings.searchModel.download')?.disabled).toBe(true)
  expect(downloadCalls()).toHaveLength(1)
  await act(() => container.querySelector<HTMLInputElement>('.search-model-consent input')!.click())
  await act(() => button('settings.searchModel.download')!.click())
  expect(downloadCalls()).toHaveLength(2)
  expect(container.querySelector('[role="alert"]')).toBeNull()
  await act(() => {
    provider = { ...provider, downloadingFormat: null }
    filesPresent = true; storedFiles = true
    downloadEvent('installed')
    resolveDownload()
  })
  expect(container.querySelector('.search-model-management')?.textContent).toContain('settings.searchModel.verified')
  expect(toggle().checked).toBe(false)
})

it('keeps setup recovery available when the initial cache-status read cannot open the model panel', async () => {
  const invokeNormally = vi.mocked(invoke).getMockImplementation()!
  let unavailable = true
  vi.mocked(invoke).mockImplementation(async (command, args, options) => {
    if (command === 'search_embedding_cache_status' && unavailable) throw new Error('Cache status unavailable')
    return invokeNormally(command, args, options)
  })
  await reopen()
  expect(container.querySelector('.search-model-management')).toBeNull()
  expect(container.querySelector('[role="alert"]')?.textContent).toContain('Cache status unavailable')
  expect(button('common.retry')?.disabled).toBe(false)
  unavailable = false
  await act(() => button('common.retry')!.click())
  expect(container.querySelector('.search-model-management')).not.toBeNull()
  expect(container.querySelector('[role="alert"]')).toBeNull()
  expect(downloadCalls()).toHaveLength(0)
})


it.each([
  { outcome: 'Semantic indexing cancelled', readyDocuments: 1, expected: 'needsIndex' },
  { outcome: 'Semantic indexing cancelled', readyDocuments: 2, expected: 'ready' },
  { outcome: 'Disk full', readyDocuments: 1, expected: 'fallback' },
  { outcome: 'Semantic indexing cancelled: cleanup failed', readyDocuments: 1, expected: 'fallback' },
] as const)('retains checkpoints after Stop and derives $expected from $outcome ($readyDocuments ready)', async ({ outcome, readyDocuments, expected }) => {
  const normally = vi.mocked(invoke).getMockImplementation()!
  const index = { running: false, cancelRequested: false, indexedDocuments: 0, eligibleDocuments: 2,
    passages: 0, hasStoredIndex: true, embeddedPassages: 0, reusedPassages: 0, error: null as string | null }
  let rejectBuild!: (error: string) => void
  let resolveBuild!: () => void
  vi.mocked(invoke).mockImplementation(async (command, args, options) => {
    if (command === 'search_semantic_index_status') return { ...index }
    if (command === 'search_semantic_index_build') {
      index.running = true
      index.error = null
      return new Promise<void>((resolve, reject) => { resolveBuild = resolve; rejectBuild = reject })
    }
    if (command === 'search_semantic_index_cancel') { index.cancelRequested = true; return }
    return normally(command, args, options)
  })
  await enable()
  await act(() => { provider = { ...provider, activeFormat: 'float32' }; resolveActivation() })
  await act(() => button('settings.semantic.build')!.click())
  index.passages = 20; index.indexedDocuments = readyDocuments; index.embeddedPassages = 15; index.reusedPassages = 5
  await act(() => semantic.refresh())
  await act(() => button('settings.semantic.stop')!.click())
  expect(semantic.busy).toBe(true)
  expect(button('settings.semantic.stop')?.disabled).toBe(true)
  expect(container.querySelector('.search-library-index')?.textContent).not.toContain('settings.semantic.activity.cancelled')
  await act(() => { index.running = false; index.error = outcome; rejectBuild(outcome) })
  expect(semantic.busy).toBe(false)
  expect(semantic.index?.passages).toBe(20)
  expect(semantic.index?.indexedDocuments).toBe(readyDocuments)
  expect(semanticSearchStatus(semantic)).toBe(expected)
  const panel = container.querySelector('.search-library-index')!
  if (expected === 'fallback') {
    expect(semantic.error).toBe(outcome)
    expect(semantic.activityNotice).toBe('failed')
    expect(container.querySelector('[role="alert"]')?.textContent).toBe(outcome)
    expect(container.textContent).not.toContain('settings.semantic.activity.cancelled')
  } else {
    expect(semantic.error).toBe('')
    expect(semantic.activityNotice).toBe('cancelled')
    expect(container.querySelector('[role="alert"]')).toBeNull()
    expect([...panel.querySelectorAll('[role="status"]')].filter(item => item.textContent === 'settings.semantic.activity.cancelled')).toHaveLength(1)
    expect(container.textContent).not.toContain('settings.semantic.summary.fallback')
    expect(button('settings.semantic.update')?.disabled).toBe(expected === 'needsIndex' ? false : undefined)
    expect(panel.querySelector('.search-index-attention') !== null).toBe(expected === 'needsIndex')
    await act(() => semantic.refresh())
    expect(semanticSearchStatus(semantic)).toBe(expected)
  }
  expect(vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === 'search_semantic_index_build')).toHaveLength(1)
  if (expected === 'needsIndex') {
    await act(() => button('settings.semantic.update')!.click())
    expect(vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === 'search_semantic_index_build')).toHaveLength(2)
    await act(() => { index.running = false; index.indexedDocuments = 2; resolveBuild() })
    expect(semanticSearchStatus(semantic)).toBe('ready')
    expect(semantic.error).toBe('')
    expect(container.textContent).not.toContain('settings.semantic.activity.cancelled')
  }
})


it('keeps the switch name and row interaction stable across coverage, native work and status recovery', async () => {
  assertToggleRow('off')
  const titleId = toggle().getAttribute('aria-labelledby')
  await act(() => container.querySelector<HTMLElement>('.search-semantic-setup label [role="status"]')!.click())
  assertToggleRow('loading')
  expect(toggle().checked).toBe(true)
  expect(toggle().disabled).toBe(false)
  await act(() => { provider = { ...provider, activeFormat: 'float32' }; resolveActivation() })
  assertToggleRow('ready')
  const normally = vi.mocked(invoke).getMockImplementation()!
  const index = { ...semantic.index!, indexedDocuments: 1 }
  let statusUnavailable = false
  vi.mocked(invoke).mockImplementation(async (command, args, options) => {
    if (command === 'search_semantic_index_status') {
      if (statusUnavailable) throw new Error('Index status unavailable')
      return { ...index }
    }
    return normally(command, args, options)
  })
  await act(() => semantic.refresh())
  assertToggleRow('needsIndex')
  index.running = true
  await act(() => semantic.refresh())
  assertToggleRow('indexing')
  index.cancelRequested = true
  await act(() => semantic.refresh())
  assertToggleRow('cancelling')
  index.running = false; index.cancelRequested = false
  statusUnavailable = true
  await act(() => semantic.refresh())
  assertToggleRow('statusUnavailable')
  expect(container.querySelector('[role="alert"]')?.textContent).toContain('Index status unavailable')
  statusUnavailable = false; index.indexedDocuments = 2
  await act(() => button('settings.semantic.retryStatus')!.click())
  assertToggleRow('ready')
  expect(container.querySelector('[role="alert"]')).toBeNull()
  expect(toggle().getAttribute('aria-labelledby')).toBe(titleId)
  await act(() => container.querySelector<HTMLElement>('.search-semantic-setup label [role="status"]')!.click())
  assertToggleRow('off')
  expect(toggle().checked).toBe(false)
  expect(vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === 'search_embedding_configure')).toHaveLength(2)
  expect(vi.mocked(invoke).mock.calls.some(([cmd]) => ['search_embedding_download', 'search_semantic_index_build'].includes(cmd))).toBe(false)
})

it('keeps the missing-model warning tied to the stable switch and blocks status-label clicks without consent or files', async () => {
  filesPresent = false; storedFiles = false
  await reopen()
  assertToggleRow('off')
  const warning = container.querySelector<HTMLElement>('.search-semantic-setup .search-model-attention')!
  expect(warning.textContent).toBe('settings.semantic.downloadFirst')
  expect(toggle().getAttribute('aria-describedby')?.split(' ')).toContain(warning.id)
  expect(toggle().disabled).toBe(true)
  await act(() => container.querySelector<HTMLElement>('.search-semantic-setup label [role="status"]')!.click())
  expect(toggle().checked).toBe(false)
  expect(vi.mocked(invoke).mock.calls.some(([cmd]) => ['search_embedding_configure', 'search_embedding_download', 'search_semantic_index_build'].includes(cmd))).toBe(false)
})
