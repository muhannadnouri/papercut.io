// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { bergamotWorkerUnavailableReason, runInstalledBergamotBatch } from './assets'

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), run: vi.fn(), validate: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke, Channel: class {} }))
vi.mock('./nativeBridge', async (original) => ({
  ...await original<typeof import('./nativeBridge')>(), runBergamotBatch: mocks.run,
}))
const segment = [{ id: 's0', text: 'Bonjour' }]
const actualValidate = WebAssembly.validate.bind(WebAssembly)

beforeEach(() => {
  vi.stubGlobal('isSecureContext', true)
  vi.stubGlobal('Worker', class {})
  vi.stubGlobal('WebAssembly', { validate: mocks.validate.mockReset().mockReturnValue(true) })
  mocks.invoke.mockReset().mockImplementation(async () => new ArrayBuffer(4))
  mocks.run.mockReset().mockResolvedValue([{ id: 's0', sourceText: 'Bonjour', text: 'Hello' }])
})
afterEach(() => { vi.unstubAllGlobals(); vi.restoreAllMocks() })

describe('verified installed Bergamot asset delivery', () => {
  it('loads only the four typed binary assets in sequence and hands them to the bounded bridge', async () => {
    const buffers = [4, 8, 12, 16].map(size => new ArrayBuffer(size))
    mocks.invoke.mockImplementation(async () => buffers[mocks.invoke.mock.calls.length - 1])
    await expect(runInstalledBergamotBatch(segment, 'fr')).resolves.toEqual([{ id: 's0', sourceText: 'Bonjour', text: 'Hello' }])
    expect(mocks.invoke.mock.calls).toEqual(['wasm', 'model', 'lex', 'vocab'].map(asset => [
      'translation_bergamot_asset', { sourceLanguage: 'fr', asset },
    ]))
    expect(mocks.run).toHaveBeenCalledWith(segment, { sourceLanguage: 'fr', wasm: buffers[0], model: buffers[1], lex: buffers[2], vocab: buffers[3] }, expect.any(AbortSignal))
    expect(actualValidate(mocks.validate.mock.calls[0][0])).toBe(true)
    expect(mocks.validate.mock.calls[1][0].buffer).toBe(buffers[0])
  })

  it.each(['insecure', 'worker', 'crypto', 'simd'])('rejects missing %s capability before any native reads', async mode => {
    if (mode === 'insecure') vi.stubGlobal('isSecureContext', false)
    if (mode === 'worker') vi.stubGlobal('Worker', undefined)
    if (mode === 'crypto') vi.stubGlobal('crypto', {})
    if (mode === 'simd') mocks.validate.mockReturnValue(false)
    expect(bergamotWorkerUnavailableReason()).toBeTypeOf('string')
    await expect(runInstalledBergamotBatch(segment, 'fr')).rejects.toThrow('Bergamot requires')
    expect(mocks.invoke).not.toHaveBeenCalled()
    expect(mocks.run).not.toHaveBeenCalled()
  })

  it('rejects the actual runtime when only the small SIMD probe passes', async () => {
    mocks.validate.mockReturnValueOnce(true).mockReturnValueOnce(false)
    await expect(runInstalledBergamotBatch(segment, 'fr')).rejects.toThrow('runtime is unsupported')
    expect(mocks.invoke).toHaveBeenCalledTimes(1)
    expect(mocks.run).not.toHaveBeenCalled()
  })

  it.each(['json', 'empty', 'oversize', 'total'])('rejects %s binary replies without starting a worker', async mode => {
    if (mode === 'json') mocks.invoke.mockResolvedValue([1, 2, 3])
    if (mode === 'empty') mocks.invoke.mockResolvedValue(new ArrayBuffer(0))
    if (mode === 'oversize') mocks.invoke.mockResolvedValue(new ArrayBuffer(32 * 1024 * 1024 + 1))
    if (mode === 'total') mocks.invoke.mockImplementation(async () => new ArrayBuffer(32 * 1024 * 1024))
    await expect(runInstalledBergamotBatch(segment, 'fr')).rejects.toThrow(/binary asset|64 MiB/)
    expect(mocks.run).not.toHaveBeenCalled()
  })

  it('propagates a missing/corrupt native asset and releases admission for a retry', async () => {
    mocks.invoke.mockRejectedValueOnce(new Error('Bergamot asset integrity check failed'))
    await expect(runInstalledBergamotBatch(segment, 'fr')).rejects.toThrow('integrity')
    expect(mocks.run).not.toHaveBeenCalled()
    await expect(runInstalledBergamotBatch(segment, 'es')).resolves.toHaveLength(1)
    expect(mocks.run.mock.calls[0][1].sourceLanguage).toBe('es')
  })

  it('rejects invalid text and pre-aborted operations before loading model files', async () => {
    const controller = new AbortController(); controller.abort()
    await expect(runInstalledBergamotBatch(segment, 'fr', controller.signal)).rejects.toBeDefined()
    await expect(runInstalledBergamotBatch([], 'fr')).rejects.toThrow('bounded')
    await expect(runInstalledBergamotBatch([{ id: 's', text: '🙂'.repeat(901) }], 'fr')).rejects.toThrow('bounded')
    expect(mocks.invoke).not.toHaveBeenCalled()
  })

  it('serializes asset loading and honors cancellation between reads without a worker', async () => {
    const controller = new AbortController()
    let finish: (buffer: ArrayBuffer) => void = () => {}
    mocks.invoke.mockImplementationOnce(() => new Promise(resolve => { finish = resolve }))
    const pending = runInstalledBergamotBatch(segment, 'fr', controller.signal)
    await expect(runInstalledBergamotBatch(segment, 'es')).rejects.toThrow('already in progress')
    controller.abort()
    const rejected = expect(pending).rejects.toBeDefined()
    finish(new ArrayBuffer(4))
    await rejected
    expect(mocks.invoke).toHaveBeenCalledTimes(1)
    expect(mocks.run).not.toHaveBeenCalled()
    await expect(runInstalledBergamotBatch(segment, 'fr')).resolves.toHaveLength(1)
  })

  it('bounds a lost binary IPC reply and releases frontend admission', async () => {
    vi.useFakeTimers()
    try {
      mocks.invoke.mockImplementationOnce(() => new Promise(() => {}))
      const pending = runInstalledBergamotBatch(segment, 'fr')
      const rejected = expect(pending).rejects.toThrow('timed out')
      await vi.advanceTimersByTimeAsync(120_000)
      await rejected
      expect(mocks.run).not.toHaveBeenCalled()
      expect(vi.getTimerCount()).toBe(0)
      await expect(runInstalledBergamotBatch(segment, 'fr')).resolves.toHaveLength(1)
    } finally { vi.useRealTimers() }
  })


  it('keeps the same absolute deadline after asset loading and aborts the active batch', async () => {
    vi.useFakeTimers()
    try {
      mocks.run.mockImplementation((_segments, _files, signal: AbortSignal) => new Promise((_, reject) => {
        signal.addEventListener('abort', () => reject(signal.reason), { once: true })
      }))
      const pending = runInstalledBergamotBatch(segment, 'fr')
      const rejected = expect(pending).rejects.toThrow('timed out')
      await vi.advanceTimersByTimeAsync(0)
      expect(mocks.run).toHaveBeenCalledTimes(1)
      await vi.advanceTimersByTimeAsync(120_000)
      await rejected
      expect(mocks.run.mock.calls[0][2].aborted).toBe(true)
      expect(vi.getTimerCount()).toBe(0)
    } finally { vi.useRealTimers() }
  })

})
