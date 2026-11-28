// @vitest-environment jsdom
import {
  act,
  useState,
} from 'react'
import {
  createRoot,
  type Root,
} from 'react-dom/client'
import {
  afterEach,
  beforeEach,
  expect,
  it,
  vi,
} from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import { AppSettings } from './AppSettings'
import type { SemanticSearchController } from '../../search-semantic/useSemanticSearch'
import { openExternalUrl } from '../../utils/openExternalUrl'

vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: (key: string) => key }) }))
vi.mock('../../i18n', () => ({
  APP_LOCALE_OPTIONS: [{
    value: 'en',
    label: 'English',
  }],
  currentAppLocale: () => 'en',
  changeAppLocale: vi.fn(),
}))
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
  isTauri: () => true,
}))
vi.mock('@tauri-apps/api/app', () => ({ getVersion: async () => '1.9.2' }))
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }))
vi.mock('@tauri-apps/api/webview', () => ({ getCurrentWebview: () => ({ setZoom: async () => {} }) }))
vi.mock('../../utils/openExternalUrl', () => ({ openExternalUrl: vi.fn() }))

let root: Root
let container: HTMLDivElement
let semantic: SemanticSearchController
function Harness({ completed = false }: { completed?: boolean }) {
  const [searchOpen, setSearchOpen] = useState(false)
  const [developerMode, setDeveloperMode] = useState(false)
  return <>
    <input aria-label="Query" defaultValue="draft query" />
    {!completed && <button onClick={() => setSearchOpen(true)}>Search Settings</button>}
    <AppSettings eraseBlockedBy={null} searchOpen={searchOpen} onSearchOpenChange={setSearchOpen} semanticSearch={semantic}
      themeChoice="light" onThemeChange={vi.fn()} developerMode={developerMode} onDeveloperModeChange={setDeveloperMode} onLibraryImported={vi.fn()} />
  </>
}

beforeEach(async () => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true)
  semantic = {
    enabled: true,
    format: 'float32',
    busy: false,
    operation: null,
    cancelling: false,
    error: '',
    statusError: '',
    downloadFailure: null,
    provider: {
      compiled: true,
      activeFormat: 'float32',
      loadingFormat: null,
      downloadingFormat: null,
    },
    index: {
      running: false,
      cancelRequested: false,
      indexedDocuments: 0,
      eligibleDocuments: 2,
      passages: 0,
      hasStoredIndex: false,
      embeddedPassages: 0,
      reusedPassages: 0,
      error: null,
    },
    setEnabled: vi.fn(),
    setFormat: vi.fn(),
    build: vi.fn(),
    remove: vi.fn(),
    cancel: vi.fn(),
    refresh: vi.fn(),
    activityNotice: null,
    dismissActivity: vi.fn(),
  }
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === 'search_embedding_status') {
      return semantic.provider
    }
    if (command === 'search_embedding_cache_status') {
      return [{
        format: 'float32',
        filesPresent: true,
        hasStoredFiles: true,
        expectedBytes: 465000000,
      }]
    }
    throw new Error(`Unexpected work while opening Settings: ${command}`)
  })
  // jsdom has no layout; keep real DOM focus/events and make visible controls focusable.
  vi.spyOn(HTMLElement.prototype, 'getClientRects').mockImplementation(function (this: HTMLElement) {
    return (this.closest('[hidden]') ? [] : [new DOMRect(0, 0, 44, 44)]) as unknown as DOMRectList
  })
  container = document.createElement('div')
  document.body.append(container)
  root = createRoot(container)
  await act(() => root.render(<Harness />))
})
afterEach(async () => {
  await act(() => root.unmount())
  container.remove()
  vi.restoreAllMocks()
  vi.clearAllMocks()
  vi.unstubAllGlobals()
})
function opener() {
  return [...container.querySelectorAll<HTMLButtonElement>('button')].find(button => button.textContent === 'Search Settings')!
}
async function click(element: HTMLElement) {
  await act(() => element.click())
}
async function openSearch() {
  await act(() => opener().focus())
  await click(opener())
}
function assertNoPreparationStarted() {
  expect(semantic.build).not.toHaveBeenCalled()
  expect(semantic.setEnabled).not.toHaveBeenCalled()
  expect(vi.mocked(invoke).mock.calls.every(([command]) => ['search_embedding_status', 'search_embedding_cache_status'].includes(command))).toBe(true)
}

it('opens the public privacy policy from Privacy & Legal using the native opener', async () => {
  await click(container.querySelector('.app-settings-btn')!)
  const about = container.querySelector('.app-settings-about')!
  expect(about.textContent).not.toContain('settings.privacyPolicy')
  await click(about.querySelector('button')!)
  const notices = container.querySelector('[aria-label="settings.privacyAndLegal"]')!
  const policy = notices.querySelector<HTMLAnchorElement>('a.app-settings-legal-notice-link')!
  expect(policy.textContent).toBe('settings.privacyPolicy')
  expect(policy.href).toBe('https://trypapercut.app/privacy/')
  await click(policy)
  expect(openExternalUrl).toHaveBeenCalledWith(policy.href)
})

it('opens search from its named status row without starting work and refreshes the description on Back', async () => {
  await act(() => container.querySelector<HTMLButtonElement>('.app-settings-btn')!.focus())
  await click(container.querySelector('.app-settings-btn')!)
  const destination = container.querySelector<HTMLButtonElement>('.app-settings-search-action')!
  expect(document.getElementById(destination.getAttribute('aria-labelledby')!)?.textContent).toBe('settings.semantic.title')
  expect(document.getElementById(destination.getAttribute('aria-describedby')!)?.textContent).toBe('settings.semantic.summary.needsIndex')
  expect(container.textContent).not.toContain('settings.semantic.manage')
  await click(destination)
  expect(container.querySelector('.app-settings-search-dialog')).not.toBeNull()
  assertNoPreparationStarted()
  semantic.index!.indexedDocuments = semantic.index!.eligibleDocuments
  await click(container.querySelector('[aria-label="common.back"]')!)
  expect(container.querySelector('#app-settings-search-status')?.textContent).toBe('settings.semantic.summary.ready')
  await click(container.querySelector('[aria-label="common.close"]')!)
  expect(document.activeElement).toBe(container.querySelector('.app-settings-btn'))
})

it('keeps related destinations together and the developer label directly operable', async () => {
  await click(container.querySelector('.app-settings-btn')!)
  const data = container.querySelector('.app-settings-data')!
  expect([...data.querySelectorAll('button')].map(button => button.textContent)).toEqual([
    'settings.transferLibrary',
    'settings.erase.open',
  ])
  expect(data.querySelector('.app-settings-erase-action')).not.toBeNull()
  const about = container.querySelector('.app-settings-about')!
  expect(about.textContent).toContain('settings.privacyAndLegal')
  expect(about.querySelector('.app-settings-version')?.textContent).toBe('settings.version1.9.2')
  expect(container.querySelector('.app-settings-developer h3')).toBeNull()
  const developer = container.querySelector<HTMLLabelElement>('.app-setting-toggle')!
  expect(developer.textContent).toBe('settings.developerMode')
  await click(developer)
  expect(developer.querySelector('input')?.checked).toBe(true)
  expect(container.querySelector('details')).toBeNull()
  expect(vi.mocked(invoke)).not.toHaveBeenCalled()
})

it('opens search setup directly, returns to root Settings on Back, then restores the original opener on Close', async () => {
  await openSearch()
  expect(container.querySelector('.app-settings-search-dialog h2')?.textContent).toBe('settings.semantic.title')
  expect(container.querySelector('.search-library-index')?.textContent).toContain('settings.semantic.build')
  expect(document.activeElement?.getAttribute('aria-label')).toBe('common.back')
  await click(container.querySelector('[aria-label="common.back"]')!)
  expect(container.querySelector('.app-settings-search-dialog')).toBeNull()
  expect(container.querySelector('[role="dialog"] h2')?.textContent).toBe('settings.title')
  await click(container.querySelector('[aria-label="common.close"]')!)
  expect(container.querySelector('[role="dialog"]')).toBeNull()
  expect(document.activeElement).toBe(opener())
  expect(container.querySelector<HTMLInputElement>('[aria-label="Query"]')?.value).toBe('draft query')
  assertNoPreparationStarted()
})

it.each(['close', 'escape', 'backdrop'])('closes a directly opened search dialog through %s without opening root or starting work', async (method) => {
  await openSearch()
  if (method === 'close') {
    await click(container.querySelector('[aria-label="common.close"]')!)
  } else if (method === 'backdrop') {
    await click(container.querySelector('.app-dialog-backdrop')!)
  } else {
    await act(() => document.activeElement!.dispatchEvent(new KeyboardEvent('keydown', {
      key: 'Escape',
      bubbles: true,
      cancelable: true,
    })))
  }
  expect(container.querySelector('[role="dialog"]')).toBeNull()
  expect(document.activeElement).toBe(opener())
  expect(container.querySelector<HTMLInputElement>('[aria-label="Query"]')?.value).toBe('draft query')
  assertNoPreparationStarted()
})

it('restores focus to the Settings cog when completing setup removes the original warning action', async () => {
  await openSearch()
  semantic.index!.indexedDocuments = semantic.index!.eligibleDocuments
  await act(() => root.render(<Harness completed />))
  expect(opener()).toBeUndefined()
  await click(container.querySelector('[aria-label="common.close"]')!)
  expect(container.querySelector('[role="dialog"]')).toBeNull()
  expect(document.activeElement).toBe(container.querySelector('.app-settings-btn'))
  assertNoPreparationStarted()
})

it('opens the erase view without starting anything, and Back returns to root Settings', async () => {
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === 'app_reset_summary') {
      return {
        documents: 1,
        audiobooks: 0,
        totalBytes: 10,
        locations: [],
        blockedBy: null,
      }
    }
    return semantic.provider
  })
  await click(container.querySelector('.app-settings-btn')!)
  await click(container.querySelector('.app-settings-erase-action')!)
  expect(container.querySelector('.app-reset-dialog h2')?.textContent).toBe('settings.erase.title')
  expect(vi.mocked(invoke).mock.calls.some(([command]) => command === 'app_reset_request')).toBe(false)
  await click(container.querySelector('[aria-label="common.back"]')!)
  expect(container.querySelector('.app-reset-dialog')).toBeNull()
  expect(container.querySelector('[role="dialog"] h2')?.textContent).toBe('settings.title')
})
