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
    return [child, ...controls((child.props.children ?? null) as ReactNode)]
  })
}
function control(type: string, text?: string) {
  const element = controls().find((item) => item.type === type && (!text || item.props.children === text))
  expect(element, `${type} ${text ?? ''}`).toBeDefined()
  return element!
}
function click(key: string) {
  const button = control('button', key)
  expect(button.props.disabled).not.toBe(true)
  ;(button.props.onClick as () => void)()
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
  present = false
  mocked.invoke.mockReset().mockImplementation(async (command: string) => {
    if (command === 'search_embedding_status') return { ...status }
    if (command === 'search_embedding_cache_status') return [
      { format: 'float32', expectedBytes: 487352505, filesPresent: present },
      { format: 'dynamic-int8', expectedBytes: 135430819, filesPresent: present },
    ]
    return { ...status }
  })
})
afterEach(() => {
  unmount()
  vi.useRealTimers()
  vi.unstubAllGlobals()
})

it('keeps normal builds unavailable and never starts downloads or activates on opening', async () => {
  status.compiled = false
  render()
  const html = await settle()
  expect(html).toContain('settings.searchModel.unavailable')
  expect(html).toContain('settings.searchModel.description')
  expect(html).not.toContain('<select')
  expect(mocked.invoke.mock.calls.map(([command]) => command)).toEqual(['search_embedding_status'])
  expect(mocked.stop).not.toHaveBeenCalled()
})

it('offers turning off a saved opt-in even when the current build has no provider', async () => {
  status.compiled = false
  semantic = { enabled: true, format: 'float32', busy: false, operation: null, error: '', cancelling: false,
    provider: { compiled: false, activeFormat: null, loadingFormat: null, downloadingFormat: null },
    index: undefined, setEnabled: vi.fn(), setFormat: vi.fn(), build: vi.fn(), remove: vi.fn(),
    cancel: vi.fn(), refresh: vi.fn() }
  render()
  const html = await settle()
  expect(html).toContain('settings.semantic.toggle')
  expect(html).toContain('role="switch" checked=""')
  expect(html).not.toContain('role="switch" disabled=""')
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
  download.reject('Embedding download cancelled')
  expect(await settle()).toContain('role="alert"')
  expect(control('button', 'settings.searchModel.remove').props.disabled).toBe(false)
  expect(control('input').props.checked).toBe(false)
  expect(mocked.invoke.mock.calls.some(([command]) => command === 'search_embedding_configure')).toBe(false)
  click('settings.searchModel.remove')
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
  expect(html).toContain('settings.searchModel.present')
  expect(html).toContain('settings.searchModel.inUse')
  expect(control('button', 'settings.searchModel.remove').props.disabled).toBe(true)
  expect(control('button', 'settings.searchModel.verify').props.disabled).toBe(true)
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
