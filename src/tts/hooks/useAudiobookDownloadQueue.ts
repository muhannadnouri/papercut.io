import { useCallback, useEffect, useRef, useState } from 'react'
import i18n from '../../i18n'
import {
  clearCompletedAudiobookDownload,
  getAudiobookDownloads,
  removeAudiobookDownload,
  upsertAudiobookDownload,
  type AudiobookDownloadInput,
  type AudiobookDownloadRecord,
} from '../storage/AudiobookDownloadQueue'
import type { TtsChunk, TtsDtype, TtsVoice } from '../types'
import { useAudiobookCache } from './useAudiobookCache'

export interface ActiveAudiobookDownload {
  title: string
  url: string
  modelId: string
  textPreprocessor: string
  voice: TtsVoice
  speed: number
  dtype: TtsDtype
  silmaNfeStep?: number
}

export interface StartAudiobookDownloadInput {
  documentUrl: string
  title: string
  modelId: string
  textPreprocessor: string
  chunks: TtsChunk[]
  voice: TtsVoice
  speed: number
  silmaNfeStep?: number
  dtype: TtsDtype
}

interface AudiobookDownloadQueueOptions {
  threadCount: number
  onCompleted: () => void | Promise<void>
}

const PERSIST_DELAY_MS = 1200

/** Owns the active native save and its resumable local queue record. */
export function useAudiobookDownloadQueue({
  threadCount,
  onCompleted,
}: AudiobookDownloadQueueOptions) {
  const [downloads, setDownloads] = useState<AudiobookDownloadRecord[]>(() => getAudiobookDownloads())
  const [activeDownload, setActiveDownload] = useState<ActiveAudiobookDownload | null>(null)
  const pendingPersistRef = useRef<AudiobookDownloadInput | null>(null)
  const persistTimerRef = useRef<number | null>(null)
  const { state, save, cancel: cancelSave } = useAudiobookCache()

  const refresh = useCallback(() => {
    setDownloads(getAudiobookDownloads())
  }, [])

  /** Flushes the latest progress snapshot; intermediate native events are intentionally coalesced. */
  const flushPersist = useCallback(() => {
    if (persistTimerRef.current !== null) {
      window.clearTimeout(persistTimerRef.current)
      persistTimerRef.current = null
    }

    const pending = pendingPersistRef.current
    if (!pending) return

    pendingPersistRef.current = null
    upsertAudiobookDownload(pending)
    refresh()
  }, [refresh])

  const schedulePersist = useCallback((input: AudiobookDownloadInput, immediate = false) => {
    pendingPersistRef.current = input
    if (immediate) {
      flushPersist()
      return
    }

    if (persistTimerRef.current !== null) return
    persistTimerRef.current = window.setTimeout(flushPersist, PERSIST_DELAY_MS)
  }, [flushPersist])

  const start = useCallback((input: StartAudiobookDownloadInput) => {
    const speakableChunks = input.chunks.filter((chunk) => chunk.text.trim())
    if (speakableChunks.length === 0) return

    // Persist before invoking native code so a process interruption still leaves a resumable job.
    schedulePersist({
      documentUrl: input.documentUrl,
      title: input.title,
      modelId: input.modelId,
      textPreprocessor: input.textPreprocessor,
      voice: input.voice,
      speed: input.speed,
      silmaNfeStep: input.silmaNfeStep,
      dtype: input.dtype,
      status: 'queued',
      cachedChunks: 0,
      totalChunks: speakableChunks.length,
      message: i18n.t('tts.status.queued'),
      audioDurationSec: 0,
    }, true)
    setActiveDownload({
      title: input.title,
      url: input.documentUrl,
      modelId: input.modelId,
      textPreprocessor: input.textPreprocessor,
      voice: input.voice,
      speed: input.speed,
      dtype: input.dtype,
      silmaNfeStep: input.silmaNfeStep,
    })
    save(input.chunks, {
      modelId: input.modelId,
      textPreprocessor: input.textPreprocessor,
      voice: input.voice,
      speed: input.speed,
      dtype: input.dtype,
      threadCount,
      silmaNfeStep: input.silmaNfeStep,
      documentUrl: input.documentUrl,
      title: input.title,
    })
  }, [save, schedulePersist, threadCount])

  const cancel = useCallback(() => {
    if (activeDownload) {
      schedulePersist(downloadInput(activeDownload, {
        status: 'paused',
        cachedChunks: state.cachedChunks,
        totalChunks: state.totalChunks,
        message: i18n.t('tts.status.pausedReady'),
        audioDurationSec: state.audioDurationSec,
        wavBytes: state.wavBytes,
      }), true)
    }
    cancelSave()
  }, [activeDownload, cancelSave, schedulePersist, state.audioDurationSec, state.cachedChunks, state.totalChunks, state.wavBytes])

  const remove = useCallback((id: string) => {
    removeAudiobookDownload(id)
    refresh()
  }, [refresh])

  useEffect(() => {
    if (!activeDownload || state.complete) return

    if (state.status === 'checking' || state.status === 'saving') {
      // eslint-disable-next-line react-hooks/set-state-in-effect
      schedulePersist(downloadInput(activeDownload, {
        status: 'saving',
        cachedChunks: state.cachedChunks,
        totalChunks: state.totalChunks,
        message: state.message,
        audioDurationSec: state.audioDurationSec,
        wavBytes: state.wavBytes,
      }))
      return
    }

    if (state.status === 'partial') {
      schedulePersist(downloadInput(activeDownload, {
        status: 'paused',
        cachedChunks: state.cachedChunks,
        totalChunks: state.totalChunks,
        message: state.message || i18n.t('tts.status.readyToResume'),
        audioDurationSec: state.audioDurationSec,
        wavBytes: state.wavBytes,
      }), true)
      return
    }

    if (state.status === 'error') {
      schedulePersist(downloadInput(activeDownload, {
        status: 'error',
        cachedChunks: state.cachedChunks,
        totalChunks: state.totalChunks,
        message: state.message,
        audioDurationSec: state.audioDurationSec,
        wavBytes: state.wavBytes,
      }), true)
    }
  }, [activeDownload, schedulePersist, state])

  useEffect(() => {
    if (!state.complete || !activeDownload) return
    // eslint-disable-next-line react-hooks/set-state-in-effect
    flushPersist()
    clearCompletedAudiobookDownload(activeDownload.url, activeDownload)
    void onCompleted()
    refresh()
  }, [activeDownload, flushPersist, onCompleted, refresh, state.complete])

  useEffect(() => {
    function flushPendingDownload() {
      flushPersist()
    }

    document.addEventListener('visibilitychange', flushPendingDownload)
    window.addEventListener('pagehide', flushPendingDownload)
    return () => {
      document.removeEventListener('visibilitychange', flushPendingDownload)
      window.removeEventListener('pagehide', flushPendingDownload)
      flushPersist()
    }
  }, [flushPersist])

  return { activeDownload, downloads, state, start, cancel, remove }
}

function downloadInput(
  active: ActiveAudiobookDownload,
  progress: Pick<AudiobookDownloadInput, 'status' | 'cachedChunks' | 'totalChunks' | 'message' | 'audioDurationSec' | 'wavBytes'>,
): AudiobookDownloadInput {
  return {
    documentUrl: active.url,
    title: active.title,
    modelId: active.modelId,
    textPreprocessor: active.textPreprocessor,
    voice: active.voice,
    speed: active.speed,
    silmaNfeStep: active.silmaNfeStep,
    dtype: active.dtype,
    ...progress,
  }
}
