import { useCallback, useEffect, useRef, useState } from 'react'
import i18n from '../../i18n'
import {
  deleteNativeAudiobook,
  exportNativeAudiobook,
  importNativeAudiobook,
  type NativeAudiobookExportFormat,
  type NativeAudiobookImportResult,
} from '../api/nativeTts'
import type { SavedAudiobookRecord } from '../storage/AudiobookLibrary'
import { isUserUploadUrl, removeUserUpload, upsertUserUpload } from '../storage/UserUploads'
import type { TtsChunk, TtsDtype } from '../types'
import { isNativeTtsCancellation, nativeTtsErrorMessage } from '../utils/errors'
import { formatAudiobookExportMessage, formatStorageSize } from '../utils/format'

type ExportState = { id: string; status: 'exporting'; message: string }
type DeleteState = { id: string; status: 'deleting'; message: string }
type ImportState = { status: 'idle' | 'importing'; message: string }
type NoticeState = { id: string; status: 'success' | 'cancelled' | 'error'; message: string }
type ExportPreparation = { chunks: TtsChunk[]; sourceHtml?: string }

interface SavedAudiobookActionsOptions {
  refreshSavedAudiobooks: () => void | Promise<void>
  onUserUploadsChanged: () => void
}

const NOTICE_TIMEOUT_MS = 10000
const IDLE_IMPORT_STATE: ImportState = { status: 'idle', message: '' }

/** Owns native saved-audiobook mutations and their transient UI state. */
export function useSavedAudiobookActions({
  refreshSavedAudiobooks,
  onUserUploadsChanged,
}: SavedAudiobookActionsOptions) {
  const [exportState, setExportState] = useState<ExportState | null>(null)
  const [deleteState, setDeleteState] = useState<DeleteState | null>(null)
  const [importState, setImportState] = useState<ImportState>(IDLE_IMPORT_STATE)
  const [noticeState, setNoticeState] = useState<NoticeState | null>(null)
  const noticeTimerRef = useRef<number | null>(null)

  const clearNoticeTimer = useCallback(() => {
    if (noticeTimerRef.current === null) return
    window.clearTimeout(noticeTimerRef.current)
    noticeTimerRef.current = null
  }, [])

  const dismissNotice = useCallback(() => {
    clearNoticeTimer()
    setNoticeState(null)
  }, [clearNoticeTimer])

  const showNotice = useCallback((nextNotice: NoticeState) => {
    clearNoticeTimer()
    setNoticeState(nextNotice)
    noticeTimerRef.current = window.setTimeout(() => {
      setNoticeState((current) => (
        current?.id === nextNotice.id &&
        current.status === nextNotice.status &&
        current.message === nextNotice.message
          ? null
          : current
      ))
      noticeTimerRef.current = null
    }, NOTICE_TIMEOUT_MS)
  }, [clearNoticeTimer])

  const exportSaved = useCallback(async (
    record: SavedAudiobookRecord,
    exportFormat: NativeAudiobookExportFormat,
    prepare: () => Promise<ExportPreparation>,
  ) => {
    dismissNotice()
    setExportState({
      id: record.id,
      status: 'exporting',
      message: exportFormat === 'wav'
        ? i18n.t('tts.audiobooks.exportingWav')
        : i18n.t('tts.audiobooks.exportingBundle'),
    })
    try {
      const { chunks, sourceHtml } = await prepare()
      const result = await exportNativeAudiobook({
        documentUrl: record.documentUrl,
        title: record.title,
        sourceHtml,
        chunks,
        options: {
          modelId: record.modelId,
          textPreprocessor: record.textPreprocessor,
          voice: record.voice,
          speed: record.speed,
          dtype: record.dtype as TtsDtype,
          silmaNfeStep: record.silmaNfeStep,
        },
        exportFormat,
      })
      setExportState(null)
      showNotice({
        id: 'export:' + record.id,
        status: 'success',
        message: formatAudiobookExportMessage(i18n.t, result.path, exportFormat),
      })
    } catch (error) {
      const cancelled = isNativeTtsCancellation(error)
      setExportState(null)
      showNotice({
        id: 'export:' + record.id,
        status: cancelled ? 'cancelled' : 'error',
        message: cancelled
          ? i18n.t('tts.audiobooks.exportCancelled')
          : nativeTtsErrorMessage(error),
      })
    }
  }, [dismissNotice, showNotice])

  const deleteSaved = useCallback(async (
    record: SavedAudiobookRecord,
    onDeleted: (deleteUserUpload: boolean) => void | Promise<void>,
  ) => {
    const deleteUserUpload = isUserUploadUrl(record.documentUrl)
    setDeleteState({ id: record.id, status: 'deleting', message: i18n.t('tts.audiobooks.deletingAction') })
    try {
      const result = await deleteNativeAudiobook({
        audiobookId: record.id,
        documentUrl: record.documentUrl,
        deleteUserUpload,
      })
      if (deleteUserUpload) removeUserUpload(record.documentUrl)
      await refreshSavedAudiobooks()
      onUserUploadsChanged()
      await onDeleted(deleteUserUpload)

      const storage = formatStorageSize(result.bytesFreed)
      setDeleteState(null)
      showNotice({
        id: 'delete:' + record.id,
        status: 'success',
        message: storage
          ? i18n.t('tts.audiobooks.deletedWithStorage', { storage })
          : i18n.t('tts.audiobooks.deleted'),
      })
    } catch (error) {
      setDeleteState(null)
      showNotice({
        id: 'delete:' + record.id,
        status: 'error',
        message: nativeTtsErrorMessage(error),
      })
    }
  }, [onUserUploadsChanged, refreshSavedAudiobooks, showNotice])

  const importSaved = useCallback(async (
    openDocument: (url: string) => Promise<void>,
    onImported: (result: NativeAudiobookImportResult) => void | Promise<void>,
  ) => {
    dismissNotice()
    setImportState({ status: 'importing', message: i18n.t('tts.audiobooks.importingAction') })
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
      await onImported(result)
      setImportState(IDLE_IMPORT_STATE)
      showNotice({
        id: 'import:' + result.documentUrl,
        status: 'success',
        message: i18n.t('tts.audiobooks.imported', { title: result.title }),
      })
      try {
        await openDocument(result.documentUrl)
      } catch (error) {
        showNotice({
          id: 'open:' + result.documentUrl,
          status: 'error',
          message: i18n.t('reader.unableToOpen') + ' ' + nativeTtsErrorMessage(error),
        })
      }
    } catch (error) {
      const cancelled = isNativeTtsCancellation(error)
      setImportState(IDLE_IMPORT_STATE)
      showNotice({
        id: 'import',
        status: cancelled ? 'cancelled' : 'error',
        message: cancelled
          ? i18n.t('tts.audiobooks.importCancelled')
          : nativeTtsErrorMessage(error),
      })
    }
  }, [dismissNotice, onUserUploadsChanged, refreshSavedAudiobooks, showNotice])

  useEffect(() => () => clearNoticeTimer(), [clearNoticeTimer])

  const actionMessage = importState.status === 'importing'
    ? importState.message || i18n.t('tts.audiobooks.importingAction')
    : exportState?.message
      ? exportState.message + '...'
      : deleteState?.message
        ? deleteState.message + '...'
        : ''

  return {
    actionBusy: Boolean(actionMessage),
    actionMessage,
    deleteState,
    dismissNotice,
    exportSaved,
    exportState,
    importSaved,
    importState,
    deleteSaved,
    noticeState,
  }
}
