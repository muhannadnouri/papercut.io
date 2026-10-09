import { afterEach, beforeEach, expect, it, vi } from 'vitest'

const mocked = vi.hoisted(() => ({ invoke: vi.fn(), isTauri: vi.fn(), cursor: 0,
  slots: [] as unknown[], effects: [] as (() => void)[] }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: mocked.invoke, isTauri: mocked.isTauri }))
// The repository uses hook slots to exercise async behavior without a DOM dependency.
vi.mock('react', () => {
  function stable<T>(value: T, deps: unknown[]): T {
    const i = mocked.cursor++
    const previous = mocked.slots[i] as { deps: unknown[]; value: T } | undefined
    if (!previous || deps.some((dep, n) => !Object.is(dep, previous.deps[n]))) mocked.slots[i] = { value, deps }
    return (mocked.slots[i] as { value: T }).value
  }
  return {
    useState: (initial: unknown) => {
      const i = mocked.cursor++
      if (i >= mocked.slots.length) mocked.slots[i] = typeof initial === 'function' ? initial() : initial
      return [mocked.slots[i], (value: unknown) => { mocked.slots[i] = value }]
    },
    useRef: (initial: unknown) => stable({ current: initial }, []),
    useCallback: stable,
    useEffect: (effect: () => void | (() => void), deps: unknown[]) => {
      const i = mocked.cursor++
      const previous = mocked.slots[i] as { deps: unknown[]; cleanup?: () => void } | undefined
      if (!previous || deps.some((dep, n) => !Object.is(dep, previous.deps[n]))) mocked.effects.push(() => {
        previous?.cleanup?.()
        mocked.slots[i] = { deps, cleanup: effect(), effect }
      })
    },
  }
})

import { useSemanticSearch, type ModelFormat, type ProviderStatus } from './useSemanticSearch'
const key = 'papercut.semantic-search.v1'
let storage: Map<string, string>
let provider: ProviderStatus
let index: NonNullable<ReturnType<typeof useSemanticSearch>['index']>
function render() {
  mocked.cursor = 0
  // eslint-disable-next-line react-hooks/rules-of-hooks -- Explicit hook harness, not a React component.
  const value = useSemanticSearch()
  for (const effect of mocked.effects.splice(0)) effect()
  return value
}
async function settle() {
  for (let i = 0; i < 30; i++) await Promise.resolve()
  return render()
}
function deferred() {
  let resolve!: (value?: unknown) => void
  let reject!: (error: unknown) => void
  const promise = new Promise((yes, no) => { resolve = yes; reject = no })
  return { promise, resolve, reject }
}
function unmount() {
  for (const slot of mocked.slots) (slot as { cleanup?: () => void } | null)?.cleanup?.()
}
function restart() {
  unmount(); mocked.slots = []; mocked.effects = []
  return render()
}
function commands() { return mocked.invoke.mock.calls.map(([command]) => command) }

beforeEach(() => {
  vi.useFakeTimers()
  storage = new Map()
  vi.stubGlobal('window', { setInterval, clearInterval,
    localStorage: { getItem: (name: string) => storage.get(name) ?? null,
      setItem: (name: string, value: string) => storage.set(name, value) } })
  mocked.slots = []; mocked.effects = []; mocked.cursor = 0
  mocked.isTauri.mockReset().mockReturnValue(true)
  provider = { compiled: true, activeFormat: null, loadingFormat: null, downloadingFormat: null }
  index = { running: false, cancelRequested: false, indexedDocuments: 0, eligibleDocuments: 2,
    passages: 0, embeddedPassages: 0, reusedPassages: 0, error: null }
  mocked.invoke.mockReset().mockImplementation(async (command: string, args?: { format: ModelFormat | null }) => {
    if (command === 'search_embedding_status') return { ...provider }
    if (command === 'search_semantic_index_status') return { ...index }
    if (command === 'search_embedding_configure') { provider.activeFormat = args!.format; return { ...provider } }
  })
})
afterEach(() => { unmount(); vi.useRealTimers(); vi.unstubAllGlobals() })

it('starts off, reads status only, and never invokes native commands in a browser', async () => {
  render()
  let value = await settle()
  expect(value.enabled).toBe(false)
  expect(commands()).toEqual(['search_embedding_status', 'search_semantic_index_status'])
  await vi.advanceTimersByTimeAsync(4000)
  expect(commands()).toHaveLength(2)
  mocked.isTauri.mockReturnValue(false)
  mocked.invoke.mockClear()
  restart(); value = await settle()
  expect(value.enabled).toBe(false)
  expect(commands()).toEqual([])
})

it('persists format and opt-in, restores only cached activation, and stays off after disable/restart', async () => {
  render(); let value = await settle()
  value.setFormat('dynamic-int8'); value = render()
  const enable = value.setEnabled(true)
  // Double-click before React renders cannot issue a duplicate activation.
  await value.setEnabled(true)
  await enable; value = await settle()
  expect(mocked.invoke.mock.calls.filter(([cmd]) => cmd === 'search_embedding_configure')).toHaveLength(1)
  expect(provider.activeFormat).toBe('dynamic-int8')
  expect(JSON.parse(storage.get(key)!)).toEqual({ enabled: true, format: 'dynamic-int8' })
  value.setFormat('float32'); expect(render().format).toBe('dynamic-int8')
  provider.activeFormat = null
  mocked.invoke.mockClear()
  restart(); value = await settle()
  expect(provider.activeFormat).toBe('dynamic-int8')
  expect(value.enabled).toBe(true)
  expect(commands()).not.toContain('search_semantic_index_build')
  expect(commands()).not.toContain('search_embedding_download')
  await value.setEnabled(false)
  provider.activeFormat = null
  mocked.invoke.mockClear(); restart(); value = await settle()
  expect(value.enabled).toBe(false)
  expect(commands()).not.toContain('search_embedding_configure')
})

it('does not activate invalid preferences or saved opt-in in a build without a provider', async () => {
  storage.set(key, '{"enabled":true,"format":"unknown"}')
  render(); expect((await settle()).enabled).toBe(false)
  expect(commands()).not.toContain('search_embedding_configure')
  storage.set(key, '{"enabled":true,"format":"float32"}')
  provider.compiled = false; mocked.invoke.mockClear()
  restart(); expect((await settle()).enabled).toBe(true)
  expect(commands()).not.toContain('search_embedding_configure')
})

it('surfaces activation failure and retry without downloading or resetting opt-in', async () => {
  render(); let value = await settle()
  mocked.invoke.mockImplementationOnce(() => Promise.reject('Cache checksum mismatch'))
  await value.setEnabled(true); value = await settle()
  expect(value.enabled).toBe(true)
  expect(value.error).toBe('Cache checksum mismatch')
  expect(value.provider?.activeFormat).toBeNull()
  await value.setEnabled(true); value = await settle()
  expect(value.error).toBe('')
  expect(value.provider?.activeFormat).toBe('float32')
  expect(commands()).not.toContain('search_embedding_download')
})

it('disable interrupts pending activation and ignores late failure', async () => {
  render(); let value = await settle()
  const loading = deferred()
  mocked.invoke.mockImplementationOnce(() => { provider.loadingFormat = 'float32'; return loading.promise })
  const enabling = value.setEnabled(true)
  value = render(); expect(value.operation).toBe('activate')
  await value.setEnabled(false); value = await settle()
  expect(value.enabled).toBe(false)
  expect(value.busy).toBe(true) // Native loading keeps its slot until cleanup.
  expect(JSON.parse(storage.get(key)!).enabled).toBe(false)
  provider.loadingFormat = null
  loading.reject('Old activation cancelled'); await enabling
  await vi.advanceTimersByTimeAsync(1000); value = await settle()
  expect(value.enabled).toBe(false)
  expect(value.busy).toBe(false)
  expect(value.error).toBe('')
})

it('explicit jobs keep cancellation busy, expose coverage, and reject conflicting operations', async () => {
  render(); let value = await settle()
  await value.build(); expect(commands()).not.toContain('search_semantic_index_build')
  await value.setEnabled(true); value = await settle()
  const building = deferred()
  mocked.invoke.mockImplementationOnce(() => { index.running = true; return building.promise })
  const work = value.build()
  await value.build(); await value.remove()
  expect(commands().filter(cmd => cmd === 'search_semantic_index_build')).toHaveLength(1)
  value = render()
  await value.cancel() // Native reservation has not been observed by status yet.
  expect(commands()).not.toContain('search_semantic_index_cancel')
  index.embeddedPassages = 3; index.reusedPassages = 2; index.indexedDocuments = 1
  value = render(); await vi.advanceTimersByTimeAsync(1000); value = await settle()
  expect(value.index?.embeddedPassages).toBe(3)
  expect(value.index?.indexedDocuments).toBe(1)
  await value.cancel(); value = await settle()
  expect(value.cancelling).toBe(true); expect(value.busy).toBe(true)
  index.running = false; building.reject('Semantic indexing cancelled')
  await work; value = await settle()
  expect(value.error).toContain('cancelled'); expect(value.busy).toBe(false)
  await value.remove()
  expect(commands()).toContain('search_semantic_index_remove')
})

it('disable during indexing cannot be overwritten by the old job completion', async () => {
  render(); let value = await settle()
  await value.setEnabled(true); value = await settle()
  const building = deferred()
  mocked.invoke.mockImplementationOnce(() => { index.running = true; return building.promise })
  const work = value.build()
  value = render(); await value.setEnabled(false); value = await settle()
  expect(value.enabled).toBe(false)
  expect(value.busy).toBe(true)
  index.running = false; building.reject('Old job cancelled'); await work
  await vi.advanceTimersByTimeAsync(1000); value = await settle()
  expect(value.enabled).toBe(false); expect(value.error).toBe('')
})

it('refuses opt-in if saving fails, but still unloads when saving disable fails', async () => {
  render(); let value = await settle()
  const write = window.localStorage.setItem
  window.localStorage.setItem = () => { throw new Error('quota') }
  await value.setEnabled(true); value = render()
  expect(value.enabled).toBe(false); expect(value.error).toContain('Unable to save')
  expect(commands()).not.toContain('search_embedding_configure')
  window.localStorage.setItem = write
  await value.setEnabled(true); value = await settle()
  window.localStorage.setItem = () => { throw new Error('quota') }
  await value.setEnabled(false); value = await settle()
  expect(value.enabled).toBe(false); expect(provider.activeFormat).toBeNull()
  expect(value.error).toContain('previous preference may return')
})

it('ignores startup completion after unmount and bounds polling while status is slow', async () => {
  storage.set(key, '{"enabled":true,"format":"float32"}')
  const pending = deferred()
  mocked.invoke.mockImplementationOnce(() => pending.promise)
  render(); unmount(); pending.resolve(provider); await settle()
  expect(commands()).not.toContain('search_embedding_configure')
  restart(); let value = await settle()
  const building = deferred()
  mocked.invoke.mockImplementationOnce(() => { index.running = true; return building.promise })
  const work = value.build(); render()
  const slow = deferred()
  mocked.invoke.mockImplementationOnce(() => slow.promise)
  await vi.advanceTimersByTimeAsync(5000)
  expect(commands().filter(cmd => cmd === 'search_embedding_status')).toHaveLength(4)
  slow.resolve(provider); await settle()
  index.running = false; building.resolve(); await work
  value = await settle(); expect(value.busy).toBe(false)
})

it('StrictMode effect replay starts only one activation from saved consent', async () => {
  storage.set(key, '{"enabled":true,"format":"float32"}')
  render()
  const effects = mocked.slots.filter(slot => (slot as { effect?: unknown } | null)?.effect) as {
    cleanup?: () => void; effect: () => void | (() => void)
  }[]
  for (const effect of effects) effect.cleanup?.()
  for (const effect of effects) effect.cleanup = effect.effect() || undefined
  await settle()
  expect(commands().filter(cmd => cmd === 'search_embedding_configure')).toHaveLength(1)
})
