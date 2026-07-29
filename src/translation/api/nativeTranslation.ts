export interface TranslationCapabilities {
  available: boolean
  backend: string
  reason: string
  platform: string
  defaultQualityMode: string
  hardwareAcceleration: {
    backend: string
    device: string
  } | null
  models: TranslationModelInfo[]
}

const TRANSLATION_MODEL_INSTALL_PROGRESS_EVENT = 'translation-model-install-progress'
const TRANSLATION_PROGRESS_EVENT = 'translation-progress'

export interface TranslationModelInfo {
  id: string
  name: string
  engine: string
  tier: string
  manifestState: string
  sourceLanguages: string[]
  targetLanguages: string[]
  defaultQualityMode: string
  recommendedPlatforms: string[]
  licenseNotes: string
  sizeNotes: string
  notes: string
}

export interface TranslationModelStatus {
  modelId: string
  installed: boolean
  installing: boolean
  modelDir?: string | null
  sourceUrl: string
  sourceLabel: string
  archiveBytes: number
  installedBytes: number
  sha256: string
  message: string
}

export interface TranslationModelInstallProgress {
  modelId: string
  status: 'starting' | 'downloading' | 'installed' | string
  message: string
  downloadedBytes: number
  totalBytes: number
  percent: number
}

export interface TranslationModelInstallResult {
  modelId: string
  modelDir: string
  bytes: number
}

export interface TranslationModelRemoveResult {
  modelId: string
  removed: boolean
  bytesFreed: number
}

export interface TranslationStartRequest {
  jobId?: string
  documentUrl: string
  sourceLanguage: string
  targetLanguage: string
  modelId: string
  qualityMode: string
  useHardwareAcceleration?: boolean
  repairMode?: 'off' | 'chapter'
  glossary?: TranslationGlossaryEntry[]
}

export interface TranslationGlossaryEntry {
  source: string
  target: string
  note?: string | null
}

export interface TranslationStartResult {
  jobId: string
  status: string
  message: string
}

export interface TranslationJobProgress {
  jobId: string
  status: 'loading-model' | 'starting' | 'translating' | 'completed' | 'cancelled' | string
  message: string
  modelId: string
  elapsedMs: number
  currentHeading: string | null
  completedSegments: number
  totalSegments: number
  cachedSegments: number
  translatedSegments: number
  reusedSegmentsInBatch: number
  completedBatches: number
  totalBatches: number
  percent: number
  preview: string
}

export interface TranslatedDocumentInfo {
  id: string
  documentUrl: string
  sourceDocumentUrl: string
  title: string
  sourceLanguage: string
  targetLanguage: string
  modelId: string
  status: string
  createdAtMs: number
  updatedAtMs: number
}

export interface TranslationDeleteResult {
  id: string
  deleted: boolean
  bytesFreed: number
  message: string
}

let capabilitiesPromise: Promise<TranslationCapabilities> | null = null

export class TranslationError extends Error {
  readonly code: string

  constructor(code: string, message: string) {
    super(message)
    this.name = 'TranslationError'
    this.code = code
  }
}

export function isNativeTranslationRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}

export function resetTranslationCapabilities(): void {
  capabilitiesPromise = null
}

export async function getTranslationCapabilities(): Promise<TranslationCapabilities> {
  if (!capabilitiesPromise) capabilitiesPromise = loadTranslationCapabilities()
  return capabilitiesPromise
}

export async function getTranslationModelStatus(modelId: string): Promise<TranslationModelStatus> {
  if (!isNativeTranslationRuntime()) {
    return {
      modelId,
      installed: false,
      installing: false,
      modelDir: null,
      sourceUrl: '',
      sourceLabel: 'Offline translation model catalog',
      archiveBytes: 0,
      installedBytes: 0,
      sha256: '',
      message: 'Offline translation is only available in the desktop or Android app.',
    }
  }
  return invokeTranslation<TranslationModelStatus>('translation_model_status', { request: { modelId } })
}

export async function installTranslationModel(modelId: string): Promise<TranslationModelInstallResult> {
  if (!isNativeTranslationRuntime()) {
    throw unavailableError()
  }
  return invokeTranslation<TranslationModelInstallResult>('translation_install_model', { modelId })
}

export async function removeTranslationModel(modelId: string): Promise<TranslationModelRemoveResult> {
  if (!isNativeTranslationRuntime()) {
    throw unavailableError()
  }
  return invokeTranslation<TranslationModelRemoveResult>('translation_remove_model', { modelId })
}

export async function listenTranslationModelInstallProgress(
  handler: (progress: TranslationModelInstallProgress) => void,
): Promise<() => void> {
  if (!isNativeTranslationRuntime()) return () => {}
  const mod = await import('@tauri-apps/api/event')
  return mod.listen<TranslationModelInstallProgress>(TRANSLATION_MODEL_INSTALL_PROGRESS_EVENT, (event) => {
    handler(event.payload)
  })
}

export async function listenTranslationProgress(
  handler: (progress: TranslationJobProgress) => void,
): Promise<() => void> {
  if (!isNativeTranslationRuntime()) return () => {}
  const mod = await import('@tauri-apps/api/event')
  return mod.listen<TranslationJobProgress>(TRANSLATION_PROGRESS_EVENT, (event) => {
    handler(event.payload)
  })
}

export async function startTranslationJob(request: TranslationStartRequest): Promise<TranslationStartResult> {
  if (!isNativeTranslationRuntime()) {
    throw unavailableError()
  }
  return invokeTranslation<TranslationStartResult>('translation_start', { request })
}

export async function cancelTranslationJob(jobId: string): Promise<void> {
  if (!isNativeTranslationRuntime()) return
  await invokeTranslation('translation_cancel', { request: { jobId } })
}

export async function listTranslatedDocuments(): Promise<TranslatedDocumentInfo[]> {
  if (!isNativeTranslationRuntime()) return []
  return invokeTranslation<TranslatedDocumentInfo[]>('translation_list_documents')
}

export async function deleteTranslatedDocument(id: string): Promise<TranslationDeleteResult> {
  if (!isNativeTranslationRuntime()) {
    return {
      id,
      deleted: false,
      bytesFreed: 0,
      message: 'Offline translation is only available in the desktop or Android app.',
    }
  }
  return invokeTranslation<TranslationDeleteResult>('translation_delete_document', { request: { id } })
}

async function loadTranslationCapabilities(): Promise<TranslationCapabilities> {
  if (!isNativeTranslationRuntime()) {
    return {
      available: false,
      backend: 'translation-unavailable',
      reason: 'Offline translation is only available in the desktop or Android app.',
      platform: 'browser',
      defaultQualityMode: 'balanced',
      hardwareAcceleration: null,
      models: [],
    }
  }

  try {
    return await invokeTranslation<TranslationCapabilities>('translation_capabilities')
  } catch (err) {
    return {
      available: false,
      backend: 'translation-unavailable',
      reason: err instanceof Error ? err.message : String(err),
      platform: 'unknown',
      defaultQualityMode: 'balanced',
      hardwareAcceleration: null,
      models: [],
    }
  }
}

async function loadTauriInvoke(): Promise<<T>(cmd: string, args?: Record<string, unknown>) => Promise<T>> {
  const mod = await import('@tauri-apps/api/core')
  return mod.invoke
}

async function invokeTranslation<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    const invoke = await loadTauriInvoke()
    return await invoke<T>(command, args)
  } catch (error) {
    throw toTranslationError(error)
  }
}

function unavailableError(): TranslationError {
  return new TranslationError(
    'translation-unavailable',
    'Offline translation is only available in the desktop or Android app.',
  )
}

// Accept object errors from current Rust commands and plain strings from older builds.
function toTranslationError(error: unknown): TranslationError {
  if (error instanceof TranslationError) return error
  if (isTranslationErrorPayload(error)) return new TranslationError(error.code, error.message)
  if (error instanceof Error) return new TranslationError('translation-failed', error.message)
  if (typeof error === 'string') {
    try {
      const parsed: unknown = JSON.parse(error)
      if (isTranslationErrorPayload(parsed)) return new TranslationError(parsed.code, parsed.message)
    } catch {
      // Older native builds reject with a plain diagnostic string.
    }
    return new TranslationError('translation-failed', error)
  }
  return new TranslationError('translation-failed', String(error))
}

function isTranslationErrorPayload(value: unknown): value is { code: string; message: string } {
  if (!value || typeof value !== 'object') return false
  const payload = value as Record<string, unknown>
  return typeof payload.code === 'string' && typeof payload.message === 'string'
}
