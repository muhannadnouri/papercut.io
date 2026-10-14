// @vitest-environment jsdom
import { act, useEffect } from 'react'
import { createRoot, type Root } from 'react-dom/client'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { SearchModelSetup } from './SearchModelSetup'
import { useSemanticSearch, type ProviderStatus, type SemanticSearchController } from '../../search-semantic/useSemanticSearch'
import { semanticSearchStatus } from '../../search-semantic/semanticSearchStatus'
import { invoke } from '@tauri-apps/api/core'

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
  provider = { compiled: true, activeFormat: null, loadingFormat: null, downloadingFormat: null }
  vi.mocked(invoke).mockImplementation(async (command, args) => {
    if (command === 'search_embedding_status') return { ...provider }
    if (command === 'search_embedding_cache_status') return [{ format: 'float32', filesPresent: true, hasStoredFiles: true, expectedBytes: 465000000 }]
    if (command === 'search_semantic_index_status') return { running: false, cancelRequested: false, indexedDocuments: 2,
      eligibleDocuments: 2, passages: 100, hasStoredIndex: true, embeddedPassages: 0, reusedPassages: 0, error: null }
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
  expect(toggle().hasAttribute('aria-describedby')).toBe(false)
  await act(() => {
    provider = { ...provider, activeFormat: 'float32' }
    resolveActivation()
  })
  expect(phase()).toBeNull()
  expect(container.querySelector('.search-model-management > summary')?.textContent).toContain('settings.searchModel.loaded')
  expect(container.querySelector('.search-semantic-setup [role="status"]')).toBeNull()
  expect(container.querySelector('.search-library-index')?.textContent).toContain('settings.semantic.indexReady')
  // Settings without the switch still need their compact readiness summary.
  expect(semanticSearchStatus(semantic)).toBe('ready')
})

it('replaces loading with the retained activation error and an enabled Retry action', async () => {
  await enable()
  await act(() => rejectActivation(new Error('Invalid model file')))
  expect(phase()).toBeNull()
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
