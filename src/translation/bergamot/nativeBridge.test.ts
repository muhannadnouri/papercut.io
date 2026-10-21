// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { runBergamotDocument, validateBergamotSegments, type BergamotFiles } from './nativeBridge'
import { runInstalledBergamotDocument } from './assets'

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
const request = { jobId: 'book-job', documentUrl: '/uploads/example.html', sourceLanguage: 'fr', targetLanguage: 'en', modelId: 'bergamot-fr-en-worker', qualityMode: 'balanced' }
const stored = { jobId: 'book-job', status: 'stored', message: 'Stored document' }
const batch = { kind: 'batch', attemptId: 'a'.repeat(32), batchId: 1, sourceLanguage: 'fr', targetLanguage: 'en', segments: [{ id: 'b0:s0', text: 'Bonjour🙂' }] }
const outputs = [{ id: 'b0:s0', sourceText: 'Bonjour🙂', text: 'Hello🙂' }]
const result = { kind: 'result', attemptId: batch.attemptId, batchId: 1, outputs }
function files(): BergamotFiles { return { sourceLanguage: 'fr', wasm: new ArrayBuffer(1), model: new ArrayBuffer(1), lex: new ArrayBuffer(1), vocab: new ArrayBuffer(1) } }
let resolveNative: (value: unknown) => void
let rejectNative: (error: Error) => void
function start(signal?: AbortSignal) {
  const binary = files()
  const promise = runBergamotDocument(request, binary, signal)
  const worker = FakeWorker.instances.at(-1)!
  return { promise, worker, binary }
}
beforeEach(() => {
  vi.useFakeTimers()
  vi.stubGlobal('isSecureContext', true)
  vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('visible')
  FakeWorker.instances = []
  mock.invoke.mockReset()
  mock.invoke.mockImplementation((command) => command === 'translation_bergamot_document'
    ? new Promise((resolve, reject) => { resolveNative = resolve; rejectNative = reject })
    : command === 'translation_bergamot_asset'
      ? Promise.resolve(Uint8Array.from([0, 97, 115, 109, 1, 0, 0, 0]).buffer) : Promise.resolve(true))
  vi.stubGlobal('Worker', FakeWorker)
})
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); vi.restoreAllMocks() })

describe('bounded Bergamot native/worker bridge', () => {
  it('relays opaque HTML intact and leaves structural validation to the Rust provider boundary', async () => {
    const { promise, worker } = start()
    const text = '<span data-pc="0">' + 'x'.repeat(900) + '</span>'
    const richBatch = { ...batch, segments: [{ id: 'b0:s0', text, html: true }] }
    validateBergamotSegments(richBatch.segments)
    expect(() => validateBergamotSegments([{ id: 'x', text }])).toThrow('bounded')
    expect(() => validateBergamotSegments([{ id: 'x', text: '🙂'.repeat(8193), html: true }])).toThrow('bounded')
    mock.channel!.onmessage(richBatch)
    worker.reply({ kind: 'ready' })
    expect(worker.postMessage).toHaveBeenLastCalledWith(richBatch)
    const reply = { ...result, outputs: [{ id: 'b0:s0', sourceText: text, text: '<span data-pc="0">translated</span>' }] }
    worker.reply(reply)
    expect(mock.invoke).toHaveBeenLastCalledWith('translation_bergamot_reply', { reply })
    resolveNative(stored)
    await promise
    expect(vi.getTimerCount()).toBe(0)
  })

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
    resolveNative(stored)
    await expect(promise).resolves.toEqual(stored)
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
    rejectNative(new Error(mode === 'abort' ? 'Translation cancelled' : 'Bergamot worker unavailable or lost'))
    await rejected
    expect(mock.invoke).toHaveBeenCalledWith('translation_cancel', { request: { jobId: request.jobId } })
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
    mock.channel!.onmessage(batch)
    expect(worker.postMessage).not.toHaveBeenCalled()
    expect(mock.invoke).toHaveBeenLastCalledWith('translation_bergamot_reply', { reply: { kind: 'failed', attemptId: batch.attemptId, batchId: 1, reason: 'cancelled' } })
    rejectNative(new Error('Translation cancelled'))
    await rejected
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
    rejectNative(new Error('Bergamot worker unavailable or lost'))
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
    rejectNative(new Error('Bergamot worker unavailable or lost'))
    await rejected
    expect(mock.invoke).toHaveBeenCalledWith('translation_bergamot_reply', { reply: { kind: 'failed', attemptId: batch.attemptId, batchId: 1, reason: 'engineFailed' } })
  })

  it('cleans up when native preflight rejects without a worker request', async () => {
    const { promise, worker } = start()
    rejectNative(new Error('busy'))
    await expect(promise).rejects.toThrow('busy')
    expect(worker.terminate).toHaveBeenCalled()
    expect(vi.getTimerCount()).toBe(0)
  })

  it('rejects pre-aborted calls before allocating a worker', async () => {
    const controller = new AbortController(); controller.abort()
    await expect(runBergamotDocument(request, files(), controller.signal)).rejects.toBeDefined()
    expect(FakeWorker.instances).toHaveLength(0)
    expect(mock.invoke).not.toHaveBeenCalled()
  })

  it.each(['count', 'scalars'])('rejects unbounded %s in native document batches before transferring model buffers', async mode => {
    const { promise, worker } = start()
    const segments = mode === 'count'
      ? Array.from({ length: 17 }, (_, i) => ({ id: `s${i}`, text: 'Bonjour' }))
      : [{ id: 's', text: '🙂'.repeat(901) }]
    mock.channel!.onmessage({ ...batch, segments })
    expect(worker.postMessage).not.toHaveBeenCalled()
    expect(worker.terminate).toHaveBeenCalled()
    expect(mock.invoke).toHaveBeenCalledWith('translation_bergamot_reply', { reply: {
      kind: 'failed', attemptId: batch.attemptId, batchId: 1, reason: 'unavailable',
    } })
    expect(mock.invoke).toHaveBeenCalledWith('translation_cancel', { request: { jobId: request.jobId } })
    rejectNative(new Error('Invalid worker input'))
    await expect(promise).rejects.toThrow('Invalid worker input')
    expect(vi.getTimerCount()).toBe(0)
  })

  it('rejects detached, aliased and excessive binary assets before worker allocation', async () => {
    const empty = files(); empty.wasm = new ArrayBuffer(0)
    const aliased = files(); aliased.model = aliased.lex
    const excessive = files(); excessive.model = new ArrayBuffer(64 * 1024 * 1024)
    for (const binary of [empty, aliased, excessive]) {
      await expect(runBergamotDocument(request, binary)).rejects.toThrow('binary assets')
    }
    expect(FakeWorker.instances).toHaveLength(0)
    expect(mock.invoke).not.toHaveBeenCalled()
  })
})

describe('page lifecycle across asset loading and the real worker bridge', () => {
  it.each(['direct', 'installed'])('rejects already-hidden %s admission without reads or workers', async mode => {
    vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('hidden')
    mock.invoke.mockRejectedValue(new Error('unexpected native call'))
    const pending = mode === 'direct'
      ? runBergamotDocument(request, files())
      : runInstalledBergamotDocument(request)
    await expect(pending).rejects.toThrow('visible page')
    expect(mock.invoke).not.toHaveBeenCalled()
    expect(FakeWorker.instances).toHaveLength(0)
    expect(vi.getTimerCount()).toBe(0)
  })

  it.each([
    ['hidden', 1], ['pagehide', 1], ['hidden', 4], ['pagehide', 4],
  ] as const)('cancels %s during asset read %s, ignores its late reply and permits retry', async (event, heldRead) => {
    const addWindow = vi.spyOn(window, 'addEventListener')
    const removeWindow = vi.spyOn(window, 'removeEventListener')
    const addDocument = vi.spyOn(document, 'addEventListener')
    const removeDocument = vi.spyOn(document, 'removeEventListener')
    const invoke = mock.invoke.getMockImplementation()!
    let reads = 0
    let finish: (value: ArrayBuffer) => void = () => {}
    mock.invoke.mockImplementation((command, ...args) => {
      if (command === 'translation_bergamot_asset' && ++reads === heldRead) {
        return new Promise(resolve => { finish = resolve })
      }
      return invoke(command, ...args)
    })
    const controller = new AbortController()
    const pending = runInstalledBergamotDocument(request, controller.signal)
    const rejected = vi.fn()
    const settled = pending.then(() => {}, rejected)
    try {
      await vi.advanceTimersByTimeAsync(0)
      expect(reads).toBe(heldRead)
      if (event === 'hidden') {
        vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('hidden')
        document.dispatchEvent(new Event('visibilitychange'))
      } else window.dispatchEvent(new Event('pagehide'))
      await vi.advanceTimersByTimeAsync(0)
      expect(rejected).toHaveBeenCalledTimes(1)
      expect(FakeWorker.instances).toHaveLength(0)
      expect(vi.getTimerCount()).toBe(0)
      for (const [, listener] of addWindow.mock.calls.filter(([name]) => name === 'pagehide')) {
        expect(removeWindow).toHaveBeenCalledWith('pagehide', listener)
      }
      for (const [, listener] of addDocument.mock.calls.filter(([name]) => name === 'visibilitychange')) {
        expect(removeDocument).toHaveBeenCalledWith('visibilitychange', listener)
      }
      finish(new ArrayBuffer(4))
      await vi.advanceTimersByTimeAsync(0)
      expect(reads).toBe(heldRead)
      expect(FakeWorker.instances).toHaveLength(0)
      vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('visible')
      const retry = runInstalledBergamotDocument(request)
      await vi.advanceTimersByTimeAsync(0)
      const worker = FakeWorker.instances.at(-1)!
      expect(FakeWorker.instances).toHaveLength(1)
      mock.channel!.onmessage(batch)
      worker.reply({ kind: 'ready' })
      worker.reply(result)
      resolveNative(stored)
      await expect(retry).resolves.toEqual(stored)
      expect(worker.terminate).toHaveBeenCalled()
      expect(vi.getTimerCount()).toBe(0)
    } finally {
      controller.abort()
      await settled
    }
  })

  it.each(['hidden', 'pagehide'])('cancels %s after handing assets to the real worker bridge', async event => {
    const pending = runInstalledBergamotDocument(request)
    const rejected = expect(pending).rejects.toThrow('cancelled')
    await vi.advanceTimersByTimeAsync(0)
    const worker = FakeWorker.instances.at(-1)!
    mock.channel!.onmessage(batch)
    if (event === 'hidden') {
      vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('hidden')
      document.dispatchEvent(new Event('visibilitychange'))
    } else window.dispatchEvent(new Event('pagehide'))
    rejectNative(new Error('Translation cancelled'))
    await rejected
    expect(worker.terminate).toHaveBeenCalled()
    expect(vi.getTimerCount()).toBe(0)
    const replies = mock.invoke.mock.calls.filter(([command]) => command === 'translation_bergamot_reply')
    expect(replies).toHaveLength(1)
    expect(replies[0][1].reply).toEqual({ kind: 'failed', attemptId: batch.attemptId, batchId: 1, reason: 'cancelled' })
    worker.reply(result)
    expect(mock.invoke.mock.calls.filter(([command]) => command === 'translation_bergamot_reply')).toHaveLength(1)
  })

  it.each(['success', 'failure'])('removes all page lifecycle listeners after %s', async mode => {
    const addWindow = vi.spyOn(window, 'addEventListener')
    const removeWindow = vi.spyOn(window, 'removeEventListener')
    const addDocument = vi.spyOn(document, 'addEventListener')
    const removeDocument = vi.spyOn(document, 'removeEventListener')
    const pending = runInstalledBergamotDocument(request)
    const checked = mode === 'success' ? expect(pending).resolves.toEqual(stored) : expect(pending).rejects.toThrow('busy')
    await vi.advanceTimersByTimeAsync(0)
    if (mode === 'success') resolveNative(stored)
    else rejectNative(new Error('busy'))
    await checked
    for (const [, listener] of addWindow.mock.calls.filter(([name]) => name === 'pagehide')) {
      expect(removeWindow).toHaveBeenCalledWith('pagehide', listener)
    }
    for (const [, listener] of addDocument.mock.calls.filter(([name]) => name === 'visibilitychange')) {
      expect(removeDocument).toHaveBeenCalledWith('visibilitychange', listener)
    }
    expect(vi.getTimerCount()).toBe(0)
    expect(FakeWorker.instances[0].terminate).toHaveBeenCalled()
  })
})

describe('full-document worker lifecycle', () => {
  async function documentJob(signal?: AbortSignal) {
    mock.invoke.mockImplementation((command) => command === 'translation_bergamot_document'
      ? new Promise((resolve, reject) => { resolveNative = resolve; rejectNative = reject }) : Promise.resolve(true))
    const promise = runBergamotDocument(request, files(), signal)
    return { promise, worker: FakeWorker.instances.at(-1)! }
  }

  it('initializes once and reuses the same worker beyond two minutes with independent batch deadlines', async () => {
    const { promise, worker } = await documentJob()
    mock.channel!.onmessage(batch)
    worker.reply({ kind: 'ready' })
    for (let sequence = 1; sequence <= 3; sequence++) {
      if (sequence > 1) mock.channel!.onmessage({ ...batch, batchId: sequence })
      await vi.advanceTimersByTimeAsync(90_000)
      worker.reply({ ...result, batchId: sequence })
      await Promise.resolve()
    }
    mock.channel!.onmessage({ kind: 'confirm', attemptId: batch.attemptId, batchId: 4 })
    expect(mock.invoke).toHaveBeenLastCalledWith('translation_bergamot_reply', { reply: {
      kind: 'confirmed', attemptId: batch.attemptId, batchId: 4,
    } })
    expect(FakeWorker.instances).toHaveLength(1)
    expect(worker.postMessage.mock.calls.filter(([v]) => v.kind === 'initialize')).toHaveLength(1)
    expect(worker.postMessage.mock.calls.filter(([v]) => v.kind === 'batch')).toHaveLength(3)
    resolveNative(stored)
    await expect(promise).resolves.toEqual(stored)
    expect(vi.getTimerCount()).toBe(0)
  })

  it.each(['abort', 'hidden', 'pagehide'])('refuses even a fully cached save after %s while waiting for Rust to finish', async mode => {
    const controller = new AbortController()
    const { promise, worker } = await documentJob(controller.signal)
    let settled = false
    void promise.then(() => { settled = true }, () => { settled = true })
    if (mode === 'abort') controller.abort()
    else if (mode === 'pagehide') window.dispatchEvent(new Event('pagehide'))
    else {
      vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('hidden')
      document.dispatchEvent(new Event('visibilitychange'))
    }
    await Promise.resolve()
    expect(settled).toBe(false)
    expect(mock.invoke).toHaveBeenCalledWith('translation_cancel', { request: { jobId: 'book-job' } })
    mock.channel!.onmessage({ kind: 'confirm', attemptId: batch.attemptId, batchId: 1 })
    expect(mock.invoke).toHaveBeenLastCalledWith('translation_bergamot_reply', { reply: {
      kind: 'failed', attemptId: batch.attemptId, batchId: 1, reason: mode === 'abort' ? 'cancelled' : 'workerLost',
    } })
    expect(worker.postMessage).not.toHaveBeenCalled()
    rejectNative(new Error('Translation cancelled before saving'))
    await expect(promise).rejects.toThrow('cancelled before saving')
    expect(vi.getTimerCount()).toBe(0)
  })

  it('keeps installed document admission through multiple long batches, then releases it for retry', async () => {
    const promise = runInstalledBergamotDocument(request)
    await vi.waitFor(() => expect(mock.invoke).toHaveBeenCalledWith('translation_bergamot_document', expect.anything()))
    const worker = FakeWorker.instances.at(-1)!
    mock.channel!.onmessage(batch)
    worker.reply({ kind: 'ready' })
    await vi.advanceTimersByTimeAsync(90_000)
    worker.reply(result)
    await Promise.resolve()
    mock.channel!.onmessage({ ...batch, batchId: 2 })
    await vi.advanceTimersByTimeAsync(90_000)
    expect(worker.terminate).not.toHaveBeenCalled()
    worker.reply({ ...result, batchId: 2 })
    resolveNative(stored)
    await expect(promise).resolves.toEqual(stored)
    expect(vi.getTimerCount()).toBe(0)
    const retry = runInstalledBergamotDocument(request)
    await vi.waitFor(() => expect(FakeWorker.instances).toHaveLength(2))
    resolveNative(stored)
    await expect(retry).resolves.toEqual(stored)
    expect(vi.getTimerCount()).toBe(0)
  })

  it('routes the public job API through asset delivery and retains structured native errors', async () => {
    const { startTranslationJob, TranslationError } = await import('../api/nativeTranslation')
    vi.stubGlobal('__TAURI_INTERNALS__', {})
    mock.invoke.mockImplementation(command => command === 'translation_bergamot_asset'
      ? Promise.resolve(Uint8Array.from([0, 97, 115, 109, 1, 0, 0, 0]).buffer)
      : Promise.reject({ code: 'model-not-installed', message: 'Pinned worker files are not installed' }))
    const error = await startTranslationJob(request).catch(error => error)
    expect(error).toBeInstanceOf(TranslationError)
    expect(error).toMatchObject({ code: 'model-not-installed', message: 'Pinned worker files are not installed' })
    expect(mock.invoke).toHaveBeenCalledWith('translation_bergamot_document', expect.anything())
    expect(mock.invoke).not.toHaveBeenCalledWith('translation_start', expect.anything())
  })

  it('keeps native model jobs on their existing command without loading worker assets', async () => {
    const { startTranslationJob } = await import('../api/nativeTranslation')
    vi.stubGlobal('__TAURI_INTERNALS__', {})
    mock.invoke.mockResolvedValue(stored)
    const native = { ...request, modelId: 'opus-mt-fr-en-ctranslate2' }
    await expect(startTranslationJob(native)).resolves.toEqual(stored)
    expect(mock.invoke).toHaveBeenCalledExactlyOnceWith('translation_start', { request: native })
    expect(FakeWorker.instances).toHaveLength(0)
  })

  it.each(['timeout', 'abort', 'pagehide'])('releases document admission when the final native reply is lost after %s', async mode => {
    const controller = new AbortController()
    const promise = runInstalledBergamotDocument(request, controller.signal)
    let finished = false
    void promise.then(() => { finished = true }, () => { finished = true })
    await vi.waitFor(() => expect(mock.invoke).toHaveBeenCalledWith('translation_bergamot_document', expect.anything()))
    const oldChannel = mock.channel!
    const oldWorker = FakeWorker.instances.at(-1)!
    const oldResolve = resolveNative
    oldChannel.onmessage({ kind: 'confirm', attemptId: batch.attemptId, batchId: 1 })
    oldChannel.onmessage({ kind: 'stop', attemptId: batch.attemptId })
    if (mode === 'timeout') await vi.advanceTimersByTimeAsync(120_000)
    else if (mode === 'abort') controller.abort()
    else window.dispatchEvent(new Event('pagehide'))
    expect(oldWorker.terminate).toHaveBeenCalled()
    await vi.advanceTimersByTimeAsync(29_999)
    oldChannel.onmessage({ kind: 'confirm', attemptId: batch.attemptId, batchId: 2 })
    oldWorker.reply(result)
    await vi.advanceTimersByTimeAsync(1)
    expect(finished).toBe(true)
    await expect(promise).rejects.toMatchObject({ code: 'translation-outcome-unknown' })
    expect(vi.getTimerCount()).toBe(0)
    const calls = mock.invoke.mock.calls.length
    window.dispatchEvent(new Event('pagehide'))
    expect(mock.invoke.mock.calls).toHaveLength(calls)
    const retry = runInstalledBergamotDocument({ ...request, jobId: 'retry-job' })
    await vi.waitFor(() => expect(FakeWorker.instances).toHaveLength(2))
    // An old command's late success cannot finish or close this new owner.
    oldResolve(stored)
    oldChannel.onmessage({ kind: 'confirm', attemptId: batch.attemptId, batchId: 2 })
    await Promise.resolve()
    expect(FakeWorker.instances[1].terminate).not.toHaveBeenCalled()
    resolveNative({ ...stored, jobId: 'retry-job' })
    await expect(retry).resolves.toMatchObject({ jobId: 'retry-job' })
    expect(vi.getTimerCount()).toBe(0)
  })

  it('keeps a late known save result authoritative during the final-response grace period', async () => {
    const { promise } = await documentJob()
    let settled = false
    void promise.then(() => { settled = true }, () => { settled = true })
    mock.channel!.onmessage({ kind: 'confirm', attemptId: batch.attemptId, batchId: 1 })
    await vi.advanceTimersByTimeAsync(120_000 + 29_999)
    expect(settled).toBe(false)
    resolveNative(stored)
    await expect(promise).resolves.toEqual(stored)
    expect(vi.getTimerCount()).toBe(0)
  })

  it.each([0, 3])('public job cancellation stops a pending asset read at index %s without starting Rust', async index => {
    const { startTranslationJob, TranslationError } = await import('../api/nativeTranslation')
    vi.stubGlobal('__TAURI_INTERNALS__', {})
    const controller = new AbortController()
    let release: (buffer: ArrayBuffer) => void = () => {}
    let reads = 0
    mock.invoke.mockImplementation(command => {
      if (command === 'translation_bergamot_asset') {
        const current = reads++
        return current === index ? new Promise(resolve => { release = resolve })
          : Promise.resolve(Uint8Array.from([0, 97, 115, 109, 1, 0, 0, 0]).buffer)
      }
      return Promise.resolve(true)
    })
    const promise = startTranslationJob(request, controller.signal)
    const rejected = expect(promise).rejects.toMatchObject({ code: 'operation-cancelled' })
    await vi.waitFor(() => expect(reads).toBe(index + 1))
    controller.abort(new TranslationError('operation-cancelled', 'Translation cancelled'))
    await rejected
    release(Uint8Array.from([0, 97, 115, 109, 1, 0, 0, 0]).buffer)
    await Promise.resolve()
    expect(reads).toBe(index + 1)
    expect(FakeWorker.instances).toHaveLength(0)
    expect(mock.invoke.mock.calls.some(([cmd]) => cmd === 'translation_bergamot_document' || cmd === 'translation_cancel')).toBe(false)
    expect(vi.getTimerCount()).toBe(0)
    mock.invoke.mockImplementation(command => command === 'translation_bergamot_asset'
      ? Promise.resolve(Uint8Array.from([0, 97, 115, 109, 1, 0, 0, 0]).buffer)
      : command === 'translation_bergamot_document'
        ? new Promise(resolve => { resolveNative = resolve }) : Promise.resolve(true))
    const retry = startTranslationJob({ ...request, jobId: 'retry-job' }, new AbortController().signal)
    await vi.waitFor(() => expect(FakeWorker.instances).toHaveLength(1))
    resolveNative({ ...stored, jobId: 'retry-job' })
    await expect(retry).resolves.toMatchObject({ jobId: 'retry-job' })
    expect(vi.getTimerCount()).toBe(0)
  })

  it('returns stored rather than cancelled if the atomic writer already owns the commit', async () => {
    const controller = new AbortController()
    const { promise } = await documentJob(controller.signal)
    mock.channel!.onmessage({ kind: 'confirm', attemptId: batch.attemptId, batchId: 1 })
    mock.invoke.mockImplementation(command => command === 'translation_cancel'
      ? Promise.reject(new Error('Translation is already being saved')) : Promise.resolve(true))
    controller.abort()
    resolveNative(stored)
    await expect(promise).resolves.toEqual(stored)
    expect(vi.getTimerCount()).toBe(0)
  })

  it('rejects overlapping batches and out-of-sequence confirmation without reinitializing', async () => {
    const { promise, worker } = await documentJob()
    mock.channel!.onmessage(batch)
    mock.channel!.onmessage({ kind: 'confirm', attemptId: batch.attemptId, batchId: 2 })
    expect(mock.invoke).not.toHaveBeenCalledWith('translation_bergamot_reply', { reply: {
      kind: 'confirmed', attemptId: batch.attemptId, batchId: 2,
    } })
    expect(worker.terminate).toHaveBeenCalled()
    rejectNative(new Error('Bergamot worker failed'))
    await expect(promise).rejects.toThrow('worker failed')
  })
})
