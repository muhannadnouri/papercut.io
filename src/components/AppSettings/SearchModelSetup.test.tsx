import { Children, isValidElement, type ReactElement, type ReactNode } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'

const mocked = vi.hoisted(() => ({
  invoke: vi.fn(), listen: vi.fn(), stop: vi.fn(), cursor: 0,
  slots: [] as unknown[], effects: [] as (() => void)[],
}))

// Reuse the repo's hook-mocking approach without adding a DOM/test dependency.
vi.mock('react', async (original) => {
  const actual = await original<typeof import('react')>()
  function stable<T>(value: T, deps: unknown[]): T {
    const index = mocked.cursor++
    const previous = mocked.slots[index] as { deps: unknown[]; value: T } | undefined
    if (!previous || deps.some((dep, i) => !Object.is(dep, previous.deps[i]))) {
      mocked.slots[index] = { value, deps }
    }
    return (mocked.slots[index] as { value: T }).value
  }
  return {
    ...actual,
    useState: (initial: unknown) => {
      const index = mocked.cursor++
      if (index >= mocked.slots.length) mocked.slots[index] = initial
      return [mocked.slots[index], (next: unknown) => {
        mocked.slots[index] = typeof next === 'function' ? next(mocked.slots[index]) : next
      }]
    },
    useRef: (initial: unknown) => stable({ current: initial }, []),
    useId: () => 'search-model-test',
    useCallback: stable,
    useEffect: (effect: () => void | (() => void), deps: unknown[]) => {
      const index = mocked.cursor++
      const previous = mocked.slots[index] as { deps: unknown[]; cleanup?: () => void } | undefined
      if (!previous || deps.some((dep, i) => !Object.is(dep, previous.deps[i]))) {
        mocked.effects.push(() => {
          previous?.cleanup?.()
          mocked.slots[index] = { deps, cleanup: effect() }
        })
      }
    },
  }
})
vi.mock('@tauri-apps/api/core', () => ({ invoke: mocked.invoke }))
vi.mock('@tauri-apps/api/event', () => ({ listen: mocked.listen }))
vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: (key: string) => key }) }))
vi.mock('../../i18n', () => ({ currentAppLocale: () => 'en' }))

import { SearchModelSetup } from './SearchModelSetup'
import type { SemanticSearchController } from '../../search-semantic/useSemanticSearch'

let status: { compiled: boolean; platform: string; activeFormat: string | null; loadingFormat: string | null; downloadingFormat: string | null }
let present: boolean
let storedFiles: boolean
let tree: ReactElement
let semantic: SemanticSearchController | undefined
function render() {
  mocked.cursor = 0
  tree = SearchModelSetup({ semantic })
  for (const effect of mocked.effects.splice(0)) effect()
  return renderToStaticMarkup(tree)
}
function controls(node: ReactNode = tree): ReactElement<Record<string, unknown>>[] {
  return Children.toArray(node).flatMap((child) => {
    if (!isValidElement<Record<string, unknown>>(child)) return []
    return [child, ...controls((child.props.children ?? null) as ReactNode), ...controls((child.props.indexRemoval ?? null) as ReactNode)]
  })
}
function control(type: string, text?: string) {
  const element = controls().find((item) => item.type === type && (type !== 'select' || item.props['aria-label'] === 'settings.searchModel.format') && (!text || item.props.children === text))
  expect(element, `${type} ${text ?? ''}`).toBeDefined()
  return element!
}
function click(key: string) {
  const button = control('button', key)
  expect(button.props.disabled).not.toBe(true)
  ;(button.props.onClick as (event: unknown) => void)({ currentTarget: { closest: () => null } })
}
function consent() {
  ;(control('input').props.onChange as (event: unknown) => void)({ target: { checked: true } })
}
async function settle() {
  for (let i = 0; i < 20; i++) await Promise.resolve()
  return render()
}
function unmount() {
  for (const slot of mocked.slots) (slot as { cleanup?: () => void } | null)?.cleanup?.()
}
function deferred<T>() {
  let resolve!: (value: T) => void
  let reject!: (error: unknown) => void
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no })
  return { promise, resolve, reject }
}

beforeEach(() => {
  semantic = undefined
  vi.useFakeTimers()
  vi.stubGlobal('window', { setInterval, clearInterval })
  mocked.cursor = 0
  mocked.slots = []
  mocked.effects = []
  mocked.stop.mockReset()
  mocked.listen.mockReset().mockResolvedValue(mocked.stop)
  status = { compiled: true, platform: 'linux', activeFormat: null, loadingFormat: null, downloadingFormat: null }
  present = false; storedFiles = false
  mocked.invoke.mockReset().mockImplementation(async (command: string) => {
    if (command === 'search_embedding_status') return { ...status }
    if (command === 'search_embedding_cache_status') return [
      { format: 'float32', expectedBytes: 487352505, filesPresent: present, hasStoredFiles: present || storedFiles },
      { format: 'dynamic-int8', expectedBytes: 135430819, filesPresent: present, hasStoredFiles: present || storedFiles },
      { format: 'base-float32', expectedBytes: 1127143136, filesPresent: false, hasStoredFiles: false },
      { format: 'large-float32', expectedBytes: 2252993226, filesPresent: false, hasStoredFiles: false },
    ]
    if (command === 'search_embedding_remove') { present = false; storedFiles = false }
    return { ...status }
  })
})
afterEach(() => {
  unmount()
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

it('selects Base and Large independently of precision without implicit work or shared consent', async () => {
  render(); await settle()
  consent(); render()
  for (const format of ['base-float32', 'large-float32']) {
    const select = controls().find((item) => item.type === 'select' && item.props['aria-label'] === 'settings.searchModel.model')!
    expect(select.props.disabled).toBe(false)
    ;(select.props.onChange as (event: unknown) => void)({ target: { value: format } })
    const html = render()
    expect(html).toContain('settings.searchModel.largerWarning')
    expect(html).toContain('settings.searchModel.switchHint')
    expect(control('select').props.disabled).toBe(true)
    expect(control('select').props.value).toBe(format)
    expect(control('input').props.checked).toBe(false)
    expect(control('button', 'settings.searchModel.download').props.disabled).toBe(true)
    expect(mocked.invoke.mock.calls.some(([cmd]) => cmd === 'search_embedding_configure' || cmd === 'search_semantic_index_build')).toBe(false)
    consent(); render(); click('settings.searchModel.download'); await settle()
  }
  expect(mocked.invoke.mock.calls.filter(([cmd]) => cmd === 'search_embedding_download')).toEqual([
    ['search_embedding_download', { format: 'base-float32' }],
    ['search_embedding_download', { format: 'large-float32' }],
  ])
})

it('keeps normal builds unavailable and never starts downloads or activates on opening', async () => {
  status.compiled = false
  render()
  const html = await settle()
  expect(html).toContain('settings.searchModel.unavailable')
  expect(html).toContain('settings.semantic.description')
  expect(html).not.toContain('<select')
  expect(mocked.invoke.mock.calls.map(([command]) => command)).toEqual(['search_embedding_status'])
  expect(mocked.stop).not.toHaveBeenCalled()
})

it('offers turning off a saved opt-in even when the current build has no provider', async () => {
  status.compiled = false
  semantic = { enabled: true, format: 'float32', busy: false, operation: null, error: '', cancelling: false,
    provider: { compiled: false, activeFormat: null, loadingFormat: null, downloadingFormat: null },
    index: undefined, setEnabled: vi.fn(), setFormat: vi.fn(), build: vi.fn(), remove: vi.fn(),
    cancel: vi.fn(), refresh: vi.fn(), activityNotice: null, dismissActivity: vi.fn() }
  render()
  const html = await settle()
  expect(html).toContain('settings.semantic.enabled')
  expect(html).toMatch(/role="switch"[^>]*checked=""/)
  expect(html).not.toMatch(/role="switch"[^>]*disabled=""/)
  expect(new Set(mocked.invoke.mock.calls.map(([command]) => command))).toEqual(new Set(['search_embedding_status']))
})

it('requires fresh consent after changing format and keeps cancellation busy through cleanup', async () => {
  render()
  await settle()
  expect(control('button', 'settings.searchModel.download').props.disabled).toBe(true)
  consent()
  render()
  ;(control('select').props.onChange as (event: unknown) => void)({ target: { value: 'dynamic-int8' } })
  expect(render()).toContain('settings.searchModel.int8Warning')
  expect(control('button', 'settings.searchModel.download').props.disabled).toBe(true)
  consent()
  render()
  const download = deferred<unknown>()
  mocked.invoke.mockImplementationOnce((command: string, args: unknown) => {
    expect(command).toBe('search_embedding_download')
    expect(args).toEqual({ format: 'dynamic-int8' })
    status.downloadingFormat = 'dynamic-int8'
    return download.promise
  })
  click('settings.searchModel.download')
  // A second click before React renders must not enqueue a second native job.
  ;(control('button', 'settings.searchModel.download').props.onClick as () => void)()
  render()
  await vi.advanceTimersByTimeAsync(1000)
  await settle()
  const onEvent = mocked.listen.mock.calls[0][1]
  onEvent({ payload: { format: 'dynamic-int8', phase: 'downloading', downloadedBytes: 50, totalBytes: 100 } })
  expect(render()).toContain('value="50"')
  click('common.cancel')
  await settle()
  expect(control('button', 'settings.searchModel.remove').props.disabled).toBe(true)
  expect(control('button', 'common.cancel').props.disabled).toBe(true)
  status.downloadingFormat = null
  storedFiles = true // Partial cache cleanup stays available after cancellation.
  download.reject('Embedding download cancelled')
  expect(await settle()).toContain('role="alert"')
  expect(control('button', 'settings.searchModel.remove').props.disabled).toBe(false)
  expect(control('input').props.checked).toBe(false)
  expect(mocked.invoke.mock.calls.some(([command]) => command === 'search_embedding_configure')).toBe(false)
  click('settings.searchModel.remove')
  render()
  click('common.confirm')
  expect(await settle()).toContain('settings.searchModel.removed')
})

it('reconnects to pending work, ignores unrelated progress and protects an active cache', async () => {
  render()
  await settle()
  status.downloadingFormat = 'dynamic-int8'
  mocked.listen.mock.calls[0][1]({ payload: { format: 'dynamic-int8', phase: 'starting', downloadedBytes: 0, totalBytes: 100 } })
  await settle()
  expect(control('select').props.disabled).toBe(true)
  expect(control('button', 'common.cancel').props.disabled).toBe(false)
  mocked.listen.mock.calls[0][1]({ payload: { format: 'float32', phase: 'installed', downloadedBytes: 100, totalBytes: 100 } })
  expect(render()).not.toContain('value="100"')
  expect(render()).not.toContain('settings.searchModel.verified')
  present = true
  status.downloadingFormat = null
  status.activeFormat = 'float32'
  await vi.advanceTimersByTimeAsync(1000)
  const html = await settle()
  expect(html).toContain('settings.searchModel.loaded')
  expect(html).not.toContain('settings.searchModel.present')
  expect(html).toContain('settings.searchModel.inUse')
  expect(control('button', 'settings.searchModel.remove').props.disabled).toBe(true)
  expect(html).not.toContain('settings.searchModel.verify')
  expect(html).not.toContain('settings.searchModel.consent')
  expect(control('select').props.disabled).toBe(true)
  status.activeFormat = null
  status.downloadingFormat = 'dynamic-int8'
  // Closing and reopening recovers native state, not a local persisted opt-in.
  unmount()
  mocked.slots = []
  render()
  await settle()
  click('common.cancel')
  await settle()
  status.downloadingFormat = null
  mocked.listen.mock.calls.at(-1)![1]({ payload: { format: 'dynamic-int8', phase: 'cancelled', downloadedBytes: 50, totalBytes: 100 } })
  await vi.advanceTimersByTimeAsync(1000)
  expect(await settle()).toContain('settings.searchModel.cancelled')
  expect(render()).not.toContain('settings.searchModel.cancelling')
})

it('checks an existing cache explicitly and recovers a download failure without activation', async () => {
  present = true
  render()
  await settle()
  consent()
  render()
  mocked.invoke.mockRejectedValueOnce('Checksum mismatch')
  click('settings.searchModel.verify')
  expect(await settle()).toContain('Checksum mismatch')
  expect(control('input').props.checked).toBe(false)
  consent()
  render()
  click('settings.searchModel.verify')
  const html = await settle()
  expect(html).toContain('settings.searchModel.verified')
  expect(html).not.toContain('Checksum mismatch')
  expect(mocked.invoke.mock.calls.filter(([command]) => command === 'search_embedding_download')).toEqual([
    ['search_embedding_download', { format: 'float32' }],
    ['search_embedding_download', { format: 'float32' }],
  ])
  expect(mocked.invoke.mock.calls.some(([command]) => command === 'search_embedding_configure')).toBe(false)
})

it('recovers failed subscriptions and releases listeners even if they arrive after closing', async () => {
  mocked.listen.mockRejectedValueOnce('listener unavailable')
  render()
  expect(await settle()).toContain('listener unavailable')
  expect(mocked.invoke).not.toHaveBeenCalled()
  click('common.retry')
  render()
  await settle()
  expect(control('button', 'settings.searchModel.download').props.disabled).toBe(true)
  unmount()
  expect(mocked.stop).toHaveBeenCalledTimes(1)
  mocked.slots = []
  const registration = deferred<() => void>()
  mocked.listen.mockReturnValueOnce(registration.promise)
  render()
  unmount()
  registration.resolve(mocked.stop)
  await settle()
  expect(mocked.stop).toHaveBeenCalledTimes(2)
})


it('requires confirmation for cleanup, cancellation preserves files, and format changes retire confirmation', async () => {
  storedFiles = true
  render(); await settle()
  click('settings.searchModel.remove'); render()
  expect(mocked.invoke.mock.calls.some(([cmd]) => cmd === 'search_embedding_remove')).toBe(false)
  expect(render()).toContain('settings.searchModel.confirmRemove')
  click('common.cancel'); render()
  expect(render()).not.toContain('settings.searchModel.confirmRemove')
  click('settings.searchModel.remove'); render()
  ;(control('select').props.onChange as (event: unknown) => void)({ target: { value: 'dynamic-int8' } })
  expect(render()).not.toContain('settings.searchModel.confirmRemove')
  click('settings.searchModel.remove'); render()
  click('common.confirm'); await settle()
  expect(mocked.invoke.mock.calls.filter(([cmd]) => cmd === 'search_embedding_remove')).toEqual([
    ['search_embedding_remove', { format: 'dynamic-int8' }],
  ])
})

it('keeps index removal behind confirmation and disables it for an empty index', async () => {
  semantic = { enabled: false, format: 'float32', busy: false, operation: null, error: '', cancelling: false,
    provider: { compiled: true, activeFormat: null, loadingFormat: null, downloadingFormat: null },
    index: { running: false, cancelRequested: false, indexedDocuments: 1, eligibleDocuments: 2,
      passages: 4, hasStoredIndex: true, embeddedPassages: 0, reusedPassages: 0, error: null },
    setEnabled: vi.fn(), setFormat: vi.fn(), build: vi.fn(), remove: vi.fn(), cancel: vi.fn(), refresh: vi.fn(), activityNotice: null, dismissActivity: vi.fn() }
  render(); await settle()
  const markup = render()
  expect(markup.indexOf('settings.semantic.controlsTitle')).toBeLessThan(markup.indexOf('settings.searchModel.title'))
  expect(markup.indexOf('settings.searchModel.title')).toBeLessThan(markup.indexOf('settings.semantic.libraryTitle'))
  expect(markup).not.toContain('settings.searchModel.closeHelp')
  expect(markup).not.toContain('settings.semantic.closeHelp')
  click('settings.semantic.remove'); render()
  expect(semantic.remove).not.toHaveBeenCalled()
  click('common.cancel'); render()
  expect(semantic.remove).not.toHaveBeenCalled()
  click('settings.semantic.remove'); render(); click('common.confirm')
  expect(semantic.remove).toHaveBeenCalledTimes(1)
  semantic.index!.passages = 0; semantic.index!.hasStoredIndex = false; render()
  expect(control('button', 'settings.semantic.remove').props.disabled).toBe(true)
})

it('keeps background guidance with active downloads or indexing without starting either on open', async () => {
  status.downloadingFormat = 'float32'
  render(); await settle()
  expect(render()).toContain('settings.searchModel.closeHelp')
  expect(render()).not.toContain('settings.semantic.closeHelp')
  expect(mocked.invoke.mock.calls.some(([cmd]) => cmd === 'search_embedding_download')).toBe(false)
  status.downloadingFormat = null
  semantic = { enabled: true, format: 'float32', busy: true, operation: 'build', error: '', cancelling: false,
    provider: { compiled: true, activeFormat: 'float32', loadingFormat: null, downloadingFormat: null },
    index: { running: true, cancelRequested: false, indexedDocuments: 1, eligibleDocuments: 2,
      passages: 4, hasStoredIndex: true, embeddedPassages: 0, reusedPassages: 0, error: null },
    setEnabled: vi.fn(), setFormat: vi.fn(), build: vi.fn(), remove: vi.fn(), cancel: vi.fn(), refresh: vi.fn(), activityNotice: null, dismissActivity: vi.fn() }
  await vi.advanceTimersByTimeAsync(1000); await settle()
  expect(render()).toContain('settings.semantic.closeHelp')
  expect(render()).not.toContain('settings.searchModel.closeHelp')
  expect(controls().find(node => node.type === 'details' && node.props.className === 'search-model-panel search-model-management')!.props.open).toBe(false)
  expect(semantic.build).not.toHaveBeenCalled()
})

it('collapses a loaded model, keeps file presence distinct from verification and restores preparation after disable', async () => {
  present = true
  render(); await settle()
  expect(render()).toContain('settings.searchModel.present')
  expect(render()).not.toContain('settings.searchModel.loaded')
  const downloadCalls = () => mocked.invoke.mock.calls.filter(([cmd]) => cmd === 'search_embedding_download')
  expect(downloadCalls()).toHaveLength(0)
  consent(); render(); click('settings.searchModel.verify')
  expect(await settle()).toContain('settings.searchModel.verified')
  expect(render()).not.toContain('settings.searchModel.present')
  status.activeFormat = 'float32'
  mocked.listen.mock.calls[0][1]({ payload: { phase: 'installed', format: 'float32' } })
  const loaded = await settle()
  expect(controls().find(node => node.type === 'details' && node.props.className === 'search-model-panel search-model-management')!.props.open).toBe(false)
  expect(loaded).toContain('settings.searchModel.loaded')
  expect(loaded).not.toContain('settings.searchModel.consent')
  status.activeFormat = null
  mocked.listen.mock.calls[0][1]({ payload: { phase: 'installed', format: 'float32' } })
  expect(await settle()).toContain('settings.searchModel.consent')
  expect(control('button', 'settings.searchModel.verify').props.disabled).toBe(true)
  expect(downloadCalls()).toHaveLength(1)
})


it('blocks repeated removal after cleanup, but permits incomplete-file cleanup', async () => {
  storedFiles = true
  render(); await settle()
  expect(render()).toContain('settings.searchModel.incomplete')
  click('settings.searchModel.remove'); render(); click('common.confirm'); await settle()
  expect(control('button', 'settings.searchModel.remove').props.disabled).toBe(true)
  ;(control('button', 'settings.searchModel.remove').props.onClick as () => void)()
  expect(render()).not.toContain('settings.searchModel.confirmRemove')
  expect(mocked.invoke.mock.calls.filter(([cmd]) => cmd === 'search_embedding_remove')).toHaveLength(1)
  expect(render()).toContain('settings.searchModel.absent')
})

it('collapses installed files while off, keeps model removal in its section, and omits separate storage', async () => {
  present = true
  render(); await settle()
  const model = controls().find(node => node.type === 'details' && node.props.className === 'search-model-panel search-model-management')!
  expect(model.props.open).toBe(false)
  expect(controls(model.props.children as ReactNode).some(node => node.props.children === 'settings.searchModel.remove')).toBe(true)
  expect(render()).not.toContain('settings.searchModel.storage')
})
