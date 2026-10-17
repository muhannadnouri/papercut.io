import { useCallback, useEffect, useId, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { TFunction } from 'i18next'
import { AppSelect } from '../components/AppSelect/AppSelect'
import { AppDialog } from '../components/AppDialog/AppDialog'
import {
  cancelNetworkTransfer, dismissNetworkTransfer, getNetworkTransferStatus, retryNetworkTransfer, startNetworkTransfer,
  isNetworkTransferActive, getTransferInterfaces, type TransferInterface,
  type TransferConnectionMode, type LibraryTransferNetworkStatus,
  exportLibrary, importLibrary, isLibraryTransferErrorPayload, listenLibraryTransferProgress,
  type LibraryTransferExportResult, type LibraryTransferImportResult, type LibraryTransferProgress,
} from './libraryTransfer'
import { listNativeSavedAudiobooks } from '../tts/api/nativeTts'
import type { SavedAudiobookRecord } from '../tts/storage/AudiobookLibrary'
import { isUserUploadUrl, upsertUserUpload } from '../tts/storage/UserUploads'
import { formatSavedAudiobookMetaParts } from '../tts/utils/format'
import { listUploadedDocuments, type UploadedDocument } from '../uploads/DocumentUploads'
import { filterTransferDocuments, updateScopedSelection } from './documentSelection'
import './LibraryTransferDialog.css'
import { TransferDiagnostics } from './TransferDiagnostics'
import { NearbySessions } from './NearbySessions'

interface LibraryTransferDialogProps {
  onBack: () => void
  onClose: () => void
  onImported: () => void | Promise<void>
}

type TransferMode = 'send' | 'receive'
type TransferStatus =
  | { state: 'idle' }
  | { state: 'exporting' }
  | { state: 'importing' }

  | { state: 'exported'; result: LibraryTransferExportResult }
  | { state: 'imported'; result: LibraryTransferImportResult }
  | { state: 'error'; message: string }

const DOCUMENT_FILTER_THRESHOLD = 20

export function LibraryTransferDialog({ onBack, onClose, onImported }: LibraryTransferDialogProps) {
  const { t, i18n } = useTranslation()
  const [mode, setMode] = useState<TransferMode>('send')
  const [status, setStatus] = useState<TransferStatus>({ state: 'idle' })
  const [progress, setProgress] = useState<LibraryTransferProgress | null>(null)
  const [progressUnavailable, setProgressUnavailable] = useState(false)
  const [documents, setDocuments] = useState<UploadedDocument[]>([])
  const [documentsLoaded, setDocumentsLoaded] = useState(false)
  const [documentsLoadFailed, setDocumentsLoadFailed] = useState(false)
  const [selectedDocumentIds, setSelectedDocumentIds] = useState<string[]>([])
  const [documentFilter, setDocumentFilter] = useState('')
  const [showSelectedDocuments, setShowSelectedDocuments] = useState(false)
  const [savedAudiobooks, setSavedAudiobooks] = useState<SavedAudiobookRecord[]>([])
  const [selectedAudiobookIds, setSelectedAudiobookIds] = useState<string[]>([])
  const [networkStatus, setNetworkStatus] = useState<LibraryTransferNetworkStatus | null>(null)
  const [ownershipState, setOwnershipState] = useState<'loading' | 'failed' | 'ready'>('loading')
  const [dismissing, setDismissing] = useState(false)
  const [connectionMode, setConnectionMode] = useState<TransferConnectionMode>('push')
  const [starting, setStarting] = useState(false)
  const [networks, setNetworks] = useState<TransferInterface[]>([])
  const [networkId, setNetworkId] = useState('')
  const [networksLoading, setNetworksLoading] = useState(true)
  const [networkError, setNetworkError] = useState(false)
  const [port, setPort] = useState('53318')
  const [statusUnavailable, setStatusUnavailable] = useState(false)
  const [controlError, setControlError] = useState<string | null>(null)
  const [cancelling, setCancelling] = useState(false)
  const operationAttempt = useRef(0)
  const appliedResult = useRef<number | null>(null)
  const alive = useRef(true)
  const validPort = /^\d+$/.test(port) && Number(port) >= 1024 && Number(port) <= 65535
  const listening = (mode === 'receive') === (connectionMode === 'push')
  const portHelpId = useId()
  const [sourceAddress, setSourceAddress] = useState('')
  const [pairingCode, setPairingCode] = useState('')
  const validConnection = Boolean(networkId) && (listening ? validPort : Boolean(sourceAddress.trim() && pairingCode.trim()))
  const selectionHelpId = useId()
  const exportPrivacyId = useId()
  const operationBusy = ['exporting', 'importing'].includes(status.state)
  const networkActive = isNetworkTransferActive(networkStatus)
  const networkSessionId = networkStatus?.id
  const networkFlowVisible = starting || Boolean(networkStatus && networkStatus.state !== 'cancelled')
  const busy = operationBusy || starting || networkActive || dismissing || ownershipState !== 'ready'
  const locale = i18n.resolvedLanguage ?? i18n.language
  const technicalT = i18n.getFixedT('en')
  const requiredDocumentIds = useMemo(() => {
    const documentIdByUrl = new Map(documents.map((document) => [document.url, document.id]))
    return new Set(savedAudiobooks
      .filter((record) => selectedAudiobookIds.includes(record.id))
      .map((record) => documentIdByUrl.get(record.documentUrl))
      .filter((id): id is string => Boolean(id)))
  }, [documents, savedAudiobooks, selectedAudiobookIds])
  const effectiveDocumentIds = useMemo(() => {
    const selected = new Set(selectedDocumentIds)
    requiredDocumentIds.forEach((id) => selected.add(id))
    return documents.filter((document) => selected.has(document.id)).map((document) => document.id)
  }, [documents, requiredDocumentIds, selectedDocumentIds])
  const filteredDocuments = useMemo(() => filterTransferDocuments(
    documents,
    documentFilter,
    locale,
    effectiveDocumentIds,
    showSelectedDocuments,
  ), [documentFilter, documents, effectiveDocumentIds, locale, showSelectedDocuments])
  const documentFilterActive = documentFilter.trim().length > 0
  const scopedDocumentIds = filteredDocuments.map((document) => document.id)
  const allScopedDocumentsSelected = scopedDocumentIds.length > 0
    && scopedDocumentIds.every((id) => selectedDocumentIds.includes(id))
  const anyScopedDocumentsSelected = scopedDocumentIds
    .some((id) => selectedDocumentIds.includes(id))
  const hasContent = effectiveDocumentIds.length > 0 || selectedAudiobookIds.length > 0
  const sendSelectionReady = documentsLoaded && !documentsLoadFailed
  const selectionEmpty = sendSelectionReady && !hasContent
  /** Reload the sendable documents while exposing the existing loading and failure states. */
  const loadDocuments = useCallback(async () => {
    setDocumentsLoaded(false)
    setDocumentsLoadFailed(false)
    try {
      const nextDocuments = await listUploadedDocuments()
      setDocuments(nextDocuments)
      setSelectedDocumentIds(nextDocuments.map((document) => document.id))
    } catch {
      setDocumentsLoadFailed(true)
    } finally {
      setDocumentsLoaded(true)
    }
  }, [])

  const loadNetworks = useCallback(async () => {
    setNetworksLoading(true)
    setNetworkError(false)
    setNetworks([])
    setNetworkId('')
    try {
      const next = await getTransferInterfaces()
      setNetworks(next)
      const automatic = next.filter(network => network.automatic)
      if (automatic.length === 1) setNetworkId(automatic[0].id)
      if (!next.length) setNetworkError(true)
    } catch { setNetworkError(true) }
    finally { setNetworksLoading(false) }
  }, [])

  useEffect(() => { void loadNetworks() }, [loadNetworks])

  useEffect(() => {
    void loadDocuments()
  }, [loadDocuments])

  useEffect(() => {
    void listNativeSavedAudiobooks()
      .then(setSavedAudiobooks)
      .catch(() => setSavedAudiobooks([]))
  }, [])

  useEffect(() => {
    let unlisten: (() => void) | undefined
    let disposed = false
    void listenLibraryTransferProgress((next) => {
      if (disposed || next.operation === 'receive') return
      setProgress(next)
    })
      .then((stop) => {
        if (disposed) stop()
        else unlisten = stop
      }).catch(() => { if (!disposed) setProgressUnavailable(true) })
    return () => {
      disposed = true
      unlisten?.()
    }
  }, [])

  const applyImportResult = useCallback(async (result: LibraryTransferImportResult, attempt = operationAttempt.current) => {
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
    if (alive.current && operationAttempt.current === attempt) {
      setProgress(null)
      setStatus({ state: 'imported', result })
    }
  }, [onImported])

  const handleExport = async () => {
    if (busy) return
    setProgress(null)
    setStatus({ state: 'exporting' })
    try {
      const result = await exportLibrary(effectiveDocumentIds, selectedAudiobookIds)
      setStatus(result ? { state: 'exported', result } : { state: 'idle' })
    } catch (error) {
      setStatus({ state: 'error', message: formatTransferError(error, locale, t) })
    }
  }

  const handleImport = async () => {
    if (busy) return
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

  useEffect(() => {
    alive.current = true
    return () => { alive.current = false }
  }, [])

  useEffect(() => {
    if (ownershipState !== 'loading') return
    let disposed = false
    void getNetworkTransferStatus().then(next => {
      if (disposed) return
      if (next) {
        setMode(next.role === 'sender' ? 'send' : 'receive')
        setConnectionMode(next.mode)
        setNetworkStatus(next)
      }
      setOwnershipState('ready')
    }).catch(() => { if (!disposed) setOwnershipState('failed') })
    return () => { disposed = true }
  }, [ownershipState])

  useEffect(() => {
    if (networkSessionId === undefined || !networkActive) return
    const id = networkSessionId
    let disposed = false, pending = false
    const timer = window.setInterval(() => {
      if (pending) return
      pending = true
      void getNetworkTransferStatus(id).then(next => {
        if (disposed || !next || next.id !== id) return
        setNetworkStatus(next)
        setStatusUnavailable(false)
        if (!isNetworkTransferActive(next) || !next.cancellable) setCancelling(false)
      }).catch(() => { if (!disposed) setStatusUnavailable(true) })
        .finally(() => { pending = false })
    }, 500)
    return () => { disposed = true; window.clearInterval(timer) }
  }, [networkSessionId, networkActive])

  useEffect(() => {
    if (networkStatus?.state !== 'complete' || networkStatus.role !== 'receiver'
      || !networkStatus.result || appliedResult.current === networkStatus.id) return
    appliedResult.current = networkStatus.id
    const attempt = operationAttempt.current
    void applyImportResult(networkStatus.result).catch(error => {
      if (alive.current && operationAttempt.current === attempt) setControlError(formatTransferError(error, locale, t))
    })
  }, [networkStatus, applyImportResult, locale, t])

  const handleStartNetwork = async () => {
    if (busy) return
    const attempt = ++operationAttempt.current
    setProgress(null)
    setNetworkStatus(null)
    setCancelling(false)
    setControlError(null)
    setStatusUnavailable(false)
    setStarting(true)
    setStatus({ state: 'idle' })
    try {
      const next = await startNetworkTransfer({
        role: mode === 'send' ? 'sender' : 'receiver', mode: connectionMode, interfaceId: networkId,
        ...(listening ? { port: Number(port) } : { address: sourceAddress.trim(), code: pairingCode }),
        ...(mode === 'send' ? { selection: { documentIds: effectiveDocumentIds, audiobookIds: selectedAudiobookIds } } : {}),
      })
      if (alive.current && operationAttempt.current === attempt) setNetworkStatus(next)
    } catch (error) {
      if (alive.current && operationAttempt.current === attempt) setStatus({ state: 'error', message: formatTransferError(error, locale, t) })
    } finally {
      if (alive.current && operationAttempt.current === attempt) setStarting(false)
    }
  }

  const handleCancelNetwork = async () => {
    if (!networkStatus || cancelling) return
    if (!networkActive) { await resetNetwork(); return }
    const id = networkStatus.id
    const attempt = operationAttempt.current
    setCancelling(true)
    setControlError(null)
    try {
      const cancelled = await cancelNetworkTransfer(id)
      if (!cancelled) {
        const next = await getNetworkTransferStatus(id)
        if (alive.current && operationAttempt.current === attempt) {
          if (next?.id === id) setNetworkStatus(next)
          setCancelling(false)
        }
      }
    } catch (error) {
      if (alive.current && operationAttempt.current === attempt) { setCancelling(false); setControlError(formatTransferError(error, locale, t)) }
    }
  }

  const handleRetryNetwork = async () => {
    if (!networkStatus?.canRetry) return
    setControlError(null)
    const id = networkStatus.id
    const attempt = operationAttempt.current
    try {
      const next = await retryNetworkTransfer(id)
      if (alive.current && operationAttempt.current === attempt && next.id === id) setNetworkStatus(next)
    } catch (error) { if (alive.current && operationAttempt.current === attempt) setControlError(formatTransferError(error, locale, t)) }
  }

  const resetNetwork = async () => {
    if (networkActive || dismissing) return false
    const previous = networkStatus
    operationAttempt.current += 1
    const attempt = operationAttempt.current
    setDismissing(true)
    setControlError(null)
    try {
      if (previous) await dismissNetworkTransfer(previous.id)
      if (!alive.current || operationAttempt.current !== attempt) return false
      setNetworkStatus(null)
      setCancelling(false)
      setStatusUnavailable(false)
      setSourceAddress('')
      setPairingCode('')
      setStatus({ state: 'idle' })
      setProgress(null)
      return true
    } catch (error) {
      if (alive.current && operationAttempt.current === attempt) setControlError(formatTransferError(error, locale, t))
      return false
    } finally {
      if (alive.current && operationAttempt.current === attempt) setDismissing(false)
    }
  }
  const selectMode = async (nextMode: TransferMode) => {
    if (busy) return
    if (!await resetNetwork()) return
    setMode(nextMode)
    setConnectionMode('push')
  }
  const selectConnectionMode = async (next: TransferConnectionMode) => {
    if (busy) return
    if (!await resetNetwork()) return
    setConnectionMode(next)
  }
  const leave = async (navigate: () => void) => {
    const attempt = operationAttempt.current
    try {
      if (networkStatus && !networkActive) await dismissNetworkTransfer(networkStatus.id)
      if (alive.current && operationAttempt.current === attempt) navigate()
    } catch (error) {
      if (alive.current) setControlError(formatTransferError(error, locale, t))
    }
  }

  return (
    <AppDialog
      title={t('libraryTransfer.title')}
      description={t('libraryTransfer.description')}
      showCloseButton
      navigationDisabled={busy && ownershipState !== 'failed'}
      onBack={() => { void leave(onBack) }}
      onCancel={() => { void leave(onClose) }}
    >
      <div className="library-transfer-mode" role="group" aria-label={t('libraryTransfer.modeLabel')}>
        <button type="button" aria-pressed={mode === 'send'} disabled={busy} onClick={() => { void selectMode('send') }}>
          {t('libraryTransfer.modeSend')}
        </button>
        <button type="button" aria-pressed={mode === 'receive'} disabled={busy} onClick={() => { void selectMode('receive') }}>
          {t('libraryTransfer.modeReceive')}
        </button>
      </div>

      {ownershipState === 'loading' && <p role="status" aria-live="polite">{t('libraryTransfer.diagnostics.loading')}</p>}
      {ownershipState === 'failed' && <div role="alert">
        <p>{t('libraryTransfer.statusUnavailable')}</p>
        <button type="button" onClick={() => setOwnershipState('loading')}>{t('common.retry')}</button>
      </div>}

      {!networkFlowVisible && status.state !== 'imported' && <section className="library-transfer-primary-section library-transfer-network">
        <div className="library-transfer-mode" role="group" aria-label={t('libraryTransfer.connectionMethod')}>
          <button type="button" aria-pressed={connectionMode === 'push'} disabled={busy} onClick={() => { void selectConnectionMode('push') }}>
            {t(mode === 'send' ? 'libraryTransfer.connectToReceiver' : 'libraryTransfer.letSenderConnect')}
          </button>
          <button type="button" aria-pressed={connectionMode === 'pull'} disabled={busy} onClick={() => { void selectConnectionMode('pull') }}>
            {t(mode === 'send' ? 'libraryTransfer.letReceiverConnect' : 'libraryTransfer.connectToSender')}
          </button>
        </div>
        <AppSelect
          ariaLabel={t('libraryTransfer.network')}
          value={networkId}
          options={[{ value: '', label: t('libraryTransfer.chooseNetwork'), disabled: true }, ...networks.map(network => ({
            value: network.id, label: `${network.name} · ${network.addresses.join(', ')}`,
          }))]}
          disabled={busy || !networks.length}
          onChange={setNetworkId}
        />
        {!networkError && <button type="button" disabled={busy || networksLoading} onClick={() => { void loadNetworks() }}>{t('libraryTransfer.refreshNetworks')}</button>}
        {networkError && <div role="alert">
          <p>{t('libraryTransfer.networkUnavailable')}</p>
          <button type="button" disabled={busy || networksLoading} onClick={() => { void loadNetworks() }}>{t('common.retry')}</button>
        </div>}
        {listening && <details className="library-transfer-alternate">
          <summary>{t('libraryTransfer.port')}</summary>
          <label className="library-transfer-port">
            <span>{t('libraryTransfer.port')}</span>
            <input type="number" min="1024" max="65535" value={port} disabled={busy} aria-invalid={!validPort} aria-describedby={portHelpId}
              onChange={event => setPort(event.target.value)} />
            <small id={portHelpId} dir="ltr">1024–65535</small>
          </label>
        </details>}
        {!listening && <NearbySessions key={`${networkId}:${connectionMode}`} interfaceId={networkId} mode={connectionMode} disabled={busy} onSelect={setSourceAddress} />}
        {!listening && <div className="library-transfer-pairing-fields">
          <label><span>{t('libraryTransfer.peerAddress')}</span>
            <input type="text" dir="ltr" value={sourceAddress} disabled={busy} placeholder="192.168.1.20:53318" onChange={event => setSourceAddress(event.target.value)} />
          </label>
          <label><span>{t('libraryTransfer.pairingCode')}</span>
            <input type="text" dir="ltr" value={pairingCode} disabled={busy} autoCapitalize="characters" autoCorrect="off" spellCheck={false}
              placeholder="ABCD-EFGH-JKLM" onChange={event => setPairingCode(event.target.value)} />
          </label>
        </div>}
      </section>}
      <div className="library-transfer-actions">
        {status.state === 'imported' ? <LibraryTransferCompletion kind="receive" result={status.result} />
        : networkStatus?.state === 'complete' && networkStatus.role === 'sender'
          ? <LibraryTransferCompletion kind="send" status={networkStatus} />
        : networkStatus?.state === 'complete' && networkStatus.result
          ? <LibraryTransferCompletion kind="receive" result={networkStatus.result} />
        : networkFlowVisible ? <LibraryNetworkFlow role={mode === 'send' ? 'sender' : 'receiver'} starting={starting} status={networkStatus} locale={locale} cancelling={cancelling} dismissing={dismissing}
            onExit={() => { void handleCancelNetwork() }} onRetry={() => { void handleRetryNetwork() }} />
        : mode === 'send' ? (
            <>
            <section className="library-transfer-primary-section">
              <header>
                <h3>{t('libraryTransfer.sendTitle')}</h3>
                <p>{t(connectionMode === 'push' ? 'libraryTransfer.sendDescription' : 'libraryTransfer.senderListensDescription')}</p>
              </header>
              {!documentsLoaded && (
                <div className="library-transfer-status library-transfer-status-busy" role="status" aria-live="polite">
                  <span className="spinner" aria-hidden="true" />
                  <span>{t('libraryTransfer.loadingDocuments')}</span>
                </div>
              )}
              {documentsLoadFailed && (
                <>
                  <p className="app-dialog-error" role="alert">
                    {t('libraryTransfer.documentsLoadFailed')}
                  </p>
                  <button type="button" onClick={() => { void loadDocuments() }}>
                    {t('common.retry')}
                  </button>
                </>
              )}
              {documents.length > 0 && (
                <details className="library-transfer-content">
                  <summary>
                    <span>
                      <strong aria-live="polite">
                        {t('libraryTransfer.includeDocuments', {
                          selected: effectiveDocumentIds.length,
                          count: documents.length,
                        })}
                      </strong>
                      <small>{t('libraryTransfer.includeDocumentsDescription')}</small>
                    </span>
                  </summary>
                  <div className="library-transfer-picker">
                    {documents.length >= DOCUMENT_FILTER_THRESHOLD && (
                      <>
                        <label className="library-transfer-filter">
                          <span>{t('libraryTransfer.filterDocuments')}</span>
                          <input
                            type="search"
                            value={documentFilter}
                            disabled={busy}
                            onChange={(event) => setDocumentFilter(event.target.value)}
                          />
                        </label>
                        <div className="library-transfer-filter-state">
                          <span role="status" aria-live="polite">
                            {t('libraryTransfer.showingDocuments', {
                              shown: filteredDocuments.length,
                              count: documents.length,
                            })}
                          </span>
                          <label>
                            <input
                              type="checkbox"
                              checked={showSelectedDocuments}
                              disabled={busy}
                              onChange={(event) => setShowSelectedDocuments(event.target.checked)}
                            />
                            <span>{t('libraryTransfer.showSelectedDocuments')}</span>
                          </label>
                        </div>
                      </>
                    )}
                    <div className="library-transfer-picker-actions">
                      {!showSelectedDocuments && (
                        <button
                          type="button"
                          disabled={busy || allScopedDocumentsSelected || scopedDocumentIds.length === 0}
                          onClick={() => setSelectedDocumentIds((current) => (
                            updateScopedSelection(current, scopedDocumentIds, true)
                          ))}
                        >
                          {t(documentFilterActive
                            ? 'libraryTransfer.selectMatchingDocuments'
                            : 'common.selectAll')}
                        </button>
                      )}
                      <button
                        type="button"
                        disabled={busy || !anyScopedDocumentsSelected}
                        onClick={() => setSelectedDocumentIds((current) => (
                          updateScopedSelection(current, scopedDocumentIds, false)
                        ))}
                      >
                        {t(showSelectedDocuments
                          ? 'libraryTransfer.deselectShownDocuments'
                          : documentFilterActive
                            ? 'libraryTransfer.deselectMatchingDocuments'
                            : 'common.deselectAll')}
                      </button>
                    </div>
                    <div className="library-transfer-picker-list">
                      {filteredDocuments.map((document) => {
                        const required = requiredDocumentIds.has(document.id)
                        return (
                          <label key={document.id}>
                            <input
                              type="checkbox"
                              checked={required || selectedDocumentIds.includes(document.id)}
                              disabled={busy || required}
                              onChange={(event) => {
                                setSelectedDocumentIds((current) => event.target.checked
                                  ? [...current, document.id]
                                  : current.filter((id) => id !== document.id))
                              }}
                            />
                            <span>
                              <strong><bdi>{document.title}</bdi></strong>
                              <small className="library-transfer-item-meta">
                                <span dir="ltr">{document.format.toUpperCase()}</span>
                                {required && <span>{t('libraryTransfer.requiredByAudiobook')}</span>}
                              </small>
                            </span>
                          </label>
                        )
                      })}
                      {filteredDocuments.length === 0 && (
                        <p className="library-transfer-empty">
                          {t(showSelectedDocuments
                            ? 'libraryTransfer.noSelectedDocuments'
                            : 'libraryTransfer.noMatchingDocuments')}
                        </p>
                      )}
                    </div>
                  </div>
                </details>
              )}
              {savedAudiobooks.length > 0 && (
                <details className="library-transfer-content">
                  <summary>
                    <span>
                      <strong aria-live="polite">
                        {t('libraryTransfer.includeAudiobooks', {
                          selected: selectedAudiobookIds.length,
                          count: savedAudiobooks.length,
                        })}
                      </strong>
                      <small>{t('libraryTransfer.includeAudiobooksDescription')}</small>
                    </span>
                  </summary>
                  <div className="library-transfer-picker">
                    <div className="library-transfer-picker-actions">
                      <button
                        type="button"
                        disabled={busy || selectedAudiobookIds.length === savedAudiobooks.length}
                        onClick={() => setSelectedAudiobookIds(savedAudiobooks.map((record) => record.id))}
                      >
                        {t('common.selectAll')}
                      </button>
                      <button
                        type="button"
                        disabled={busy || selectedAudiobookIds.length === 0}
                        onClick={() => setSelectedAudiobookIds([])}
                      >
                        {t('common.deselectAll')}
                      </button>
                    </div>
                    <div className="library-transfer-picker-list">
                      {savedAudiobooks.map((record) => {
                        const meta = formatSavedAudiobookMetaParts(
                          technicalT,
                          record.modelId,
                          record.voice,
                          record.speed,
                          record.textPreprocessor,
                          record.audioDurationSec,
                          record.wavBytes,
                        )
                        return (
                          <label key={record.id}>
                            <input
                              type="checkbox"
                              checked={selectedAudiobookIds.includes(record.id)}
                              disabled={busy}
                              onChange={(event) => {
                                setSelectedAudiobookIds((current) => event.target.checked
                                  ? [...current, record.id]
                                  : current.filter((id) => id !== record.id))
                              }}
                            />
                            <span>
                              <strong><bdi>{record.title}</bdi></strong>
                              <small className="library-transfer-item-meta" dir="ltr">
                                {meta.map((part) => <span key={part}>{part}</span>)}
                              </small>
                            </span>
                          </label>
                        )
                      })}
                    </div>
                  </div>
                </details>
              )}
              {selectionEmpty && (
                <p id={selectionHelpId} role="status">{t('libraryTransfer.selectAtLeastOne')}</p>
              )}
              <button
                type="button"
                className="library-transfer-primary"
                disabled={busy || !sendSelectionReady || !hasContent || !validConnection}
                aria-describedby={selectionEmpty ? selectionHelpId : undefined}
                onClick={() => { void handleStartNetwork() }}
              >
                {t('libraryTransfer.startSending')}
              </button>
            </section>

            <details className="library-transfer-alternate">
              <summary>{t('libraryTransfer.exportTitle')}</summary>
              <div className="library-transfer-alternate-content">
                <p>{t('libraryTransfer.exportDescription', {
                  documents: effectiveDocumentIds.length,
                  audiobooks: selectedAudiobookIds.length,
                })}</p>
                <p id={exportPrivacyId} className="library-transfer-privacy-note">
                  {t('libraryTransfer.exportPrivacy')}
                </p>
                <button
                  type="button"
                  disabled={busy || !sendSelectionReady || !hasContent}
                  aria-busy={status.state === 'exporting' || undefined}
                  aria-describedby={selectionEmpty
                    ? `${selectionHelpId} ${exportPrivacyId}`
                    : exportPrivacyId}
                  onClick={() => { void handleExport() }}
                >
                  {status.state === 'exporting' ? t('libraryTransfer.exporting') : t('libraryTransfer.export')}
                </button>
                {status.state === 'exporting' && <TransferPreparationMessage />}
              </div>
            </details>
            </>
        ) : (
          <>
            <section className="library-transfer-primary-section">
              <header>
                <h3>{t('libraryTransfer.receiveTitle')}</h3>
                <p>{t(listening ? 'libraryTransfer.receiveDescription' : 'libraryTransfer.receiverConnectsDescription')}</p>
              </header>
              <button type="button" className="library-transfer-primary" disabled={busy || !validConnection} onClick={() => { void handleStartNetwork() }}>
                {t(listening ? 'libraryTransfer.showPairingDetails' : 'libraryTransfer.receive')}
              </button>
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
      {statusUnavailable && busy && <p role="alert" className="app-dialog-error">{t('libraryTransfer.statusUnavailable')}</p>}
      {controlError && <p role="alert" className="app-dialog-error">{controlError}</p>}
      {progressUnavailable && <p role="alert">{t('libraryTransfer.diagnostics.progressUnavailable')}</p>}
      <TransferDiagnostics active={busy} revision={`${status.state}:${networkStatus?.state ?? ''}`} />
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

export function LibraryNetworkFlow({ role, starting, status, locale, onExit, onRetry, cancelling = false, dismissing = false }: {
  role: 'sender' | 'receiver'; starting: boolean; status: LibraryTransferNetworkStatus | null; locale: string; onExit: () => void; onRetry: () => void; cancelling?: boolean; dismissing?: boolean
}) {
  const { t } = useTranslation()
  const active = isNetworkTransferActive(status)
  return <section className="library-transfer-primary-section library-transfer-guided">
    <header><h3>{t(role === 'receiver' ? 'libraryTransfer.receiveTitle' : 'libraryTransfer.sendTitle')}</h3></header>
    {starting || status?.state === 'preparing' || status?.state === 'binding' ? <TransferPreparationMessage receiving={role === 'receiver'} />
      : <LibraryNetworkStatusMessage status={status} locale={locale} />}
    {status?.canRetry && <button type="button" disabled={cancelling || dismissing} onClick={onRetry}>{t('common.retry')}</button>}
    <button type="button" disabled={starting || cancelling || dismissing || (active && !status?.cancellable)} onClick={onExit}
      className={active ? 'library-transfer-stop' : undefined}>
      {t(cancelling ? 'libraryTransfer.cancelling' : active ? 'libraryTransfer.cancelTransfer' : 'libraryTransfer.changeSelection')}
    </button>
  </section>
}

export function TransferPreparationMessage({ receiving = false }: { receiving?: boolean }) {
  const { t } = useTranslation()
  return (
    <div className="library-transfer-status library-transfer-status-busy" role="status" aria-live="polite">
      <span className="spinner" aria-hidden="true" />
      <span>{t(receiving ? 'libraryTransfer.exporting' : 'libraryTransfer.preparingSelection')}</span>
    </div>
  )
}

function TransferStatusMessage({ status }: { status: TransferStatus }) {
  const { t } = useTranslation()
  if (
    status.state === 'idle'
    || status.state === 'exporting'
    || status.state === 'importing'
    || status.state === 'imported'
  ) {
    return null
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

  return null
}

type LibraryTransferCompletionProps =
  | { kind: 'send'; status: LibraryTransferNetworkStatus }
  | { kind: 'receive'; result: LibraryTransferImportResult }

export function LibraryTransferCompletion(props: LibraryTransferCompletionProps) {
  const { t } = useTranslation()
  const result = props.kind === 'receive' ? props.result : null
  const hasFailures = Boolean(result && (result.failed > 0 || result.audiobooksFailed > 0))

  return (
    <section
      className={`library-transfer-completion${hasFailures ? ' library-transfer-completion-issues' : ''}`}
      role="status"
      aria-live="polite"
    >
      <span className="library-transfer-completion-icon" aria-hidden="true">
        {hasFailures ? '!' : '✓'}
      </span>
      <h3>{t(hasFailures
        ? 'libraryTransfer.completionPartialTitle'
        : 'libraryTransfer.completionTitle')}</h3>
      {props.kind === 'send' ? (
        <>
          {/* The sender receives package counts, not the target's per-item import result. */}
          <p>{t('libraryTransfer.sendComplete')}</p>
          <div className="library-transfer-result">
            <span>{t('libraryTransfer.documentsSent', { count: props.status.documents })}</span>
            {props.status.audiobooks > 0 && (
              <span>{t('libraryTransfer.audiobooksSent', { count: props.status.audiobooks })}</span>
            )}
          </div>
        </>
      ) : (
        <div className="library-transfer-result">
          <span>{t('libraryTransfer.importComplete', {
            imported: props.result.imported,
            skipped: props.result.skipped,
            failed: props.result.failed,
          })}</span>
          {props.result.audiobooksSelected > 0 && (
            <span>{t('libraryTransfer.audiobooksImportComplete', {
              imported: props.result.audiobooksImported,
              skipped: props.result.audiobooksSkipped,
              failed: props.result.audiobooksFailed,
            })}</span>
          )}
          {props.result.failures.length > 0 && (
            <details>
              <summary>{t('libraryTransfer.failureDetails')}</summary>
              <ul>
                {props.result.failures.map((failure, index) => (
                  <li key={`${failure.item}-${index}`}>
                    <bdi>{failure.item}</bdi>: <span dir="auto">{failure.error}</span>
                  </li>
                ))}
              </ul>
            </details>
          )}
        </div>
      )}
    </section>
  )
}

function LibraryNetworkStatusMessage({ status, locale }: { status: LibraryTransferNetworkStatus | null; locale: string }) {
  const { t } = useTranslation()
  if (!status || status.state === 'cancelled') return null
  if (status.state === 'outcomeUnknown') return <p className="library-transfer-outcome-warning" role="alert">{t('libraryTransfer.outcomeUnknown')}</p>
  if (status.state === 'expired') return <p role="alert">{t('libraryTransfer.expired')}</p>
  if (status.state === 'failed') return <p className="app-dialog-error" role="alert" dir="auto">{formatTransferError(status.error, locale, t)}</p>
  const phase = status.state === 'waitingResume' ? 'waitingToResume'
    : status.state === 'waiting' ? 'waitingForReceiver'
    : status.state === 'finishingImport' ? 'finishingImport'
    : status.state === 'awaitingResult' ? 'awaitingResult'
    : status.state === 'verifying' ? 'progressVerifying' : 'progressConnecting'
  const copying = status.state === 'sending' || status.state === 'receiving'
  return <div className="library-transfer-session">
    {status.receiverProgress ? <TransferProgressMessage progress={status.receiverProgress} locale={locale} />
      : <div role="status" aria-live="polite">
        <span>{copying ? t(status.role === 'sender' ? 'libraryTransfer.progressSending' : 'libraryTransfer.progressReceiving', {
          processed: formatBytes(status.bytesTransferred, locale), total: formatBytes(status.packageBytes, locale),
        }) : t(`libraryTransfer.${phase}`)}</span>
        {copying && <progress value={status.packageBytes > 0 ? status.bytesTransferred : undefined} max={status.packageBytes || undefined} />}
      </div>}
    {status.error && <p className="app-dialog-error" role="alert" dir="auto">{formatTransferError(status.error, locale, t)}</p>}
    {status.listenerReady && <>
      {status.discovery?.state === 'advertising' && status.discovery.name && <p>
        {t('libraryTransfer.nearby.sessionName')}: <strong><bdi>{status.discovery.name}</bdi></strong>
      </p>}
      {status.discovery && status.discovery.state !== 'advertising' && <p role="status">{t(`libraryTransfer.nearby.${status.discovery.state}`)}</p>}
      <ol><li>{t(status.role === 'receiver' ? 'libraryTransfer.receiverPairingStep' : 'libraryTransfer.senderPairingStep')}</li>
        <li>{t('libraryTransfer.connectStepEnter')}</li></ol>
      <dl><div><dt>{t('libraryTransfer.peerAddress')}</dt><dd>{status.addresses.map(address => <code key={address} dir="ltr">{address}<br /></code>)}</dd></div>
        <div><dt>{t('libraryTransfer.pairingCode')}</dt><dd><code dir="ltr">{status.code}</code></dd></div></dl>
    </>}
    {status.listenerReady && status.expiresAtMs && <small>{t('libraryTransfer.expiresAt', { time: new Intl.DateTimeFormat(locale, { timeStyle: 'short' }).format(status.expiresAtMs) })}</small>}
    <small>{t('libraryTransfer.keepOpen')}</small>
  </div>
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
  if (isLibraryTransferErrorPayload(error) && error.code === 'TRANSFER_PORT_IN_USE') return t('libraryTransfer.portInUse')
  if (isLibraryTransferErrorPayload(error)) return error.message
  return error instanceof Error ? error.message : String(error)
}
