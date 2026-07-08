import {
  TTS_AUDIO_CACHE_VERSION,
  NATIVE_TTS_DTYPE,
  DEFAULT_TTS_MODEL_ID,
  resolveTtsDtype,
  resolveTextPreprocessor,
  type TtsOptions,
  TEXT_PREPROCESSOR_NONE,
} from '../types'

// This registry stores completed-audiobook metadata only. Generated WAV
// chunks live in native app data under the audiobook cache directory.
const STORAGE_KEY = 'papercut.savedAudiobooks.v1'

export interface SavedAudiobookRecord {
  id: string
  documentUrl: string
  title: string
  voice: string
  speed: number
  modelId: string
  textPreprocessor: string
  cacheVersion?: string
  dtype: string
  savedAt: number
  chunks: number
  audioDurationSec?: number
  wavBytes?: number
}

export function createAudiobookId(documentUrl: string, options: TtsOptions): string {
  const dtype = resolveTtsDtype(options)
  const textPreprocessor = resolveTextPreprocessor(options)
  // Include model identity and playback options so saved-audio records invalidate
  // when the selected model, dtype, voice, speed, or source document changes.
  const parts = [
    options.modelId,
    TTS_AUDIO_CACHE_VERSION,
    dtype,
    options.voice,
    options.speed.toFixed(2),
  ]
  if (textPreprocessor !== TEXT_PREPROCESSOR_NONE) parts.push(textPreprocessor)
  parts.push(normalizeDocumentUrl(documentUrl))
  return parts.join('|')
}

export function getSavedAudiobooks(): SavedAudiobookRecord[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw)
    return Array.isArray(parsed)
      ? parsed.filter(isSavedAudiobookRecord).map((record) => ({
        ...record,
        modelId: record.modelId || DEFAULT_TTS_MODEL_ID,
        cacheVersion: record.cacheVersion || 'legacy',
        dtype: record.dtype || NATIVE_TTS_DTYPE,
        textPreprocessor: record.textPreprocessor ?? TEXT_PREPROCESSOR_NONE,
      }))
      .filter((record) => record.cacheVersion === TTS_AUDIO_CACHE_VERSION)
      : []
  } catch {
    return []
  }
}

export function markAudiobookSaved(record: Omit<SavedAudiobookRecord, 'id' | 'savedAt'>): SavedAudiobookRecord {
  const dtype = record.dtype || NATIVE_TTS_DTYPE
  const saved: SavedAudiobookRecord = {
    ...record,
    id: createAudiobookId(record.documentUrl, {
      modelId: record.modelId,
      voice: record.voice as TtsOptions['voice'],
      speed: record.speed,
      dtype: dtype as TtsOptions['dtype'],
      textPreprocessor: record.textPreprocessor,
    }),
    modelId: record.modelId,
    cacheVersion: TTS_AUDIO_CACHE_VERSION,
    dtype,
    textPreprocessor: record.textPreprocessor ?? TEXT_PREPROCESSOR_NONE,
    savedAt: Date.now(),
  }

  const records = getSavedAudiobooks().filter((item) => item.id !== saved.id)
  records.push(saved)
  writeSavedAudiobooks(records)
  return saved
}

export function removeSavedAudiobook(id: string): void {
  const records = getSavedAudiobooks().filter((item) => item.id !== id)
  writeSavedAudiobooks(records)
}

export function hasSavedAudiobook(documentUrl: string, options: TtsOptions): boolean {
  const id = createAudiobookId(documentUrl, options)
  return getSavedAudiobooks().some((item) => item.id === id)
}

function normalizeDocumentUrl(documentUrl: string): string {
  try {
    return new URL(documentUrl, window.location.href).pathname
  } catch {
    return documentUrl.split('#')[0].split('?')[0]
  }
}

function writeSavedAudiobooks(records: SavedAudiobookRecord[]): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(records))
  } catch {
    // Saved-audio metadata is best effort; native audio files are managed separately.
  }
}

function isSavedAudiobookRecord(value: unknown): value is SavedAudiobookRecord {
  if (!value || typeof value !== 'object') return false
  const record = value as Partial<SavedAudiobookRecord>
  return typeof record.id === 'string' &&
    typeof record.documentUrl === 'string' &&
    typeof record.title === 'string' &&
    typeof record.voice === 'string' &&
    typeof record.speed === 'number' &&
    (typeof record.modelId === 'string' || record.modelId === undefined) &&
    (typeof record.cacheVersion === 'string' || record.cacheVersion === undefined) &&
    (typeof record.textPreprocessor === 'string' || record.textPreprocessor === undefined) &&
    typeof record.savedAt === 'number' &&
    typeof record.chunks === 'number' &&
    (typeof record.audioDurationSec === 'number' || record.audioDurationSec === undefined) &&
    (typeof record.wavBytes === 'number' || record.wavBytes === undefined)
}
