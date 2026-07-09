import { useEffect, useState } from 'react'
import type { SavedAudiobookRecord } from '../storage/AudiobookLibrary'
import type { AudiobookDownloadRecord } from '../storage/AudiobookDownloadQueue'
import type { TtsDtype, TtsVoice } from '../types'
import type { NativeAudiobookExportFormat } from '../api/nativeTts'
import type { AudiobookCacheState } from '../hooks/useAudiobookCache'
import {
  formatAudiobookVoiceMeta,
  formatDownloadSavedStatus,
  formatSavedAudiobookMeta,
  formatSpeedLabel,
} from '../utils/format'
import { Panel } from '../../components/Panel/Panel'
import { AudioSetupPanel, type AudioSetupPanelProps } from './AudioSetupPanel'
import './AudiobooksPanel.css'

interface ActiveAudiobookSave {
  title: string
  url: string
  modelId: string
  textPreprocessor: string
  voice: TtsVoice
  speed: number
  dtype: TtsDtype
}

interface AudiobookExportState {
  id: string
  status: 'exporting' | 'exported' | 'cancelled' | 'error'
  message: string
}

interface AudiobookDeleteState {
  id: string
  status: 'deleting' | 'deleted' | 'error'
  message: string
}

interface AudiobookImportState {
  status: 'idle' | 'importing' | 'imported' | 'cancelled' | 'error'
  message: string
}

interface AudiobookNoticeState {
  status: 'success' | 'cancelled' | 'error'
  message: string
}

interface AudiobooksPanelProps {
  activeDownload: ActiveAudiobookSave | null
  audioSetup: AudioSetupPanelProps
  activeDownloadTitle: string
  deleteState: AudiobookDeleteState | null
  downloadState: AudiobookCacheState
  exportState: AudiobookExportState | null
  importState: AudiobookImportState
  noticeState: AudiobookNoticeState | null
  documentOpening?: boolean
  isSaving: boolean
  queuedDownloads: AudiobookDownloadRecord[]
  savedAudiobooks: SavedAudiobookRecord[]
  onCancelSave: () => void
  onDeleteSaved: (record: SavedAudiobookRecord) => void
  onDismissNotice: () => void
  onExportSaved: (record: SavedAudiobookRecord, format: NativeAudiobookExportFormat) => void
  onImportAudiobook: () => void
  onOpenSaved: (record: SavedAudiobookRecord) => void
  onRemoveQueued: (id: string) => void
  onResumeQueued: (record: AudiobookDownloadRecord) => void
}

const AUDIOBOOK_EXPORT_OPTIONS: Array<{
  format: NativeAudiobookExportFormat
  label: string
  detail: string
  code?: string
}> = [
  { format: 'bundle', label: 'Papercut Bundle', detail: 'Export as', code: '.papercut-audiobook' },
  { format: 'wav', label: 'WAV', detail: 'Export as', code: '.wav' },
]

export function AudiobooksPanel({
  activeDownload,
  audioSetup,
  activeDownloadTitle,
  deleteState,
  downloadState,
  exportState,
  importState,
  noticeState,
  documentOpening = false,
  isSaving,
  queuedDownloads,
  savedAudiobooks,
  onCancelSave,
  onDeleteSaved,
  onDismissNotice,
  onExportSaved,
  onImportAudiobook,
  onOpenSaved,
  onRemoveQueued,
  onResumeQueued,
}: AudiobooksPanelProps) {
  const [setupOpen, setSetupOpen] = useState(false)
  const [exportMenuOpen, setExportMenuOpen] = useState<string | null>(null)
  const activePercent = getDownloadPercent(downloadState.cachedChunks, downloadState.totalChunks)
  const savedCount = savedAudiobooks.length
  const queueCount = queuedDownloads.length
  const exportInProgress = exportState?.status === 'exporting'
  const importInProgress = importState.status === 'importing'
  const deleteInProgress = deleteState?.status === 'deleting'
  const panelBusy = exportInProgress || importInProgress || deleteInProgress
  const meta = formatAudiobookMeta(isSaving, queueCount, savedCount)
  const hasAudiobooks = isSaving || queueCount > 0 || savedCount > 0
  const setupSummary = formatAudioSetupSummary(audioSetup)

  useEffect(() => {
    if (!exportMenuOpen) return

    function handlePointerDown(event: PointerEvent) {
      const target = event.target
      if (target instanceof Element && target.closest('.audiobook-export-menu')) return
      setExportMenuOpen(null)
    }

    document.addEventListener('pointerdown', handlePointerDown)
    return () => document.removeEventListener('pointerdown', handlePointerDown)
  }, [exportMenuOpen])

  return (
    <Panel
      className={'audiobooks-panel' + (exportMenuOpen ? ' audiobooks-panel-menu-open' : '')}
      ariaLabel="Audiobooks"
      title="Audiobooks"
      meta={meta}
      defaultOpen
    >
      <div className="audiobooks-panel-content" aria-busy={panelBusy}>
        <div className="audiobooks-panel-interactive" inert={panelBusy ? true : undefined}>
          <div className="audiobooks-actions-row">
            <button
              type="button"
              className="audiobooks-import-btn"
              disabled={panelBusy}
              onClick={onImportAudiobook}
            >
              <AudiobooksPanelIcon name={importInProgress ? 'folder-open' : 'folder'} />
              {importInProgress ? 'Importing Bundle' : 'Import Bundle'}
            </button>

            <button
              type="button"
              className={'audiobooks-setup-disclosure' + (setupOpen ? ' audiobooks-setup-disclosure-open' : '')}
              aria-expanded={setupOpen}
              aria-controls="audiobooks-audio-setup"
              disabled={panelBusy}
              onClick={() => setSetupOpen((value) => !value)}
            >
              <span className="audiobooks-setup-disclosure-icon" aria-hidden="true">
                <AudiobooksPanelIcon name="settings" />
              </span>
              <span className="audiobooks-setup-disclosure-main">
                <span className="audiobooks-setup-disclosure-title">Audio Setup</span>
                <span className="audiobooks-setup-disclosure-summary">{setupSummary}</span>
              </span>
              <span className="audiobooks-setup-disclosure-chevron" aria-hidden="true">{setupOpen ? '▲' : '▼'}</span>
            </button>
          </div>

          {setupOpen && (
            <section id="audiobooks-audio-setup" className="audiobooks-section audiobooks-setup" aria-label="Audio Setup">
              <AudioSetupPanel {...audioSetup} />
            </section>
          )}

          {!hasAudiobooks && (
            <div className="audiobooks-empty">
              <h2>No saved audiobooks yet</h2>
              <p>Save audio from a document or import a Papercut audiobook bundle.</p>
            </div>
          )}

          <div className="audiobooks-list">
            {isSaving && (
          <section className="audiobooks-section" aria-label="Saving audiobook">
            <h3 className="audiobooks-section-title">Saving</h3>
            <div className="audiobook-item audiobook-item-active">
              <div className="audiobook-row">
                <span className="audiobook-title">{activeDownloadTitle}</span>
                <span className="audiobook-meta">{downloadState.cachedChunks}/{downloadState.totalChunks}</span>
              </div>
              <div className="audiobook-status-text">
                {activeDownload ? formatAudiobookVoiceMeta(activeDownload.modelId, activeDownload.voice, activeDownload.speed, activeDownload.dtype, activeDownload.textPreprocessor) + ' - ' : ''}{formatDownloadSavedStatus(downloadState.audioDurationSec, activePercent, downloadState.wavBytes)}
              </div>
              <div className="audio-progress-meter" aria-label={'Saving audiobook ' + activePercent + '% complete'}>
                <span style={{ width: activePercent + '%' }} />
              </div>
              <button className="audiobook-text-action audiobook-secondary" disabled={panelBusy} onClick={onCancelSave}>Pause</button>
            </div>
          </section>
        )}

            {queueCount > 0 && (
          <section className="audiobooks-section" aria-label="Audiobook queue">
            <h3 className="audiobooks-section-title">Queue</h3>
            {queuedDownloads.map((record) => {
              const percent = getDownloadPercent(record.cachedChunks, record.totalChunks)
              return (
                <div key={record.id} className={'audiobook-item audiobook-item-' + record.status}>
                  <div className="audiobook-row">
                    <span className="audiobook-title">{record.title}</span>
                    <span className="audiobook-meta">{record.cachedChunks}/{record.totalChunks}</span>
                  </div>
                  <div className="audiobook-status-text">
                    {formatAudiobookVoiceMeta(record.modelId, record.voice, record.speed, record.dtype, record.textPreprocessor) + ' - ' + formatDownloadSavedStatus(record.audioDurationSec, percent, record.wavBytes)}
                  </div>
                  <div className="audio-progress-meter" aria-label={'Audiobook save ' + percent + '% complete'}>
                    <span style={{ width: percent + '%' }} />
                  </div>
                  <div className="audiobook-actions">
                    <span className="audiobook-status-text">{record.message || record.status}</span>
                    <button className="audiobook-text-action audiobook-resume" disabled={panelBusy} onClick={() => onResumeQueued(record)}>
                      {record.status === 'error' ? 'Retry' : 'Resume'}
                    </button>
                    <button className="audiobook-text-action audiobook-secondary" disabled={panelBusy} onClick={() => onRemoveQueued(record.id)}>Remove</button>
                  </div>
                </div>
              )
            })}
          </section>
        )}

            {savedCount > 0 && (
          <section className="audiobooks-section" aria-label="Saved audiobooks">
            <h3 className="audiobooks-section-title">Saved</h3>
            {savedAudiobooks.map((record) => {
              const recordExportState = exportState?.id === record.id ? exportState : null
              const recordDeleteState = deleteState?.id === record.id ? deleteState : null
              const exporting = recordExportState?.status === 'exporting'
              const deleting = recordDeleteState?.status === 'deleting'
              const exportDisabled = panelBusy || deleting
              const deleteDisabled = panelBusy || deleting
              return (
                <div key={record.id} className="audiobook-item audiobook-item-saved">
                  <button
                    className="audiobook-saved-main"
                    disabled={documentOpening || panelBusy}
                    onClick={() => { if (!documentOpening) onOpenSaved(record) }}
                  >
                    <span className="audiobook-title">{record.title}</span>
                    <span className="audiobook-meta">
                      {formatSavedAudiobookMeta(
                        record.modelId,
                        record.voice,
                        record.speed,
                        record.textPreprocessor,
                        record.audioDurationSec,
                        record.wavBytes,
                      )}
                    </span>
                  </button>
                  <div className="audiobook-export-menu">
                    <button
                      className="audiobook-text-action audiobook-export"
                      disabled={exportDisabled}
                      aria-expanded={exportMenuOpen === record.id && !exportDisabled}
                      aria-haspopup="menu"
                      onClick={() => setExportMenuOpen((current) => current === record.id ? null : record.id)}
                    >
                      {exporting ? 'Exporting' : 'Export'}
                      <span className="audiobook-export-arrow" aria-hidden="true">&#9662;</span>
                    </button>
                    {exportMenuOpen === record.id && !exportDisabled && (
                      <div className="audiobook-export-options" role="menu">
                        {AUDIOBOOK_EXPORT_OPTIONS.map((option) => (
                          <button
                            key={option.format}
                            type="button"
                            className="audiobook-export-option"
                            role="menuitem"
                            onClick={() => {
                              setExportMenuOpen(null)
                              onExportSaved(record, option.format)
                            }}
                          >
                            <span>{option.label}</span>
                            <small>
                              {option.detail}
                              {option.code ? <> <code>{option.code}</code></> : null}
                            </small>
                          </button>
                        ))}
                      </div>
                    )}
                  </div>
                  <button
                    className="audiobook-text-action audiobook-delete"
                    disabled={deleteDisabled}
                    onClick={() => onDeleteSaved(record)}
                  >
                    {deleting ? 'Deleting' : 'Delete'}
                  </button>
                  {recordDeleteState && (
                    <div
                      className={'audiobook-status-text audiobook-operation-status audiobook-delete-' + recordDeleteState.status}
                      title={recordDeleteState.message}
                    >
                      {recordDeleteState.message}
                    </div>
                  )}
                </div>
              )
            })}
          </section>
            )}
          </div>
        </div>
      </div>
      {noticeState && (
        <div
          className={'audiobook-action-toast audiobook-action-toast-' + noticeState.status}
          role={noticeState.status === 'error' ? 'alert' : 'status'}
          aria-live="polite"
        >
          <span>{noticeState.message}</span>
          <button
            type="button"
            className="audiobook-action-toast-dismiss"
            aria-label="Dismiss audiobook notice"
            onClick={onDismissNotice}
          >
            ×
          </button>
        </div>
      )}
    </Panel>
  )
}

function AudiobooksPanelIcon({
  name,
}: {
  name: 'folder' | 'folder-open' | 'settings'
}) {
  return (
    <svg className="audiobooks-panel-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      {renderAudiobooksPanelIconPath(name)}
    </svg>
  )
}

function renderAudiobooksPanelIconPath(name: 'folder' | 'folder-open' | 'settings') {
  switch (name) {
    case 'folder':
      return <path d="M3 7.5A2.5 2.5 0 0 1 5.5 5h4l2 2.5h7A2.5 2.5 0 0 1 21 10v6.5a2.5 2.5 0 0 1-2.5 2.5h-13A2.5 2.5 0 0 1 3 16.5Z" />
    case 'folder-open':
      return <path d="M3 7.5A2.5 2.5 0 0 1 5.5 5h4l2 2.5h6A2.5 2.5 0 0 1 20 10v1M3 10.5h6.7a2 2 0 0 1 1.68.92L12.4 13H21l-2.2 5.1A2.5 2.5 0 0 1 16.5 19h-11A2.5 2.5 0 0 1 3 16.5Z" />
    case 'settings':
      return (
        <>
          <path d="M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7Z" />
          <path d="M19.4 15a1.7 1.7 0 0 0 .34 1.88l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.88-.34 1.7 1.7 0 0 0-1.03 1.56V21a2 2 0 1 1-4 0v-.08A1.7 1.7 0 0 0 9 19.36a1.7 1.7 0 0 0-1.88.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.7 1.7 0 0 0 4.63 15a1.7 1.7 0 0 0-1.56-1.03H3a2 2 0 1 1 0-4h.08A1.7 1.7 0 0 0 4.64 9a1.7 1.7 0 0 0-.34-1.88l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.7 1.7 0 0 0 9 4.63a1.7 1.7 0 0 0 1-1.56V3a2 2 0 1 1 4 0v.08A1.7 1.7 0 0 0 15 4.64a1.7 1.7 0 0 0 1.88-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.7 1.7 0 0 0 19.36 9a1.7 1.7 0 0 0 1.56 1H21a2 2 0 1 1 0 4h-.08A1.7 1.7 0 0 0 19.4 15Z" />
        </>
      )
  }
}

function formatAudiobookMeta(isSaving: boolean, queueCount: number, savedCount: number): string | undefined {
  const parts: string[] = []
  if (isSaving) parts.push('saving')
  if (queueCount > 0) parts.push(queueCount + ' queued')
  if (savedCount > 0) parts.push(savedCount + ' saved')
  return parts.length > 0 ? parts.join(' / ') : undefined
}

function getDownloadPercent(cachedChunks: number, totalChunks: number): number {
  if (totalChunks <= 0) return 0
  return Math.round((cachedChunks / totalChunks) * 100)
}

function formatAudioSetupSummary(audioSetup: AudioSetupPanelProps): string {
  const model = audioSetup.models.find((item) => item.id === audioSetup.modelId)
  const voice = audioSetup.voices.find((item) => item.id === audioSetup.voice)
  const pieces = [
    '🤖 ' + (model?.name ?? 'Model'),
    '🔊 ' + (voice?.name ?? audioSetup.voice),
    '⚡ ' + formatSpeedLabel(audioSetup.speed),
  ]

  const installSummary = formatModelInstallSummary(audioSetup.modelInstallProgress)
  if (installSummary) {
    pieces.push(installSummary)
  } else if (audioSetup.modelStatus?.installed) {
    pieces.push('✓ Installed')
  }

  return pieces.join(' · ')
}

function formatModelInstallSummary(
  progress: AudioSetupPanelProps['modelInstallProgress'],
): string | undefined {
  if (!progress || progress.status === 'installed') return undefined
  if (progress.status === 'error') return 'Install failed'
  if (progress.status === 'extracting') return 'Extracting'
  if (progress.status === 'starting') return 'Starting download'
  if (progress.status === 'downloading') return 'Downloading ' + progress.percent + '%'
  return progress.message || progress.status
}
