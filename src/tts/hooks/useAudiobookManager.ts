import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import i18n from '../../i18n'
import type { DocumentInfo, SearchResult } from '../../types/search'
import type { UploadedDocument } from '../../uploads/DocumentUploads'
import {
  createAudiobookId,
  type SavedAudiobookRecord,
} from '../storage/AudiobookLibrary'
import {
  createAudiobookDownloadId,
  type AudiobookDownloadRecord,
} from '../storage/AudiobookDownloadQueue'
import { getAudioPreferences, saveAudioPreferences } from '../storage/audioPreferences'
import { getTtsModel, getTtsVoiceName, suggestTtsModel } from '../models'
import {
  formatDuration,
  formatSpeedLabel,
  formatStorageSize,
  formatTextPreprocessorLabel,
} from '../utils/format'
import {
  getImportedAudiobookMetadata,
  listNativeSavedAudiobooks,
  type NativeAudiobookExportFormat,
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
import { isUserUploadUrl, type UserUploadDocument } from '../storage/UserUploads'
import { logTtsDiagnostic } from '../diagnostics/TtsDiagnostics'
import { useAudiobookCache } from './useAudiobookCache'
import { useAudiobookDownloadQueue } from './useAudiobookDownloadQueue'
import { useSavedAudiobookActions } from './useSavedAudiobookActions'
import { useTtsModelRuntime } from './useTtsModelRuntime'
import { useTtsPlayer } from './useTtsPlayer'
import { useAppConfirmation } from '../../components/AppDialog/useAppConfirmation'
import { nativeTtsErrorDetail } from '../utils/errors'
import {
  chunksHaveDurableSourceSpans,
  countChunkSourceSpans,
  graftImportedSourceSpans,
} from '../alignment/importedSourceSpans'

type ImportedHighlightStatus = 'idle' | 'preparing' | 'ready' | 'unavailable'

function getAiAudioExportDescription(record: SavedAudiobookRecord): string {
  const description = i18n.t('tts.confirm.exportDescription')
  if (record.modelId !== SILMA_MODEL_ID) return description
  return description + i18n.t('tts.confirm.exportReferenceDescription')
}

function getAiAudioExportDisclosure(record: SavedAudiobookRecord): string {
  if (record.modelId === SILMA_MODEL_ID) return i18n.t('tts.confirm.disclosureReference')
  return i18n.t('tts.confirm.disclosureAi')
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
  const [ttsVoice, setTtsVoice] = useState<TtsVoice>(initialAudioPreferences.voice)
  const [ttsSpeed, setTtsSpeed] = useState(DEFAULT_TTS_SPEED)
  const [ttsPlaybackRate, setTtsPlaybackRate] = useState(initialAudioPreferences.playbackRate)
  const [ttsWordHighlightEnabled, setTtsWordHighlightEnabled] = useState(initialAudioPreferences.wordHighlightEnabled)
  const [ttsTextPreprocessor, setTtsTextPreprocessor] = useState<TextPreprocessorId>(initialAudioPreferences.textPreprocessor)
  const [silmaNfeStep, setSilmaNfeStep] = useState(() => resolveSilmaNfeStep(initialAudioPreferences))
  const ttsDtype: TtsDtype = initialAudioPreferences.dtype
  const [ttsSaveChunks, setTtsSaveChunks] = useState<TtsChunk[] | null>(null)
  const [importedHighlightStatus, setImportedHighlightStatus] = useState<ImportedHighlightStatus>('idle')
  const [savedAudiobooks, setSavedAudiobooks] = useState<SavedAudiobookRecord[]>([])
  const [audioSavedOnly, setAudioSavedOnly] = useState(initialAudioPreferences.audioSavedOnly)
  const { confirm: confirmAudiobookAction, dialog: confirmationDialog } = useAppConfirmation()
  const autoSelectedDocumentRef = useRef<string | null>(null)
  const preserveGeneratedSpeedOnOpenRef = useRef(false)
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
    defaultThreadCount: ttsDefaultThreadCount,
    installModel: handleInstallTtsModel,
    maxThreadCount: ttsMaxThreadCount,
    modelId: ttsModelId,
    modelProgress: ttsModelProgress,
    models: ttsModels,
    modelStatus: ttsModelStatus,
    onThreadCountChange: handleThreadCountChange,
    probeSilmaSidecar: handleProbeSilmaSidecar,
    setModelId: setTtsModelId,
    silmaProbeRunning,
    threadCount: ttsThreadCount,
  } = useTtsModelRuntime({
    initialModelId: initialAudioPreferences.modelId,
    preload: preloadTts,
  })
  const selectedTtsModel = getTtsModel(ttsModels, ttsModelId)
  const {
    state: selectedAudiobookState,
    check: checkSelectedAudiobook,
    reset: resetSelectedAudiobookState,
  } = useAudiobookCache()

  const savedAudiobookIds = useMemo(() => new Set(savedAudiobooks.map((record) => record.id)), [savedAudiobooks])

  const getDocumentTitle = useCallback((url: string): string => {
    return uploadedDocuments.find((doc) => doc.url === url)?.title
      ?? userUploads.find((doc) => doc.url === url)?.title
      ?? allDocuments.find((doc) => doc.url === url)?.title
      ?? decodeURIComponent(url.split('/').pop() ?? url)
  }, [allDocuments, uploadedDocuments, userUploads])

  const refreshSavedAudiobooks = useCallback(async () => {
    try {
      setSavedAudiobooks(await listNativeSavedAudiobooks())
    } catch (err) {
      logTtsDiagnostic('[tts-native] saved audiobook registry failed', {
        error: nativeTtsErrorDetail(err),
      }, 'warn')
    }
  }, [])

  const {
    actionBusy: audiobookActionBusy,
    actionMessage: audiobookActionMessage,
    deleteState: audiobookDelete,
    deleteSaved: deleteSavedAudiobook,
    dismissNotice: dismissAudiobookNotice,
    exportSaved: exportSavedAudiobook,
    exportState: audiobookExport,
    importSaved: importSavedAudiobook,
    importState: audiobookImport,
    noticeState: audiobookNotice,
  } = useSavedAudiobookActions({
    refreshSavedAudiobooks,
    onUserUploadsChanged,
  })

  const {
    activeDownload: audiobookDownload,
    downloads: audiobookDownloads,
    state: downloadAudiobookState,
    start: startAudiobookSave,
    cancel: handleCancelAudiobookSave,
    remove: removeQueuedAudiobookDownload,
  } = useAudiobookDownloadQueue({
    threadCount: ttsThreadCount,
    onCompleted: refreshSavedAudiobooks,
  })

  useEffect(() => {
    // Native manifests are the completed-audio registry; WebView storage is not durable enough.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    void refreshSavedAudiobooks()
  }, [refreshSavedAudiobooks])

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
      setTtsModelId(suggested.id)
      // eslint-disable-next-line react-hooks/set-state-in-effect
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
    setTtsModelId(model.id)
    setTtsVoice(model.defaultVoice)
    setTtsTextPreprocessor(model.defaultTextPreprocessor)
  }, [resetSelectedAudiobookState, setTtsModelId, stopTts, ttsModels])

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

    removeQueuedAudiobookDownload(id)
  }, [audiobookDownloads, confirmAudiobookAction, removeQueuedAudiobookDownload])

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

    await exportSavedAudiobook(record, exportFormat, async () => {
      const chunks = await getAudiobookSaveChunksForDocument(record.documentUrl, record.modelId)
      const sourceHtml = exportFormat === 'bundle'
        ? await loadHtmlDocument(record.documentUrl)
        : undefined
      return { chunks, sourceHtml }
    })
  }, [confirmAudiobookAction, exportSavedAudiobook, getAudiobookSaveChunksForDocument, loadHtmlDocument, ttsModels])

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

    await deleteSavedAudiobook(record, (deletedUserUpload) => {
      if (selectedDoc === record.documentUrl) {
        stopTts()
        resetSelectedAudiobookState()
        if (deletedUserUpload) {
          onClearDocument()
          setTtsSaveChunks(null)
        }
      }
    })
  }, [confirmAudiobookAction, deleteSavedAudiobook, onClearDocument, resetSelectedAudiobookState, selectedDoc, stopTts])

  const importAudiobook = useCallback(async (openDocument: (url: string) => Promise<void>) => {
    await importSavedAudiobook(openDocument, (result) => {
      autoSelectedDocumentRef.current = result.documentUrl
      preserveGeneratedSpeedOnOpenRef.current = true
      setTtsModelId(result.modelId)
      setTtsVoice(result.voice as TtsVoice)
      setTtsTextPreprocessor(result.textPreprocessor)
      setTtsSpeed(result.speed)
      setSilmaNfeStep(resolveSilmaNfeStep({ silmaNfeStep: result.silmaNfeStep }))
    })
  }, [importSavedAudiobook, setTtsModelId])

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
      defaultThreadCount: ttsDefaultThreadCount,
      maxThreadCount: ttsMaxThreadCount,
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
    refreshSavedAudiobooks,
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
