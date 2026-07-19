import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { AppDialog } from '../components/AppDialog/AppDialog'
import {
  cancelLibrarySend,
  exportLibrary,
  getLibrarySendStatus,
  importLibrary,
  receiveLibrary,
  startLibrarySend,
  type LibraryTransferExportResult,
  type LibraryTransferImportResult,
  type LibraryTransferSendStatus,
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
  | { state: 'preparingSend' }
  | { state: 'receiving' }
  | { state: 'exported'; result: LibraryTransferExportResult }
  | { state: 'imported'; result: LibraryTransferImportResult }
  | { state: 'error'; message: string }

export function LibraryTransferDialog({ documentCount, onClose, onImported }: LibraryTransferDialogProps) {
  const { t } = useTranslation()
  const [status, setStatus] = useState<TransferStatus>({ state: 'idle' })
  const [savedAudiobookCount, setSavedAudiobookCount] = useState(0)
  const [includeAudiobooks, setIncludeAudiobooks] = useState(false)
  const [sendStatus, setSendStatus] = useState<LibraryTransferSendStatus | null>(null)
  const [sourceAddress, setSourceAddress] = useState('')
  const [pairingCode, setPairingCode] = useState('')
  const operationBusy = ['exporting', 'importing', 'preparingSend', 'receiving'].includes(status.state)
  const sendActive = sendStatus?.state === 'waiting' || sendStatus?.state === 'sending'
  const busy = operationBusy || sendActive

  useEffect(() => {
    void listNativeSavedAudiobooks()
      .then((records) => setSavedAudiobookCount(records.length))
      .catch(() => setSavedAudiobookCount(0))
  }, [])

  useEffect(() => {
    if (!sendActive) return
    const timer = window.setInterval(() => {
      void getLibrarySendStatus()
        .then((next) => {
          if (!next) return
          setSendStatus(next)
          if (next.state === 'failed') {
            setStatus({ state: 'error', message: next.error ?? t('libraryTransfer.sendFailed') })
          }
        })
        .catch((error) => {
          setStatus({ state: 'error', message: error instanceof Error ? error.message : String(error) })
        })
    }, 750)
    return () => window.clearInterval(timer)
  }, [sendActive, t])

  const applyImportResult = async (result: LibraryTransferImportResult) => {
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
  }

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
      await applyImportResult(result)
    } catch (error) {
      setStatus({ state: 'error', message: error instanceof Error ? error.message : String(error) })
    }
  }

  const handleStartSend = async () => {
    setStatus({ state: 'preparingSend' })
    try {
      setSendStatus(await startLibrarySend(includeAudiobooks))
      setStatus({ state: 'idle' })
    } catch (error) {
      setStatus({ state: 'error', message: error instanceof Error ? error.message : String(error) })
    }
  }

  const handleCancelSend = async () => {
    try {
      await cancelLibrarySend()
      setSendStatus((current) => current ? { ...current, state: 'cancelled' } : current)
    } catch (error) {
      setStatus({ state: 'error', message: error instanceof Error ? error.message : String(error) })
    }
  }

  const handleReceive = async () => {
    setStatus({ state: 'receiving' })
    try {
      await applyImportResult(await receiveLibrary(sourceAddress, pairingCode))
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

        <div className="library-transfer-action">
          <div>
            <strong>{t('libraryTransfer.exportTitle')}</strong>
            <p>{t('libraryTransfer.exportDescription', { count: documentCount })}</p>
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

        <h3 className="library-transfer-section-title">{t('libraryTransfer.nearbyTitle')}</h3>

        <div className="library-transfer-action library-transfer-nearby-action">
          <div>
            <strong>{t('libraryTransfer.sendTitle')}</strong>
            <p>{t('libraryTransfer.sendDescription')}</p>
          </div>
          {sendActive ? (
            <button type="button" className="library-transfer-stop" onClick={() => { void handleCancelSend() }}>
              {t('libraryTransfer.stopSending')}
            </button>
          ) : (
            <button
              type="button"
              disabled={operationBusy || (documentCount === 0 && !(includeAudiobooks && savedAudiobookCount > 0))}
              onClick={() => { void handleStartSend() }}
            >
              {status.state === 'preparingSend'
                ? t('libraryTransfer.preparingSend')
                : t('libraryTransfer.startSending')}
            </button>
          )}
        </div>

        <LibrarySendStatusMessage status={sendStatus} />

        <div className="library-transfer-action library-transfer-receive-action">
          <div>
            <strong>{t('libraryTransfer.receiveTitle')}</strong>
            <p>{t('libraryTransfer.receiveDescription')}</p>
            <div className="library-transfer-pairing-fields">
              <label>
                <span>{t('libraryTransfer.sourceAddress')}</span>
                <input
                  type="text"
                  dir="ltr"
                  value={sourceAddress}
                  disabled={busy}
                  placeholder="192.168.1.20:49152"
                  onChange={(event) => setSourceAddress(event.target.value)}
                />
              </label>
              <label>
                <span>{t('libraryTransfer.pairingCode')}</span>
                <input
                  type="text"
                  dir="ltr"
                  value={pairingCode}
                  disabled={busy}
                  autoCapitalize="characters"
                  autoCorrect="off"
                  spellCheck={false}
                  placeholder="ABCD-EFGH"
                  onChange={(event) => setPairingCode(event.target.value)}
                />
              </label>
            </div>
          </div>
          <button
            type="button"
            disabled={busy || !sourceAddress.trim() || !pairingCode.trim()}
            onClick={() => { void handleReceive() }}
          >
            {status.state === 'receiving' ? t('libraryTransfer.receiving') : t('libraryTransfer.receive')}
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
  if (
    status.state === 'exporting'
    || status.state === 'importing'
    || status.state === 'preparingSend'
    || status.state === 'receiving'
  ) {
    const message = status.state === 'exporting'
      ? t('libraryTransfer.exporting')
      : status.state === 'importing'
        ? t('libraryTransfer.importing')
        : status.state === 'preparingSend'
          ? t('libraryTransfer.preparingSend')
          : t('libraryTransfer.receiving')
    return (
      <div className="library-transfer-status library-transfer-status-busy" role="status" aria-live="polite">
        <span className="spinner" aria-hidden="true" />
        <span>{message}</span>
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

function LibrarySendStatusMessage({ status }: { status: LibraryTransferSendStatus | null }) {
  const { t } = useTranslation()
  if (!status || status.state === 'cancelled') return null
  if (status.state === 'failed') {
    return <p className="app-dialog-error" role="alert" dir="auto">{status.error ?? t('libraryTransfer.sendFailed')}</p>
  }
  if (status.state === 'complete') {
    return <p className="library-transfer-status" role="status">{t('libraryTransfer.sendComplete')}</p>
  }
  return (
    <div className="library-transfer-session" role="status" aria-live="polite">
      <span>{status.state === 'sending'
        ? t('libraryTransfer.sending')
        : t('libraryTransfer.waitingForReceiver')}</span>
      <dl>
        <div>
          <dt>{t('libraryTransfer.sourceAddress')}</dt>
          <dd><code dir="ltr">{status.address}</code></dd>
        </div>
        <div>
          <dt>{t('libraryTransfer.pairingCode')}</dt>
          <dd><code dir="ltr">{status.code}</code></dd>
        </div>
      </dl>
      <small>{t('libraryTransfer.keepOpen')}</small>
    </div>
  )
}
