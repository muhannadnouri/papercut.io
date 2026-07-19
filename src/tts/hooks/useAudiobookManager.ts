import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import i18n from '../../i18n'
import type { DocumentInfo, SearchResult } from '../../types/search'
import type { UploadedDocument } from '../../uploads/DocumentUploads'
import {
  createAudiobookId,
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
import {
  formatAudiobookExportMessage,
  formatDuration,
  formatSpeedLabel,
  formatStorageSize,
  formatTextPreprocessorLabel,
} from '../utils/format'
import {
  deleteNativeAudiobook,
  exportNativeAudiobook,
  getNativeTtsCapabilities,
  getNativeTtsModelStatus,
  getImportedAudiobookMetadata,
  importNativeAudiobook,
  installNativeTtsModel,
  listNativeSavedAudiobooks,
  listenNativeTtsModelInstallProgress,
  probeNativeSilmaSidecar,
  type NativeTtsCapabilities,
  type NativeAudiobookExportFormat,
  type NativeTtsModelInstallProgress,
  type NativeTtsModelStatus,
} from '../api/nativeTts'
import {
  chunkAudiobookSaveHtmlWithSpans,
  SILMA_AUDIOBOOK_SAVE_CHUNK_PROFILE,
  type SpeechChunk,
} from '../utils/text'
import {
  DEFAULT_TTS_SPEED,
  SILMA_MODEL_ID,
  resolveSilmaNfeStep,
  type TextPreprocessorId,
  type TtsDtype,
  type TtsVoice,
  type TtsChunk,
} from '../types'
import { isUserUploadUrl, removeUserUpload, upsertUserUpload, type UserUploadDocument } from '../storage/UserUploads'
import { logTtsDiagnostic } from '../diagnostics/TtsDiagnostics'
import { useAudiobookCache } from './useAudiobookCache'
import { useTtsPlayer } from './useTtsPlayer'
import { useAppConfirmation } from '../../components/AppDialog/useAppConfirmation'
import {
  isNativeTtsCancellation,
  nativeTtsErrorDetail,
  nativeTtsErrorMessage,
} from '../utils/errors'

type ImportedHighlightStatus = 'idle' | 'preparing' | 'ready' | 'unavailable'
type AudiobookExportState = { id: string; status: 'exporting' | 'exported' | 'cancelled' | 'error'; message: string }
type AudiobookNoticeState = { id: string; status: 'success' | 'cancelled' | 'error'; message: string }

const AUDIOBOOK_NOTICE_TIMEOUT_MS = 10000

function getAiAudioExportDescription(record: SavedAudiobookRecord): string {
  const description = i18n.t('tts.confirm.exportDescription')
  if (record.modelId !== SILMA_MODEL_ID) return description
  return description + i18n.t('tts.confirm.exportReferenceDescription')
}

function getAiAudioExportDisclosure(record: SavedAudiobookRecord): string {
  if (record.modelId === SILMA_MODEL_ID) return i18n.t('tts.confirm.disclosureReference')
  return i18n.t('tts.confirm.disclosureAi')
}

function summarizeTtsModelStatus(status: NativeTtsModelStatus | null): Record<string, unknown> {
  if (!status) return {}
  return {
    modelId: status.modelId,
    installed: status.installed,
    installing: status.installing,
    installSupported: status.installSupported,
    runtimeInstalled: status.runtimeInstalled,
    archiveBytes: status.archiveBytes,
    installedBytes: status.installedBytes,
    modelDir: status.modelDir ?? '',
    runtimeDir: status.runtimeDir ?? '',
    message: status.message,
    runtimeMessage: status.runtimeMessage,
  }
}

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
  const [silmaNfeStep, setSilmaNfeStep] = useState(() => resolveSilmaNfeStep(initialAudioPreferences))
  const [ttsCapabilities, setTtsCapabilities] = useState<NativeTtsCapabilities | null>(null)
  const ttsDtype: TtsDtype = initialAudioPreferences.dtype
  const [ttsSaveChunks, setTtsSaveChunks] = useState<TtsChunk[] | null>(null)
  const [importedHighlightStatus, setImportedHighlightStatus] = useState<ImportedHighlightStatus>('idle')
  const [ttsModelStatus, setTtsModelStatus] = useState<NativeTtsModelStatus | null>(null)
  const [ttsModelProgress, setTtsModelProgress] = useState<NativeTtsModelInstallProgress | null>(null)
  const [savedAudiobooks, setSavedAudiobooks] = useState<SavedAudiobookRecord[]>([])
  const [audioSavedOnly, setAudioSavedOnly] = useState(initialAudioPreferences.audioSavedOnly)
  const [audiobookDownloads, setAudiobookDownloads] = useState<AudiobookDownloadRecord[]>(() => getAudiobookDownloads())
  const [audiobookDownload, setAudiobookDownload] = useState<{ title: string; url: string; modelId: string; textPreprocessor: string; voice: TtsVoice; speed: number; dtype: TtsDtype; silmaNfeStep?: number } | null>(null)
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

  const refreshSavedAudiobooks = useCallback(async () => {
    try {
      setSavedAudiobooks(await listNativeSavedAudiobooks())
    } catch (err) {
      logTtsDiagnostic('[tts-native] saved audiobook registry failed', {
        error: nativeTtsErrorDetail(err),
      }, 'warn')
    }
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
    // Native manifests are the completed-audio registry; WebView storage is not durable enough.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    void refreshSavedAudiobooks()
  }, [refreshSavedAudiobooks])

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
    const installingSilmaRuntime = ttsModelStatus?.runtimeInstalled === false
    logTtsDiagnostic('[tts-native] model install started', {
      modelId: ttsModelId,
      installingSilmaRuntime,
      ...summarizeTtsModelStatus(ttsModelStatus),
    })
    setTtsModelProgress({
      modelId: ttsModelId,
      status: 'starting',
      message: installingSilmaRuntime ? i18n.t('tts.setup.installingSilma') : i18n.t('tts.setup.preparingDownload'),
      downloadedBytes: 0,
      totalBytes: ttsModelStatus?.archiveBytes ?? 0,
      percent: 0,
    })
    try {
      const result = await installNativeTtsModel(ttsModelId)
      const status = await refreshTtsModelStatus()
      await syncTtsRuntimeSettings()
      if (ttsModelIdRef.current !== ttsModelId) return
      logTtsDiagnostic('[tts-native] model install completed', {
        resultModelDir: result.modelDir,
        resultBytes: result.bytes,
        ...summarizeTtsModelStatus(status),
      }, status.installed && status.runtimeInstalled ? 'info' : 'warn')
      if (!status.installed || !status.runtimeInstalled) {
        setTtsModelProgress(null)
        return
      }
      setTtsModelProgress((prev) => ({
        modelId: ttsModelId,
        status: 'installed',
        message: i18n.t('tts.setup.installed'),
        downloadedBytes: prev?.totalBytes ?? ttsModelStatus?.archiveBytes ?? 0,
        totalBytes: prev?.totalBytes ?? ttsModelStatus?.archiveBytes ?? 0,
        percent: 100,
      }))
      preloadTts()
    } catch (err) {
      if (ttsModelIdRef.current !== ttsModelId) return
      logTtsDiagnostic('[tts-native] model install failed', {
        modelId: ttsModelId,
        error: nativeTtsErrorDetail(err),
        ...summarizeTtsModelStatus(ttsModelStatus),
      }, 'error')
      setTtsModelProgress({
        modelId: ttsModelId,
        status: 'error',
        message: nativeTtsErrorMessage(err),
        downloadedBytes: 0,
        totalBytes: ttsModelStatus?.archiveBytes ?? 0,
        percent: 0,
      })
      void refreshTtsModelStatus()
    }
  }, [preloadTts, syncTtsRuntimeSettings, refreshTtsModelStatus, ttsModelId, ttsModelStatus])

  const handleProbeSilmaSidecar = useCallback(async () => {
    // Diagnostics-only smoke path; real model loading/synthesis still runs through save.
    if (silmaProbeRunning) return
    setSilmaProbeRunning(true)
    try {
      const result = await probeNativeSilmaSidecar()
      logTtsDiagnostic('[tts-native] SILMA sidecar probe passed', { ...result })
    } catch (err) {
      logTtsDiagnostic('[tts-native] SILMA sidecar probe failed', {
        error: nativeTtsErrorDetail(err),
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
          setSilmaNfeStep(resolveSilmaNfeStep(metadata))
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
            const rebuiltChunks = audiobookSaveChunksFromHtml(docContent, metadata.modelId)
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
          setTtsSaveChunks(audiobookSaveChunksFromHtml(docContent, ttsModelId))
          setImportedHighlightStatus('unavailable')
        })
      return () => {
        cancelled = true
        cancelHighlightBuild?.()
      }
    }

    setImportedHighlightStatus('ready')
    setTtsSaveChunks(audiobookSaveChunksFromHtml(docContent, ttsModelId))
  }, [docContent, selectedDoc, setTtsModelId, ttsModelId])

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
      silmaNfeStep,
      documentUrl: selectedDoc,
      title: getDocumentTitle(selectedDoc),
    })
  }, [checkSelectedAudiobook, getDocumentTitle, selectedDoc, silmaNfeStep, ttsDtype, ttsModelId, ttsSaveChunks, ttsSpeed, ttsTextPreprocessor, ttsThreadCount, ttsVoice])

  useEffect(() => {
    saveAudioPreferences({ modelId: ttsModelId, voice: ttsVoice, textPreprocessor: ttsTextPreprocessor, silmaNfeStep })
  }, [silmaNfeStep, ttsModelId, ttsTextPreprocessor, ttsVoice])

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
        silmaNfeStep: audiobookDownload.silmaNfeStep,
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
        silmaNfeStep: audiobookDownload.silmaNfeStep,
        dtype: audiobookDownload.dtype,
        status: 'paused',
        cachedChunks: downloadAudiobookState.cachedChunks,
        totalChunks: downloadAudiobookState.totalChunks,
        message: downloadAudiobookState.message || i18n.t('tts.status.readyToResume'),
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
        silmaNfeStep: audiobookDownload.silmaNfeStep,
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
    clearCompletedAudiobookDownload(audiobookDownload.url, {
      modelId: audiobookDownload.modelId,
      textPreprocessor: audiobookDownload.textPreprocessor,
      voice: audiobookDownload.voice,
      speed: audiobookDownload.speed,
      silmaNfeStep: audiobookDownload.silmaNfeStep,
      dtype: audiobookDownload.dtype,
    })
    void refreshSavedAudiobooks()
    refreshAudiobookDownloads()
  }, [audiobookDownload, downloadAudiobookState.complete, flushAudiobookDownloadPersist, refreshAudiobookDownloads, refreshSavedAudiobooks])

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

  const getAudiobookSaveChunksForDocument = useCallback(async (
    documentUrl: string,
    modelId = ttsModelId,
  ): Promise<TtsChunk[]> => {
    if (isUserUploadUrl(documentUrl)) {
      return (await getImportedAudiobookMetadata(documentUrl)).chunks
    }

    const html = await loadHtmlDocument(documentUrl)
    return audiobookSaveChunksFromHtml(html, modelId)
  }, [loadHtmlDocument, ttsModelId])

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
      silmaNfeStep,
      documentUrl: selectedDoc ?? undefined,
      title: selectedDoc ? getDocumentTitle(selectedDoc) : undefined,
    })
  }, [getDocumentTitle, getSelectedAudiobookSaveChunks, selectedAudiobookState.complete, selectedDoc, silmaNfeStep, speakTts, ttsDtype, ttsModelId, ttsSpeed, ttsTextPreprocessor, ttsThreadCount, ttsVoice])

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
    silmaNfeStep?: number
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
      silmaNfeStep: input.silmaNfeStep,
      dtype: input.dtype,
      status: 'queued',
      cachedChunks: 0,
      totalChunks: speakableChunks.length,
      message: i18n.t('tts.status.queued'),
      audioDurationSec: 0,
    }, true)
    setAudiobookDownload({ title: input.title, url: input.documentUrl, modelId: input.modelId, textPreprocessor: input.textPreprocessor, voice: input.voice, speed: input.speed, dtype: input.dtype, silmaNfeStep: input.silmaNfeStep })
    saveAudiobook(input.chunks, {
      modelId: input.modelId,
      textPreprocessor: input.textPreprocessor,
      voice: input.voice,
      speed: input.speed,
      dtype: input.dtype,
      threadCount: ttsThreadCount,
      silmaNfeStep: input.silmaNfeStep,
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
        title: i18n.t('tts.confirm.nothingTitle'),
        description: i18n.t('tts.confirm.nothingDescription'),
        confirmLabel: i18n.t('tts.confirm.ok'),
        cancelLabel: null,
      })
      return
    }

    const textPreprocessorName = formatTextPreprocessorLabel(
      i18n.t,
      ttsTextPreprocessor,
      selectedTtsModel.textPreprocessors.find((item) => item.id === ttsTextPreprocessor)?.name,
    )
    const confirmed = await confirmAudiobookAction({
      title: i18n.t('tts.confirm.saveTitle'),
      description: i18n.t('tts.confirm.saveDescription'),
      details: [
        { label: i18n.t('tts.confirm.document'), value: title },
        { label: i18n.t('tts.confirm.model'), value: selectedTtsModel.name },
        { label: i18n.t('tts.confirm.voice'), value: getTtsVoiceName(ttsModels, ttsModelId, ttsVoice) },
        { label: i18n.t('tts.confirm.generatedSpeed'), value: formatSpeedLabel(DEFAULT_TTS_SPEED) },
        { label: i18n.t('tts.confirm.processing'), value: textPreprocessorName },
        { label: i18n.t('tts.confirm.threads'), value: ttsThreadCount },
        ...(selectedTtsModel.family === 'silma-f5'
          ? [{ label: i18n.t('tts.confirm.silmaQuality'), value: 'NFE ' + silmaNfeStep }]
          : []),
        { label: i18n.t('tts.confirm.chunks'), value: speakableChunks.length },
      ],
      confirmLabel: i18n.t('tts.confirm.startSaving'),
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
      silmaNfeStep,
      dtype: ttsDtype,
    })
  }, [confirmAudiobookAction, getDocumentTitle, getSelectedAudiobookSaveChunks, selectedDoc, selectedTtsModel.family, selectedTtsModel.name, selectedTtsModel.textPreprocessors, silmaNfeStep, startAudiobookSave, ttsDtype, ttsModelId, ttsModels, ttsTextPreprocessor, ttsThreadCount, ttsVoice])

  const handleResumeAudiobookDownload = useCallback(async (record: AudiobookDownloadRecord) => {
    startAudiobookSave({
      documentUrl: record.documentUrl,
      title: record.title,
      chunks: await getAudiobookSaveChunksForDocument(record.documentUrl, record.modelId),
      modelId: record.modelId,
      textPreprocessor: record.textPreprocessor,
      voice: record.voice,
      speed: record.speed,
      silmaNfeStep: record.silmaNfeStep,
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
        silmaNfeStep: audiobookDownload.silmaNfeStep,
        dtype: audiobookDownload.dtype,
        status: 'paused',
        cachedChunks: downloadAudiobookState.cachedChunks,
        totalChunks: downloadAudiobookState.totalChunks,
        message: i18n.t('tts.status.pausedReady'),
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
      ? i18n.t('tts.controls.chunkProgress', {
          current: record.cachedChunks,
          total: record.totalChunks,
        })
      : null
    const duration = record.audioDurationSec ? formatDuration(record.audioDurationSec) : null
    const storage = formatStorageSize(record.wavBytes)
    const confirmed = await confirmAudiobookAction({
      title: i18n.t('tts.confirm.removeJobTitle'),
      description: i18n.t('tts.confirm.removeJobDescription'),
      details: [
        { label: i18n.t('tts.confirm.title'), value: record.title },
        ...(progress ? [{ label: i18n.t('tts.confirm.progress'), value: progress }] : []),
        ...(duration ? [{ label: i18n.t('tts.confirm.duration'), value: duration }] : []),
        ...(storage ? [{ label: i18n.t('tts.confirm.storage'), value: storage }] : []),
      ],
      confirmLabel: i18n.t('tts.confirm.removeJob'),
      tone: 'danger',
    })
    if (!confirmed) return

    removeAudiobookDownload(id)
    refreshAudiobookDownloads()
  }, [audiobookDownloads, confirmAudiobookAction, refreshAudiobookDownloads])

  const handleExportSavedAudiobook = useCallback(async (record: SavedAudiobookRecord, exportFormat: NativeAudiobookExportFormat) => {
    const confirmed = await confirmAudiobookAction({
      title: i18n.t('tts.confirm.exportTitle'),
      description: getAiAudioExportDescription(record),
      details: [
        { label: i18n.t('tts.confirm.title'), value: record.title },
        { label: i18n.t('tts.confirm.voice'), value: getTtsVoiceName(ttsModels, record.modelId, record.voice) },
        { label: i18n.t('tts.confirm.format'), value: exportFormat === 'wav' ? 'WAV' : i18n.t('tts.audiobooks.exportBundle') },
        { label: i18n.t('tts.confirm.disclosure'), value: getAiAudioExportDisclosure(record) },
      ],
      confirmLabel: i18n.t('tts.audiobooks.export'),
    })
    if (!confirmed) return

    dismissAudiobookNotice()
    setAudiobookExport({
      id: record.id,
      status: 'exporting',
      message: exportFormat === 'wav' ? i18n.t('tts.audiobooks.exportingWav') : i18n.t('tts.audiobooks.exportingBundle'),
    })
    try {
      const chunks = await getAudiobookSaveChunksForDocument(record.documentUrl, record.modelId)
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
          silmaNfeStep: record.silmaNfeStep,
        },
        exportFormat,
      })
      setAudiobookExport(null)
      showAudiobookNotice({
        id: 'export:' + record.id,
        status: 'success',
        message: formatAudiobookExportMessage(i18n.t, result.path, exportFormat),
      })
    } catch (err) {
      const message = nativeTtsErrorMessage(err)
      const cancelled = isNativeTtsCancellation(err)
      setAudiobookExport(null)
      showAudiobookNotice({
        id: 'export:' + record.id,
        status: cancelled ? 'cancelled' : 'error',
        message: cancelled ? i18n.t('tts.audiobooks.exportCancelled') : message,
      })
    }
  }, [confirmAudiobookAction, dismissAudiobookNotice, getAudiobookSaveChunksForDocument, loadHtmlDocument, showAudiobookNotice, ttsModels])

  const handleDeleteSavedAudiobook = useCallback(async (record: SavedAudiobookRecord) => {
    const deleteUserUpload = isUserUploadUrl(record.documentUrl)
    const duration = record.audioDurationSec ? formatDuration(record.audioDurationSec) : null
    const storage = formatStorageSize(record.wavBytes)
    const confirmed = await confirmAudiobookAction({
      title: deleteUserUpload ? i18n.t('tts.confirm.deleteImportTitle') : i18n.t('tts.confirm.deleteTitle'),
      description: deleteUserUpload
        ? i18n.t('tts.confirm.deleteImportDescription')
        : i18n.t('tts.confirm.deleteDescription'),
      details: [
        { label: i18n.t('tts.confirm.title'), value: record.title },
        ...(duration ? [{ label: i18n.t('tts.confirm.duration'), value: duration }] : []),
        ...(storage ? [{ label: i18n.t('tts.confirm.storage'), value: storage }] : []),
      ],
      confirmLabel: i18n.t('tts.confirm.deleteAudiobook'),
      tone: 'danger',
    })
    if (!confirmed) return

    setAudiobookDelete({ id: record.id, status: 'deleting', message: i18n.t('tts.audiobooks.deletingAction') })
    try {
      const result = await deleteNativeAudiobook({
        audiobookId: record.id,
        documentUrl: record.documentUrl,
        deleteUserUpload,
      })

      if (deleteUserUpload) removeUserUpload(record.documentUrl)
      await refreshSavedAudiobooks()
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
        message: storage
          ? i18n.t('tts.audiobooks.deletedWithStorage', { storage })
          : i18n.t('tts.audiobooks.deleted'),
      })
    } catch (err) {
      setAudiobookDelete(null)
      showAudiobookNotice({
        id: 'delete:' + record.id,
        status: 'error',
        message: nativeTtsErrorMessage(err),
      })
    }
  }, [confirmAudiobookAction, onClearDocument, onUserUploadsChanged, refreshSavedAudiobooks, resetSelectedAudiobookState, selectedDoc, showAudiobookNotice, stopTts])

  const importAudiobook = useCallback(async (openDocument: (url: string) => Promise<void>) => {
    dismissAudiobookNotice()
    setAudiobookImport({ status: 'importing', message: i18n.t('tts.audiobooks.importingAction') })
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
        silmaNfeStep: result.silmaNfeStep,
        chunks: result.chunks,
        audioDurationSec: result.audioDurationSec,
        wavBytes: result.wavBytes,
      })
      onUserUploadsChanged()
      await refreshSavedAudiobooks()
      autoSelectedDocumentRef.current = result.documentUrl
      preserveGeneratedSpeedOnOpenRef.current = true
      setTtsModelId(result.modelId)
      setTtsVoice(result.voice as TtsVoice)
      setTtsTextPreprocessor(result.textPreprocessor)
      setTtsSpeed(result.speed)
      setSilmaNfeStep(resolveSilmaNfeStep({ silmaNfeStep: result.silmaNfeStep }))
      setAudiobookImport({ status: 'idle', message: '' })
      showAudiobookNotice({
        id: 'import:' + result.documentUrl,
        status: 'success',
        message: i18n.t('tts.audiobooks.imported', { title: result.title }),
      })
      try {
        await openDocument(result.documentUrl)
      } catch (err) {
        showAudiobookNotice({
          id: 'open:' + result.documentUrl,
          status: 'error',
          message: i18n.t('reader.unableToOpen') + ' ' + nativeTtsErrorMessage(err),
        })
      }
    } catch (err) {
      const message = nativeTtsErrorMessage(err)
      const cancelled = isNativeTtsCancellation(err)
      setAudiobookImport({ status: 'idle', message: '' })
      showAudiobookNotice({
        id: 'import',
        status: cancelled ? 'cancelled' : 'error',
        message: cancelled ? i18n.t('tts.audiobooks.importCancelled') : message,
      })
    }
  }, [dismissAudiobookNotice, onUserUploadsChanged, refreshSavedAudiobooks, setTtsModelId, showAudiobookNotice])

  const openSavedAudiobook = useCallback(async (record: SavedAudiobookRecord, openDocument: (url: string) => Promise<void>) => {
    autoSelectedDocumentRef.current = record.documentUrl
    preserveGeneratedSpeedOnOpenRef.current = true
    setTtsModelId(record.modelId)
    setTtsVoice(record.voice as TtsVoice)
    setTtsTextPreprocessor(record.textPreprocessor)
    setTtsSpeed(record.speed)
    setSilmaNfeStep(resolveSilmaNfeStep(record))
    await openDocument(record.documentUrl)
  }, [setTtsModelId])

  const includeDocumentInList = useCallback((doc: DocumentInfo) => (
    !audioSavedOnly || savedAudiobookIds.has(createAudiobookId(doc.url, {
      modelId: ttsModelId,
      textPreprocessor: ttsTextPreprocessor,
      voice: ttsVoice,
      speed: ttsSpeed,
      dtype: ttsDtype,
      silmaNfeStep,
    }))
  ), [audioSavedOnly, savedAudiobookIds, silmaNfeStep, ttsDtype, ttsModelId, ttsSpeed, ttsTextPreprocessor, ttsVoice])

  const filterResults = useCallback((results: SearchResult[]) => (
    audioSavedOnly
      ? results.filter((result) => savedAudiobookIds.has(createAudiobookId(result.url, {
        modelId: ttsModelId,
        textPreprocessor: ttsTextPreprocessor,
        voice: ttsVoice,
        speed: ttsSpeed,
        dtype: ttsDtype,
        silmaNfeStep,
      })))
      : results
  ), [audioSavedOnly, savedAudiobookIds, silmaNfeStep, ttsDtype, ttsModelId, ttsSpeed, ttsTextPreprocessor, ttsVoice])

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
      silmaNfeStep,
    })
    : null
  const activeDownloadId = audiobookDownload
    ? createAudiobookDownloadId(audiobookDownload.url, {
      modelId: audiobookDownload.modelId,
      textPreprocessor: audiobookDownload.textPreprocessor,
      voice: audiobookDownload.voice,
      speed: audiobookDownload.speed,
      dtype: audiobookDownload.dtype,
      silmaNfeStep: audiobookDownload.silmaNfeStep,
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
  const canSaveAudiobook = Boolean(ttsModelStatus?.installed && ttsModelStatus.runtimeInstalled) &&
    audioControlsAudiobookState.status !== 'checking' &&
    !isDifferentAudiobookSaving
  const isSavingAudiobook = activeDownloadIsRunning
  const activeDownloadTitle = audiobookDownload?.title ?? i18n.t('tts.audiobooks.defaultTitle')
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
    ? audiobookImport.message || i18n.t('tts.audiobooks.importingAction')
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
      playbackNotice: importedHighlightPreparing ? i18n.t('tts.status.preparingHighlights') : undefined,
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
      onSilmaNfeStepChange: (nfeStep: number) => setSilmaNfeStep(resolveSilmaNfeStep({ silmaNfeStep: nfeStep })),
      onSpeedChange: () => {},
      onTextPreprocessorChange: setTtsTextPreprocessor,
      onThreadCountChange: handleThreadCountChange,
      onVoiceChange: setTtsVoice,
      textPreprocessor: ttsTextPreprocessor,
      textPreprocessors: selectedTtsModel.textPreprocessors,
      speed: DEFAULT_TTS_SPEED,
      silmaProbeRunning,
      silmaNfeStep,
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

// Rebuild runtime source spans from current HTML every open. The selected model
// chooses the chunk profile, so SILMA can keep requests below its F5 subchunker.
function audiobookSaveChunksFromHtml(html: string, modelId: string): TtsChunk[] {
  const profile = modelId === SILMA_MODEL_ID ? SILMA_AUDIOBOOK_SAVE_CHUNK_PROFILE : undefined
  return buildRuntimeChunks(chunkAudiobookSaveHtmlWithSpans(html, profile), 'save-c')
}

// Assign deterministic cache ids while carrying optional UI-only highlight spans.
function buildRuntimeChunks(chunks: SpeechChunk[], prefix: string): TtsChunk[] {
  return chunks.map((chunk, index) => ({
    id: prefix + String(index + 1).padStart(5, '0'),
    text: chunk.text,
    sourceSpan: chunk.sourceSpan,
  }))
}
