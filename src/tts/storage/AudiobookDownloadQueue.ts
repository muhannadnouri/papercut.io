import { createAudiobookId } from './AudiobookLibrary'
import { DEFAULT_TTS_MODEL_ID, TTS_AUDIO_CACHE_VERSION, NATIVE_TTS_DTYPE, TEXT_PREPROCESSOR_NONE, type TtsDtype, type TtsOptions, type TtsVoice } from '../types'

const STORAGE_KEY = 'papercut.audiobookDownloads.v1'
const SESSION_ID = crypto.randomUUID?.() ?? String(Date.now())

export type AudiobookDownloadStatus = 'queued' | 'saving' | 'paused' | 'error'

export interface AudiobookDownloadRecord {
  id: string
  documentUrl: string
  title: string
  modelId: string
  textPreprocessor: string
  voice: TtsVoice
  speed: number
  silmaNfeStep?: number
  dtype: TtsDtype
  cacheVersion?: string
  status: AudiobookDownloadStatus
  cachedChunks: number
  totalChunks: number
  message: string
  audioDurationSec?: number
  wavBytes?: number
  createdAt: number
  updatedAt: number
  sessionId: string | null
}

export interface AudiobookDownloadInput {
  documentUrl: string
  title: string
  modelId: string
  textPreprocessor?: string
  voice: TtsVoice
  speed: number
  silmaNfeStep?: number
  dtype?: TtsDtype
  status: AudiobookDownloadStatus
  cachedChunks?: number
  totalChunks?: number
  message?: string
  audioDurationSec?: number
  wavBytes?: number
}

export function createAudiobookDownloadId(documentUrl: string, options: TtsOptions): string {
  return createAudiobookId(documentUrl, options)
}

export function getAudiobookDownloads(): AudiobookDownloadRecord[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return []

    const parsed = JSON.parse(raw)
    if (!Array.isArray(parsed)) return []

    return parsed
      .filter(isAudiobookDownloadRecord)
      .map((record) => ({
        ...record,
        modelId: record.modelId ?? DEFAULT_TTS_MODEL_ID,
        dtype: record.dtype ?? NATIVE_TTS_DTYPE,
        textPreprocessor: record.textPreprocessor ?? TEXT_PREPROCESSOR_NONE,
        cacheVersion: record.cacheVersion ?? 'legacy',
        sessionId: record.sessionId ?? null,
      }))
      .filter((record) => record.cacheVersion === TTS_AUDIO_CACHE_VERSION)
      .map(normalizeInterruptedDownload)
      .sort((a, b) => b.updatedAt - a.updatedAt)
  } catch {
    return []
  }
}

export function upsertAudiobookDownload(input: AudiobookDownloadInput): AudiobookDownloadRecord {
  const now = Date.now()
  const id = createAudiobookDownloadId(input.documentUrl, {
    modelId: input.modelId,
    voice: input.voice,
    speed: input.speed,
    silmaNfeStep: input.silmaNfeStep,
    dtype: input.dtype ?? NATIVE_TTS_DTYPE,
    textPreprocessor: input.textPreprocessor,
  })
  const existing = getAudiobookDownloads().find((item) => item.id === id)
  const record: AudiobookDownloadRecord = {
    id,
    documentUrl: input.documentUrl,
    title: input.title,
    modelId: input.modelId,
    voice: input.voice,
    speed: input.speed,
    silmaNfeStep: input.silmaNfeStep,
    dtype: input.dtype ?? existing?.dtype ?? NATIVE_TTS_DTYPE,
    textPreprocessor: input.textPreprocessor ?? existing?.textPreprocessor ?? TEXT_PREPROCESSOR_NONE,
    cacheVersion: TTS_AUDIO_CACHE_VERSION,
    status: input.status,
    cachedChunks: input.cachedChunks ?? existing?.cachedChunks ?? 0,
    totalChunks: input.totalChunks ?? existing?.totalChunks ?? 0,
    message: input.message ?? existing?.message ?? '',
    audioDurationSec: input.audioDurationSec ?? existing?.audioDurationSec,
    wavBytes: input.wavBytes ?? existing?.wavBytes,
    createdAt: existing?.createdAt ?? now,
    updatedAt: now,
    sessionId: SESSION_ID,
  }

  const next = getAudiobookDownloads().filter((item) => item.id !== id)
  next.push(record)
  writeDownloads(next)
  return record
}

export function removeAudiobookDownload(id: string): void {
  writeDownloads(getAudiobookDownloads().filter((item) => item.id !== id))
}

export function clearCompletedAudiobookDownload(documentUrl: string, options: TtsOptions): void {
  removeAudiobookDownload(createAudiobookDownloadId(documentUrl, options))
}

function normalizeInterruptedDownload(record: AudiobookDownloadRecord): AudiobookDownloadRecord {
  if (record.status !== 'saving' && record.status !== 'queued') return record
  if (record.sessionId === SESSION_ID) return record

  return {
    ...record,
    status: 'paused',
    message: record.cachedChunks > 0
      ? 'Interrupted. Ready to resume from saved chunks.'
      : 'Interrupted. Ready to resume.',
  }
}

function writeDownloads(records: AudiobookDownloadRecord[]): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(records))
  } catch {
    // Queue metadata is best effort; active native saves should not crash if storage is unavailable.
  }
}

function isAudiobookDownloadRecord(value: unknown): value is AudiobookDownloadRecord {
  if (!value || typeof value !== 'object') return false
  const record = value as Partial<AudiobookDownloadRecord>
  return typeof record.id === 'string' &&
    typeof record.documentUrl === 'string' &&
    typeof record.title === 'string' &&
    (typeof record.modelId === 'string' || record.modelId === undefined) &&
    typeof record.voice === 'string' &&
    typeof record.speed === 'number' &&
    (typeof record.dtype === 'string' || record.dtype === undefined) &&
    (typeof record.cacheVersion === 'string' || record.cacheVersion === undefined) &&
    (typeof record.textPreprocessor === 'string' || record.textPreprocessor === undefined) &&
    (typeof record.silmaNfeStep === 'number' || record.silmaNfeStep === undefined) &&
    isDownloadStatus(record.status) &&
    typeof record.cachedChunks === 'number' &&
    typeof record.totalChunks === 'number' &&
    typeof record.message === 'string' &&
    (typeof record.audioDurationSec === 'number' || record.audioDurationSec === undefined) &&
    (typeof record.wavBytes === 'number' || record.wavBytes === undefined) &&
    typeof record.createdAt === 'number' &&
    typeof record.updatedAt === 'number' &&
    (typeof record.sessionId === 'string' || record.sessionId === null || record.sessionId === undefined)
}

function isDownloadStatus(value: unknown): value is AudiobookDownloadStatus {
  return value === 'queued' || value === 'saving' || value === 'paused' || value === 'error'
}
