import { createAudiobookId, type SavedAudiobookRecord } from '../storage/AudiobookLibrary'
import { resolveSilmaNfeStep, resolveTextPreprocessor, type TtsModelInfo, type TtsOptions, type TtsChunk } from '../types'
import { FALLBACK_TTS_MODELS } from '../models'

const SAVE_PROGRESS_EVENT = 'tts-native-save-progress'
const MODEL_INSTALL_PROGRESS_EVENT = 'tts-model-install-progress'

export interface NativeTtsCapabilities {
  available: boolean
  backend: string
  reason: string
  modelDir?: string | null
  platform: string
  compiledExecutionProviders: string[]
  executionProviderProbeError?: string | null
  defaultThreadCount: number
  maxThreadCount: number
  models: TtsModelInfo[]
}

export interface NativeTtsModelStatus {
  modelId: string
  installed: boolean
  installing: boolean
  installSupported: boolean
  runtimeInstalled: boolean
  modelDir?: string | null
  runtimeDir?: string | null
  sourceUrl: string
  sourceLabel: string
  archiveBytes: number
  installedBytes: number
  sha256: string
  message: string
  runtimeMessage: string
}

export interface NativeTtsModelInstallProgress {
  modelId: string
  status: 'starting' | 'downloading' | 'extracting' | 'installed' | string
  message: string
  downloadedBytes: number
  totalBytes: number
  percent: number
}

export interface NativeTtsModelInstallResult {
  modelId: string
  modelDir: string
  bytes: number
}

export interface NativeSilmaSidecarProbeResult {
  workerPath: string
  pythonCommand: string
  probeWavPath: string
  healthVersion: string
  sampleRate: number
  audioDurationSec: number
  wavBytes: number
}

export interface NativeTtsChunkResult {
  chunk: TtsChunk
  wav: ArrayBuffer
  sampleRate: number
  audioDurationSec: number
  wavBytes: number
  generateMs: number
  backend: string
}

export interface NativeAudiobookPlaybackChunk {
  index: number
  chunkId: string
  startSec: number
  durationSec: number
}

export interface NativeAudiobookPlayback {
  title: string
  audioUrl: string
  audioDurationSec: number
  wavBytes: number
  chunks: NativeAudiobookPlaybackChunk[]
}

export interface NativeAudiobookStatus {
  cachedChunks: number
  totalChunks: number
  complete: boolean
  dir: string
  audioDurationSec: number
  wavBytes: number
}

export interface NativeAudiobookSaveProgress {
  jobId: string
  status: 'checking' | 'saving' | 'saved' | 'cancelled' | string
  message: string
  cachedChunks: number
  totalChunks: number
  generatedChunks: number
  chunkId?: string | null
  chunkNumber?: number | null
  textChars?: number | null
  textPreview?: string | null
  generateMs?: number | null
  preprocessMs?: number | null
  synthesisMs?: number | null
  writeMs?: number | null
  validateMs?: number | null
  indexingMs?: number | null
  synthesisTextChars?: number | null
  totalSourceChars?: number | null
  totalSynthesisChars?: number | null
  audioDurationSec?: number | null
  wavBytes?: number | null
  totalAudioDurationSec: number
  totalWavBytes: number
  appliedThreadCount: number
  backend: string
}

export interface NativeAudiobookSaveResult {
  jobId: string
  cachedChunks: number
  totalChunks: number
  generatedChunks: number
  complete: boolean
  dir: string
  generateMs: number
  audioDurationSec: number
  wavBytes: number
  appliedThreadCount: number
  backend: string
}

export interface NativeAudiobookExportResult {
  path: string
  audioPath: string
  metadataPath: string
  htmlPath: string
  chunks: number
  audioDurationSec: number
  wavBytes: number
}

export type NativeAudiobookExportFormat = 'bundle' | 'wav'

export interface NativeImportedAudiobookMetadata {
  documentUrl: string
  modelId: string
  textPreprocessor: string
  title: string
  voice: string
  speed: number
  dtype: string
  silmaNfeStep?: number
  chunks: TtsChunk[]
  audioDurationSec: number
  wavBytes: number
}

export interface NativeAudiobookImportResult {
  documentUrl: string
  sourceKind: 'html' | 'pdf'
  modelId: string
  textPreprocessor: string
  title: string
  voice: string
  speed: number
  dtype: string
  silmaNfeStep?: number
  chunks: number
  audioDurationSec: number
  wavBytes: number
}

export interface NativeAudiobookDeleteResult {
  deletedAudio: boolean
  deletedUserUpload: boolean
  bytesFreed: number
}

interface NativeTtsChunkResponse {
  chunkId?: string | null
  wavBase64: string
  sampleRate: number
  audioDurationSec: number
  wavBytes: number
  generateMs: number
  backend: string
}

type NativeTtsInputChunk = Pick<TtsChunk, 'id' | 'text' | 'textHash' | 'sourceSpan'>

// Preserve sourceSpan across native save/export so new bundles can restore
// highlighting without rediscovering DOM positions from text alone.
function toNativeTtsChunk(chunk: TtsChunk): NativeTtsInputChunk {
  const textHash = typeof chunk.textHash === 'string' ? chunk.textHash : undefined
  return { id: chunk.id, text: chunk.text, textHash, sourceSpan: chunk.sourceSpan }
}

function toNativeTtsChunks(chunks: TtsChunk[]): NativeTtsInputChunk[] {
  return chunks.map(toNativeTtsChunk)
}

// 64-bit FNV-1a over UTF-8. Rust uses the same algorithm for manifest identity.
function stableUtf8Hash(value: string): string {
  let hash = 0xcbf29ce484222325n
  for (const byte of new TextEncoder().encode(value)) {
    hash ^= BigInt(byte)
    hash = BigInt.asUintN(64, hash * 0x100000001b3n)
  }
  return hash.toString(16).padStart(16, '0')
}

// Compact ordered audiobook identity sent across IPC instead of all chunk text.
// Delimiters and filtering must stay byte-for-byte aligned with Rust.
function createChunkSourceSignature(chunks: TtsChunk[]): string {
  const canonical = chunks
    .filter((chunk) => chunk.text.trim())
    .map((chunk) => {
      // Rust Option<String> values from imported metadata arrive as null, not undefined.
      const contentHash = typeof chunk.textHash === 'string' ? chunk.textHash : stableUtf8Hash(chunk.text)
      return `${chunk.id}\0${contentHash}\n`
    })
    .join('')
  return stableUtf8Hash(canonical)
}

let capabilitiesPromise: Promise<NativeTtsCapabilities> | null = null

export class NativeTtsError extends Error {
  readonly code: string

  constructor(code: string, message: string) {
    super(message)
    this.name = 'NativeTtsError'
    this.code = code
  }
}

export function isNativeTtsRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

export function resetNativeTtsCapabilities(): void {
  capabilitiesPromise = null
}

export async function getNativeTtsCapabilities(): Promise<NativeTtsCapabilities> {
  if (!capabilitiesPromise) {
    capabilitiesPromise = loadNativeTtsCapabilities()
  }
  return capabilitiesPromise
}

export async function requireNativeTtsCapabilities(): Promise<NativeTtsCapabilities> {
  const capabilities = await getNativeTtsCapabilities()
  if (!capabilities.available) {
    throw new NativeTtsError(
      'native-tts-unavailable',
      capabilities.reason || 'Native TTS is not available',
    )
  }
  return capabilities
}

export async function getNativeTtsModelStatus(modelId: string): Promise<NativeTtsModelStatus> {
  if (!isNativeTtsRuntime()) {
    return {
      modelId,
      installed: false,
      installing: false,
      installSupported: false,
      runtimeInstalled: false,
      modelDir: null,
      runtimeDir: null,
      sourceUrl: '',
      sourceLabel: 'sherpa-onnx offline TTS',
      archiveBytes: 0,
      installedBytes: 0,
      sha256: '',
      message: 'Native TTS is only available in the desktop or Android app.',
      runtimeMessage: 'Native TTS is only available in the desktop or Android app.',
    }
  }
  return invokeNative<NativeTtsModelStatus>('tts_model_status', { modelId })
}

export async function installNativeTtsModel(modelId: string): Promise<NativeTtsModelInstallResult> {
  const result = await invokeNative<NativeTtsModelInstallResult>('tts_install_model', { modelId })
  resetNativeTtsCapabilities()
  return result
}

export async function listenNativeTtsModelInstallProgress(
  handler: (progress: NativeTtsModelInstallProgress) => void,
): Promise<() => void> {
  if (!isNativeTtsRuntime()) return () => {}
  const mod = await import('@tauri-apps/api/event')
  return mod.listen<NativeTtsModelInstallProgress>(MODEL_INSTALL_PROGRESS_EVENT, (event) => {
    handler(event.payload)
  })
}
export async function getNativeAudiobookStatus(
  documentUrl: string,
  chunks: TtsChunk[],
  options: TtsOptions,
): Promise<NativeAudiobookStatus> {
  await requireNativeTtsCapabilities()
  return invokeNative<NativeAudiobookStatus>('tts_native_audiobook_status', {
    request: {
      audiobookId: createAudiobookId(documentUrl, options),
      sourceSignature: createChunkSourceSignature(chunks),
      totalChunks: chunks.filter((chunk) => chunk.text.trim()).length,
    },
  })
}

export async function listNativeSavedAudiobooks(): Promise<SavedAudiobookRecord[]> {
  if (!isNativeTtsRuntime()) return []
  return invokeNative<SavedAudiobookRecord[]>('tts_list_saved_audiobooks')
}


export async function prepareNativeAudiobookPlayback(
  documentUrl: string,
  chunks: TtsChunk[],
  options: TtsOptions,
): Promise<NativeAudiobookPlayback> {
  await requireNativeTtsCapabilities()
  return invokeNative<NativeAudiobookPlayback>('tts_prepare_native_audiobook_playback', {
    request: {
      audiobookId: createAudiobookId(documentUrl, options),
      sourceSignature: createChunkSourceSignature(chunks),
    },
  })
}

export async function getNativeSavedAudiobookChunk(
  documentUrl: string,
  chunk: TtsChunk,
  index: number,
  options: TtsOptions,
): Promise<NativeTtsChunkResult | null> {
  if (!isNativeTtsRuntime()) return null
  try {
    const response = await invokeNative<NativeTtsChunkResponse>('tts_get_native_audiobook_chunk', {
      request: {
        audiobookId: createAudiobookId(documentUrl, options),
        chunk: toNativeTtsChunk(chunk),
        index,
      },
    })
    return responseToChunkResult(chunk, response)
  } catch {
    return null
  }
}

export async function saveNativeAudiobook(
  input: {
    jobId: string
    documentUrl: string
    title: string
    chunks: TtsChunk[]
    options: TtsOptions
  },
): Promise<NativeAudiobookSaveResult> {
  await requireNativeTtsCapabilities()
  return invokeNative<NativeAudiobookSaveResult>('tts_save_audiobook_native', {
    request: {
      jobId: input.jobId,
      audiobookId: createAudiobookId(input.documentUrl, input.options),
      documentUrl: input.documentUrl,
      title: input.title,
      chunks: toNativeTtsChunks(input.chunks),
      modelId: input.options.modelId,
      textPreprocessor: resolveTextPreprocessor(input.options),
      voice: input.options.voice,
      speed: input.options.speed,
      threadCount: input.options.threadCount,
      silmaNfeStep: resolveSilmaNfeStep(input.options),
    },
  })
}

export async function cancelNativeAudiobookSave(jobId: string): Promise<void> {
  if (!isNativeTtsRuntime()) return
  await invokeNative('tts_cancel_audiobook_save', { jobId })
}

export async function exportNativeAudiobook(
  input: {
    documentUrl: string
    title: string
    sourceHtml?: string
    chunks: TtsChunk[]
    options: TtsOptions
    exportFormat?: NativeAudiobookExportFormat
  },
): Promise<NativeAudiobookExportResult> {
  await requireNativeTtsCapabilities()
  return invokeNative<NativeAudiobookExportResult>('tts_export_audiobook_native', {
    request: {
      audiobookId: createAudiobookId(input.documentUrl, input.options),
      documentUrl: input.documentUrl,
      title: input.title,
      sourceHtml: input.sourceHtml ?? null,
      chunks: toNativeTtsChunks(input.chunks),
      modelId: input.options.modelId,
      textPreprocessor: resolveTextPreprocessor(input.options),
      voice: input.options.voice,
      speed: input.options.speed,
      dtype: input.options.dtype ?? 'native',
      silmaNfeStep: resolveSilmaNfeStep(input.options),
      exportFormat: input.exportFormat ?? 'bundle',
    },
  })
}

export async function importNativeAudiobook(): Promise<NativeAudiobookImportResult> {
  await requireNativeTtsCapabilities()
  return invokeNative<NativeAudiobookImportResult>('tts_import_audiobook_native')
}

export async function deleteNativeAudiobook(input: {
  audiobookId: string
  documentUrl: string
  deleteUserUpload: boolean
}): Promise<NativeAudiobookDeleteResult> {
  await requireNativeTtsCapabilities()
  return invokeNative<NativeAudiobookDeleteResult>('tts_delete_audiobook_native', {
    request: input,
  })
}

export async function probeNativeSilmaSidecar(): Promise<NativeSilmaSidecarProbeResult> {
  // Dev-only bridge: validates the packaged worker can start and write app-owned WAVs.
  await requireNativeTtsCapabilities()
  return invokeNative<NativeSilmaSidecarProbeResult>('tts_probe_silma_sidecar')
}

export async function getImportedAudiobookSource(documentUrl: string): Promise<string> {
  return invokeNative<string>('tts_get_imported_audiobook_source', {
    request: { documentUrl },
  })
}

export async function getImportedAudiobookMetadata(documentUrl: string): Promise<NativeImportedAudiobookMetadata> {
  return invokeNative<NativeImportedAudiobookMetadata>('tts_get_imported_audiobook_metadata', {
    request: { documentUrl },
  })
}

export async function listenNativeAudiobookSaveProgress(
  handler: (progress: NativeAudiobookSaveProgress) => void,
): Promise<() => void> {
  if (!isNativeTtsRuntime()) return () => {}
  const mod = await import('@tauri-apps/api/event')
  return mod.listen<NativeAudiobookSaveProgress>(SAVE_PROGRESS_EVENT, (event) => {
    handler(event.payload)
  })
}

async function loadNativeTtsCapabilities(): Promise<NativeTtsCapabilities> {
  if (!isNativeTtsRuntime()) {
    return {
      available: false,
      backend: 'native-unavailable',
      reason: 'Native sherpa-onnx TTS is only available in the desktop or Android app.',
      platform: 'browser',
      compiledExecutionProviders: [],
      models: FALLBACK_TTS_MODELS,
      defaultThreadCount: 1,
      maxThreadCount: 1,
    }
  }

  try {
    return await invokeNative<NativeTtsCapabilities>('tts_native_capabilities')
  } catch (err) {
    return {
      available: false,
      backend: 'native-unavailable',
      reason: err instanceof Error ? err.message : String(err),
      platform: 'unknown',
      compiledExecutionProviders: [],
      models: FALLBACK_TTS_MODELS,
      defaultThreadCount: 1,
      maxThreadCount: 1,
    }
  }
}

async function loadTauriInvoke() {
  const mod = await import('@tauri-apps/api/core')
  return mod.invoke
}

// Normalize Tauri's object rejection into an Error while retaining its stable code.
async function invokeNative<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    const invoke = await loadTauriInvoke()
    return await invoke<T>(command, args)
  } catch (error) {
    throw toNativeTtsError(error)
  }
}

// Accept object errors from current Rust commands and plain strings from older builds.
function toNativeTtsError(error: unknown): NativeTtsError {
  if (error instanceof NativeTtsError) return error
  if (isNativeTtsErrorPayload(error)) return new NativeTtsError(error.code, error.message)
  if (error instanceof Error) return new NativeTtsError('native-tts-failed', error.message)
  if (typeof error === 'string') {
    try {
      const parsed: unknown = JSON.parse(error)
      if (isNativeTtsErrorPayload(parsed)) return new NativeTtsError(parsed.code, parsed.message)
    } catch {
      // Older native builds reject with a plain diagnostic string.
    }
    return new NativeTtsError('native-tts-failed', error)
  }
  return new NativeTtsError('native-tts-failed', String(error))
}

function isNativeTtsErrorPayload(value: unknown): value is { code: string; message: string } {
  if (!value || typeof value !== 'object') return false
  const payload = value as Record<string, unknown>
  return typeof payload.code === 'string' && typeof payload.message === 'string'
}

function responseToChunkResult(chunk: TtsChunk, response: NativeTtsChunkResponse): NativeTtsChunkResult {
  return {
    chunk,
    wav: base64ToArrayBuffer(response.wavBase64),
    sampleRate: response.sampleRate,
    audioDurationSec: response.audioDurationSec,
    wavBytes: response.wavBytes,
    generateMs: response.generateMs,
    backend: response.backend,
  }
}

function base64ToArrayBuffer(value: string): ArrayBuffer {
  const binary = atob(value)
  const bytes = new Uint8Array(binary.length)
  for (let index = 0; index < binary.length; index++) {
    bytes[index] = binary.charCodeAt(index)
  }
  return bytes.buffer
}
