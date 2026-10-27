import {
  afterEach,
  beforeEach,
  expect,
  it,
  vi,
} from 'vitest'
import type { SemanticSearchController } from './useSemanticSearch'
import type { ConceptualResponse } from './useConceptualSearch'

const mocked = vi.hoisted(() => ({
  invoke: vi.fn(),
  isTauri: vi.fn(),
  cursor: 0,
  slots: [] as unknown[],
  effects: [] as (() => void)[],
}))
vi.mock('@tauri-apps/api/core', () => ({
  invoke: mocked.invoke,
  isTauri: mocked.isTauri,
}))
// Match the existing lifecycle hook harness without adding a DOM test dependency.
vi.mock('react', () => {
  function stable<T>(value: T, deps: unknown[]): T {
    const i = mocked.cursor++
    const old = mocked.slots[i] as { value: T;
      deps: unknown[] } | undefined
    if (!old || deps.some((dep, n) => !Object.is(dep, old.deps[n]))) {
      mocked.slots[i] = {
        value,
        deps,
      }
    }
    return (mocked.slots[i] as { value: T }).value
  }
  return {
    useMemo: (make: () => unknown, deps: unknown[]) => stable(make(), deps),
    useRef: (initial: unknown) => {
      const i = mocked.cursor++
      if (i >= mocked.slots.length) {
        mocked.slots[i] = { current: initial }
      }
      return mocked.slots[i]
    },
    useState: (initial: unknown) => {
      const i = mocked.cursor++
      if (i >= mocked.slots.length) {
        mocked.slots[i] = initial
      }
      return [mocked.slots[i], (value: unknown) => {
        mocked.slots[i] = value
      }]
    },
    useEffect: (effect: () => void | (() => void), deps: unknown[]) => {
      const i = mocked.cursor++
      const old = mocked.slots[i] as { deps: unknown[];
        cleanup?: () => void } | undefined
      if (!old || deps.some((dep, n) => !Object.is(dep, old.deps[n]))) {
        mocked.effects.push(() => {
          old?.cleanup?.()
          mocked.slots[i] = {
            deps,
            cleanup: effect(),
            effect,
          }
        })
      }
    },
  }
})
import { useConceptualSearch } from './useConceptualSearch'
let options: Parameters<typeof useConceptualSearch>[0]
const response = (availability: ConceptualResponse['availability'] = 'ready'): ConceptualResponse => ({
  availability,
  indexedDocuments: 1,
  eligibleDocuments: availability === 'partial' ? 2 : 1,
  scannedPassages: 1,
  results: [{
    documentId: 'guide',
    url: '/uploads/guide.html',
    title: 'Transit guide',
    format: 'html',
    context: [],
    provenance: 'conceptual',
    score: 0.5,
    sectionIndex: 2,
    pageIndex: null,
    sectionTitle: 'Connections',
    byteStart: 0,
    byteEnd: 20,
    text: 'Electric buses run.',
  }],
})
function render(patch: Partial<typeof options> = {}) {
  options = {
    ...options,
    ...patch,
  }
  mocked.cursor = 0
  // eslint-disable-next-line react-hooks/rules-of-hooks -- Explicit async hook harness.
  const value = useConceptualSearch(options)
  for (const effect of mocked.effects.splice(0)) {
    effect()
  }
  return value
}
async function settle() {
  for (let i = 0; i < 12; i++) {
    await Promise.resolve()
  }
  return render()
}
function unmount() {
  for (const slot of mocked.slots) {
    (slot as { cleanup?: () => void } | null)?.cleanup?.()
  }
}
function pending() {
  let resolve!: (value: ConceptualResponse) => void
  let reject!: (reason: unknown) => void
  const promise = new Promise<ConceptualResponse>((done, fail) => {
    resolve = done
    reject = fail
  })
  return {
    promise,
    resolve,
    reject,
  }
}
beforeEach(() => {
  mocked.cursor = 0
  mocked.slots = []
  mocked.effects = []
  mocked.invoke.mockReset().mockResolvedValue(response())
  mocked.isTauri.mockReset().mockReturnValue(true)
  options = {
    semantic: {
      enabled: true,
      format: 'float32',
      busy: false,
      provider: {
        compiled: true,
        activeFormat: 'float32',
      },
      index: {},
    } as SemanticSearchController,
    submittedQuery: 'evening travel "night-time service"',
    searchRequest: {
      revision: 1,
      scope: '',
    },
    queryError: null,
    scopeUrls: new Set(),
    scopeActive: false,
    library: [],
  }
})
afterEach(unmount)

it('does no work and renders no fallback while off, empty or invalid; unavailable/preparing paths never query', async () => {
  expect(render({
    semantic: {
      ...options.semantic,
      enabled: false,
    },
  })).toBeNull()
  await settle()
  expect(mocked.invoke).not.toHaveBeenCalled()
  render({
    semantic: {
      ...options.semantic,
      enabled: true,
    },
    submittedQuery: '"unfinished',
  })
  expect(await settle()).toBeNull()
  render({ submittedQuery: '' })
  expect(await settle()).toBeNull()
  render({
    submittedQuery: 'travel',
    queryError: 'tooLong',
  })
  expect(await settle()).toBeNull()
  render({
    queryError: null,
    semantic: {
      ...options.semantic,
      provider: undefined,
    },
  })
  expect(await settle()).toEqual({
    phase: 'fallback',
    reason: 'unavailable',
  })
  render({
    semantic: {
      ...options.semantic,
      provider: {
        compiled: true,
        activeFormat: 'float32',
        loadingFormat: null,
        downloadingFormat: null,
      },
      busy: true,
    },
  })
  expect(await settle()).toEqual({
    phase: 'fallback',
    reason: 'preparing',
  })
  expect(mocked.invoke).not.toHaveBeenCalled()
  mocked.isTauri.mockReturnValue(false)
  render({
    semantic: {
      ...options.semantic,
      busy: false,
    },
  })
  expect(await settle()).toEqual({
    phase: 'fallback',
    reason: 'unavailable',
  })
  expect(mocked.invoke).not.toHaveBeenCalled()
})

it('forwards complete authored clauses and all/empty/filtered scope, and reissues the same submitted query', async () => {
  render()
  expect((await settle())?.phase).toBe('ready')
  expect(mocked.invoke).toHaveBeenLastCalledWith('search_semantic_query', {
    request: {
      query: 'evening travel',
      exactPhrases: ['night-time service'],
      documentUrls: undefined,
      limit: 50,
    },
  })
  render({
    searchRequest: {
      revision: 2,
      scope: '',
    },
  })
  await settle()
  expect(mocked.invoke).toHaveBeenCalledTimes(2)
  // Scope changes arrive before App reruns lexical search; avoid an extra native reservation.
  expect(render({
    scopeActive: true,
    scopeUrls: new Set(['/uploads/guide.html']),
  })).toBeNull()
  await settle()
  expect(mocked.invoke).toHaveBeenCalledTimes(2)
  render({
    searchRequest: {
      revision: 3,
      scope: '["/uploads/guide.html"]',
    },
  })
  await settle()
  expect(mocked.invoke.mock.calls.at(-1)?.[1].request.documentUrls).toEqual(['/uploads/guide.html'])
  render({
    scopeUrls: new Set(),
    searchRequest: {
      revision: 4,
      scope: '[]',
    },
  })
  await settle()
  expect(mocked.invoke.mock.calls.at(-1)?.[1].request.documentUrls).toEqual([])
  render({
    scopeActive: false,
    searchRequest: {
      revision: 5,
      scope: '',
    },
    submittedQuery: '"electric bus"',
  })
  await settle()
  expect(mocked.invoke.mock.calls.at(-1)?.[1].request).toMatchObject({
    query: '',
    exactPhrases: ['electric bus'],
    documentUrls: undefined,
  })
})

it('ignores stale A → B → A, disable, library mutation and unmount completions', async () => {
  const a = pending()
  const latest = pending()
  const edited = pending()
  mocked.invoke.mockReturnValueOnce(a.promise)
    .mockReturnValueOnce(latest.promise).mockReturnValueOnce(edited.promise)
  render({ submittedQuery: 'alpha' })
  await settle()
  render({
    submittedQuery: 'beta',
    searchRequest: {
      revision: 2,
      scope: '',
    },
  })
  await settle()
  render({
    submittedQuery: 'alpha',
    searchRequest: {
      revision: 3,
      scope: '',
    },
  })
  await settle()
  expect(mocked.invoke).toHaveBeenCalledTimes(1)
  a.resolve(response())
  expect((await settle())?.phase).toBe('loading')
  latest.resolve(response())
  expect((await settle())?.phase).toBe('ready')
  expect(render({
    library: [{
      title: 'Edited',
      url: '/uploads/guide.html',
    }],
  })?.phase).toBe('loading')
  await settle()
  expect(render({
    semantic: {
      ...options.semantic,
      enabled: false,
    },
  })).toBeNull()
  edited.resolve(response())
  expect(await settle()).toBeNull()
  const abandoned = pending()
  mocked.invoke.mockReturnValueOnce(abandoned.promise)
  render({
    semantic: {
      ...options.semantic,
      enabled: true,
    },
  })
  await settle()
  unmount()
  const before = [...mocked.slots]
  abandoned.resolve(response())
  for (let i = 0; i < 12; i++) {
    await Promise.resolve()
  }
  expect(mocked.slots).toEqual(before)
})

it('coalesces replacements behind the native single slot and drains the latest scoped clauses after failure', async () => {
  const first = pending()
  const latest = pending()
  const requests = [first, latest]
  let running = false
  mocked.invoke.mockImplementation(() => {
    if (running) {
      return Promise.reject('A conceptual query is already running')
    }
    running = true
    return requests.shift()!.promise.finally(() => {
      running = false
    })
  })
  render()
  await settle()
  render({
    submittedQuery: 'obsolete',
    searchRequest: {
      revision: 2,
      scope: '',
    },
  })
  await settle()
  const scope = '["/uploads/guide.html"]'
  render({
    submittedQuery: 'morning travel "electric bus"',
    scopeActive: true,
    scopeUrls: new Set(['/uploads/guide.html']),
    searchRequest: {
      revision: 3,
      scope,
    },
  })
  expect((await settle())?.phase).toBe('loading')
  expect(mocked.invoke).toHaveBeenCalledTimes(1)
  first.reject('old native query failed')
  await settle()
  expect(mocked.invoke).toHaveBeenCalledTimes(2)
  expect(mocked.invoke).toHaveBeenLastCalledWith('search_semantic_query', {
    request: {
      query: 'morning travel',
      exactPhrases: ['electric bus'],
      documentUrls: ['/uploads/guide.html'],
      limit: 50,
    },
  })
  expect((await settle())?.phase).toBe('loading')
  latest.resolve(response())
  expect((await settle())?.phase).toBe('ready')
})

it.each(['disabled', 'invalid', 'scope-pending', 'preparing', 'unavailable', 'unmount'])('drops a queued replacement when %s before the running call settles', async (reason) => {
  const first = pending()
  mocked.invoke.mockReturnValue(first.promise)
  render()
  await settle()
  render({
    submittedQuery: 'queued travel',
    searchRequest: {
      revision: 2,
      scope: '',
    },
  })
  await settle()
  if (reason === 'disabled') {
    render({
      semantic: {
        ...options.semantic,
        enabled: false,
      },
    })
  }
  if (reason === 'invalid') {
    render({ submittedQuery: '"unfinished' })
  }
  if (reason === 'scope-pending') {
    render({
      scopeActive: true,
      scopeUrls: new Set(),
    })
  }
  if (reason === 'preparing') {
    render({
      semantic: {
        ...options.semantic,
        busy: true,
      },
    })
  }
  if (reason === 'unavailable') {
    render({
      semantic: {
        ...options.semantic,
        provider: undefined,
      },
    })
  }
  if (reason === 'unmount') {
    unmount()
  }
  first.resolve(response())
  for (let i = 0; i < 12; i++) {
    await Promise.resolve()
  }
  expect(mocked.invoke).toHaveBeenCalledTimes(1)
})

it('distinguishes empty ready results from missing/incompatible/partial/error lexical fallback', async () => {
  for (const availability of ['missing-index', 'incompatible-index', 'partial'] as const) {
    mocked.invoke.mockResolvedValue(response(availability))
    render({
      searchRequest: {
        revision: options.searchRequest.revision + 1,
        scope: '',
      },
    })
    const state = await settle()
    expect(state?.phase).toBe('fallback')
    expect(state?.reason).toBe(availability)
    expect(state?.response?.indexedDocuments).toBe(1)
  }
  mocked.invoke.mockRejectedValue('native query failed')
  render({
    searchRequest: {
      revision: 9,
      scope: '',
    },
  })
  expect(await settle()).toEqual({
    phase: 'fallback',
    reason: 'failed',
  })
  mocked.invoke.mockResolvedValue({
    ...response(),
    results: [],
  })
  render({
    searchRequest: {
      revision: 10,
      scope: '',
    },
  })
  expect((await settle())?.phase).toBe('ready')
})

it('StrictMode effect replay reserves just one query and normal rerenders do not query again', async () => {
  render()
  const effect = mocked.slots.at(-1) as { effect: () => (() => void);
    cleanup: () => void }
  effect.cleanup()
  effect.cleanup = effect.effect()
  await settle()
  render()
  await settle()
  expect(mocked.invoke).toHaveBeenCalledTimes(1)
})
