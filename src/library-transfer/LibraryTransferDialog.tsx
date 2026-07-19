import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { AppDialog } from '../components/AppDialog/AppDialog'
import {
  exportLibrary,
  importLibrary,
  type LibraryTransferExportResult,
  type LibraryTransferImportResult,
} from './libraryTransfer'
import { listNativeSavedAudiobooks } from '../tts/api/nativeTts'
import { isUserUploadUrl, upsertUserUpload } from '../tts/storage/UserUploads'
import './LibraryTransferDialog.css'

interface LibraryTransferDialogProps {
  documentCount: number
  onClose: () => void
  onImported: () => void | Promise<void>
}

type TransferStatus =
  | { state: 'idle' }
  | { state: 'exporting' }
  | { state: 'importing' }
  | { state: 'exported'; result: LibraryTransferExportResult }
  | { state: 'imported'; result: LibraryTransferImportResult }
  | { state: 'error'; message: string }

export function LibraryTransferDialog({ documentCount, onClose, onImported }: LibraryTransferDialogProps) {
  const { t } = useTranslation()
  const [status, setStatus] = useState<TransferStatus>({ state: 'idle' })
  const [savedAudiobookCount, setSavedAudiobookCount] = useState(0)
  const [includeAudiobooks, setIncludeAudiobooks] = useState(false)
  const busy = status.state === 'exporting' || status.state === 'importing'

  useEffect(() => {
    void listNativeSavedAudiobooks()
      .then((records) => setSavedAudiobookCount(records.length))
      .catch(() => setSavedAudiobookCount(0))
  }, [])

  const handleExport = async () => {
    setStatus({ state: 'exporting' })
    try {
      const result = await exportLibrary(includeAudiobooks)
      setStatus(result ? { state: 'exported', result } : { state: 'idle' })
    } catch (error) {
      setStatus({ state: 'error', message: error instanceof Error ? error.message : String(error) })
    }
  }

  const handleImport = async () => {
    setStatus({ state: 'importing' })
    try {
      const result = await importLibrary()
      if (!result) {
        setStatus({ state: 'idle' })
        return
      }
      for (const record of result.importedAudiobooks) {
        if (!isUserUploadUrl(record.documentUrl)) continue
        upsertUserUpload({
          url: record.documentUrl,
          title: record.title,
          modelId: record.modelId,
          textPreprocessor: record.textPreprocessor,
          voice: record.voice,
          speed: record.speed,
          dtype: record.dtype,
          silmaNfeStep: record.silmaNfeStep,
          chunks: record.chunks,
          audioDurationSec: record.audioDurationSec,
          wavBytes: record.wavBytes,
        })
      }
      if (result.imported > 0 || result.audiobooksImported > 0) await onImported()
      setStatus({ state: 'imported', result })
    } catch (error) {
      setStatus({ state: 'error', message: error instanceof Error ? error.message : String(error) })
    }
  }

  return (
    <AppDialog
      title={t('libraryTransfer.title')}
      description={t('libraryTransfer.description')}
      onCancel={busy ? () => {} : onClose}
      actions={(
        <button type="button" className="app-dialog-submit" disabled={busy} onClick={onClose}>
          {t('common.done')}
        </button>
      )}
    >
      <div className="library-transfer-actions">
        <div className="library-transfer-action">
          <div>
            <strong>{t('libraryTransfer.exportTitle')}</strong>
            <p>{t('libraryTransfer.exportDescription', { count: documentCount })}</p>
            {savedAudiobookCount > 0 && (
              <label className="library-transfer-audiobooks">
                <input
                  type="checkbox"
                  checked={includeAudiobooks}
                  disabled={busy}
                  onChange={(event) => setIncludeAudiobooks(event.target.checked)}
                />
                <span>
                  <strong>{t('libraryTransfer.includeAudiobooks', { count: savedAudiobookCount })}</strong>
                  <small>{t('libraryTransfer.includeAudiobooksDescription')}</small>
                </span>
              </label>
            )}
          </div>
          <button
            type="button"
            disabled={busy || (documentCount === 0 && !(includeAudiobooks && savedAudiobookCount > 0))}
            onClick={() => { void handleExport() }}
          >
            {status.state === 'exporting' ? t('libraryTransfer.exporting') : t('libraryTransfer.export')}
          </button>
        </div>

        <div className="library-transfer-action">
          <div>
            <strong>{t('libraryTransfer.importTitle')}</strong>
            <p>{t('libraryTransfer.importDescription')}</p>
          </div>
          <button type="button" disabled={busy} onClick={() => { void handleImport() }}>
            {status.state === 'importing' ? t('libraryTransfer.importing') : t('libraryTransfer.import')}
          </button>
        </div>
      </div>

      <TransferStatusMessage status={status} />
    </AppDialog>
  )
}

function TransferStatusMessage({ status }: { status: TransferStatus }) {
  const { t } = useTranslation()
  if (status.state === 'idle') return null
  if (status.state === 'exporting' || status.state === 'importing') {
    return (
      <div className="library-transfer-status library-transfer-status-busy" role="status" aria-live="polite">
        <span className="spinner" aria-hidden="true" />
        <span>{status.state === 'exporting'
          ? t('libraryTransfer.exporting')
          : t('libraryTransfer.importing')}</span>
      </div>
    )
  }
  if (status.state === 'error') {
    return <p className="app-dialog-error" role="alert" dir="auto">{status.message}</p>
  }
  if (status.state === 'exported') {
    return (
      <div className="library-transfer-status" role="status" aria-live="polite">
        <span>{t('libraryTransfer.exportComplete', { count: status.result.documents })}</span>
        {status.result.audiobooks > 0 && (
          <span>{t('libraryTransfer.audiobooksExported', { count: status.result.audiobooks })}</span>
        )}
      </div>
    )
  }

  return (
    <div className="library-transfer-result" role="status" aria-live="polite">
      <span>{t('libraryTransfer.importComplete', {
        imported: status.result.imported,
        skipped: status.result.skipped,
        failed: status.result.failed,
      })}</span>
      {status.result.audiobooksSelected > 0 && (
        <span>{t('libraryTransfer.audiobooksImportComplete', {
          imported: status.result.audiobooksImported,
          skipped: status.result.audiobooksSkipped,
          failed: status.result.audiobooksFailed,
        })}</span>
      )}
      {status.result.failures.length > 0 && (
        <details>
          <summary>{t('libraryTransfer.failureDetails')}</summary>
          <ul>
            {status.result.failures.map((failure, index) => (
              <li key={`${failure.item}-${index}`}>
                <bdi>{failure.item}</bdi>: <span dir="auto">{failure.error}</span>
              </li>
            ))}
          </ul>
        </details>
      )}
    </div>
  )
}
