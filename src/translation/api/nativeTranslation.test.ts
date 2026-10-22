// @vitest-environment jsdom
import { webcrypto } from 'node:crypto'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { finishTranslationReview, getTranslationCapabilities, loadTranslationReview, resetTranslationCapabilities, saveTranslationReview, startTranslationJob, TranslationError } from './nativeTranslation'

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
  it.each([true, false])('preserves backend acceptance gates when WebView compatibility is %s', async compatible => {
    if (!compatible) vi.stubGlobal('Worker', undefined)
    const pending = { ...worker, available: false, unavailableReason: 'acceptance-pending' }
    mocks.invoke.mockResolvedValue({ available: false, models: [pending] })
    const caps = await getTranslationCapabilities()
    expect(caps.available).toBe(false)
    expect(caps.models).toEqual([pending])
    expect(mocks.document).not.toHaveBeenCalled()
  })

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

describe('revision-bound formatting review IPC', () => {
  const expected = { schemaVersion: 1, policy: 'opus-assisted-emphasis-v1', variantId: 'document', generation: 'generation', revision: 7 }
  const request = { expected, block: { ordinal: 2, sourceIdentity: 'source' }, edit: { action: 'replaceMarks' as const, marks: [{ kind: 'bold' as const, range: { start: 1, end: 4 } }] } }

  it('loads saved evidence and sends only bounded edit JSON or the publication token without inference', async () => {
    const draft = { document: { expected }, previewHtml: '<p>Exact English</p>' }
    mocks.invoke.mockResolvedValueOnce(draft).mockResolvedValueOnce(draft).mockResolvedValueOnce({ id: 'document', status: 'completed' })
    expect(await loadTranslationReview('document')).toBe(draft)
    expect(await saveTranslationReview(request)).toBe(draft)
    expect(await finishTranslationReview(expected)).toEqual({ id: 'document', status: 'completed' })
    expect(mocks.invoke.mock.calls).toEqual([
      ['translation_review_load', { id: 'document' }],
      ['translation_review_save', { request: JSON.stringify(request) }],
      ['translation_review_finish', { request: JSON.stringify(expected) }],
    ])
    expect(mocks.document).not.toHaveBeenCalled()
  })

  it('keeps legacy no-evidence results readable without inventing editable identities', async () => {
    mocks.invoke.mockResolvedValue(null)
    expect(await loadTranslationReview('legacy')).toBeNull()
    expect(mocks.invoke).toHaveBeenCalledExactlyOnceWith('translation_review_load', { id: 'legacy' })
  })

  it.each(['review-conflict', 'review-pending', 'review-unavailable'])('preserves actionable native %s errors', async code => {
    mocks.invoke.mockRejectedValue({ code, message: 'Reload the saved draft' })
    await expect(saveTranslationReview(request)).rejects.toMatchObject({ name: 'TranslationError', code, message: 'Reload the saved draft' })
  })

  it('keeps error conversion for publication and older native string errors', async () => {
    mocks.invoke.mockRejectedValue(JSON.stringify({ code: 'review-conflict', message: 'Reload' }))
    await expect(finishTranslationReview(expected)).rejects.toMatchObject({ code: 'review-conflict', message: 'Reload' })
    mocks.invoke.mockRejectedValue('Older build error')
    await expect(loadTranslationReview('document')).rejects.toMatchObject({ code: 'translation-failed', message: 'Older build error' })
  })

  it('rejects oversized UTF-8 requests before crossing IPC, with the same native 16KiB bound', async () => {
    await expect(saveTranslationReview({ ...request, block: { ordinal: 2, sourceIdentity: '😀'.repeat(5000) } })).rejects.toMatchObject({ code: 'translation-review-limit' })
    await expect(finishTranslationReview({ ...expected, generation: '😀'.repeat(5000) })).rejects.toBeInstanceOf(TranslationError)
    expect(mocks.invoke).not.toHaveBeenCalled()
  })

  it('does not invoke native commands in a browser', async () => {
    vi.stubGlobal('window', {})
    await expect(loadTranslationReview('document')).rejects.toMatchObject({ code: 'translation-unavailable' })
    await expect(saveTranslationReview(request)).rejects.toMatchObject({ code: 'translation-unavailable' })
    await expect(finishTranslationReview(expected)).rejects.toMatchObject({ code: 'translation-unavailable' })
    expect(mocks.invoke).not.toHaveBeenCalled()
  })
})
