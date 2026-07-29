import { DEFAULT_TTS_MODEL_ID, NATIVE_TTS_DTYPE, resolveSilmaNfeStep, SILMA_MODEL_ID, TEXT_PREPROCESSOR_NONE } from '../types'

const STORAGE_KEY = 'papercut.userUploads.v1'

export interface UserUploadDocument {
  url: string
  title: string
  importedAt: number
  modelId: string
  textPreprocessor: string
  voice: string
  speed: number
  dtype: string
  silmaNfeStep?: number
  chunks: number
  audioDurationSec?: number
  wavBytes?: number
}

export function isUserUploadUrl(url: string): boolean {
  return /^\/user-uploads\/[a-fA-F0-9]+\.html(?:[#?].*)?$/.test(url)
}

export function getUserUploads(): UserUploadDocument[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw)
    return Array.isArray(parsed)
      ? parsed
        .filter(isUserUploadDocument)
        .map((record) => ({
          ...record,
          modelId: record.modelId ?? DEFAULT_TTS_MODEL_ID,
          textPreprocessor: record.textPreprocessor ?? TEXT_PREPROCESSOR_NONE,
          silmaNfeStep: record.modelId === SILMA_MODEL_ID ? resolveSilmaNfeStep(record) : record.silmaNfeStep,
        }))
        .sort((a, b) => b.importedAt - a.importedAt)
      : []
  } catch {
    return []
  }
}

export function upsertUserUpload(input: Omit<UserUploadDocument, 'importedAt'>): UserUploadDocument {
  const upload: UserUploadDocument = {
    ...input,
    modelId: input.modelId || DEFAULT_TTS_MODEL_ID,
    dtype: input.dtype || NATIVE_TTS_DTYPE,
    textPreprocessor: input.textPreprocessor ?? TEXT_PREPROCESSOR_NONE,
    silmaNfeStep: input.modelId === SILMA_MODEL_ID ? resolveSilmaNfeStep(input) : input.silmaNfeStep,
    importedAt: Date.now(),
  }
  const records = getUserUploads().filter((record) => record.url !== upload.url)
  records.push(upload)
  writeUserUploads(records)
  return upload
}

export function removeUserUpload(url: string): void {
  const records = getUserUploads().filter((record) => record.url !== url)
  writeUserUploads(records)
}

function writeUserUploads(records: UserUploadDocument[]): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(records))
  } catch {
    // Metadata writes are best effort; native imported files still remain on disk if storage is unavailable.
  }
}

function isUserUploadDocument(value: unknown): value is UserUploadDocument {
  if (!value || typeof value !== 'object') return false
  const record = value as Partial<UserUploadDocument>
  return typeof record.url === 'string' &&
    typeof record.title === 'string' &&
    typeof record.importedAt === 'number' &&
    (typeof record.modelId === 'string' || record.modelId === undefined) &&
    (typeof record.textPreprocessor === 'string' || record.textPreprocessor === undefined) &&
    typeof record.voice === 'string' &&
    typeof record.speed === 'number' &&
    typeof record.dtype === 'string' &&
    typeof record.chunks === 'number' &&
    (typeof record.silmaNfeStep === 'number' || record.silmaNfeStep === undefined) &&
    (typeof record.audioDurationSec === 'number' || record.audioDurationSec === undefined) &&
    (typeof record.wavBytes === 'number' || record.wavBytes === undefined)
}
