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
beforeEach(() => { slots.values = []; slots.cursor = 0 })

it('keeps model/index controls out of general Settings and restores the summary on Back', () => {
  render()
  const settings = elements().find(node => node.props['aria-label'] === 'settings.button')!
  ;(settings.props.onClick as () => void)()
  expect(render()).toContain('settings.semantic.summaryOff')
  expect(render()).not.toContain('model-and-index-controls')
  const manage = elements().find(node => node.type === 'button' && node.props.children === 'settings.semantic.manage')!
  ;(manage.props.onClick as () => void)()
  expect(render()).toContain('model-and-index-controls')
  const dialog = elements().find(node => node.props.className === 'app-settings-search-dialog')!
  const actions = elements(dialog.props.actions as ReactNode)
  const back = actions.find(node => node.type === 'button' && node.props.children === 'common.back')!
  ;(back.props.onClick as () => void)()
  expect(render()).toContain('settings.semantic.summaryOff')
  expect(render()).not.toContain('model-and-index-controls')
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
