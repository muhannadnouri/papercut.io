import { Children, isValidElement, type ReactElement, type ReactNode } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { beforeEach, expect, it, vi } from 'vitest'
import type { SemanticSearchController } from '../../search-semantic/useSemanticSearch'

const slots = vi.hoisted(() => ({ values: [] as unknown[], cursor: 0 }))
vi.mock('react', async (original) => ({
  ...await original<typeof import('react')>(),
  useState: (initial: unknown) => {
    const i = slots.cursor++
    if (i >= slots.values.length) slots.values[i] = typeof initial === 'function' ? initial() : initial
    return [slots.values[i], (next: unknown) => { slots.values[i] = next }]
  },
  useCallback: (callback: unknown) => callback,
  useEffect: () => {},
}))
vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: (key: string) => key }) }))
vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => true }))
vi.mock('../../i18n', () => ({ APP_LOCALE_OPTIONS: [], currentAppLocale: () => 'en' }))
vi.mock('../../utils/platform', () => ({ isMobileUserAgent: () => true }))
vi.mock('../AppSelect/AppSelect', () => ({ AppSelect: () => null }))
vi.mock('../AppDialog/AppDialog', () => ({ AppDialog: ({ title, children }: { title: ReactNode; children: ReactNode }) => <div>{title}{children}</div> }))
vi.mock('./SearchModelSetup', () => ({ SearchModelSetup: () => <div>model-and-index-controls</div> }))
vi.mock('../../library-transfer/LibraryTransferDialog', () => ({ LibraryTransferDialog: () => null }))

import { AppSettings } from './AppSettings'
import { SearchActivity } from '../../search-semantic/SearchActivity'
let tree: ReactElement
const semantic = { enabled: false, busy: false } as SemanticSearchController
function render() {
  slots.cursor = 0
  tree = AppSettings({ themeChoice: 'light', onThemeChange: vi.fn(), developerMode: false,
    onDeveloperModeChange: vi.fn(), onLibraryImported: vi.fn(), semanticSearch: semantic })
  return renderToStaticMarkup(tree)
}
function elements(node: ReactNode = tree): ReactElement<Record<string, unknown>>[] {
  return Children.toArray(node).flatMap(child => isValidElement<Record<string, unknown>>(child)
    ? [child, ...elements((child.props.children ?? null) as ReactNode)] : [])
}
beforeEach(() => {
  slots.values = []; slots.cursor = 0
  semantic.busy = false; semantic.activityNotice = null; semantic.dismissActivity = vi.fn()
  semantic.enabled = false; semantic.operation = null; semantic.error = ''; semantic.provider = undefined; semantic.index = undefined
})

it('keeps model/index controls out of general Settings and restores the summary on Back', () => {
  render()
  const settings = elements().find(node => node.props['aria-label'] === 'settings.button')!
  ;(settings.props.onClick as () => void)()
  expect(render()).toContain('settings.semantic.summary.off')
  expect(render()).not.toContain('model-and-index-controls')
  const manage = elements().find(node => node.type === 'button' && node.props.children === 'settings.semantic.manage')!
  ;(manage.props.onClick as () => void)()
  expect(render()).toContain('model-and-index-controls')
  const dialog = elements().find(node => node.props.className === 'app-settings-search-dialog')!
  expect(dialog.props.showCloseButton).toBe(true)
  expect(dialog.props.actions).toBeUndefined()
  ;(dialog.props.onBack as () => void)()
  expect(render()).toContain('settings.semantic.summary.off')
  expect(render()).not.toContain('model-and-index-controls')
})

it('uses Close at root and Back one level at a time in legal Settings', () => {
  render()
  ;(elements().find(node => node.props['aria-label'] === 'settings.button')!.props.onClick as () => void)()
  render()
  const root = elements().find(node => node.props.title === 'settings.title')!
  expect(root.props.showCloseButton).toBe(true)
  expect(root.props.onBack).toBeUndefined()
  expect(root.props.actions).toBeUndefined()
  const openLegal = () => (elements().find(node => node.type === 'button' && node.props.children === 'settings.legalAndNotices')!.props.onClick as () => void)()
  openLegal(); render()
  ;(elements().find(node => node.props.className === 'app-settings-legal-notice-link')!.props.onClick as () => void)()
  render()
  const notice = elements().find(node => node.props.className === 'app-settings-legal-dialog')!
  expect(notice.props.showCloseButton).toBe(true)
  expect(notice.props.actions).toBeUndefined()
  ;(notice.props.onBack as () => void)()
  expect(render()).toContain('app-settings-legal-notice-link')
  const list = elements().find(node => node.props.className === 'app-settings-legal-dialog')!
  ;(list.props.onBack as () => void)()
  expect(render()).toContain('settings.title')
  openLegal(); render()
  ;(elements().find(node => node.props.className === 'app-settings-legal-notice-link')!.props.onClick as () => void)()
  render()
  ;(elements().find(node => node.props.className === 'app-settings-legal-dialog')!.props.onCancel as () => void)()
  expect(render()).not.toContain('settings.title')
  ;(elements().find(node => node.props['aria-label'] === 'settings.button')!.props.onClick as () => void)()
  expect(render()).toContain('settings.title')
  expect(render()).not.toContain('app-settings-legal-notice-link')
})

it('closes the search view through its existing dialog cancellation boundary', () => {
  render()
  const button = elements().find(node => node.props['aria-label'] === 'settings.button')!
  ;(button.props.onClick as () => void)(); render()
  const manage = elements().find(node => node.type === 'button' && node.props.children === 'settings.semantic.manage')!
  ;(manage.props.onClick as () => void)(); render()
  const dialog = elements().find(node => node.props.className === 'app-settings-search-dialog')!
  ;(dialog.props.onCancel as () => void)()
  expect(render()).not.toContain('model-and-index-controls')
  expect(render()).not.toContain('settings.title')
})

it('retains the Settings gear while activity opens the model/index controls directly', () => {
  semantic.busy = true; semantic.operation = 'build'
  expect(render()).toContain('aria-label="settings.button"')
  expect(render()).toContain('settings.semantic.activity.indexing')
  const activity = elements().find(node => node.type === SearchActivity)!
  ;(activity.props.onManage as () => void)()
  expect(render()).toContain('model-and-index-controls')
  expect(render()).not.toContain('settings.semantic.summary.off')
})

it('shows current readiness in general Settings rather than a generic enabled promise', () => {
  render()
  const settings = elements().find(node => node.props['aria-label'] === 'settings.button')!
  ;(settings.props.onClick as () => void)()
  semantic.enabled = true
  expect(render()).toContain('settings.semantic.summary.fallback')
  semantic.operation = 'activate'
  expect(render()).toContain('settings.semantic.summary.loading')
  semantic.operation = null
  semantic.provider = { compiled: true, activeFormat: 'float32', loadingFormat: null, downloadingFormat: null }
  semantic.format = 'float32'
  semantic.index = { running: false, cancelRequested: false, indexedDocuments: 1, eligibleDocuments: 2,
    passages: 3, hasStoredIndex: true, embeddedPassages: 0, reusedPassages: 0, error: null }
  expect(render()).toContain('settings.semantic.summary.needsIndex')
  semantic.index.indexedDocuments = 2
  expect(render()).toContain('settings.semantic.summary.ready')
})
