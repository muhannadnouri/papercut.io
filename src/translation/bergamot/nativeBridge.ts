import { Channel, invoke } from '@tauri-apps/api/core'
import { TranslationError, type TranslationStartRequest, type TranslationStartResult } from '../api/nativeTranslation'

export interface BergamotFiles {
  sourceLanguage: 'fr' | 'es'
  wasm: ArrayBuffer
  model: ArrayBuffer
  lex: ArrayBuffer
  vocab: ArrayBuffer
}
export interface TextSegment { id: string; text: string; html?: boolean }
export interface WorkerOutput extends TextSegment { sourceText: string }
interface Batch {
  kind: 'batch'
  attemptId: string
  batchId: number
  sourceLanguage: string
  targetLanguage: string
  segments: TextSegment[]
}
interface Confirm { kind: 'confirm'; attemptId: string; batchId: number }
interface Stop { kind: 'stop'; attemptId: string }
type Failure = 'cancelled' | 'workerLost' | 'unavailable' | 'engineFailed'

/** Enforce transport ceilings; protected HTML identities are validated against retained plans in Rust. */
export function validateBergamotSegments(segments: TextSegment[]): void {
  if (!segments.length || segments.length > 16
      || new Set(segments.map(s => s.id)).size !== segments.length
      || segments.some(s => !s.id || new TextEncoder().encode(s.id).byteLength > 128
        || !s.text.trim() || (s.html !== undefined && typeof s.html !== 'boolean')
        || (s.html ? new TextEncoder().encode(s.text).byteLength > 32 * 1024 : s.text.length > 1800 || Array.from(s.text).length > 900))) {
    throw new Error('Bergamot requires a bounded French/Spanish to English text batch')
  }
}

// Internal text boundary; installed callers deliver verified buffers through assets.ts.
export async function runBergamotBatch(
  segments: TextSegment[], files: BergamotFiles, signal?: AbortSignal,
): Promise<WorkerOutput[]> {
  validateBergamotSegments(segments)
  return runWorker<WorkerOutput[]>(files, signal, crypto.randomUUID(), channel => invoke('translation_bergamot_batch', {
    request: { jobId: channel.jobId, sourceLanguage: files.sourceLanguage, targetLanguage: 'en', segments },
    onRequest: channel.requests,
  }), false)
}

/** Keep one host for all rich document batches; only Rust may authorize persistence. */
export async function runBergamotDocument(
  request: TranslationStartRequest, files: BergamotFiles, signal?: AbortSignal,
): Promise<TranslationStartResult> {
  const jobId = request.jobId || crypto.randomUUID()
  return runWorker<TranslationStartResult>(files, signal, jobId, channel => invoke('translation_bergamot_document', {
    request: { ...request, jobId }, onRequest: channel.requests,
  }), true)
}

/** Bind callbacks to one attempt and await Rust through the save/cancel cutoff, with bounded loss recovery. */
async function runWorker<T>(
  files: BergamotFiles, signal: AbortSignal | undefined, jobId: string,
  start: (channel: { jobId: string; requests: Channel<Batch | Confirm | Stop> }) => Promise<T>,
  documentJob: boolean,
): Promise<T> {
  signal?.throwIfAborted()
  if (document.visibilityState === 'hidden') throw new Error('Bergamot requires a visible page')
  const buffers = [files.wasm, files.model, files.lex, files.vocab]
  if (buffers.some(buffer => !(buffer instanceof ArrayBuffer) || !buffer.byteLength)
      || new Set(buffers).size !== 4 || buffers.reduce((bytes, buffer) => bytes + buffer.byteLength, 0) > 64 * 1024 * 1024) {
    throw new Error('Bergamot requires four distinct binary assets within 64 MiB')
  }
  if (!['fr', 'es'].includes(files.sourceLanguage)) {
    throw new Error('Bergamot requires a bounded French/Spanish to English text batch')
  }
  const worker = new Worker('/translation/bergamot/worker.js')
  let current: Batch | Confirm | undefined
  let initialized = false
  let ready = false
  let closed = false
  let replied = false
  let failure: Failure = 'workerLost'
  let rejectLifecycle: (error: Error) => void = () => {}
  const lifecycle = new Promise<never>((_, reject) => { rejectLifecycle = reject })
  const reportFailure = (batch: Batch | Confirm, reason: Failure) => {
    void invoke('translation_bergamot_reply', { reply: {
      kind: 'failed', attemptId: batch.attemptId, batchId: batch.batchId, reason,
    } }).catch(() => { /* Rust's independent deadline still releases a lost IPC channel. */ })
  }
  const fail = (reason: Failure) => {
    if (closed) return
    closed = true
    failure = reason
    worker.terminate()
    if (current) reportFailure(current, reason)
    if (documentJob) {
      void invoke('translation_cancel', { request: { jobId } }).catch(() => {
        // Saving may already own the atomic commit; await Rust's final outcome below.
      })
    }
    rejectLifecycle(new Error(reason === 'cancelled' ? 'Translation cancelled' : 'Bergamot worker unavailable or lost'))
  }
  const onAbort = () => fail('cancelled')
  const onPageHide = () => fail('workerLost')
  const onVisibility = () => { if (document.visibilityState === 'hidden') fail('workerLost') }
  const onError = (event: Event) => { event.preventDefault(); fail('workerLost') }
  const sendBatch = () => {
    if (!current || current.kind !== 'batch' || closed) return
    try { worker.postMessage(current) } catch { fail('workerLost') }
  }
  worker.onmessage = ({ data }: MessageEvent<unknown>) => {
    if (closed || !current || current.kind !== 'batch' || !data || typeof data !== 'object') return
    const reply = data as Record<string, unknown>
    if (reply.kind === 'ready') {
      if (ready) { fail('engineFailed'); return }
      ready = true
      sendBatch()
    } else if (reply.kind === 'failed') {
      if (ready && (reply.attemptId !== current.attemptId || reply.batchId !== current.batchId)) return
      // Initialization fails before the worker has a batch identity.
      fail(ready ? 'engineFailed' : 'unavailable')
    } else if (reply.kind === 'result') {
      if (replied) return
      if (reply.attemptId !== current.attemptId || reply.batchId !== current.batchId) return
      if (!ready || !validOutputs(reply.outputs, current)) { fail('engineFailed'); return }
      replied = true
      void invoke('translation_bergamot_reply', { reply }).catch(() => fail('workerLost'))
    } else { fail('engineFailed') }
  }
  worker.addEventListener('error', onError)
  worker.addEventListener('messageerror', onError)
  window.addEventListener('pagehide', onPageHide)
  document.addEventListener('visibilitychange', onVisibility)
  signal?.addEventListener('abort', onAbort, { once: true })
  let timer = setTimeout(() => fail('workerLost'), 120_000)
  const renewDeadline = () => {
    clearTimeout(timer)
    timer = setTimeout(() => fail('workerLost'), 120_000)
  }
  const onRequest = (message: Batch | Confirm | Stop) => {
    if (message.kind === 'stop') {
      if (message.attemptId === current?.attemptId) worker.terminate()
      return // Rust still owns final success/failure of the command.
    }
    if (closed) { reportFailure(message, failure); return }
    if (current && (message.attemptId !== current.attemptId || message.batchId !== current.batchId + 1 || !replied)) {
      reportFailure(message, 'engineFailed')
      fail('engineFailed')
      return
    }
    if (!current && message.batchId !== 1) { reportFailure(message, 'engineFailed'); fail('engineFailed'); return }
    current = message
    replied = false
    renewDeadline()
    if (message.kind === 'confirm') {
      replied = true
      void invoke('translation_bergamot_reply', { reply: {
        kind: 'confirmed', attemptId: message.attemptId, batchId: message.batchId,
      } }).catch(() => fail('workerLost'))
      return
    }
    try {
      validateBergamotSegments(message.segments)
      if (message.sourceLanguage !== files.sourceLanguage || message.targetLanguage !== 'en') throw new Error('Wrong language pair')
      if (initialized) { if (ready) sendBatch(); return }
      initialized = true
      worker.postMessage({ kind: 'initialize', sourceLanguage: files.sourceLanguage, targetLanguage: 'en', files: {
        wasm: files.wasm, model: files.model, lex: files.lex, vocab: files.vocab,
      } }, buffers)
    } catch { fail('unavailable') }
  }
  try {
    if (signal?.aborted) fail('cancelled')
    if (closed) return await lifecycle
    const channel = new Channel<Batch | Confirm | Stop>(onRequest)
    const native = start({ jobId, requests: channel })
    if (documentJob) {
      // Rust may already be committing. Allow its result a bounded grace period.
      const finalResponse = lifecycle.catch(() => new Promise<never>((_, reject) => {
        clearTimeout(timer)
        timer = setTimeout(() => reject(new TranslationError(
          'translation-outcome-unknown',
          'The translation outcome could not be confirmed. Check the Library before trying again.',
        )), 30_000)
      }))
      return await Promise.race([native, finalResponse])
    }
    return await Promise.race([native, lifecycle])
  } finally {
    closed = true
    clearTimeout(timer)
    signal?.removeEventListener('abort', onAbort)
    window.removeEventListener('pagehide', onPageHide)
    document.removeEventListener('visibilitychange', onVisibility)
    worker.removeEventListener('error', onError)
    worker.removeEventListener('messageerror', onError)
    worker.onmessage = null
    worker.terminate()
  }
}

/** Match the exact transport echo before IPC; HTML is still untrusted until Rust restores it. */
function validOutputs(value: unknown, batch: Batch): value is WorkerOutput[] {
  if (!Array.isArray(value) || value.length !== batch.segments.length || value.length > 16) return false
  let bytes = 0
  const encoder = new TextEncoder()
  return value.every((output: unknown, i) => {
    if (!output || typeof output !== 'object') return false
    const o = output as Record<string, unknown>
    const source = batch.segments[i]
    if (o.id !== source.id || o.sourceText !== source.text || typeof o.text !== 'string' || !o.text.trim()) return false
    bytes += encoder.encode(source.id + source.text + o.text).byteLength
    return bytes <= 256 * 1024
  })
}
