// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { bergamotWorkerUnavailableReason, runInstalledBergamotDocument } from './assets'

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), run: vi.fn(), validate: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke, Channel: class {} }))
vi.mock('./nativeBridge', async (original) => ({
  ...await original<typeof import('./nativeBridge')>(), runBergamotDocument: mocks.run,
}))
const request = { documentUrl: '/uploads/example.html', sourceLanguage: 'fr', targetLanguage: 'en', modelId: 'bergamot-fr-en-worker', qualityMode: 'balanced' }
const stored = { jobId: 'job', status: 'stored', message: 'Stored document' }
const actualValidate = WebAssembly.validate.bind(WebAssembly)

beforeEach(() => {
  vi.stubGlobal('isSecureContext', true)
  vi.stubGlobal('Worker', class {})
  vi.stubGlobal('WebAssembly', { validate: mocks.validate.mockReset().mockReturnValue(true) })
  mocks.invoke.mockReset().mockImplementation(async () => new ArrayBuffer(4))
  mocks.run.mockReset().mockResolvedValue(stored)
})
afterEach(() => { vi.unstubAllGlobals(); vi.restoreAllMocks() })

describe('verified installed Bergamot asset delivery', () => {
  it('loads only the four typed binary assets in sequence and hands them to the bounded bridge', async () => {
    const buffers = [4, 8, 12, 16].map(size => new ArrayBuffer(size))
    mocks.invoke.mockImplementation(async () => buffers[mocks.invoke.mock.calls.length - 1])
    await expect(runInstalledBergamotDocument(request)).resolves.toEqual(stored)
    expect(mocks.invoke.mock.calls).toEqual(['wasm', 'model', 'lex', 'vocab'].map(asset => [
      'translation_bergamot_asset', { sourceLanguage: 'fr', asset },
    ]))
    expect(mocks.run).toHaveBeenCalledWith(request, { sourceLanguage: 'fr', wasm: buffers[0], model: buffers[1], lex: buffers[2], vocab: buffers[3] }, expect.any(AbortSignal))
    expect(actualValidate(mocks.validate.mock.calls[0][0])).toBe(true)
    expect(mocks.validate.mock.calls[1][0].buffer).toBe(buffers[0])
  })

  it.each(['insecure', 'worker', 'crypto', 'simd'])('rejects missing %s capability before any native reads', async mode => {
    if (mode === 'insecure') vi.stubGlobal('isSecureContext', false)
    if (mode === 'worker') vi.stubGlobal('Worker', undefined)
    if (mode === 'crypto') vi.stubGlobal('crypto', {})
    if (mode === 'simd') mocks.validate.mockReturnValue(false)
    expect(bergamotWorkerUnavailableReason()).toBeTypeOf('string')
    await expect(runInstalledBergamotDocument(request)).rejects.toThrow('Bergamot requires')
    expect(mocks.invoke).not.toHaveBeenCalled()
    expect(mocks.run).not.toHaveBeenCalled()
  })

  it('rejects the actual runtime when only the small SIMD probe passes', async () => {
    mocks.validate.mockReturnValueOnce(true).mockReturnValueOnce(false)
    await expect(runInstalledBergamotDocument(request)).rejects.toThrow('runtime is unsupported')
    expect(mocks.invoke).toHaveBeenCalledTimes(1)
    expect(mocks.run).not.toHaveBeenCalled()
  })

  it.each(['json', 'empty', 'oversize', 'total'])('rejects %s binary replies without starting a worker', async mode => {
    if (mode === 'json') mocks.invoke.mockResolvedValue([1, 2, 3])
    if (mode === 'empty') mocks.invoke.mockResolvedValue(new ArrayBuffer(0))
    if (mode === 'oversize') mocks.invoke.mockResolvedValue(new ArrayBuffer(32 * 1024 * 1024 + 1))
    if (mode === 'total') mocks.invoke.mockImplementation(async () => new ArrayBuffer(32 * 1024 * 1024))
    await expect(runInstalledBergamotDocument(request)).rejects.toThrow(/binary asset|64 MiB/)
    expect(mocks.run).not.toHaveBeenCalled()
  })

  it('propagates a missing/corrupt native asset and releases admission for a retry', async () => {
    mocks.invoke.mockRejectedValueOnce(new Error('Bergamot asset integrity check failed'))
    await expect(runInstalledBergamotDocument(request)).rejects.toThrow('integrity')
    expect(mocks.run).not.toHaveBeenCalled()
    await expect(runInstalledBergamotDocument({ ...request, sourceLanguage: 'es', modelId: 'bergamot-es-en-worker' })).resolves.toEqual(stored)
    expect(mocks.run.mock.calls[0][1].sourceLanguage).toBe('es')
  })

  it('rejects invalid model/language pairs and pre-aborted operations before loading model files', async () => {
    const controller = new AbortController(); controller.abort()
    await expect(runInstalledBergamotDocument(request, controller.signal)).rejects.toBeDefined()
    await expect(runInstalledBergamotDocument({ ...request, modelId: 'bergamot-es-en-worker' })).rejects.toThrow('model or language pair')
    await expect(runInstalledBergamotDocument({ ...request, targetLanguage: 'es' })).rejects.toThrow('model or language pair')
    expect(mocks.invoke).not.toHaveBeenCalled()
  })

  it('serializes asset loading and honors cancellation between reads without a worker', async () => {
    const controller = new AbortController()
    let finish: (buffer: ArrayBuffer) => void = () => {}
    mocks.invoke.mockImplementationOnce(() => new Promise(resolve => { finish = resolve }))
    const pending = runInstalledBergamotDocument(request, controller.signal)
    await expect(runInstalledBergamotDocument({ ...request, sourceLanguage: 'es', modelId: 'bergamot-es-en-worker' })).rejects.toThrow('already in progress')
    controller.abort()
    const rejected = expect(pending).rejects.toBeDefined()
    finish(new ArrayBuffer(4))
    await rejected
    expect(mocks.invoke).toHaveBeenCalledTimes(1)
    expect(mocks.run).not.toHaveBeenCalled()
    await expect(runInstalledBergamotDocument(request)).resolves.toEqual(stored)
  })

  it('bounds a lost binary IPC reply and releases frontend admission', async () => {
    vi.useFakeTimers()
    try {
      mocks.invoke.mockImplementationOnce(() => new Promise(() => {}))
      const pending = runInstalledBergamotDocument(request)
      const rejected = expect(pending).rejects.toThrow('timed out')
      await vi.advanceTimersByTimeAsync(120_000)
      await rejected
      expect(mocks.run).not.toHaveBeenCalled()
      expect(vi.getTimerCount()).toBe(0)
      await expect(runInstalledBergamotDocument(request)).resolves.toEqual(stored)
    } finally { vi.useRealTimers() }
  })


  it('clears the asset deadline before document inference but keeps external cancellation', async () => {
    vi.useFakeTimers()
    try {
      mocks.run.mockImplementation((_request, _files, signal: AbortSignal) => new Promise((_, reject) => {
        signal.addEventListener('abort', () => reject(signal.reason), { once: true })
      }))
      const controller = new AbortController()
      const pending = runInstalledBergamotDocument(request, controller.signal)
      const rejected = expect(pending).rejects.toThrow('Stopped document')
      await vi.advanceTimersByTimeAsync(0)
      expect(mocks.run).toHaveBeenCalledTimes(1)
      await vi.advanceTimersByTimeAsync(120_000)
      expect(mocks.run.mock.calls[0][2].aborted).toBe(false)
      expect(vi.getTimerCount()).toBe(0)
      controller.abort(new Error('Stopped document'))
      await rejected
      expect(mocks.run.mock.calls[0][2].aborted).toBe(true)
      expect(vi.getTimerCount()).toBe(0)
    } finally { vi.useRealTimers() }
  })

})
