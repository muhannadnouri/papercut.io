import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { TFunction } from 'i18next'
import { AppDialog } from '../components/AppDialog/AppDialog'
import {
  cancelLibrarySend,
  exportLibrary,
  getLibrarySendStatus,
  importLibrary,
  isLibraryTransferErrorPayload,
  listenLibraryTransferProgress,
  receiveLibrary,
  startLibrarySend,
  type LibraryTransferExportResult,
  type LibraryTransferImportResult,
  type LibraryTransferProgress,
  type LibraryTransferSendStatus,
} from './libraryTransfer'
import { listNativeSavedAudiobooks } from '../tts/api/nativeTts'
import { isUserUploadUrl, upsertUserUpload } from '../tts/storage/UserUploads'
import './LibraryTransferDialog.css'

interface LibraryTransferDialogProps {
  documentCount: number
  onBack: () => void
  onImported: () => void | Promise<void>
}

type TransferMode = 'send' | 'receive'
type TransferStatus =
  | { state: 'idle' }
  | { state: 'exporting' }
  | { state: 'importing' }
  | { state: 'preparingSend' }
  | { state: 'receiving' }
  | { state: 'exported'; result: LibraryTransferExportResult }
  | { state: 'imported'; result: LibraryTransferImportResult }
  | { state: 'error'; message: string }

export function LibraryTransferDialog({ documentCount, onBack, onImported }: LibraryTransferDialogProps) {
  const { t, i18n } = useTranslation()
  const [mode, setMode] = useState<TransferMode>('send')
  const [status, setStatus] = useState<TransferStatus>({ state: 'idle' })
  const [progress, setProgress] = useState<LibraryTransferProgress | null>(null)
  const [savedAudiobookCount, setSavedAudiobookCount] = useState(0)
  const [includeAudiobooks, setIncludeAudiobooks] = useState(false)
  const [sendStatus, setSendStatus] = useState<LibraryTransferSendStatus | null>(null)
  const [sourceAddress, setSourceAddress] = useState('')
  const [pairingCode, setPairingCode] = useState('')
  const operationBusy = ['exporting', 'importing', 'preparingSend', 'receiving'].includes(status.state)
  const sendActive = sendStatus?.state === 'waiting' || sendStatus?.state === 'sending'
  const busy = operationBusy || sendActive
  const hasContent = documentCount > 0 || (includeAudiobooks && savedAudiobookCount > 0)
  const locale = i18n.resolvedLanguage ?? i18n.language

  useEffect(() => {
    void listNativeSavedAudiobooks()
      .then((records) => setSavedAudiobookCount(records.length))
      .catch(() => setSavedAudiobookCount(0))
  }, [])

  useEffect(() => {
    let unlisten: (() => void) | undefined
    let disposed = false
    void listenLibraryTransferProgress((next) => setProgress(next))
      .then((stop) => {
        if (disposed) stop()
        else unlisten = stop
      })
    return () => {
      disposed = true
      unlisten?.()
    }
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
          setStatus({ state: 'error', message: formatTransferError(error, locale, t) })
        })
    }, 500)
    return () => window.clearInterval(timer)
  }, [sendActive, locale, t])

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
    setProgress(null)
    setStatus({ state: 'imported', result })
  }

  const handleExport = async () => {
    setProgress(null)
    setStatus({ state: 'exporting' })
    try {
      const result = await exportLibrary(includeAudiobooks)
      setStatus(result ? { state: 'exported', result } : { state: 'idle' })
    } catch (error) {
      setStatus({ state: 'error', message: formatTransferError(error, locale, t) })
    }
  }

  const handleImport = async () => {
    setProgress(null)
    setStatus({ state: 'importing' })
    try {
      const result = await importLibrary()
      if (!result) {
        setStatus({ state: 'idle' })
        return
      }
      await applyImportResult(result)
    } catch (error) {
      setStatus({ state: 'error', message: formatTransferError(error, locale, t) })
    }
  }

  const handleStartSend = async () => {
    setProgress(null)
    setStatus({ state: 'preparingSend' })
    try {
      setSendStatus(await startLibrarySend(includeAudiobooks))
      setStatus({ state: 'idle' })
    } catch (error) {
      setStatus({ state: 'error', message: formatTransferError(error, locale, t) })
    }
  }

  const handleCancelSend = async () => {
    try {
      await cancelLibrarySend()
      setSendStatus((current) => current ? { ...current, state: 'cancelled' } : current)
    } catch (error) {
      setStatus({ state: 'error', message: formatTransferError(error, locale, t) })
    }
  }

  const handleReceive = async () => {
    setProgress(null)
    setStatus({ state: 'receiving' })
    try {
      await applyImportResult(await receiveLibrary(sourceAddress, pairingCode))
    } catch (error) {
      setStatus({ state: 'error', message: formatTransferError(error, locale, t) })
    }
  }

  const selectMode = (nextMode: TransferMode) => {
    if (busy) return
    setMode(nextMode)
    setStatus({ state: 'idle' })
    setProgress(null)
  }

  return (
    <AppDialog
      title={t('libraryTransfer.title')}
      description={t('libraryTransfer.description')}
      onCancel={busy ? () => {} : onBack}
      actions={(
        <button type="button" className="app-dialog-cancel" disabled={busy} onClick={onBack}>
          {t('common.back')}
        </button>
      )}
    >
      <div className="library-transfer-mode" role="group" aria-label={t('libraryTransfer.modeLabel')}>
        <button type="button" aria-pressed={mode === 'send'} disabled={busy} onClick={() => selectMode('send')}>
          {t('libraryTransfer.modeSend')}
        </button>
        <button type="button" aria-pressed={mode === 'receive'} disabled={busy} onClick={() => selectMode('receive')}>
          {t('libraryTransfer.modeReceive')}
        </button>
      </div>

      <div className="library-transfer-actions">
        {mode === 'send' ? (
          <>
            <section className="library-transfer-primary-section">
              <header>
                <h3>{t('libraryTransfer.sendTitle')}</h3>
                <p>{t('libraryTransfer.sendDescription')}</p>
              </header>
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
              {sendActive ? (
                <button type="button" className="library-transfer-stop" onClick={() => { void handleCancelSend() }}>
                  {t('libraryTransfer.stopSending')}
                </button>
              ) : (
                <button
                  type="button"
                  className="library-transfer-primary"
                  disabled={operationBusy || !hasContent}
                  onClick={() => { void handleStartSend() }}
                >
                  {status.state === 'preparingSend'
                    ? t('libraryTransfer.preparingSend')
                    : t('libraryTransfer.startSending')}
                </button>
              )}
              <LibrarySendStatusMessage status={sendStatus} locale={locale} />
            </section>

            <details className="library-transfer-alternate">
              <summary>{t('libraryTransfer.exportTitle')}</summary>
              <div className="library-transfer-alternate-content">
                <p>{t('libraryTransfer.exportDescription', { count: documentCount })}</p>
                <button type="button" disabled={busy || !hasContent} onClick={() => { void handleExport() }}>
                  {status.state === 'exporting' ? t('libraryTransfer.exporting') : t('libraryTransfer.export')}
                </button>
              </div>
            </details>
          </>
        ) : (
          <>
            <section className="library-transfer-primary-section">
              <header>
                <h3>{t('libraryTransfer.receiveTitle')}</h3>
                <p>{t('libraryTransfer.receiveDescription')}</p>
              </header>
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
              <button
                type="button"
                className="library-transfer-primary"
                disabled={busy || !sourceAddress.trim() || !pairingCode.trim()}
                onClick={() => { void handleReceive() }}
              >
                {status.state === 'receiving' ? t('libraryTransfer.receiving') : t('libraryTransfer.receive')}
              </button>
              {status.state === 'receiving' && progress?.operation === 'receive' && (
                <TransferProgressMessage progress={progress} locale={locale} />
              )}
            </section>

            <details className="library-transfer-alternate">
              <summary>{t('libraryTransfer.importTitle')}</summary>
              <div className="library-transfer-alternate-content">
                <p>{t('libraryTransfer.importDescription')}</p>
                <button type="button" disabled={busy} onClick={() => { void handleImport() }}>
                  {status.state === 'importing' ? t('libraryTransfer.importing') : t('libraryTransfer.import')}
                </button>
              </div>
            </details>
            {status.state === 'importing' && progress?.operation === 'import' && (
              <TransferProgressMessage progress={progress} locale={locale} />
            )}
          </>
        )}
      </div>

      <TransferStatusMessage status={status} />
    </AppDialog>
  )
}

function TransferProgressMessage({ progress, locale }: { progress: LibraryTransferProgress; locale: string }) {
  const { t } = useTranslation()
  const bytesKnown = progress.bytesProcessed !== undefined && progress.bytesTotal !== undefined
  const itemsKnown = progress.itemsProcessed !== undefined && progress.itemsTotal !== undefined
  const value = bytesKnown ? progress.bytesProcessed : itemsKnown ? progress.itemsProcessed : undefined
  const max = bytesKnown ? progress.bytesTotal : itemsKnown ? progress.itemsTotal : undefined
  const message = progress.phase === 'connecting'
    ? t('libraryTransfer.progressConnecting')
    : progress.phase === 'receiving' && bytesKnown
      ? t('libraryTransfer.progressReceiving', {
          processed: formatBytes(progress.bytesProcessed!, locale),
          total: formatBytes(progress.bytesTotal!, locale),
        })
      : progress.phase === 'verifying'
        ? t('libraryTransfer.progressVerifying')
        : progress.phase === 'importingDocuments' && itemsKnown
          ? t('libraryTransfer.progressImportingDocuments', {
              processed: progress.itemsProcessed,
              total: progress.itemsTotal,
            })
          : progress.phase === 'restoringAudiobooks' && itemsKnown
            ? t('libraryTransfer.progressRestoringAudiobooks', {
                processed: progress.itemsProcessed,
                total: progress.itemsTotal,
              })
            : t('libraryTransfer.importing')

  return (
    <div className="library-transfer-progress" role="status" aria-live="polite">
      <span>{message}</span>
      <progress value={value} max={max} />
      {progress.item && <small dir="auto"><bdi>{progress.item}</bdi></small>}
    </div>
  )
}

function TransferStatusMessage({ status }: { status: TransferStatus }) {
  const { t } = useTranslation()
  if (status.state === 'idle' || status.state === 'preparingSend' || status.state === 'receiving' || status.state === 'importing') {
    return null
  }
  if (status.state === 'exporting') {
    return (
      <div className="library-transfer-status library-transfer-status-busy" role="status" aria-live="polite">
        <span className="spinner" aria-hidden="true" />
        <span>{t('libraryTransfer.exporting')}</span>
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

function LibrarySendStatusMessage({ status, locale }: { status: LibraryTransferSendStatus | null; locale: string }) {
  const { t } = useTranslation()
  if (!status || status.state === 'cancelled') return null
  if (status.state === 'failed') {
    return <p className="app-dialog-error" role="alert" dir="auto">{status.error ?? t('libraryTransfer.sendFailed')}</p>
  }
  if (status.state === 'complete') {
    return <p className="library-transfer-status" role="status">{t('libraryTransfer.sendComplete')}</p>
  }
  const sending = status.state === 'sending'
  const receivingProgress = sending ? status.receiverProgress : undefined
  return (
    <div
      className="library-transfer-session"
      role={receivingProgress ? undefined : 'status'}
      aria-live={receivingProgress ? undefined : 'polite'}
    >
      {receivingProgress ? (
        <TransferProgressMessage progress={receivingProgress} locale={locale} />
      ) : (
        <>
          <span>{sending
            ? t('libraryTransfer.progressSending', {
                processed: formatBytes(status.bytesTransferred, locale),
                total: formatBytes(status.packageBytes, locale),
              })
            : status.error
              ? t('libraryTransfer.waitingToResume')
              : t('libraryTransfer.waitingForReceiver')}</span>
          {sending && <progress value={status.bytesTransferred} max={status.packageBytes} />}
        </>
      )}
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

function formatBytes(bytes: number, locale: string): string {
  const units = ['B', 'KB', 'MB', 'GB']
  let value = bytes
  let unit = 0
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024
    unit += 1
  }
  return `${new Intl.NumberFormat(locale, { maximumFractionDigits: unit === 0 ? 0 : 1 }).format(value)} ${units[unit]}`
}

function formatTransferError(error: unknown, locale: string, t: TFunction): string {
  if (
    isLibraryTransferErrorPayload(error)
    && error.code === 'insufficient-space'
    && error.requiredBytes !== undefined
    && error.availableBytes !== undefined
  ) {
    return t('libraryTransfer.insufficientSpace', {
      required: formatBytes(error.requiredBytes, locale),
      available: formatBytes(error.availableBytes, locale),
    })
  }
  if (isLibraryTransferErrorPayload(error)) return error.message
  return error instanceof Error ? error.message : String(error)
}
