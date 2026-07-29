import {
  TTS_AUDIO_CACHE_VERSION,
  DEFAULT_SILMA_NFE_STEP,
  resolveTtsDtype,
  resolveSilmaNfeStep,
  resolveTextPreprocessor,
  SILMA_MODEL_ID,
  type TtsOptions,
  TEXT_PREPROCESSOR_NONE,
} from '../types'

export interface SavedAudiobookRecord {
  id: string
  documentUrl: string
  title: string
  voice: string
  speed: number
  modelId: string
  textPreprocessor: string
  silmaNfeStep?: number
  cacheVersion?: string
  dtype: string
  savedAt: number
  chunks: number
  audioDurationSec?: number
  wavBytes?: number
}

export function getSavedAudiobooksForDocument(
  records: SavedAudiobookRecord[],
  documentUrl: string,
): SavedAudiobookRecord[] {
  return records.filter((record) => record.documentUrl === documentUrl)
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
  const silmaNfeStep = resolveSilmaNfeStep(options)
  if (options.modelId === SILMA_MODEL_ID && silmaNfeStep !== DEFAULT_SILMA_NFE_STEP) {
    parts.push('nfe' + silmaNfeStep)
  }
  parts.push(normalizeDocumentUrl(documentUrl))
  return parts.join('|')
}

function normalizeDocumentUrl(documentUrl: string): string {
  try {
    return new URL(documentUrl, window.location.href).pathname
  } catch {
    return documentUrl.split('#')[0].split('?')[0]
  }
}
