/* Bergamot text-only worker. Uses the unmodified MPL-2.0 loader beside this file.
 * API reference: mozilla/translations 69455acaecbe8650cdba988dbcf7c10ca20e7c48,
 * inference/wasm/tests/engine/translations-engine.worker.mjs.
 * Asset pins come from docs/offline-translation-evaluation.json. */
const ARTIFACTS = {
  wasm: [4960506, 'a3a89d9ad0a4ed8f27bf3e403701b23f5709816f6376438503f2fa5b0182c2dc'],
  fr: {
    model: [31561787, '15f997bc0d13808b0b0fbd0786e684a3c8a52adcd8071844b76123fdacbf2b90'],
    lex: [4824120, '87c6752ea908f5f0347c10ac0cf7d80d9c2f4f20c81c90168f3e8230b56d4440'],
    vocab: [814404, '783abf3abe075afdf8d85d233994bef2c3a064e935ab1bed946820aff6ac002a'],
  },
  es: {
    model: [31561787, '4aed7734152ae0045d1a69ae49c86cfda18f53c61f90e95e1d1de1c7c7c3b033'],
    lex: [4636248, 'e2610211d3b9577d012638fe7e7e74ed7b4b708ce96b9e792e67c282a6492daa'],
    vocab: [816054, '5ae254fa9b15aa182e70fd2a6186b1333c63a29a48043a9224c6aa4fcac058ad'],
  },
}
let loading = false
let engine = null
let busy = false

async function initialize(data) {
  if (loading || engine) throw new Error('Already initialized')
  loading = true
  if (!['fr', 'es'].includes(data.sourceLanguage) || data.targetLanguage !== 'en') throw new Error('Unsupported pair')
  const pins = ARTIFACTS[data.sourceLanguage]
  for (const [key, pin] of Object.entries({ wasm: ARTIFACTS.wasm, ...pins })) {
    const buffer = data.files?.[key]
    if (!(buffer instanceof ArrayBuffer) || buffer.byteLength !== pin[0]) throw new Error('Invalid asset size')
    const hash = Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', buffer)), b => b.toString(16).padStart(2, '0')).join('')
    if (hash !== pin[1]) throw new Error('Invalid asset hash')
  }
  // Fixed app-owned script, not a URL supplied by the IPC caller.
  importScripts('bergamot-translator.js')
  const module = await new Promise((resolve, reject) => {
    let module
    module = loadBergamot({
      wasmBinary: data.files.wasm, INITIAL_MEMORY: 41943040,
      print() {}, printErr() {}, onAbort() { reject(new Error('WASM unavailable')) },
      onRuntimeInitialized() { resolve(module) },
    })
  })
  const memories = []
  const allocate = (buffer, alignment) => {
    const memory = new module.AlignedMemory(buffer.byteLength, alignment)
    memory.getByteArrayView().set(new Uint8Array(buffer))
    memories.push(memory)
    return memory
  }
  const model = allocate(data.files.model, 256)
  const lex = allocate(data.files.lex, 64)
  const vocab = allocate(data.files.vocab, 64)
  const vocabs = new module.AlignedMemoryList()
  vocabs.push_back(vocab)
  const config = 'beam-size: 1\nnormalize: 1.0\nword-penalty: 0\nmax-length-break: 128\nmini-batch-words: 1024\nworkspace: 128\nmax-length-factor: 2.0\nskip-cost: true\ncpu-threads: 0\nquiet: true\nquiet-translation: true\ngemm-precision: int8shiftAlphaAll\nalignment: soft\n'
  engine = {
    module, memories, vocabs, sourceLanguage: data.sourceLanguage,
    model: new module.TranslationModel(data.sourceLanguage, 'en', config, model, lex, vocabs, null),
    service: new module.BlockingService({ cacheSize: 0 }),
  }
  self.postMessage({ kind: 'ready' })
}

function translate(data) {
  if (!engine || busy || data.sourceLanguage !== engine.sourceLanguage || data.targetLanguage !== 'en'
      || typeof data.attemptId !== 'string' || data.attemptId.length !== 32
      || !Number.isSafeInteger(data.batchId) || data.batchId < 1
      || !Array.isArray(data.segments) || !data.segments.length || data.segments.length > 16) throw new Error('Invalid batch')
  const ids = new Set()
  for (const s of data.segments) {
    if (typeof s.id !== 'string' || !s.id || s.id.length > 128 || ids.has(s.id)
        || typeof s.text !== 'string' || !s.text.trim() || s.text.length > 1800 || Array.from(s.text).length > 900) throw new Error('Invalid segment')
    ids.add(s.id)
  }
  busy = true
  const messages = new engine.module.VectorString()
  const options = new engine.module.VectorResponseOptions()
  let responses
  try {
    for (const s of data.segments) {
      messages.push_back(s.text)
      options.push_back({ qualityScores: false, alignment: false, html: false })
    }
    responses = engine.service.translate(engine.model, messages, options)
    if (responses.size() !== data.segments.length) throw new Error('Invalid reply count')
    let bytes = 0
    const encoder = new TextEncoder()
    const outputs = data.segments.map((s, i) => {
      const text = responses.get(i).getTranslatedText()
      if (typeof text !== 'string' || !text.trim()) throw new Error('Empty translation')
      bytes += encoder.encode(s.id + s.text + text).byteLength
      if (bytes > 256 * 1024) throw new Error('Oversize reply')
      return { id: s.id, sourceText: s.text, text }
    })
    self.postMessage({ kind: 'result', attemptId: data.attemptId, batchId: data.batchId, outputs })
  } finally {
    responses?.delete()
    options.delete()
    messages.delete()
    busy = false
  }
}

self.onmessage = async ({ data }) => {
  try {
    if (data?.kind === 'initialize') await initialize(data)
    else if (data?.kind === 'batch') translate(data)
    else throw new Error('Unknown request')
  } catch {
    // Never send model text, private paths or runtime stacks through diagnostics.
    self.postMessage({ kind: 'failed', attemptId: data?.attemptId, batchId: data?.batchId, reason: engine ? 'engineFailed' : 'unavailable' })
  }
}
