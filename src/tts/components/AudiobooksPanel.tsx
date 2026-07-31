import { useState } from 'react'
import type { TFunction } from 'i18next'
import { useTranslation } from 'react-i18next'
import type { SavedAudiobookRecord } from '../storage/AudiobookLibrary'
import type { AudiobookDownloadRecord } from '../storage/AudiobookDownloadQueue'
import type { TtsDtype, TtsVoice } from '../types'
import type { NativeAudiobookExportFormat } from '../api/nativeTts'
import type { AudiobookCacheState } from '../hooks/useAudiobookCache'
import {
  formatAudiobookVoiceMeta,
  formatDownloadSavedStatus,
  formatSavedAudiobookMeta,
} from '../utils/format'
import { Panel } from '../../components/Panel/Panel'
import { AudiobookExportMenu } from './AudiobookExportMenu'
import { AudioSetupPanel, type AudioSetupPanelProps } from './AudioSetupPanel'
import './AudiobooksPanel.css'

interface ActiveAudiobookSave {
  title: string
  url: string
  modelId: string
  textPreprocessor: string
  voice: TtsVoice
  speed: number
  silmaNfeStep?: number
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
  const { t, i18n } = useTranslation()
  const [setupOpen, setSetupOpen] = useState(false)
  const [exportMenuOpen, setExportMenuOpen] = useState<string | null>(null)
  const activePercent = getDownloadPercent(downloadState.cachedChunks, downloadState.totalChunks)
  const savedCount = savedAudiobooks.length
  const queueCount = queuedDownloads.length
  const exportInProgress = exportState?.status === 'exporting'
  const importInProgress = importState.status === 'importing'
  const deleteInProgress = deleteState?.status === 'deleting'
  const panelBusy = exportInProgress || importInProgress || deleteInProgress
  const meta = formatAudiobookMeta(isSaving, queueCount, savedCount, t)
  const hasAudiobooks = isSaving || queueCount > 0 || savedCount > 0
  // Keep model, voice, and runtime metadata stable and LTR across UI locales.
  const setupSummary = formatAudioSetupSummary(audioSetup, i18n.getFixedT('en'))
  const exportOptions = [
    { format: 'bundle' as const, label: t('tts.audiobooks.exportBundle'), code: '.papercut-audiobook' },
    { format: 'wav' as const, label: t('tts.audiobooks.exportWav'), code: '.wav' },
  ]

  return (
    <Panel
      className="audiobooks-panel"
      ariaLabel={t('tts.audiobooks.title')}
      title={t('tts.audiobooks.title')}
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
              {importInProgress ? t('tts.audiobooks.importingBundle') : t('tts.audiobooks.importBundle')}
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
                <span className="audiobooks-setup-disclosure-title">{t('tts.audiobooks.audioSetup')}</span>
                <span className="audiobooks-setup-disclosure-summary" dir="ltr">{setupSummary}</span>
              </span>
              <span className="audiobooks-setup-disclosure-chevron" aria-hidden="true">{setupOpen ? '▲' : '▼'}</span>
            </button>
          </div>

          {setupOpen && (
            <section id="audiobooks-audio-setup" className="audiobooks-section audiobooks-setup" aria-label={t('tts.audiobooks.audioSetup')}>
              <AudioSetupPanel {...audioSetup} />
            </section>
          )}

          {!hasAudiobooks && (
            <div className="audiobooks-empty">
              <h2>{t('tts.audiobooks.emptyTitle')}</h2>
              <p>{t('tts.audiobooks.emptyDescription')}</p>
            </div>
          )}

          <div className="audiobooks-list">
            {isSaving && (
          <section className="audiobooks-section" aria-label={t('tts.audiobooks.savingAria')}>
            <h3 className="audiobooks-section-title">{t('tts.audiobooks.savingSection')}</h3>
            <div className="audiobook-item audiobook-item-active">
              <div className="audiobook-row">
                <bdi className="audiobook-title">{activeDownloadTitle}</bdi>
                <span className="audiobook-meta" dir="ltr">{downloadState.cachedChunks}/{downloadState.totalChunks}</span>
              </div>
              <div className="audiobook-status-text" dir="auto">
                {activeDownload ? formatAudiobookVoiceMeta(t, activeDownload.modelId, activeDownload.voice, activeDownload.speed, activeDownload.dtype, activeDownload.textPreprocessor, activeDownload.silmaNfeStep) + ' - ' : ''}{formatDownloadSavedStatus(t, downloadState.audioDurationSec, activePercent, downloadState.wavBytes)}
              </div>
              <div className="audio-progress-meter" aria-label={t('tts.audiobooks.savePercent', { percent: activePercent })}>
                <span style={{ width: activePercent + '%' }} />
              </div>
              <button className="audiobook-text-action audiobook-secondary" disabled={panelBusy} onClick={onCancelSave}>{t('tts.audiobooks.pause')}</button>
            </div>
          </section>
        )}

            {queueCount > 0 && (
          <section className="audiobooks-section" aria-label={t('tts.audiobooks.queueAria')}>
            <h3 className="audiobooks-section-title">{t('tts.audiobooks.queueSection')}</h3>
            {queuedDownloads.map((record) => {
              const percent = getDownloadPercent(record.cachedChunks, record.totalChunks)
              return (
                <div key={record.id} className={'audiobook-item audiobook-item-' + record.status}>
                  <div className="audiobook-row">
                    <bdi className="audiobook-title">{record.title}</bdi>
                    <span className="audiobook-meta" dir="ltr">{record.cachedChunks}/{record.totalChunks}</span>
                  </div>
                  <div className="audiobook-status-text" dir="auto">
                    {formatAudiobookVoiceMeta(t, record.modelId, record.voice, record.speed, record.dtype, record.textPreprocessor, record.silmaNfeStep) + ' - ' + formatDownloadSavedStatus(t, record.audioDurationSec, percent, record.wavBytes)}
                  </div>
                  <div className="audio-progress-meter" aria-label={t('tts.audiobooks.queuePercent', { percent })}>
                    <span style={{ width: percent + '%' }} />
                  </div>
                  <div className="audiobook-actions">
                    <span className="audiobook-status-text" dir="auto">{formatQueuedStatus(record, t)}</span>
                    <button className="audiobook-text-action audiobook-resume" disabled={panelBusy} onClick={() => onResumeQueued(record)}>
                      {record.status === 'error' ? t('tts.audiobooks.retry') : t('tts.audiobooks.resume')}
                    </button>
                    <button className="audiobook-text-action audiobook-secondary" disabled={panelBusy} onClick={() => onRemoveQueued(record.id)}>{t('tts.audiobooks.remove')}</button>
                  </div>
                </div>
              )
            })}
          </section>
        )}

            {savedCount > 0 && (
          <section className="audiobooks-section" aria-label={t('tts.audiobooks.savedAria')}>
            <h3 className="audiobooks-section-title">{t('tts.audiobooks.savedSection')}</h3>
            {savedAudiobooks.map((record) => {
              const recordExportState = exportState?.id === record.id ? exportState : null
              const recordDeleteState = deleteState?.id === record.id ? deleteState : null
              const exporting = recordExportState?.status === 'exporting'
              const deleting = recordDeleteState?.status === 'deleting'
              const exportDisabled = panelBusy || deleting
              const deleteDisabled = panelBusy || deleting
              return (
                <div
                  key={record.id}
                  className={'audiobook-item audiobook-item-saved' + (exportMenuOpen === record.id && !exportDisabled ? ' audiobook-item-menu-open' : '')}
                >
                  <button
                    className="audiobook-saved-main"
                    disabled={documentOpening || panelBusy}
                    onClick={() => { if (!documentOpening) onOpenSaved(record) }}
                  >
                    <bdi className="audiobook-title">{record.title}</bdi>
                    <span className="audiobook-meta" dir="auto">
                      {formatSavedAudiobookMeta(
                        t,
                        record.modelId,
                        record.voice,
                        record.speed,
                        record.textPreprocessor,
                        record.audioDurationSec,
                        record.wavBytes,
                      )}
                    </span>
                  </button>
                  <AudiobookExportMenu
                    t={t}
                    record={record}
                    options={exportOptions}
                    open={exportMenuOpen === record.id && !exportDisabled}
                    disabled={exportDisabled}
                    exporting={exporting}
                    onOpenChange={(open) => setExportMenuOpen(open ? record.id : null)}
                    onExport={(format) => {
                      setExportMenuOpen(null)
                      onExportSaved(record, format)
                    }}
                  />
                  <button
                    className="audiobook-text-action audiobook-delete"
                    disabled={deleteDisabled}
                    onClick={() => onDeleteSaved(record)}
                  >
                    {deleting ? t('tts.audiobooks.deleting') : t('tts.audiobooks.delete')}
                  </button>
                  {recordDeleteState && (
                    <div
                      className={'audiobook-status-text audiobook-operation-status audiobook-delete-' + recordDeleteState.status}
                      title={recordDeleteState.message}
                      dir="auto"
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
          <span dir="auto">{noticeState.message}</span>
          <button
            type="button"
            className="audiobook-action-toast-dismiss"
            aria-label={t('tts.audiobooks.dismissNotice')}
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

function formatAudiobookMeta(
  isSaving: boolean,
  queueCount: number,
  savedCount: number,
  t: TFunction,
): string | undefined {
  const parts: string[] = []
  if (isSaving) parts.push(t('tts.audiobooks.metaSaving'))
  if (queueCount > 0) parts.push(t('tts.audiobooks.metaQueued', { count: queueCount }))
  if (savedCount > 0) parts.push(t('tts.audiobooks.metaSaved', { count: savedCount }))
  return parts.length > 0 ? parts.join(' / ') : undefined
}

function getDownloadPercent(cachedChunks: number, totalChunks: number): number {
  if (totalChunks <= 0) return 0
  return Math.round((cachedChunks / totalChunks) * 100)
}

function formatAudioSetupSummary(audioSetup: AudioSetupPanelProps, t: TFunction): string {
  const model = audioSetup.models.find((item) => item.id === audioSetup.modelId)
  const voice = audioSetup.voices.find((item) => item.id === audioSetup.voice)
  const pieces = [
    '🤖 ' + (model?.name ?? t('tts.audiobooks.modelFallback')),
    '🔊 ' + formatVoiceSummary(voice?.name ?? audioSetup.voice),
  ]
  if (model?.family === 'silma-f5') pieces.push('🎚️ NFE ' + audioSetup.silmaNfeStep)

  const installSummary = formatModelInstallSummary(audioSetup.modelInstallProgress, t)
  if (installSummary) {
    pieces.push(installSummary)
  } else if (audioSetup.modelStatus?.installed) {
    pieces.push('✓ ' + t('tts.audiobooks.installed'))
  }
  if (model?.family === 'silma-f5' && audioSetup.modelStatus?.runtimeInstalled === false) {
    pieces.push(t('tts.audiobooks.runtimeMissing'))
  }

  return pieces.join(' · ')
}

// Accent flags and quality grades help when choosing a voice but add noise to
// the collapsed setup summary, where only the selected voice name is needed.
function formatVoiceSummary(name: string): string {
  return name
    .replace(/^(?:\p{Regional_Indicator}{2})+\s*/u, '')
    .replace(/\s+\([A-F][+-]?\)$/, '')
}

function formatModelInstallSummary(
  progress: AudioSetupPanelProps['modelInstallProgress'],
  t: TFunction,
): string | undefined {
  if (!progress || progress.status === 'installed') return undefined
  if (progress.status === 'error') return t('tts.audiobooks.installFailed')
  if (progress.status === 'extracting') return t('tts.audiobooks.extracting')
  if (progress.status === 'starting') return t('tts.audiobooks.startingDownload')
  if (progress.status === 'downloading') return t('tts.audiobooks.downloadingPercent', { percent: progress.percent })
  return progress.message || progress.status
}

function formatQueuedStatus(record: AudiobookDownloadRecord, t: TFunction): string {
  if (record.status === 'error') return record.message || record.status
  if (record.status === 'paused') return t('tts.status.readyToResume')
  if (record.status === 'saving') return t('tts.audiobooks.metaSaving')
  return t('tts.status.queued')
}
