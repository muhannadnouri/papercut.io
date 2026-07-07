import { useState } from 'react'
import type { SavedAudiobookRecord } from '../storage/AudiobookLibrary'
import type { AudiobookDownloadRecord } from '../storage/AudiobookDownloadQueue'
import type { TtsDtype, TtsVoice } from '../types'
import type { AudiobookCacheState } from '../hooks/useAudiobookCache'
import {
  formatAudiobookVoiceMeta,
  formatDownloadSavedStatus,
  formatDuration,
  formatSpeedLabel,
  formatStorageSize,
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

interface AudiobooksPanelProps {
  activeDownload: ActiveAudiobookSave | null
  audioSetup: AudioSetupPanelProps
  activeDownloadTitle: string
  deleteState: AudiobookDeleteState | null
  downloadState: AudiobookCacheState
  exportState: AudiobookExportState | null
  importState: AudiobookImportState
  documentOpening?: boolean
  isSaving: boolean
  queuedDownloads: AudiobookDownloadRecord[]
  savedAudiobooks: SavedAudiobookRecord[]
  onCancelSave: () => void
  onDeleteSaved: (record: SavedAudiobookRecord) => void
  onExportSaved: (record: SavedAudiobookRecord) => void
  onImportAudiobook: () => void
  onOpenSaved: (record: SavedAudiobookRecord) => void
  onRemoveQueued: (id: string) => void
  onResumeQueued: (record: AudiobookDownloadRecord) => void
}

export function AudiobooksPanel({
  activeDownload,
  audioSetup,
  activeDownloadTitle,
  deleteState,
  downloadState,
  exportState,
  importState,
  documentOpening = false,
  isSaving,
  queuedDownloads,
  savedAudiobooks,
  onCancelSave,
  onDeleteSaved,
  onExportSaved,
  onImportAudiobook,
  onOpenSaved,
  onRemoveQueued,
  onResumeQueued,
}: AudiobooksPanelProps) {
  const [setupOpen, setSetupOpen] = useState(false)
  const activePercent = getDownloadPercent(downloadState.cachedChunks, downloadState.totalChunks)
  const savedCount = savedAudiobooks.length
  const queueCount = queuedDownloads.length
  const meta = formatAudiobookMeta(isSaving, queueCount, savedCount)
  const hasAudiobooks = isSaving || queueCount > 0 || savedCount > 0
  const setupSummary = formatAudioSetupSummary(audioSetup)

  return (
    <Panel
      className="audiobooks-panel"
      ariaLabel="Audiobooks"
      title="Audiobooks"
      meta={meta}
      defaultOpen
    >
      <div className="audiobooks-actions-row">
        <button
          type="button"
          className="audiobooks-import-btn"
          disabled={importState.status === 'importing'}
          onClick={onImportAudiobook}
        >
          <AudiobooksPanelIcon name={importState.status === 'importing' ? 'folder-open' : 'folder'} />
          {importState.status === 'importing' ? 'Importing Bundle' : 'Import Bundle'}
        </button>

        <button
          type="button"
          className={'audiobooks-setup-disclosure' + (setupOpen ? ' audiobooks-setup-disclosure-open' : '')}
          aria-expanded={setupOpen}
          aria-controls="audiobooks-audio-setup"
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
        {importState.message && importState.status !== 'idle' && (
          <span className={'audiobooks-import-status document-import-' + importState.status}>
            {importState.message}
          </span>
        )}
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
              <button className="audiobook-text-action audiobook-secondary" onClick={onCancelSave}>Pause</button>
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
                    <button className="audiobook-text-action audiobook-resume" onClick={() => onResumeQueued(record)}>
                      {record.status === 'error' ? 'Retry' : 'Resume'}
                    </button>
                    <button className="audiobook-text-action audiobook-secondary" onClick={() => onRemoveQueued(record.id)}>Remove</button>
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
              const storage = formatStorageSize(record.wavBytes)
              return (
                <div key={record.id} className="audiobook-item audiobook-item-saved">
                  <button
                    className="audiobook-saved-main"
                    disabled={documentOpening}
                    onClick={() => { if (!documentOpening) onOpenSaved(record) }}
                  >
                    <span className="audiobook-title">{record.title}</span>
                    <span className="audiobook-meta">
                      {formatAudiobookVoiceMeta(record.modelId, record.voice, record.speed, record.dtype, record.textPreprocessor)}
                      {' - ' + record.chunks + ' chunks'}
                      {record.audioDurationSec ? ' - ' + formatDuration(record.audioDurationSec) : ''}
                      {storage ? ' - ' + storage : ''}
                    </span>
                  </button>
                  <button
                    className="audiobook-text-action audiobook-export"
                    disabled={exporting || deleting}
                    onClick={() => onExportSaved(record)}
                  >
                    {exporting ? 'Exporting' : 'Export Bundle'}
                  </button>
                  <button
                    className="audiobook-text-action audiobook-delete"
                    disabled={exporting || deleting}
                    onClick={() => onDeleteSaved(record)}
                  >
                    {deleting ? 'Deleting' : 'Delete'}
                  </button>
                  {recordExportState && (
                    <div
                      className={'audiobook-status-text audiobook-operation-status audiobook-export-' + recordExportState.status}
                      title={recordExportState.message}
                    >
                      {recordExportState.message}
                    </div>
                  )}
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

        {deleteState && deleteState.status !== 'deleting' && !savedAudiobooks.some((record) => record.id === deleteState.id) && (
          <div className={'audiobook-status-text audiobook-delete-summary audiobook-delete-' + deleteState.status}>
            {deleteState.message}
          </div>
        )}
      </div>
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
    model?.name ?? 'Model',
    voice?.name ?? audioSetup.voice,
    formatSpeedLabel(audioSetup.speed),
  ]

  const installSummary = formatModelInstallSummary(audioSetup.modelInstallProgress)
  if (installSummary) {
    pieces.push(installSummary)
  } else if (audioSetup.modelStatus?.installed) {
    pieces.push('Installed')
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
