import { readFileSync } from 'node:fs'
import { createHash } from 'node:crypto'
import { createContext, runInContext } from 'node:vm'
import { describe, expect, it, vi } from 'vitest'

const root = new URL('../../../', import.meta.url)
const source = readFileSync(new URL('public/translation/bergamot/worker.js', root), 'utf8')
const evidence = JSON.parse(readFileSync(new URL('docs/offline-translation-evaluation.json', root), 'utf8'))
function harness() {
  const messages: Record<string, unknown>[] = []
  const vectors: { values: unknown[]; deleted: boolean }[] = []
  class Vector {
    values: unknown[] = []; deleted = false
    constructor() { vectors.push(this) }
    push_back(value: unknown) { this.values.push(value) }
    delete() { this.deleted = true }
    size() { return this.values.length }
    get(i: number) { return this.values[i] }
  }
  const translated = { values: [{ getTranslatedText: () => 'Hello🙂' }], size: () => 1, get: (i: number) => translated.values[i], delete: vi.fn() }
  const translate = vi.fn(() => translated)
  const module = {
    AlignedMemory: class { getByteArrayView() { return { set() {} } } },
    AlignedMemoryList: Vector, VectorString: Vector, VectorResponseOptions: Vector,
    TranslationModel: class {}, BlockingService: class { translate = translate },
  }
  const digest = vi.fn(async (_: string, buffer: ArrayBuffer) => Uint8Array.from(Buffer.from(hashes.get(buffer)!, 'hex')).buffer)
  const imports = vi.fn()
  const context = createContext({ ArrayBuffer, Uint8Array, TextEncoder, crypto: { subtle: { digest } },
    importScripts: imports,
    loadBergamot: (options: { onRuntimeInitialized: () => void }) => { queueMicrotask(options.onRuntimeInitialized); return module },
    self: { postMessage: (message: Record<string, unknown>) => messages.push(message) },
  })
  runInContext(source, context)
  const pins = runInContext('ARTIFACTS', context)
  const hashes = new Map<ArrayBuffer, string>()
  const files = Object.fromEntries(Object.entries({ wasm: pins.wasm, ...pins.fr }).map(([key, pin]) => {
    const [size, hash] = pin as [number, string]; const buffer = new ArrayBuffer(size); hashes.set(buffer, hash); return [key, buffer]
  }))
  const send = (data: unknown): Promise<void> => context.self.onmessage({ data })
  return { context, pins, files, send, messages, digest, imports, vectors, translated, translate }
}

describe('stock Bergamot worker adapter', () => {
  it('keeps the vendored loader unmodified and pins coherent runtime/model artifacts', () => {
    const glue = readFileSync(new URL('public/translation/bergamot/bergamot-translator.js', root))
    expect(glue.byteLength).toBe(evidence.bergamotGlue.bytes)
    expect(createHash('sha256').update(glue).digest('hex')).toBe(evidence.bergamotGlue.sha256)
    const { pins } = harness()
    expect(pins.wasm).toEqual([evidence.bergamotWasmRecord.attachment.size, evidence.bergamotWasmRecord.attachment.hash])
    for (const language of ['fr', 'es']) for (const record of evidence.bergamotModelRecords[language]) {
      expect(pins[language][record.fileType]).toEqual([record.attachment.size, record.attachment.hash])
    }
  })

  it('verifies assets before loading and uses plain text, exact source echoes and freed response vectors', async () => {
    const h = harness()
    await h.send({ kind: 'initialize', sourceLanguage: 'fr', targetLanguage: 'en', files: h.files })
    expect(h.digest).toHaveBeenCalledTimes(4)
    expect(h.imports).toHaveBeenCalledWith('bergamot-translator.js')
    expect(h.messages).toEqual([{ kind: 'ready' }])
    await h.send({ kind: 'batch', attemptId: 'a'.repeat(32), batchId: 1, sourceLanguage: 'fr', targetLanguage: 'en', segments: [{ id: 'b0:s0', text: 'Résumé🙂' }] })
    expect(h.messages.at(-1)).toEqual({ kind: 'result', attemptId: 'a'.repeat(32), batchId: 1, outputs: [{ id: 'b0:s0', sourceText: 'Résumé🙂', text: 'Hello🙂' }] })
    expect(h.vectors.at(-1)!.values).toEqual([{ qualityScores: false, alignment: false, html: false }])
    expect(h.vectors.at(-1)!.deleted).toBe(true)
    expect(h.vectors.at(-2)!.deleted).toBe(true)
    expect(h.translated.delete).toHaveBeenCalled()
  })

  it('rejects wrong sizes and checksums before loading any executable code', async () => {
    for (const mode of ['size', 'hash', 'pair']) {
      const h = harness()
      if (mode === 'size') h.files.wasm = new ArrayBuffer(1)
      else if (mode === 'hash') h.digest.mockImplementation(async () => new Uint8Array(32).buffer)
      await h.send({ kind: 'initialize', sourceLanguage: mode === 'pair' ? 'constructor' : 'fr', targetLanguage: 'en', files: h.files })
      expect(h.imports).not.toHaveBeenCalled()
      expect(h.messages.at(-1)!.reason).toBe('unavailable')
      expect(JSON.stringify(h.messages)).not.toContain('stack')
    }
  })

  it('cleans per-call allocations on engine failure without exposing the runtime error', async () => {
    const h = harness()
    await h.send({ kind: 'initialize', sourceLanguage: 'fr', targetLanguage: 'en', files: h.files })
    h.translate.mockImplementation(() => { throw new Error('private path and source text') })
    await h.send({ kind: 'batch', attemptId: 'a'.repeat(32), batchId: 1, sourceLanguage: 'fr', targetLanguage: 'en', segments: [{ id: 'x', text: 'Bonjour' }] })
    expect(h.messages.at(-1)!.reason).toBe('engineFailed')
    expect(JSON.stringify(h.messages)).not.toContain('private')
    expect(h.vectors.at(-1)!.deleted).toBe(true)
    expect(h.vectors.at(-2)!.deleted).toBe(true)
  })
})

