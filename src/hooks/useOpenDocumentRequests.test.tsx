// @vitest-environment jsdom
import {
  act,
  StrictMode,
  useLayoutEffect,
  useState,
} from 'react'
import { createRoot } from 'react-dom/client'
import {
  afterEach,
  beforeEach,
  expect,
  it,
  vi,
} from 'vitest'
import { useOpenDocumentRequests } from './useOpenDocumentRequests'
import {
  listenOpenDocumentRequests,
  takeOpenDocumentSources,
} from '../uploads/DocumentUploads'

vi.mock('../uploads/DocumentUploads', () => ({
  listenOpenDocumentRequests: vi.fn(),
  takeOpenDocumentSources: vi.fn(),
}))
vi.mock('../tts/api/nativeTts', () => ({
  partitionOpenDocumentSources: async (sources: string[]) => ({
    documents: sources.filter(source => source.endsWith('.epub')),
    bundles: sources.filter(source => !source.endsWith('.epub')),
  }),
}))

let queued: string[]
let notify: () => void
let unmount: (() => Promise<void>) | undefined
const stop = vi.fn()

beforeEach(() => {
  vi.resetAllMocks()
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true)
  vi.stubGlobal('__TAURI_INTERNALS__', {})
  queued = []
  vi.mocked(listenOpenDocumentRequests).mockImplementation(async handler => {
    notify = handler
    return stop
  })
  vi.mocked(takeOpenDocumentSources).mockImplementation(async () => queued.splice(0))
})
afterEach(async () => {
  await unmount?.()
  unmount = undefined
  vi.unstubAllGlobals()
  document.body.replaceChildren()
})

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>(done => {
    resolve = done
  })
  return {
    promise,
    resolve,
  }
}

async function mount(onDocuments: (sources: string[]) => Promise<void>, onBundle: (source: string) => Promise<void>) {
  const host = document.createElement('div')
  document.body.append(host)
  const root = createRoot(host)
  let setBusy!: (busy: boolean) => void
  function Harness({ bundle }: { bundle: typeof onBundle }) {
    const [busy, updateBusy] = useState(false)
    useLayoutEffect(() => {
      setBusy = updateBusy
    }, [])
    useOpenDocumentRequests({
      busy,
      onDocuments,
      onBundle: bundle,
    })
    return null
  }
  unmount = () => act(async () => root.unmount())
  await act(async () => root.render(<StrictMode><Harness bundle={onBundle} /></StrictMode>))
  return {
    setBusy: (busy: boolean) => act(async () => setBusy(busy)),
    render: (bundle: typeof onBundle) => act(async () => root.render(<StrictMode><Harness bundle={bundle} /></StrictMode>)),
  }
}

it('retains mixed-batch bundles when its document import makes the app busy', async () => {
  queued = ['book.epub', 'one.bundle', 'two.bundle']
  const pending = deferred<void>()
  const documents = vi.fn(async () => pending.promise)
  const oldBundle = vi.fn<(source: string) => Promise<void>>(async () => {})
  const app = await mount(documents, oldBundle)
  expect(documents).toHaveBeenCalledExactlyOnceWith(['book.epub'])
  await app.setBusy(true)
  const bundle = vi.fn<(source: string) => Promise<void>>(async () => {})
  await app.render(bundle)
  await act(async () => pending.resolve())
  expect(bundle).not.toHaveBeenCalled()
  await app.setBusy(false)
  expect(bundle.mock.calls.map(([source]) => source)).toEqual(['one.bundle', 'two.bundle'])
  expect(oldBundle).not.toHaveBeenCalled()
  expect(documents).toHaveBeenCalledOnce()
})

it('serializes bundles and drains requests received while an earlier batch is active', async () => {
  queued = ['one.bundle', 'two.bundle']
  const pending = deferred<void>()
  let active = 0
  let maximum = 0
  const bundle = vi.fn(async (source: string) => {
    maximum = Math.max(maximum, ++active)
    if (source === 'one.bundle') {
      await pending.promise
    }
    active--
  })
  const app = await mount(async () => {}, bundle)
  await app.setBusy(true)
  queued.push('three.bundle')
  await act(async () => notify())
  await app.setBusy(false)
  expect(bundle).toHaveBeenCalledOnce()
  await act(async () => pending.resolve())
  expect(bundle.mock.calls.map(([source]) => source)).toEqual(['one.bundle', 'two.bundle', 'three.bundle'])
  expect(maximum).toBe(1)
})

it('retains sources drained after effect cleanup and stops queued work on unmount', async () => {
  const pending = deferred<string[]>()
  vi.mocked(takeOpenDocumentSources).mockReturnValueOnce(pending.promise)
  const bundle = vi.fn<(source: string) => Promise<void>>(async () => {})
  const app = await mount(async () => {}, bundle)
  await app.setBusy(true)
  await act(async () => pending.resolve(['late.bundle']))
  expect(bundle).not.toHaveBeenCalled()
  await app.setBusy(false)
  expect(bundle).toHaveBeenCalledExactlyOnceWith('late.bundle')
  const importing = deferred<void>()
  bundle.mockReturnValueOnce(importing.promise)
  queued.push('active.bundle', 'cancelled.bundle')
  await act(async () => notify())
  await unmount!()
  unmount = undefined
  await act(async () => importing.resolve())
  expect(bundle.mock.calls.map(([source]) => source)).toEqual(['late.bundle', 'active.bundle'])
  expect(stop).toHaveBeenCalledTimes(2)
})
