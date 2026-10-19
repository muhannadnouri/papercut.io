// @vitest-environment jsdom
import { webcrypto } from 'node:crypto'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { getTranslationCapabilities, resetTranslationCapabilities, startTranslationJob } from './nativeTranslation'

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), document: vi.fn() }))
vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke }))
vi.mock('../bergamot/assets', async original => ({
  ...await original<typeof import('../bergamot/assets')>(), runInstalledBergamotDocument: mocks.document,
}))
const native = { id: 'opus-mt-fr-en-ctranslate2', engine: 'ctranslate2', available: true }
const worker = { id: 'bergamot-fr-en-worker', engine: 'bergamot-worker', available: true }

beforeEach(() => {
  vi.stubGlobal('window', { __TAURI_INTERNALS__: {} })
  vi.stubGlobal('isSecureContext', true)
  vi.stubGlobal('Worker', class {})
  vi.stubGlobal('crypto', webcrypto)
  resetTranslationCapabilities()
  mocks.invoke.mockReset().mockResolvedValue({ available: true, models: [native, worker] })
  mocks.document.mockReset().mockResolvedValue({ jobId: 'worker', status: 'stored' })
})
afterEach(() => { vi.unstubAllGlobals(); vi.restoreAllMocks(); resetTranslationCapabilities() })

describe('translation catalog WebView admission', () => {
  it('keeps compatible worker and native entries in their original order without installing or inferring', async () => {
    const caps = await getTranslationCapabilities()
    expect(caps.available).toBe(true)
    expect(caps.models).toEqual([native, worker])
    expect(mocks.invoke).toHaveBeenCalledExactlyOnceWith('translation_capabilities', undefined)
    expect(mocks.document).not.toHaveBeenCalled()
  })

  it.each(['secure', 'worker', 'crypto', 'simd'])('gates missing %s support without disabling native models', async missing => {
    if (missing === 'secure') vi.stubGlobal('isSecureContext', false)
    if (missing === 'worker') vi.stubGlobal('Worker', undefined)
    if (missing === 'crypto') vi.stubGlobal('crypto', {})
    if (missing === 'simd') vi.spyOn(WebAssembly, 'validate').mockReturnValue(false)
    const caps = await getTranslationCapabilities()
    expect(caps.available).toBe(true)
    expect(caps.models[0]).toEqual(native)
    expect(caps.models[1]).toMatchObject({ available: false, unavailableReason: 'webview' })
  })

  it('worker-only builds become unavailable when the WebView fails, and never override a disabled native feature', async () => {
    mocks.invoke.mockResolvedValue({ available: true, models: [{ ...native, available: false }, worker] })
    expect((await getTranslationCapabilities()).models.map(m => m.available)).toEqual([false, true])
    resetTranslationCapabilities()
    vi.stubGlobal('Worker', undefined)
    expect(await getTranslationCapabilities()).toMatchObject({ available: false, models: [{ available: false }, { available: false, unavailableReason: 'webview' }] })
    resetTranslationCapabilities()
    mocks.invoke.mockResolvedValue({ available: false, models: [{ ...worker, available: false }] })
    expect((await getTranslationCapabilities()).models[0]).toEqual({ ...worker, available: false })
  })

  it.each(['fr', 'es'])('routes %s document jobs to the installed worker with cancellation ownership', async sourceLanguage => {
    const request = { modelId: `bergamot-${sourceLanguage}-en-worker`, documentUrl: '/uploads/book.html', sourceLanguage, targetLanguage: 'en', qualityMode: 'balanced' }
    const controller = new AbortController()
    await startTranslationJob(request, controller.signal)
    expect(mocks.document).toHaveBeenCalledExactlyOnceWith(request, controller.signal)
    expect(mocks.invoke).not.toHaveBeenCalled()
  })

  it('keeps native job dispatch unchanged', async () => {
    const request = { modelId: native.id, documentUrl: '/uploads/book.epub', sourceLanguage: 'fr', targetLanguage: 'en', qualityMode: 'balanced' }
    await startTranslationJob(request)
    expect(mocks.invoke).toHaveBeenCalledExactlyOnceWith('translation_start', { request })
    expect(mocks.document).not.toHaveBeenCalled()
  })
})
