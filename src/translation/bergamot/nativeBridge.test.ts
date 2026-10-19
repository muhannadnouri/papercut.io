// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { runBergamotBatch, type BergamotFiles } from './nativeBridge'

const mock = vi.hoisted(() => ({ invoke: vi.fn(), channel: null as null | { onmessage: (message: unknown) => void } }))
vi.mock('@tauri-apps/api/core', () => ({
  invoke: mock.invoke,
  Channel: class {
    onmessage: (message: unknown) => void
    constructor(handler: (message: unknown) => void) { this.onmessage = handler; mock.channel = this }
  },
}))
class FakeWorker extends EventTarget {
  static instances: FakeWorker[] = []
  onmessage: ((event: MessageEvent) => void) | null = null
  postMessage = vi.fn()
  terminate = vi.fn()
  constructor() { super(); FakeWorker.instances.push(this) }
  reply(data: unknown) { this.onmessage?.(new MessageEvent('message', { data })) }
}
const batch = { kind: 'batch', attemptId: 'a'.repeat(32), batchId: 1, sourceLanguage: 'fr', targetLanguage: 'en', segments: [{ id: 'b0:s0', text: 'Bonjour🙂' }] }
const outputs = [{ id: 'b0:s0', sourceText: 'Bonjour🙂', text: 'Hello🙂' }]
const result = { kind: 'result', attemptId: batch.attemptId, batchId: 1, outputs }
function files(): BergamotFiles { return { sourceLanguage: 'fr', wasm: new ArrayBuffer(1), model: new ArrayBuffer(1), lex: new ArrayBuffer(1), vocab: new ArrayBuffer(1) } }
let resolveNative: (value: unknown) => void
let rejectNative: (error: Error) => void
function start(signal?: AbortSignal) {
  const binary = files()
  const promise = runBergamotBatch(batch.segments, binary, signal)
  const worker = FakeWorker.instances.at(-1)!
  return { promise, worker, binary }
}
beforeEach(() => {
  vi.useFakeTimers()
  FakeWorker.instances = []
  mock.invoke.mockReset()
  mock.invoke.mockImplementation((command) => command === 'translation_bergamot_batch'
    ? new Promise((resolve, reject) => { resolveNative = resolve; rejectNative = reject }) : Promise.resolve(true))
  vi.stubGlobal('Worker', FakeWorker)
})
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); vi.restoreAllMocks() })

describe('bounded Bergamot native/worker bridge', () => {
  it('transfers binary files outside JSON and lets Rust own final success', async () => {
    const { promise, worker, binary } = start()
    mock.channel!.onmessage(batch)
    const [init, transfer] = worker.postMessage.mock.calls[0]
    expect(init.files.model).toBe(binary.model)
    expect(transfer).toHaveLength(4)
    expect(mock.invoke.mock.calls[0][1].request).not.toHaveProperty('files')
    worker.reply({ kind: 'ready' })
    expect(worker.postMessage).toHaveBeenLastCalledWith(batch)
    worker.reply(result)
    worker.reply(result)
    expect(mock.invoke.mock.calls.filter(([name]) => name === 'translation_bergamot_reply')).toHaveLength(1)
    mock.channel!.onmessage({ kind: 'stop', attemptId: batch.attemptId })
    resolveNative(outputs)
    await expect(promise).resolves.toEqual(outputs)
    expect(worker.terminate).toHaveBeenCalled()
    expect(vi.getTimerCount()).toBe(0)
    window.dispatchEvent(new Event('pagehide'))
    expect(mock.invoke.mock.calls.filter(([name]) => name === 'translation_bergamot_reply')).toHaveLength(1)
  })

  it.each(['abort', 'error', 'messageerror', 'pagehide', 'hidden', 'timeout'])('closes on %s and reports only a bounded failure code', async (mode) => {
    const controller = new AbortController()
    const { promise, worker } = start(controller.signal)
    mock.channel!.onmessage(batch)
    const rejected = expect(promise).rejects.toThrow(mode === 'abort' ? 'cancelled' : 'unavailable or lost')
    if (mode === 'abort') controller.abort()
    if (mode === 'error' || mode === 'messageerror') worker.dispatchEvent(new Event(mode))
    if (mode === 'pagehide') window.dispatchEvent(new Event('pagehide'))
    if (mode === 'hidden') {
      vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('hidden')
      document.dispatchEvent(new Event('visibilitychange'))
    }
    if (mode === 'timeout') await vi.advanceTimersByTimeAsync(120_000)
    await rejected
    expect(worker.terminate).toHaveBeenCalled()
    const failure = mock.invoke.mock.calls.find(([name]) => name === 'translation_bergamot_reply')![1].reply
    expect(failure).toEqual({ kind: 'failed', attemptId: batch.attemptId, batchId: 1, reason: mode === 'abort' ? 'cancelled' : 'workerLost' })
    expect(vi.getTimerCount()).toBe(0)
  })

  it('reports cancellation even when the Rust request arrives after the UI already closed', async () => {
    const controller = new AbortController()
    const { promise, worker } = start(controller.signal)
    const rejected = expect(promise).rejects.toThrow('cancelled')
    controller.abort()
    await rejected
    mock.channel!.onmessage(batch)
    expect(worker.postMessage).not.toHaveBeenCalled()
    expect(mock.invoke).toHaveBeenLastCalledWith('translation_bergamot_reply', { reply: { kind: 'failed', attemptId: batch.attemptId, batchId: 1, reason: 'cancelled' } })
  })

  it('ignores stale result/failure identities without extending the deadline', async () => {
    const { promise, worker } = start()
    mock.channel!.onmessage(batch)
    worker.reply({ kind: 'ready' })
    await vi.advanceTimersByTimeAsync(90_000)
    worker.reply({ ...result, batchId: 0 })
    worker.reply({ kind: 'failed', attemptId: 'old', batchId: 1, reason: 'private stack' })
    expect(mock.invoke).toHaveBeenCalledTimes(1)
    const rejected = expect(promise).rejects.toThrow('unavailable or lost')
    await vi.advanceTimersByTimeAsync(30_000)
    await rejected
  })

  it.each(['count', 'source', 'empty', 'oversize'])('rejects malformed %s results', async (mode) => {
    const { promise, worker } = start()
    mock.channel!.onmessage(batch)
    worker.reply({ kind: 'ready' })
    const bad = structuredClone(result)
    if (mode === 'count') bad.outputs = []
    if (mode === 'source') bad.outputs[0].sourceText = 'stale'
    if (mode === 'empty') bad.outputs[0].text = ' \n'
    if (mode === 'oversize') bad.outputs[0].text = 'x'.repeat(256 * 1024 + 1)
    const rejected = expect(promise).rejects.toThrow('unavailable or lost')
    worker.reply(bad)
    await rejected
    expect(mock.invoke).toHaveBeenLastCalledWith('translation_bergamot_reply', { reply: { kind: 'failed', attemptId: batch.attemptId, batchId: 1, reason: 'engineFailed' } })
  })

  it('cleans up when native preflight rejects without a worker request', async () => {
    const { promise, worker } = start()
    rejectNative(new Error('busy'))
    await expect(promise).rejects.toThrow('busy')
    expect(worker.terminate).toHaveBeenCalled()
    expect(vi.getTimerCount()).toBe(0)
  })

  it('rejects unbounded input and pre-aborted calls before allocating a worker', async () => {
    const controller = new AbortController(); controller.abort()
    await expect(runBergamotBatch(batch.segments, files(), controller.signal)).rejects.toBeDefined()
    await expect(runBergamotBatch(Array(17).fill(batch.segments[0]), files())).rejects.toThrow('bounded')
    await expect(runBergamotBatch([{ id: 'x', text: '🙂'.repeat(901) }], files())).rejects.toThrow('bounded')
    expect(FakeWorker.instances).toHaveLength(0)
    expect(mock.invoke).not.toHaveBeenCalled()
  })

  it('rejects detached, aliased and excessive binary assets before worker allocation', async () => {
    const empty = files(); empty.wasm = new ArrayBuffer(0)
    const aliased = files(); aliased.model = aliased.lex
    const excessive = files(); excessive.model = new ArrayBuffer(64 * 1024 * 1024)
    for (const binary of [empty, aliased, excessive]) {
      await expect(runBergamotBatch(batch.segments, binary)).rejects.toThrow('binary assets')
    }
    expect(FakeWorker.instances).toHaveLength(0)
    expect(mock.invoke).not.toHaveBeenCalled()
  })
})
