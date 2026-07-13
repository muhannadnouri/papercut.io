import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type { DocumentInfo, SearchResult } from '../../types/search'
import type { UploadedDocument } from '../../uploads/DocumentUploads'
import {
  createAudiobookId,
  getSavedAudiobooks,
  markAudiobookSaved,
  removeSavedAudiobook,
  type SavedAudiobookRecord,
} from '../storage/AudiobookLibrary'
import {
  clearCompletedAudiobookDownload,
  createAudiobookDownloadId,
  getAudiobookDownloads,
  removeAudiobookDownload,
  upsertAudiobookDownload,
  type AudiobookDownloadInput,
  type AudiobookDownloadRecord,
} from '../storage/AudiobookDownloadQueue'
import { getAudioPreferences, saveAudioPreferences } from '../storage/audioPreferences'
import { FALLBACK_TTS_MODELS, getTtsModel, getTtsVoiceName, suggestTtsModel } from '../models'
import { formatAudiobookExportMessage, formatDuration, formatSpeedLabel, formatStorageSize } from '../utils/format'
import {
  deleteNativeAudiobook,
  exportNativeAudiobook,
  getNativeTtsCapabilities,
  getNativeTtsModelStatus,
  getImportedAudiobookMetadata,
  importNativeAudiobook,
  installNativeTtsModel,
  listenNativeTtsModelInstallProgress,
  probeNativeSilmaSidecar,
  type NativeTtsCapabilities,
  type NativeAudiobookExportFormat,
  type NativeTtsModelInstallProgress,
  type NativeTtsModelStatus,
} from '../api/nativeTts'
import { chunkAudiobookSaveHtmlWithSpans, type SpeechChunk } from '../utils/text'
import { DEFAULT_TTS_SPEED, type TextPreprocessorId, type TtsDtype, type TtsVoice, type TtsChunk } from '../types'
import { isUserUploadUrl, removeUserUpload, upsertUserUpload, type UserUploadDocument } from '../storage/UserUploads'
import { logTtsDiagnostic } from '../diagnostics/TtsDiagnostics'
import { useAudiobookCache } from './useAudiobookCache'
import { useTtsPlayer } from './useTtsPlayer'
import { useAppConfirmation } from '../../components/AppDialog/useAppConfirmation'

type ImportedHighlightStatus = 'idle' | 'preparing' | 'ready' | 'unavailable'
type AudiobookExportState = { id: string; status: 'exporting' | 'exported' | 'cancelled' | 'error'; message: string }
type AudiobookNoticeState = { id: string; status: 'success' | 'cancelled' | 'error'; message: string }

const AUDIOBOOK_NOTICE_TIMEOUT_MS = 10000

interface AudiobookManagerOptions {
  allDocuments: DocumentInfo[]
  docContent: string
  loadHtmlDocument: (url: string) => Promise<string>
  selectedDoc: string | null
  uploadedDocuments: UploadedDocument[]
  userUploads: UserUploadDocument[]
  onClearDocument: () => void
  onUserUploadsChanged: () => void
}

export function useAudiobookManager({
  allDocuments,
  docContent,
  loadHtmlDocument,
  selectedDoc,
  uploadedDocuments,
  userUploads,
  onClearDocument,
  onUserUploadsChanged,
}: AudiobookManagerOptions) {
  const initialAudioPreferences = getAudioPreferences()
  const [ttsModelId, setTtsModelIdState] = useState(initialAudioPreferences.modelId)
  const [ttsVoice, setTtsVoice] = useState<TtsVoice>(initialAudioPreferences.voice)
  const [ttsSpeed, setTtsSpeed] = useState(DEFAULT_TTS_SPEED)
  const [ttsPlaybackRate, setTtsPlaybackRate] = useState(initialAudioPreferences.playbackRate)
  const [ttsWordHighlightEnabled, setTtsWordHighlightEnabled] = useState(initialAudioPreferences.wordHighlightEnabled)
  const [ttsTextPreprocessor, setTtsTextPreprocessor] = useState<TextPreprocessorId>(initialAudioPreferences.textPreprocessor)
  const [ttsThreadCount, setTtsThreadCount] = useState(1)
  const [ttsCapabilities, setTtsCapabilities] = useState<NativeTtsCapabilities | null>(null)
  const ttsDtype: TtsDtype = initialAudioPreferences.dtype
  const [ttsSaveChunks, setTtsSaveChunks] = useState<TtsChunk[] | null>(null)
  const [importedHighlightStatus, setImportedHighlightStatus] = useState<ImportedHighlightStatus>('idle')
  const [ttsModelStatus, setTtsModelStatus] = useState<NativeTtsModelStatus | null>(null)
  const [ttsModelProgress, setTtsModelProgress] = useState<NativeTtsModelInstallProgress | null>(null)
  const [savedAudiobooks, setSavedAudiobooks] = useState<SavedAudiobookRecord[]>(() => getSavedAudiobooks())
  const [audioSavedOnly, setAudioSavedOnly] = useState(initialAudioPreferences.audioSavedOnly)
  const [audiobookDownloads, setAudiobookDownloads] = useState<AudiobookDownloadRecord[]>(() => getAudiobookDownloads())
  const [audiobookDownload, setAudiobookDownload] = useState<{ title: string; url: string; modelId: string; textPreprocessor: string; voice: TtsVoice; speed: number; dtype: TtsDtype } | null>(null)
  const [audiobookExport, setAudiobookExport] = useState<AudiobookExportState | null>(null)
  const [audiobookDelete, setAudiobookDelete] = useState<{ id: string; status: 'deleting' | 'deleted' | 'error'; message: string } | null>(null)
  const [audiobookImport, setAudiobookImport] = useState<{ status: 'idle' | 'importing' | 'imported' | 'cancelled' | 'error'; message: string }>({ status: 'idle', message: '' })
  const [silmaProbeRunning, setSilmaProbeRunning] = useState(false)
  const [audiobookNotice, setAudiobookNotice] = useState<AudiobookNoticeState | null>(null)
  const { confirm: confirmAudiobookAction, dialog: confirmationDialog } = useAppConfirmation()
  const ttsModels = ttsCapabilities?.models.length ? ttsCapabilities.models : FALLBACK_TTS_MODELS
  const selectedTtsModel = getTtsModel(ttsModels, ttsModelId)
  const pendingDownloadPersistRef = useRef<AudiobookDownloadInput | null>(null)
  const downloadPersistTimerRef = useRef<number | null>(null)
  const audiobookNoticeTimerRef = useRef<number | null>(null)
  const autoSelectedDocumentRef = useRef<string | null>(null)
  const preserveGeneratedSpeedOnOpenRef = useRef(false)
  const ttsModelIdRef = useRef(ttsModelId)
  const setTtsModelId = useCallback((modelId: string) => {
    ttsModelIdRef.current = modelId
    setTtsModelIdState(modelId)
  }, [])

  const {
    state: ttsState,
    preload: preloadTts,
    speak: speakTts,
    pause: pauseTts,
    resume: resumeTts,
    jumpToChunk: jumpTtsToChunk,
    skipBackward: skipTtsBackward,
    skipForward: skipTtsForward,
    stop: stopTts,
  } = useTtsPlayer(ttsPlaybackRate)
  const {
    state: selectedAudiobookState,
    check: checkSelectedAudiobook,
    reset: resetSelectedAudiobookState,
  } = useAudiobookCache()
  const {
    state: downloadAudiobookState,
    save: saveAudiobook,
    cancel: cancelAudiobookSave,
  } = useAudiobookCache()

  const savedAudiobookIds = useMemo(() => new Set(savedAudiobooks.map((record) => record.id)), [savedAudiobooks])

  const getDocumentTitle = useCallback((url: string): string => {
    return uploadedDocuments.find((doc) => doc.url === url)?.title
      ?? userUploads.find((doc) => doc.url === url)?.title
      ?? allDocuments.find((doc) => doc.url === url)?.title
      ?? decodeURIComponent(url.split('/').pop() ?? url)
  }, [allDocuments, uploadedDocuments, userUploads])

  const refreshAudiobookDownloads = useCallback(() => {
    setAudiobookDownloads(getAudiobookDownloads())
  }, [])

  const clearAudiobookNoticeTimer = useCallback(() => {
    if (audiobookNoticeTimerRef.current !== null) {
      window.clearTimeout(audiobookNoticeTimerRef.current)
      audiobookNoticeTimerRef.current = null
    }
  }, [])

  const dismissAudiobookNotice = useCallback(() => {
    clearAudiobookNoticeTimer()
    setAudiobookNotice(null)
  }, [clearAudiobookNoticeTimer])

  const showAudiobookNotice = useCallback((nextNotice: AudiobookNoticeState) => {
    clearAudiobookNoticeTimer()
    setAudiobookNotice(nextNotice)
    audiobookNoticeTimerRef.current = window.setTimeout(() => {
      setAudiobookNotice((current) => (
        current?.id === nextNotice.id &&
        current.status === nextNotice.status &&
        current.message === nextNotice.message
          ? null
          : current
      ))
      audiobookNoticeTimerRef.current = null
    }, AUDIOBOOK_NOTICE_TIMEOUT_MS)
  }, [clearAudiobookNoticeTimer])

  const flushAudiobookDownloadPersist = useCallback(() => {
    if (downloadPersistTimerRef.current !== null) {
      window.clearTimeout(downloadPersistTimerRef.current)
      downloadPersistTimerRef.current = null
    }

    const pending = pendingDownloadPersistRef.current
    if (!pending) return

    pendingDownloadPersistRef.current = null
    upsertAudiobookDownload(pending)
    refreshAudiobookDownloads()
  }, [refreshAudiobookDownloads])

  const scheduleAudiobookDownloadPersist = useCallback((input: AudiobookDownloadInput, immediate = false) => {
    pendingDownloadPersistRef.current = input
    if (immediate) {
      flushAudiobookDownloadPersist()
      return
    }

    if (downloadPersistTimerRef.current !== null) return
    downloadPersistTimerRef.current = window.setTimeout(() => {
      flushAudiobookDownloadPersist()
    }, 1200)
  }, [flushAudiobookDownloadPersist])

  // Loads and normalizes native TTS capabilities for the UI, then synchronizes
  // this session's thread selection: initialize from the platform default at
  // startup, or preserve the current choice while clamping it to the detected max.
  const syncTtsRuntimeSettings = useCallback(async (initializeThreadCount = false) => {
    const capabilities = await getNativeTtsCapabilities()
    const maxThreadCount = Math.max(1, capabilities.maxThreadCount)
    const defaultThreadCount = Math.min(maxThreadCount, Math.max(1, capabilities.defaultThreadCount))
    setTtsCapabilities({ ...capabilities, defaultThreadCount, maxThreadCount })
    setTtsThreadCount((current) => initializeThreadCount
      ? defaultThreadCount
      : Math.min(maxThreadCount, Math.max(1, current)))
    return capabilities
  }, [])

  const refreshTtsModelStatus = useCallback(async () => {
    const status = await getNativeTtsModelStatus(ttsModelId)
    if (ttsModelIdRef.current === status.modelId) setTtsModelStatus(status)
    return status
  }, [ttsModelId])

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect
    void syncTtsRuntimeSettings(true)
  }, [syncTtsRuntimeSettings])

  useEffect(() => {
    return () => clearAudiobookNoticeTimer()
  }, [clearAudiobookNoticeTimer])

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect
    void refreshTtsModelStatus()
    let cancelled = false
    let unlisten: (() => void) | null = null
    listenNativeTtsModelInstallProgress((progress) => {
      if (!cancelled && progress.modelId === ttsModelId) setTtsModelProgress(progress)
    }).then((value) => {
      if (cancelled) value()
      else unlisten = value
    })
    return () => {
      cancelled = true
      unlisten?.()
    }
  }, [refreshTtsModelStatus, ttsModelId])

  const handleInstallTtsModel = useCallback(async () => {
    setTtsModelProgress({
      modelId: ttsModelId,
      status: 'starting',
      message: 'Preparing offline voice model download',
      downloadedBytes: 0,
      totalBytes: ttsModelStatus?.archiveBytes ?? 0,
      percent: 0,
    })
    try {
      await installNativeTtsModel(ttsModelId)
      await refreshTtsModelStatus()
      await syncTtsRuntimeSettings()
      if (ttsModelIdRef.current !== ttsModelId) return
      setTtsModelProgress((prev) => ({
        modelId: ttsModelId,
        status: 'installed',
        message: 'Offline voice model installed',
        downloadedBytes: prev?.totalBytes ?? ttsModelStatus?.archiveBytes ?? 0,
        totalBytes: prev?.totalBytes ?? ttsModelStatus?.archiveBytes ?? 0,
        percent: 100,
      }))
      preloadTts()
    } catch (err) {
      if (ttsModelIdRef.current !== ttsModelId) return
      setTtsModelProgress({
        modelId: ttsModelId,
        status: 'error',
        message: err instanceof Error ? err.message : String(err),
        downloadedBytes: 0,
        totalBytes: ttsModelStatus?.archiveBytes ?? 0,
        percent: 0,
      })
      void refreshTtsModelStatus()
    }
  }, [preloadTts, syncTtsRuntimeSettings, refreshTtsModelStatus, ttsModelId, ttsModelStatus?.archiveBytes])

  const handleProbeSilmaSidecar = useCallback(async () => {
    if (silmaProbeRunning) return
    setSilmaProbeRunning(true)
    try {
      const result = await probeNativeSilmaSidecar()
      logTtsDiagnostic('[tts-native] SILMA sidecar probe passed', { ...result })
    } catch (err) {
      logTtsDiagnostic('[tts-native] SILMA sidecar probe failed', {
        error: err instanceof Error ? err.message : String(err),
      }, 'error')
    } finally {
      setSilmaProbeRunning(false)
    }
  }, [silmaProbeRunning])

  useEffect(() => {
    if (window.requestIdleCallback) {
      const handle = window.requestIdleCallback(() => preloadTts(), { timeout: 4000 })
      return () => window.cancelIdleCallback(handle)
    }

    const timeout = window.setTimeout(() => preloadTts(), 1500)
    return () => window.clearTimeout(timeout)
  }, [preloadTts])

  useEffect(() => {
    if (!selectedDoc || !docContent) {
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setTtsSaveChunks(null)
      setImportedHighlightStatus('idle')
      return
    }

    let cancelled = false
    let cancelHighlightBuild: (() => void) | null = null
    if (isUserUploadUrl(selectedDoc)) {
      // Imported audiobook bundles must play against their saved manifest chunks.
      // Highlight spans are rebuilt lazily and grafted only on an exact match.
      setTtsSaveChunks(null)
      setImportedHighlightStatus('preparing')
      void getImportedAudiobookMetadata(selectedDoc)
        .then((metadata) => {
          if (cancelled) return
          setTtsModelId(metadata.modelId)
          setTtsVoice(metadata.voice as TtsVoice)
          setTtsTextPreprocessor(metadata.textPreprocessor)
          setTtsSpeed(metadata.speed)
          setTtsSaveChunks(metadata.chunks)
          if (chunksHaveDurableSourceSpans(metadata.chunks)) {
            logTtsDiagnostic('[tts-highlight] imported durable source spans ready', {
              chunks: metadata.chunks.length,
              sourceSpans: countChunkSourceSpans(metadata.chunks),
              modelId: metadata.modelId,
              textPreprocessor: metadata.textPreprocessor,
              documentUrl: selectedDoc,
            })
            setImportedHighlightStatus('ready')
            return
          }
          cancelHighlightBuild = scheduleImportedHighlightBuild(() => {
            if (cancelled) return
            const rebuiltChunks = audiobookSaveChunksFromHtml(docContent)
            const graftedChunks = graftImportedSourceSpans(metadata.chunks, rebuiltChunks, {
              documentUrl: selectedDoc,
              modelId: metadata.modelId,
              textPreprocessor: metadata.textPreprocessor,
            })
            if (cancelled) return
            if (graftedChunks) {
              setTtsSaveChunks(graftedChunks)
              setImportedHighlightStatus('ready')
            } else {
              setImportedHighlightStatus('unavailable')
            }
          })
        })
        .catch(() => {
          if (cancelled) return
          setTtsSaveChunks(audiobookSaveChunksFromHtml(docContent))
          setImportedHighlightStatus('unavailable')
        })
      return () => {
        cancelled = true
        cancelHighlightBuild?.()
      }
    }

    setImportedHighlightStatus('ready')
    setTtsSaveChunks(audiobookSaveChunksFromHtml(docContent))
  }, [docContent, selectedDoc, setTtsModelId])

  useEffect(() => {
    if (!selectedDoc || !ttsSaveChunks || autoSelectedDocumentRef.current === selectedDoc) return
    autoSelectedDocumentRef.current = selectedDoc
    if (isUserUploadUrl(selectedDoc)) return

    const alreadySavedWithCurrentSettings = savedAudiobooks.some((record) =>
      record.documentUrl === selectedDoc &&
      record.modelId === ttsModelId &&
      record.textPreprocessor === ttsTextPreprocessor &&
      record.voice === ttsVoice &&
      record.speed === ttsSpeed &&
      record.dtype === ttsDtype
    )
    if (alreadySavedWithCurrentSettings) return
    const suggested = suggestTtsModel(ttsModels, ttsSaveChunks)
    if (suggested.id !== ttsModelId) {
      // One-time per-document language suggestion; user changes remain authoritative afterward.
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setTtsModelId(suggested.id)
      setTtsVoice(suggested.defaultVoice)
      setTtsTextPreprocessor(suggested.defaultTextPreprocessor)
    }
  }, [savedAudiobooks, selectedDoc, setTtsModelId, ttsDtype, ttsModelId, ttsModels, ttsSaveChunks, ttsSpeed, ttsTextPreprocessor, ttsVoice])

  useEffect(() => {
    if (!selectedDoc || !ttsSaveChunks) return

    checkSelectedAudiobook(ttsSaveChunks, {
      modelId: ttsModelId,
      textPreprocessor: ttsTextPreprocessor,
      voice: ttsVoice,
      speed: ttsSpeed,
      dtype: ttsDtype,
      threadCount: ttsThreadCount,
      documentUrl: selectedDoc,
      title: getDocumentTitle(selectedDoc),
    })
  }, [checkSelectedAudiobook, getDocumentTitle, selectedDoc, ttsDtype, ttsModelId, ttsSaveChunks, ttsSpeed, ttsTextPreprocessor, ttsThreadCount, ttsVoice])

  useEffect(() => {
    saveAudioPreferences({ modelId: ttsModelId, voice: ttsVoice, textPreprocessor: ttsTextPreprocessor })
  }, [ttsModelId, ttsTextPreprocessor, ttsVoice])

  useEffect(() => {
    saveAudioPreferences({ playbackRate: ttsPlaybackRate })
  }, [ttsPlaybackRate])

  useEffect(() => {
    saveAudioPreferences({ wordHighlightEnabled: ttsWordHighlightEnabled })
  }, [ttsWordHighlightEnabled])

  useEffect(() => {
    saveAudioPreferences({ audioSavedOnly })
  }, [audioSavedOnly])

  useEffect(() => {
    if (!audiobookDownload || downloadAudiobookState.complete) return

    if (downloadAudiobookState.status === 'checking' || downloadAudiobookState.status === 'saving') {
      // eslint-disable-next-line react-hooks/set-state-in-effect
      scheduleAudiobookDownloadPersist({
        documentUrl: audiobookDownload.url,
        title: audiobookDownload.title,
        modelId: audiobookDownload.modelId,
        textPreprocessor: audiobookDownload.textPreprocessor,
        voice: audiobookDownload.voice,
        speed: audiobookDownload.speed,
        dtype: audiobookDownload.dtype,
        status: 'saving',
        cachedChunks: downloadAudiobookState.cachedChunks,
        totalChunks: downloadAudiobookState.totalChunks,
        message: downloadAudiobookState.message,
        audioDurationSec: downloadAudiobookState.audioDurationSec,
        wavBytes: downloadAudiobookState.wavBytes,
      })
      return
    }

    if (downloadAudiobookState.status === 'partial') {
      scheduleAudiobookDownloadPersist({
        documentUrl: audiobookDownload.url,
        title: audiobookDownload.title,
        modelId: audiobookDownload.modelId,
        textPreprocessor: audiobookDownload.textPreprocessor,
        voice: audiobookDownload.voice,
        speed: audiobookDownload.speed,
        dtype: audiobookDownload.dtype,
        status: 'paused',
        cachedChunks: downloadAudiobookState.cachedChunks,
        totalChunks: downloadAudiobookState.totalChunks,
        message: downloadAudiobookState.message || 'Ready to resume',
        audioDurationSec: downloadAudiobookState.audioDurationSec,
        wavBytes: downloadAudiobookState.wavBytes,
      }, true)
      return
    }

    if (downloadAudiobookState.status === 'error') {
      scheduleAudiobookDownloadPersist({
        documentUrl: audiobookDownload.url,
        title: audiobookDownload.title,
        modelId: audiobookDownload.modelId,
        textPreprocessor: audiobookDownload.textPreprocessor,
        voice: audiobookDownload.voice,
        speed: audiobookDownload.speed,
        dtype: audiobookDownload.dtype,
        status: 'error',
        cachedChunks: downloadAudiobookState.cachedChunks,
        totalChunks: downloadAudiobookState.totalChunks,
        message: downloadAudiobookState.message,
        audioDurationSec: downloadAudiobookState.audioDurationSec,
        wavBytes: downloadAudiobookState.wavBytes,
      }, true)
    }
  }, [audiobookDownload, downloadAudiobookState, scheduleAudiobookDownloadPersist])

  useEffect(() => {
    if (!downloadAudiobookState.complete || !audiobookDownload) return
    // eslint-disable-next-line react-hooks/set-state-in-effect
    flushAudiobookDownloadPersist()
    markAudiobookSaved({
      documentUrl: audiobookDownload.url,
      title: audiobookDownload.title,
        modelId: audiobookDownload.modelId,
      textPreprocessor: audiobookDownload.textPreprocessor,
      voice: audiobookDownload.voice,
      speed: audiobookDownload.speed,
      dtype: audiobookDownload.dtype,
      chunks: downloadAudiobookState.totalChunks,
      audioDurationSec: downloadAudiobookState.audioDurationSec,
      wavBytes: downloadAudiobookState.wavBytes,
    })
    clearCompletedAudiobookDownload(audiobookDownload.url, {
      modelId: audiobookDownload.modelId,
      textPreprocessor: audiobookDownload.textPreprocessor,
      voice: audiobookDownload.voice,
      speed: audiobookDownload.speed,
      dtype: audiobookDownload.dtype,
    })
    setSavedAudiobooks(getSavedAudiobooks())
    refreshAudiobookDownloads()
  }, [audiobookDownload, downloadAudiobookState.audioDurationSec, downloadAudiobookState.complete, downloadAudiobookState.totalChunks, downloadAudiobookState.wavBytes, flushAudiobookDownloadPersist, refreshAudiobookDownloads])

  useEffect(() => {
    function flushPendingDownload() {
      flushAudiobookDownloadPersist()
    }

    document.addEventListener('visibilitychange', flushPendingDownload)
    window.addEventListener('pagehide', flushPendingDownload)
    return () => {
      document.removeEventListener('visibilitychange', flushPendingDownload)
      window.removeEventListener('pagehide', flushPendingDownload)
      flushAudiobookDownloadPersist()
    }
  }, [flushAudiobookDownloadPersist])

  useEffect(() => {
    if (!selectedDoc) return
    const timeout = window.setTimeout(() => preloadTts(), 250)
    return () => window.clearTimeout(timeout)
  }, [preloadTts, selectedDoc])

  const prepareDocumentOpen = useCallback(() => {
    if (preserveGeneratedSpeedOnOpenRef.current) {
      preserveGeneratedSpeedOnOpenRef.current = false
    } else {
      setTtsSpeed(DEFAULT_TTS_SPEED)
    }
    setTtsSaveChunks(null)
    resetSelectedAudiobookState()
  }, [resetSelectedAudiobookState])

  const closeDocumentAudio = useCallback(() => {
    stopTts()
    setTtsSaveChunks(null)
    setImportedHighlightStatus('idle')
  }, [stopTts])

  const getAudiobookSaveChunksForDocument = useCallback(async (documentUrl: string): Promise<TtsChunk[]> => {
    if (isUserUploadUrl(documentUrl)) {
      return (await getImportedAudiobookMetadata(documentUrl)).chunks
    }

    const html = await loadHtmlDocument(documentUrl)
    return audiobookSaveChunksFromHtml(html)
  }, [loadHtmlDocument])

  const getSelectedAudiobookSaveChunks = useCallback(async (): Promise<TtsChunk[]> => {
    if (!selectedDoc) return []
    return ttsSaveChunks ?? getAudiobookSaveChunksForDocument(selectedDoc)
  }, [getAudiobookSaveChunksForDocument, selectedDoc, ttsSaveChunks])

  const handleReadDocument = useCallback(async () => {
    if (!selectedAudiobookState.complete) return

    const chunks = await getSelectedAudiobookSaveChunks()
    speakTts(chunks, {
      modelId: ttsModelId,
      textPreprocessor: ttsTextPreprocessor,
      voice: ttsVoice,
      speed: ttsSpeed,
      dtype: ttsDtype,
      threadCount: ttsThreadCount,
      documentUrl: selectedDoc ?? undefined,
      title: selectedDoc ? getDocumentTitle(selectedDoc) : undefined,
    })
  }, [getDocumentTitle, getSelectedAudiobookSaveChunks, selectedAudiobookState.complete, selectedDoc, speakTts, ttsDtype, ttsModelId, ttsSpeed, ttsTextPreprocessor, ttsThreadCount, ttsVoice])

  useEffect(() => {
    if (!selectedDoc || !selectedAudiobookState.complete) return

    markAudiobookSaved({
      documentUrl: selectedDoc,
      title: getDocumentTitle(selectedDoc),
      modelId: ttsModelId,
      textPreprocessor: ttsTextPreprocessor,
      voice: ttsVoice,
      speed: ttsSpeed,
      dtype: ttsDtype,
      chunks: selectedAudiobookState.totalChunks,
      audioDurationSec: selectedAudiobookState.audioDurationSec,
      wavBytes: selectedAudiobookState.wavBytes,
    })
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setSavedAudiobooks(getSavedAudiobooks())
  }, [getDocumentTitle, selectedAudiobookState.audioDurationSec, selectedAudiobookState.complete, selectedAudiobookState.totalChunks, selectedAudiobookState.wavBytes, selectedDoc, ttsDtype, ttsModelId, ttsSpeed, ttsTextPreprocessor, ttsVoice])

  const handleModelChange = useCallback((modelId: string) => {
    const model = getTtsModel(ttsModels, modelId)
    stopTts()
    resetSelectedAudiobookState()
    setTtsModelProgress(null)
    setTtsModelStatus(null)
    setTtsModelId(model.id)
    setTtsVoice(model.defaultVoice)
    setTtsTextPreprocessor(model.defaultTextPreprocessor)
  }, [resetSelectedAudiobookState, setTtsModelId, stopTts, ttsModels])

  const handleThreadCountChange = useCallback((threadCount: number) => {
    const maxThreadCount = ttsCapabilities?.maxThreadCount ?? 1
    setTtsThreadCount(Math.min(maxThreadCount, Math.max(1, threadCount)))
  }, [ttsCapabilities?.maxThreadCount])

  const startAudiobookSave = useCallback((input: {
    documentUrl: string
    title: string
    modelId: string
    textPreprocessor: string
    chunks: TtsChunk[]
    voice: TtsVoice
    speed: number
    dtype: TtsDtype
  }) => {
    const speakableChunks = input.chunks.filter((chunk) => chunk.text.trim())
    if (speakableChunks.length === 0) return

    // Queue state is persisted before the native save starts so interrupted saves can resume.
    scheduleAudiobookDownloadPersist({
      documentUrl: input.documentUrl,
      title: input.title,
      modelId: input.modelId,
      textPreprocessor: input.textPreprocessor,
      voice: input.voice,
      speed: input.speed,
      dtype: input.dtype,
      status: 'queued',
      cachedChunks: 0,
      totalChunks: speakableChunks.length,
      message: 'Queued',
      audioDurationSec: 0,
    }, true)
    setAudiobookDownload({ title: input.title, url: input.documentUrl, modelId: input.modelId, textPreprocessor: input.textPreprocessor, voice: input.voice, speed: input.speed, dtype: input.dtype })
    saveAudiobook(input.chunks, {
      modelId: input.modelId,
      textPreprocessor: input.textPreprocessor,
      voice: input.voice,
      speed: input.speed,
      dtype: input.dtype,
      threadCount: ttsThreadCount,
      documentUrl: input.documentUrl,
      title: input.title,
    })
  }, [saveAudiobook, scheduleAudiobookDownloadPersist, ttsThreadCount])

  const handleSaveAudiobook = useCallback(async () => {
    if (!selectedDoc) return
    const title = getDocumentTitle(selectedDoc)
    const chunks = await getSelectedAudiobookSaveChunks()
    const speakableChunks = chunks.filter((chunk) => chunk.text.trim())
    if (speakableChunks.length === 0) {
      await confirmAudiobookAction({
        title: 'Nothing to save',
        description: 'This document does not contain any text that can be saved as audio.',
        confirmLabel: 'OK',
        cancelLabel: null,
      })
      return
    }

    const textPreprocessorName = selectedTtsModel.textPreprocessors.find((item) => item.id === ttsTextPreprocessor)?.name ?? ttsTextPreprocessor
    const confirmed = await confirmAudiobookAction({
      title: 'Save audiobook?',
      description: 'Review these settings before Papercut starts generating and saving audio.',
      details: [
        { label: 'Document', value: title },
        { label: 'Model', value: selectedTtsModel.name },
        { label: 'Voice', value: getTtsVoiceName(ttsModels, ttsModelId, ttsVoice) },
        { label: 'Generated Speed', value: formatSpeedLabel(DEFAULT_TTS_SPEED) },
        { label: 'Processing', value: textPreprocessorName },
        { label: 'Threads', value: ttsThreadCount },
        { label: 'Chunks', value: speakableChunks.length },
      ],
      confirmLabel: 'Start Saving',
    })
    if (!confirmed) return

    startAudiobookSave({
      documentUrl: selectedDoc,
      title,
      modelId: ttsModelId,
      textPreprocessor: ttsTextPreprocessor,
      chunks,
      voice: ttsVoice,
      speed: DEFAULT_TTS_SPEED,
      dtype: ttsDtype,
    })
  }, [confirmAudiobookAction, getDocumentTitle, getSelectedAudiobookSaveChunks, selectedDoc, selectedTtsModel.name, selectedTtsModel.textPreprocessors, startAudiobookSave, ttsDtype, ttsModelId, ttsModels, ttsTextPreprocessor, ttsThreadCount, ttsVoice])

  const handleResumeAudiobookDownload = useCallback(async (record: AudiobookDownloadRecord) => {
    startAudiobookSave({
      documentUrl: record.documentUrl,
      title: record.title,
      chunks: await getAudiobookSaveChunksForDocument(record.documentUrl),
      modelId: record.modelId,
      textPreprocessor: record.textPreprocessor,
      voice: record.voice,
      speed: record.speed,
      dtype: record.dtype,
    })
  }, [getAudiobookSaveChunksForDocument, startAudiobookSave])

  const handleCancelAudiobookSave = useCallback(() => {
    if (audiobookDownload) {
      scheduleAudiobookDownloadPersist({
        documentUrl: audiobookDownload.url,
        title: audiobookDownload.title,
        modelId: audiobookDownload.modelId,
        textPreprocessor: audiobookDownload.textPreprocessor,
        voice: audiobookDownload.voice,
        speed: audiobookDownload.speed,
        dtype: audiobookDownload.dtype,
        status: 'paused',
        cachedChunks: downloadAudiobookState.cachedChunks,
        totalChunks: downloadAudiobookState.totalChunks,
        message: 'Paused. Ready to resume.',
        audioDurationSec: downloadAudiobookState.audioDurationSec,
        wavBytes: downloadAudiobookState.wavBytes,
      }, true)
    }
    cancelAudiobookSave()
  }, [audiobookDownload, cancelAudiobookSave, downloadAudiobookState.audioDurationSec, downloadAudiobookState.cachedChunks, downloadAudiobookState.totalChunks, downloadAudiobookState.wavBytes, scheduleAudiobookDownloadPersist])

  const handleRemoveAudiobookDownload = useCallback(async (id: string) => {
    const record = audiobookDownloads.find((item) => item.id === id)
    if (!record) return

    const progress = record.totalChunks > 0
      ? record.cachedChunks + '/' + record.totalChunks + ' chunks'
      : null
    const duration = record.audioDurationSec ? formatDuration(record.audioDurationSec) : null
    const storage = formatStorageSize(record.wavBytes)
    const confirmed = await confirmAudiobookAction({
      title: 'Remove audiobook save job?',
      description: 'This removes the queued save job and its resume state. Saved audiobooks are not deleted.',
      details: [
        { label: 'Title', value: record.title },
        ...(progress ? [{ label: 'Progress', value: progress }] : []),
        ...(duration ? [{ label: 'Duration', value: duration }] : []),
        ...(storage ? [{ label: 'Storage', value: storage }] : []),
      ],
      confirmLabel: 'Remove Job',
      tone: 'danger',
    })
    if (!confirmed) return

    removeAudiobookDownload(id)
    refreshAudiobookDownloads()
  }, [audiobookDownloads, confirmAudiobookAction, refreshAudiobookDownloads])

  const handleExportSavedAudiobook = useCallback(async (record: SavedAudiobookRecord, exportFormat: NativeAudiobookExportFormat) => {
    dismissAudiobookNotice()
    setAudiobookExport({
      id: record.id,
      status: 'exporting',
      message: exportFormat === 'wav' ? 'Exporting WAV' : 'Exporting Papercut Bundle',
    })
    try {
      const chunks = await getAudiobookSaveChunksForDocument(record.documentUrl)
      const sourceHtml = exportFormat === 'bundle'
        ? await loadHtmlDocument(record.documentUrl)
        : undefined
      const result = await exportNativeAudiobook({
        documentUrl: record.documentUrl,
        title: record.title,
        sourceHtml,
        chunks,
        options: {
          modelId: record.modelId,
          textPreprocessor: record.textPreprocessor,
          voice: record.voice as TtsVoice,
          speed: record.speed,
          dtype: record.dtype as TtsDtype,
        },
        exportFormat,
      })
      setAudiobookExport(null)
      showAudiobookNotice({
        id: 'export:' + record.id,
        status: 'success',
        message: formatAudiobookExportMessage(result.path, exportFormat),
      })
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      const cancelled = message.toLowerCase().includes('cancelled')
      setAudiobookExport(null)
      showAudiobookNotice({
        id: 'export:' + record.id,
        status: cancelled ? 'cancelled' : 'error',
        message: cancelled ? 'Export cancelled.' : message,
      })
    }
  }, [dismissAudiobookNotice, getAudiobookSaveChunksForDocument, loadHtmlDocument, showAudiobookNotice])

  const handleDeleteSavedAudiobook = useCallback(async (record: SavedAudiobookRecord) => {
    const deleteUserUpload = isUserUploadUrl(record.documentUrl)
    const duration = record.audioDurationSec ? formatDuration(record.audioDurationSec) : null
    const storage = formatStorageSize(record.wavBytes)
    const confirmed = await confirmAudiobookAction({
      title: deleteUserUpload ? 'Delete saved audiobook and import?' : 'Delete saved audiobook?',
      description: deleteUserUpload
        ? 'This removes the generated audio and imported audiobook document from this device.'
        : 'This removes the generated audio from this device.',
      details: [
        { label: 'Title', value: record.title },
        ...(duration ? [{ label: 'Duration', value: duration }] : []),
        ...(storage ? [{ label: 'Storage', value: storage }] : []),
      ],
      confirmLabel: 'Delete Audiobook',
      tone: 'danger',
    })
    if (!confirmed) return

    setAudiobookDelete({ id: record.id, status: 'deleting', message: 'Deleting Saved Audio' })
    try {
      const result = await deleteNativeAudiobook({
        audiobookId: record.id,
        documentUrl: record.documentUrl,
        deleteUserUpload,
      })

      removeSavedAudiobook(record.id)
      if (deleteUserUpload) removeUserUpload(record.documentUrl)
      setSavedAudiobooks(getSavedAudiobooks())
      onUserUploadsChanged()
      if (selectedDoc === record.documentUrl) {
        stopTts()
        resetSelectedAudiobookState()
        if (deleteUserUpload) {
          onClearDocument()
          setTtsSaveChunks(null)
        }
      }

      const storage = formatStorageSize(result.bytesFreed)
      setAudiobookDelete(null)
      showAudiobookNotice({
        id: 'delete:' + record.id,
        status: 'success',
        message: storage ? 'Deleted saved audio and freed ' + storage + '.' : 'Deleted saved audio.',
      })
    } catch (err) {
      setAudiobookDelete(null)
      showAudiobookNotice({
        id: 'delete:' + record.id,
        status: 'error',
        message: err instanceof Error ? err.message : String(err),
      })
    }
  }, [confirmAudiobookAction, onClearDocument, onUserUploadsChanged, resetSelectedAudiobookState, selectedDoc, showAudiobookNotice, stopTts])

  const importAudiobook = useCallback(async (openDocument: (url: string) => Promise<void>) => {
    dismissAudiobookNotice()
    setAudiobookImport({ status: 'importing', message: 'Importing Audiobook Bundle...' })
    try {
      const result = await importNativeAudiobook()
      upsertUserUpload({
        url: result.documentUrl,
        title: result.title,
        modelId: result.modelId,
        textPreprocessor: result.textPreprocessor,
        voice: result.voice,
        speed: result.speed,
        dtype: result.dtype,
        chunks: result.chunks,
        audioDurationSec: result.audioDurationSec,
        wavBytes: result.wavBytes,
      })
      markAudiobookSaved({
        documentUrl: result.documentUrl,
        title: result.title,
        modelId: result.modelId,
        textPreprocessor: result.textPreprocessor,
        voice: result.voice,
        speed: result.speed,
        dtype: result.dtype,
        chunks: result.chunks,
        audioDurationSec: result.audioDurationSec,
        wavBytes: result.wavBytes,
      })
      onUserUploadsChanged()
      setSavedAudiobooks(getSavedAudiobooks())
      autoSelectedDocumentRef.current = result.documentUrl
      preserveGeneratedSpeedOnOpenRef.current = true
      setTtsModelId(result.modelId)
      setTtsVoice(result.voice as TtsVoice)
      setTtsTextPreprocessor(result.textPreprocessor)
      setTtsSpeed(result.speed)
      setAudiobookImport({ status: 'idle', message: '' })
      showAudiobookNotice({ id: 'import:' + result.documentUrl, status: 'success', message: 'Imported ' + result.title })
      await openDocument(result.documentUrl)
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      const cancelled = message.toLowerCase().includes('cancelled')
      setAudiobookImport({ status: 'idle', message: '' })
      showAudiobookNotice({ id: 'import', status: cancelled ? 'cancelled' : 'error', message: cancelled ? 'Import cancelled.' : message })
    }
  }, [dismissAudiobookNotice, onUserUploadsChanged, setTtsModelId, showAudiobookNotice])

  const openSavedAudiobook = useCallback(async (record: SavedAudiobookRecord, openDocument: (url: string) => Promise<void>) => {
    autoSelectedDocumentRef.current = record.documentUrl
    preserveGeneratedSpeedOnOpenRef.current = true
    setTtsModelId(record.modelId)
    setTtsVoice(record.voice as TtsVoice)
    setTtsTextPreprocessor(record.textPreprocessor)
    setTtsSpeed(record.speed)
    await openDocument(record.documentUrl)
  }, [setTtsModelId])

  const includeDocumentInList = useCallback((doc: DocumentInfo) => (
    !audioSavedOnly || savedAudiobookIds.has(createAudiobookId(doc.url, {
      modelId: ttsModelId,
      textPreprocessor: ttsTextPreprocessor,
      voice: ttsVoice,
      speed: ttsSpeed,
      dtype: ttsDtype,
    }))
  ), [audioSavedOnly, savedAudiobookIds, ttsDtype, ttsModelId, ttsSpeed, ttsTextPreprocessor, ttsVoice])

  const filterResults = useCallback((results: SearchResult[]) => (
    audioSavedOnly
      ? results.filter((result) => savedAudiobookIds.has(createAudiobookId(result.url, {
        modelId: ttsModelId,
        textPreprocessor: ttsTextPreprocessor,
        voice: ttsVoice,
        speed: ttsSpeed,
        dtype: ttsDtype,
      })))
      : results
  ), [audioSavedOnly, savedAudiobookIds, ttsDtype, ttsModelId, ttsSpeed, ttsTextPreprocessor, ttsVoice])

  const ttsIsNavigable = ttsState.status === 'playing' ||
    ttsState.status === 'loading' ||
    ttsState.status === 'paused'
  const ttsCurrentChunkIndex = ttsState.pendingChunkIndex ?? ttsState.currentChunkIndex ?? ttsState.chunksPlayed
  const ttsCanSkipBackward = ttsIsNavigable && ttsCurrentChunkIndex > 0
  const ttsCanSkipForward = ttsIsNavigable &&
    ttsState.chunksTotal > 0 &&
    ttsCurrentChunkIndex < ttsState.chunksTotal - 1
  const selectedAudiobookId = selectedDoc
    ? createAudiobookDownloadId(selectedDoc, {
      modelId: ttsModelId,
      textPreprocessor: ttsTextPreprocessor,
      voice: ttsVoice,
      speed: ttsSpeed,
      dtype: ttsDtype,
    })
    : null
  const activeDownloadId = audiobookDownload
    ? createAudiobookDownloadId(audiobookDownload.url, {
      modelId: audiobookDownload.modelId,
      textPreprocessor: audiobookDownload.textPreprocessor,
      voice: audiobookDownload.voice,
      speed: audiobookDownload.speed,
      dtype: audiobookDownload.dtype,
    })
    : null
  const downloadIsForSelectedDoc = Boolean(selectedAudiobookId && activeDownloadId === selectedAudiobookId)
  const activeDownloadIsRunning = downloadAudiobookState.status === 'checking' ||
    downloadAudiobookState.status === 'saving'
  const audioControlsAudiobookState = downloadIsForSelectedDoc && downloadAudiobookState.status !== 'idle'
    ? downloadAudiobookState
    : selectedAudiobookState
  const isDifferentAudiobookSaving = Boolean(
    activeDownloadId &&
    activeDownloadId !== selectedAudiobookId &&
    activeDownloadIsRunning,
  )
  const canSaveAudiobook = Boolean(ttsModelStatus?.installed) &&
    audioControlsAudiobookState.status !== 'checking' &&
    !isDifferentAudiobookSaving
  const isSavingAudiobook = activeDownloadIsRunning
  const activeDownloadTitle = audiobookDownload?.title ?? 'Audiobook'
  const queuedAudiobookDownloads = audiobookDownloads.filter((record) => (
    !(activeDownloadIsRunning && activeDownloadId === record.id)
  ))
  const visibleSavedAudiobooks = savedAudiobooks.slice().sort((a, b) => b.savedAt - a.savedAt)
  const importedHighlightPreparing = Boolean(
    selectedDoc &&
    isUserUploadUrl(selectedDoc) &&
    importedHighlightStatus === 'preparing' &&
    ttsIsNavigable &&
    ttsState.chunksTotal > 0,
  )
  const ttsHighlightChunks = ttsSaveChunks && ttsSaveChunks.length === ttsState.chunks.length
    ? ttsSaveChunks
    : ttsState.chunks
  const audiobookActionMessage = audiobookImport.status === 'importing'
    ? audiobookImport.message || 'Importing Audiobook Bundle...'
    : audiobookExport?.status === 'exporting'
      ? audiobookExport.message + '...'
      : audiobookDelete?.status === 'deleting'
        ? audiobookDelete.message + '...'
        : ''
  const audiobookActionBusy = Boolean(audiobookActionMessage)

  return {
    audiobookActionBusy,
    audiobookActionMessage,
    audioControlsProps: {
      audiobookState: audioControlsAudiobookState,
      canPlayAudiobook: audioControlsAudiobookState.complete,
      canSaveAudiobook,
      canSkipBackward: ttsCanSkipBackward,
      canSkipForward: ttsCanSkipForward,
      isPdf: false,
      saveInProgress: downloadIsForSelectedDoc && activeDownloadIsRunning,
      onCancelSave: handleCancelAudiobookSave,
      onPause: pauseTts,
      onRead: handleReadDocument,
      onResume: resumeTts,
      onJumpToChunk: jumpTtsToChunk,
      onSave: handleSaveAudiobook,
      onSkipBackward: skipTtsBackward,
      onSkipForward: skipTtsForward,
      onStop: stopTts,
      onPlaybackRateChange: setTtsPlaybackRate,
      onWordHighlightEnabledChange: setTtsWordHighlightEnabled,
      playbackDurationSec: audioControlsAudiobookState.audioDurationSec,
      playbackNotice: importedHighlightPreparing ? 'Preparing highlights...' : undefined,
      playbackRate: ttsPlaybackRate,
      ttsState,
      wordHighlightEnabled: ttsWordHighlightEnabled,
    },
    audioSetupProps: {
      appliedThreadCount: downloadAudiobookState.appliedThreadCount,
      defaultThreadCount: ttsCapabilities?.defaultThreadCount ?? 1,
      maxThreadCount: ttsCapabilities?.maxThreadCount ?? 1,
      modelId: ttsModelId,
      models: ttsModels,
      modelInstallProgress: ttsModelProgress,
      modelStatus: ttsModelStatus,
      onInstallModel: handleInstallTtsModel,
      onModelChange: handleModelChange,
      onProbeSilmaSidecar: handleProbeSilmaSidecar,
      onSpeedChange: () => {},
      onTextPreprocessorChange: setTtsTextPreprocessor,
      onThreadCountChange: handleThreadCountChange,
      onVoiceChange: setTtsVoice,
      textPreprocessor: ttsTextPreprocessor,
      textPreprocessors: selectedTtsModel.textPreprocessors,
      speed: DEFAULT_TTS_SPEED,
      silmaProbeRunning,
      threadCount: ttsThreadCount,
      voice: ttsVoice,
      voices: selectedTtsModel.voices,
    },
    audiobookImport,
    audioSavedOnly,
    closeDocumentAudio,
    confirmationDialog,
    audiobooksPanelProps: {
      activeDownload: audiobookDownload,
      activeDownloadTitle,
      deleteState: audiobookDelete,
      downloadState: downloadAudiobookState,
      exportState: audiobookExport,
      noticeState: audiobookNotice,
      isSaving: isSavingAudiobook,
      queuedDownloads: queuedAudiobookDownloads,
      savedAudiobooks: visibleSavedAudiobooks,
      onCancelSave: handleCancelAudiobookSave,
      onDeleteSaved: handleDeleteSavedAudiobook,
      onExportSaved: handleExportSavedAudiobook,
      onDismissNotice: dismissAudiobookNotice,
      onRemoveQueued: handleRemoveAudiobookDownload,
      onResumeQueued: handleResumeAudiobookDownload,
    },
    filterResults,
    hasFloatingAudioControls: ttsIsNavigable,
    importAudiobook,
    includeDocumentInList,
    openSavedAudiobook,
    prepareDocumentOpen,
    setAudioSavedOnly,
    ttsHighlight: {
      enabled: Boolean(ttsState.currentText),
      currentChunkIndex: ttsState.currentChunkIndex,
      currentChunkTime: ttsState.currentChunkTime,
      currentChunkDuration: ttsState.currentChunkDuration,
      isPlaying: ttsState.status === 'playing',
      wordHighlightEnabled: ttsWordHighlightEnabled,
      chunks: ttsHighlightChunks,
      allowDomFallback: Boolean(selectedDoc && isUserUploadUrl(selectedDoc)),
    },
  }
}

function chunksHaveDurableSourceSpans(chunks: TtsChunk[]): boolean {
  const speakableChunks = chunks.filter((chunk) => chunk.text.trim())
  return Boolean(speakableChunks.length && speakableChunks.every((chunk) => Boolean(chunk.sourceSpan)))
}

function countChunkSourceSpans(chunks: TtsChunk[]): number {
  return chunks.filter((chunk) => Boolean(chunk.sourceSpan)).length
}

// Defer imported highlight rebuilding so Play can become available from the
// bundle manifest before DOM span work finishes.
function scheduleImportedHighlightBuild(task: () => void): () => void {
  if (window.requestIdleCallback) {
    const handle = window.requestIdleCallback(task, { timeout: 1500 })
    return () => window.cancelIdleCallback(handle)
  }

  const handle = window.setTimeout(task, 0)
  return () => window.clearTimeout(handle)
}

// Attach freshly rebuilt DOM spans only when restored HTML still chunks exactly
// like the imported bundle. Playback keeps using bundle identity either way.
function graftImportedSourceSpans(
  importedChunks: TtsChunk[],
  rebuiltChunks: TtsChunk[],
  context: ImportedGraftDiagnosticContext = {},
): TtsChunk[] | null {
  if (importedChunks.length !== rebuiltChunks.length) {
    logImportedGraftFailure('chunk-count-mismatch', importedChunks, rebuiltChunks, -1, context)
    return null
  }

  const grafted: TtsChunk[] = []
  for (let index = 0; index < importedChunks.length; index++) {
    const imported = importedChunks[index]
    const rebuilt = rebuiltChunks[index]
    if (imported.id !== rebuilt.id) {
      logImportedGraftFailure('chunk-id-mismatch', importedChunks, rebuiltChunks, index, context)
      return null
    }
    if (imported.text !== rebuilt.text) {
      logImportedGraftFailure('chunk-text-mismatch', importedChunks, rebuiltChunks, index, context)
      return null
    }
    grafted.push({ ...imported, sourceSpan: rebuilt.sourceSpan })
  }

  logTtsDiagnostic('[tts-highlight] imported source-span graft ready', {
    chunks: importedChunks.length,
    rebuiltSourceSpans: rebuiltChunks.filter((chunk) => Boolean(chunk.sourceSpan)).length,
    modelId: context.modelId ?? '',
    textPreprocessor: context.textPreprocessor ?? '',
    documentUrl: context.documentUrl ?? '',
  })
  return grafted
}

interface ImportedGraftDiagnosticContext {
  documentUrl?: string
  modelId?: string
  textPreprocessor?: string
}

// Keep import-graft diagnostics compact. Arabic failures often hide in Unicode
// details, so we include code point samples without storing large document text.
function logImportedGraftFailure(
  reason: string,
  importedChunks: TtsChunk[],
  rebuiltChunks: TtsChunk[],
  mismatchIndex: number,
  context: ImportedGraftDiagnosticContext,
): void {
  const imported = mismatchIndex >= 0 ? importedChunks[mismatchIndex] : undefined
  const rebuilt = mismatchIndex >= 0 ? rebuiltChunks[mismatchIndex] : undefined
  const importedText = imported?.text ?? ''
  const rebuiltText = rebuilt?.text ?? ''

  logTtsDiagnostic('[tts-highlight] imported source-span graft failed', {
    reason,
    mismatchIndex,
    importedChunks: importedChunks.length,
    rebuiltChunks: rebuiltChunks.length,
    importedId: imported?.id ?? '',
    rebuiltId: rebuilt?.id ?? '',
    importedLength: importedText.length,
    rebuiltLength: rebuiltText.length,
    sameAfterWhitespace: normalizeImportedGraftDiagnosticText(importedText) === normalizeImportedGraftDiagnosticText(rebuiltText),
    sameAfterNfc: importedText.normalize('NFC') === rebuiltText.normalize('NFC'),
    sameAfterNfkc: importedText.normalize('NFKC') === rebuiltText.normalize('NFKC'),
    importedPreview: previewImportedGraftDiagnosticText(importedText),
    rebuiltPreview: previewImportedGraftDiagnosticText(rebuiltText),
    importedCodePoints: previewImportedGraftCodePoints(importedText),
    rebuiltCodePoints: previewImportedGraftCodePoints(rebuiltText),
    modelId: context.modelId ?? '',
    textPreprocessor: context.textPreprocessor ?? '',
    documentUrl: context.documentUrl ?? '',
  }, 'warn')
}

function normalizeImportedGraftDiagnosticText(text: string): string {
  return text.replace(/\s+/g, ' ').trim()
}

function previewImportedGraftDiagnosticText(text: string): string {
  const normalized = normalizeImportedGraftDiagnosticText(text)
  return normalized.length <= 160 ? normalized : normalized.slice(0, 157).trimEnd() + '...'
}

function previewImportedGraftCodePoints(text: string): string {
  return Array.from(text)
    .slice(0, 32)
    .map((char) => 'U+' + (char.codePointAt(0) ?? 0).toString(16).toUpperCase().padStart(4, '0'))
    .join(' ')
}

// Rebuild runtime source spans from current HTML every open. Saved audio remains
// compatible because ids/text are unchanged and spans never cross native IPC.
function audiobookSaveChunksFromHtml(html: string): TtsChunk[] {
  return buildRuntimeChunks(chunkAudiobookSaveHtmlWithSpans(html), 'save-c')
}

// Assign deterministic cache ids while carrying optional UI-only highlight spans.
function buildRuntimeChunks(chunks: SpeechChunk[], prefix: string): TtsChunk[] {
  return chunks.map((chunk, index) => ({
    id: prefix + String(index + 1).padStart(5, '0'),
    text: chunk.text,
    sourceSpan: chunk.sourceSpan,
  }))
}
