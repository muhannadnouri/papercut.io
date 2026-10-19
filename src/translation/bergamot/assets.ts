import { invoke } from '@tauri-apps/api/core'
import { runBergamotBatch, validateBergamotSegments, type BergamotFiles, type TextSegment } from './nativeBridge'

type Language = BergamotFiles['sourceLanguage']

/** Feature detection is a prerequisite, not a packaged-device support claim. */
export function bergamotWorkerUnavailableReason(): string | null {
  if (!globalThis.isSecureContext || typeof Worker !== 'function'
      || typeof globalThis.crypto?.subtle?.digest !== 'function' || typeof crypto.randomUUID !== 'function') {
    return 'Bergamot requires a secure WebView with workers and WebCrypto'
  }
  try {
    // Minimal module returning v128.const; detect SIMD rather than guessing from OS versions.
    if (typeof WebAssembly !== 'object' || !WebAssembly.validate(new Uint8Array([
      0, 97, 115, 109, 1, 0, 0, 0, 1, 5, 1, 96, 0, 1, 123, 3, 2, 1, 0,
      10, 22, 1, 20, 0, 253, 12, ...Array<number>(16).fill(0), 11,
    ]))) return 'Bergamot requires WebAssembly SIMD support'
  } catch { return 'Bergamot requires WebAssembly SIMD support' }
  return null
}

let active = false
// shortcut: assets and one text batch only; enable document selection after full-job integration.
export async function runInstalledBergamotBatch(
  segments: TextSegment[], sourceLanguage: Language, signal?: AbortSignal,
) {
  signal?.throwIfAborted()
  if (document.visibilityState === 'hidden') throw new Error('Bergamot requires a visible page')
  const unavailable = bergamotWorkerUnavailableReason()
  if (unavailable) throw new Error(unavailable)
  if (!['fr', 'es'].includes(sourceLanguage)) throw new Error('Unsupported Bergamot language pair')
  validateBergamotSegments(segments)
  if (active) throw new Error('A Bergamot asset operation is already in progress')
  active = true
  const controller = new AbortController()
  const onAbort = () => controller.abort(signal?.reason)
  const onPageHide = () => controller.abort(new Error('Bergamot page was hidden'))
  const onVisibility = () => { if (document.visibilityState === 'hidden') onPageHide() }
  let onStop: () => void = () => {}
  const stopped = new Promise<never>((_, reject) => {
    onStop = () => reject(controller.signal.reason)
    controller.signal.addEventListener('abort', onStop, { once: true })
  })
  signal?.addEventListener('abort', onAbort, { once: true })
  window.addEventListener('pagehide', onPageHide)
  document.addEventListener('visibilitychange', onVisibility)
  const timer = setTimeout(() => controller.abort(new Error('Bergamot operation timed out')), 120_000)
  try {
    const files = { sourceLanguage } as BergamotFiles
    let total = 0
    for (const asset of ['wasm', 'model', 'lex', 'vocab'] as const) {
      controller.signal.throwIfAborted()
      const buffer = await Promise.race([
        invoke<ArrayBuffer>('translation_bergamot_asset', { sourceLanguage, asset }), stopped,
      ])
      controller.signal.throwIfAborted()
      if (!(buffer instanceof ArrayBuffer) || !buffer.byteLength || buffer.byteLength > 32 * 1024 * 1024) {
        throw new Error('Invalid Bergamot binary asset response')
      }
      total += buffer.byteLength
      if (total > 64 * 1024 * 1024) throw new Error('Bergamot assets exceed 64 MiB')
      files[asset] = buffer
      if (asset === 'wasm' && !WebAssembly.validate(new Uint8Array(buffer))) {
        throw new Error('Bergamot runtime is unsupported by this WebView')
      }
    }
    return await runBergamotBatch(segments, files, controller.signal)
  } finally {
    clearTimeout(timer)
    signal?.removeEventListener('abort', onAbort)
    window.removeEventListener('pagehide', onPageHide)
    document.removeEventListener('visibilitychange', onVisibility)
    controller.signal.removeEventListener('abort', onStop)
    active = false
  }
}
