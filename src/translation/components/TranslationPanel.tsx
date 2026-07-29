import { useMemo, useState } from 'react'
import { AppDialog } from '../../components/AppDialog/AppDialog'
import { Panel } from '../../components/Panel/Panel'
import './TranslationPanel.css'
import type {
  TranslatedDocumentInfo,
  TranslationCapabilities,
  TranslationDeleteResult,
  TranslationJobProgress,
  TranslationModelInstallProgress,
  TranslationModelInstallResult,
  TranslationModelInfo,
  TranslationModelStatus,
  TranslationStartRequest,
  TranslationStartResult,
} from '../api/nativeTranslation'

export interface TranslationSeedDocument {
  title: string
  url: string
  format?: string
}

interface TranslationPanelProps {
  capabilities: TranslationCapabilities | null
  deleteState: TranslationDeleteResult | null
  error: string
  loading: boolean
  modelInstallState: {
    installingModelId: string
    progress: TranslationModelInstallProgress | null
    result: TranslationModelInstallResult | null
    message: string
  }
  modelStatuses: Record<string, TranslationModelStatus>
  selectedDocument: TranslationSeedDocument | null
  startState: {
    cancelling: boolean
    checking: boolean
    jobId: string
    progress: TranslationJobProgress | null
    result: TranslationStartResult | null
    message: string
  }
  translatedDocuments: TranslatedDocumentInfo[]
  onCancelTranslation: () => Promise<void>
  onDeleteTranslatedDocument: (id: string) => Promise<void>
  onOpenTranslatedDocument: (url: string) => void | Promise<void>
  onInstallTranslationModel: (modelId: string) => Promise<void>
  onStartTranslationPreflight: (request: TranslationStartRequest) => Promise<void>
  refresh: () => Promise<void>
}

export function TranslationPanel({
  capabilities,
  deleteState,
  error,
  loading,
  modelInstallState,
  modelStatuses,
  selectedDocument,
  startState,
  translatedDocuments,
  onCancelTranslation,
  onDeleteTranslatedDocument,
  onOpenTranslatedDocument,
  onInstallTranslationModel,
  onStartTranslationPreflight,
  refresh,
}: TranslationPanelProps) {
  const statusLabel = loading
    ? 'Checking'
    : capabilities?.available
      ? 'Available'
      : 'Unavailable in this build'
  const modelOptions = useMemo(() => capabilities?.models ?? [], [capabilities])
  const installableModels = useMemo(
    () => modelOptions.filter((model) => model.manifestState === 'pinned-file-manifest'),
    [modelOptions],
  )
  const plannedModels = useMemo(
    () => modelOptions.filter((model) => model.manifestState !== 'pinned-file-manifest'),
    [modelOptions],
  )
  const [confirmingDeleteId, setConfirmingDeleteId] = useState('')
  const confirmingDocument = useMemo(
    () => translatedDocuments.find((doc) => doc.id === confirmingDeleteId) ?? null,
    [confirmingDeleteId, translatedDocuments],
  )
  const [docsOpen, setDocsOpen] = useState(true)
  // Local-only dismissal of finished status messages; a new message (different
  // key) reappears without needing hook state changes.
  const [dismissedStatusKeys, setDismissedStatusKeys] = useState<string[]>([])
  const [modelId, setModelId] = useState('')
  const [sourceLanguage, setSourceLanguage] = useState('')
  const [targetLanguage, setTargetLanguage] = useState('en')
  const activeModelId = installableModels.some((model) => model.id === modelId)
    ? modelId
    : installableModels[0]?.id ?? ''
  const selectedModel = useMemo(
    () => installableModels.find((model) => model.id === activeModelId) ?? null,
    [activeModelId, installableModels],
  )
  const sourceLanguages = useMemo(
    () => uniqueOptions(selectedModel?.sourceLanguages ?? []),
    [selectedModel],
  )
  const targetLanguages = useMemo(
    () => uniqueOptions(selectedModel?.targetLanguages.length ? selectedModel.targetLanguages : ['en']),
    [selectedModel],
  )
  const activeSourceLanguage = sourceLanguages.includes(sourceLanguage)
    ? sourceLanguage
    : sourceLanguages[0] ?? ''
  const activeTargetLanguage = targetLanguages.includes(targetLanguage) ? targetLanguage : targetLanguages[0] ?? 'en'
  const activeQualityMode = selectedModel?.defaultQualityMode
    ?? capabilities?.defaultQualityMode
    ?? 'balanced'
  const modelNameById = useMemo(
    () => new Map(modelOptions.map((model) => [model.id, model.name])),
    [modelOptions],
  )
  const modelsSummary = formatModelsSummary(
    installableModels,
    plannedModels,
    modelStatuses,
    modelInstallState.progress,
  )
  const jobStatusKey = 'job:' + startState.message
  const installStatusKey = 'install:' + modelInstallState.message
  const deleteStatusKey = deleteState ? 'delete:' + deleteState.id + ':' + deleteState.message : ''
  const dismissStatus = (key: string) => {
    setDismissedStatusKeys((previous) => (previous.includes(key) ? previous : [...previous, key]))
  }
  const showJobStatus =
    Boolean(startState.message) && !(startState.result && dismissedStatusKeys.includes(jobStatusKey))
  const showInstallStatus =
    Boolean(modelInstallState.message) &&
    !(modelInstallState.result && dismissedStatusKeys.includes(installStatusKey))
  const showDeleteStatus = Boolean(deleteState) && !dismissedStatusKeys.includes(deleteStatusKey)
  const backendUnavailable = Boolean(capabilities && !capabilities.available)

  return (
    <Panel
      className="translation-panel"
      ariaLabel="Offline translation"
      title="Offline Translation"
      meta={statusLabel + (translatedDocuments.length ? ' · ' + translatedDocuments.length + ' translated' : '')}
      defaultOpen
    >
      <div className="translation-body">
      {(error || backendUnavailable || showJobStatus || showInstallStatus) && (
        <div className="translation-status-stack">
          {error && (
            <div className="translation-alert translation-alert-error" role="alert">
              <span className="translation-alert-message">{error}</span>
              <button
                type="button"
                className="translation-retry-btn"
                disabled={loading}
                onClick={() => { void refresh() }}
              >
                Retry
              </button>
            </div>
          )}

          {backendUnavailable && capabilities && (
            <div className="translation-alert">
              <span title={capabilities.reason}>{firstSentence(capabilities.reason)}</span>
            </div>
          )}

          {showJobStatus && (
            <div
              className={'translation-alert' + (startState.result ? '' : ' translation-alert-neutral')}
              role="status"
            >
              <strong>{startState.result ? 'Translation complete' : 'Translation in progress'}</strong>
              {startState.result && (
                <button
                  type="button"
                  className="translation-alert-dismiss"
                  aria-label="Dismiss translation status"
                  onClick={() => dismissStatus(jobStatusKey)}
                >
                  ×
                </button>
              )}
              {(startState.result || !startState.progress) && (
                <span className="translation-alert-message" title={startState.message}>
                  {startState.message}
                </span>
              )}
              {startState.progress && (
                <TranslationProgressMeter progress={startState.progress} />
              )}
            </div>
          )}

          {showInstallStatus && (
            <div
              className={'translation-alert' + (modelInstallState.result ? '' : ' translation-alert-neutral')}
              role="status"
            >
              <strong>{modelInstallState.result ? 'Model installed' : 'Model install'}</strong>
              {modelInstallState.result && (
                <button
                  type="button"
                  className="translation-alert-dismiss"
                  aria-label="Dismiss model install status"
                  onClick={() => dismissStatus(installStatusKey)}
                >
                  ×
                </button>
              )}
              <span className="translation-alert-message" title={modelInstallState.message}>
                {modelInstallState.message}
              </span>
              {modelInstallState.progress && modelInstallState.progress.status !== 'installed' && (
                <div className="translation-progress-meter" aria-label={'Translation model download ' + modelInstallState.progress.percent + '% complete'}>
                  <span style={{ width: modelInstallState.progress.percent + '%' }} />
                </div>
              )}
            </div>
          )}
        </div>
      )}

      {/* Without a working backend the translate workbench is a dead end;
          models can still be pre-installed and stored variants stay usable. */}
      {backendUnavailable ? null : selectedDocument ? (
        <div className="translation-selected-document">
          <span className="translation-kicker">Selected Document</span>
          <strong>{selectedDocument.title}</strong>
          <span>{formatDocumentFormat(selectedDocument.format)} translation runs as a separate generated document copy.</span>
          <div className="translation-preflight-controls" aria-label="Translation readiness options">
            <label>
              <span>Model</span>
              <select
                value={activeModelId}
                disabled={!installableModels.length || startState.checking}
                onChange={(event) => {
                  const nextModelId = event.target.value
                  const nextModel = installableModels.find((model) => model.id === nextModelId)
                  setModelId(nextModelId)
                  setSourceLanguage(nextModel?.sourceLanguages[0] ?? '')
                  setTargetLanguage(nextModel?.targetLanguages[0] ?? 'en')
                }}
              >
                {installableModels.length ? installableModels.map((model) => (
                  <option key={model.id} value={model.id}>
                    {model.name}
                  </option>
                )) : (
                  <option value="">No supported models</option>
                )}
              </select>
            </label>
            <label>
              <span>Source</span>
              <select
                value={activeSourceLanguage}
                disabled={startState.checking || sourceLanguages.length <= 1}
                onChange={(event) => setSourceLanguage(event.target.value)}
              >
                {sourceLanguages.map((language) => (
                  <option key={language} value={language}>
                    {formatLanguageLabel(language)}
                  </option>
                ))}
              </select>
            </label>
            <label>
              <span>Target</span>
              <select
                value={activeTargetLanguage}
                disabled={startState.checking}
                onChange={(event) => setTargetLanguage(event.target.value)}
              >
                {targetLanguages.map((language) => (
                  <option key={language} value={language}>
                    {formatLanguageLabel(language)}
                  </option>
                ))}
              </select>
            </label>
          </div>
          <div className="translation-action-row">
            <button
              type="button"
              disabled={startState.checking || !activeModelId}
              title="Run the selected document through the translation job pipeline"
              onClick={() => {
                void onStartTranslationPreflight({
                  documentUrl: selectedDocument.url,
                  sourceLanguage: activeSourceLanguage,
                  targetLanguage: activeTargetLanguage,
                  modelId: activeModelId,
                  qualityMode: activeQualityMode,
                })
              }}
            >
              {startState.checking ? 'Translating...' : 'Run Translation'}
            </button>
            {startState.checking && (
              <button
                type="button"
                className="translation-cancel-btn"
                disabled={startState.cancelling}
                onClick={() => { void onCancelTranslation() }}
              >
                {startState.cancelling ? 'Cancelling...' : 'Cancel'}
              </button>
            )}
          </div>
        </div>
      ) : (
        <div className="translation-empty-state">
          <p>No document selected — open a document and choose Translate from its actions menu.</p>
        </div>
      )}

      <Panel
        className="translation-subpanel"
        ariaLabel="Translation models"
        title="Translation Models"
        meta={modelsSummary}
      >
        <div className="translation-model-list">
          {installableModels.map((model) => {
            const size = formatModelSize(modelStatuses[model.id])
            return (
              <div key={model.id} className="translation-model-item">
                <div>
                  <strong title={model.notes}>{model.name}</strong>
                  <span>{formatModelLanguagePair(model)}{size ? ' · ' + size : ''}</span>
                </div>
                <TranslationModelInstallButton
                  model={model}
                  progress={modelInstallState.progress?.modelId === model.id ? modelInstallState.progress : null}
                  status={modelStatuses[model.id]}
                  disabled={Boolean(modelInstallState.installingModelId)}
                  onInstall={onInstallTranslationModel}
                />
              </div>
            )
          })}
          {plannedModels.length > 0 && (
            <div className="translation-planned-models">
              <span className="translation-kicker">Planned</span>
              {plannedModels.map((model) => (
                <div key={model.id} className="translation-planned-model-row" title={model.notes}>
                  <strong>{model.name}</strong>
                  <span>{formatModelLanguagePair(model)} · {formatTierLabel(model.tier)}</span>
                </div>
              ))}
            </div>
          )}
          {!modelOptions.length && (
            <p className="translation-section-empty">No model metadata available in this runtime.</p>
          )}
        </div>
      </Panel>

      <Panel
        className="translation-subpanel"
        ariaLabel="Translated documents"
        title="Translated Documents"
        meta={translatedDocuments.length + ' saved'}
        open={docsOpen}
        onToggle={() => setDocsOpen((value) => !value)}
      >
        {deleteState && showDeleteStatus && (
          <div className={'translation-alert' + (deleteState.deleted ? '' : ' translation-alert-error')} role="status">
            <button
              type="button"
              className="translation-alert-dismiss"
              aria-label="Dismiss delete status"
              onClick={() => dismissStatus(deleteStatusKey)}
            >
              ×
            </button>
            <span className="translation-alert-message">{deleteState.message}</span>
          </div>
        )}
        {translatedDocuments.length > 0 ? (
          <div className="translation-document-list">
            {translatedDocuments.map((doc) => (
              <div key={doc.id} className="translation-document-item">
                <div>
                  <strong>{doc.title}</strong>
                  <span>
                    {formatLanguageLabel(doc.sourceLanguage)} → {formatLanguageLabel(doc.targetLanguage)}
                    {' · '}{modelNameById.get(doc.modelId) ?? doc.modelId}
                    {' · '}{formatQualityLabel(doc.status)}
                  </span>
                </div>
                <div className="translation-document-actions">
                  <button
                    type="button"
                    className="translation-document-view-btn"
                    aria-label={'View ' + doc.title}
                    onClick={() => { void onOpenTranslatedDocument(doc.documentUrl) }}
                  >
                    View
                  </button>
                  <button
                    type="button"
                    className="translation-document-delete-btn"
                    aria-label={'Delete ' + doc.title}
                    onClick={() => setConfirmingDeleteId(doc.id)}
                  >
                    Delete
                  </button>
                </div>
              </div>
            ))}
          </div>
        ) : (
          <p className="translation-section-empty">No translated documents yet.</p>
        )}
      </Panel>
      </div>

      {confirmingDocument && (
        <AppDialog
          title="Delete translated document?"
          description={'"' + confirmingDocument.title + '" will be removed. The original document is not affected; re-running the translation recreates it.'}
          onCancel={() => setConfirmingDeleteId('')}
          actions={
            <>
              <button
                type="button"
                className="app-dialog-cancel"
                onClick={() => setConfirmingDeleteId('')}
              >
                Keep
              </button>
              <button
                type="button"
                className="app-dialog-danger"
                onClick={() => {
                  setConfirmingDeleteId('')
                  void onDeleteTranslatedDocument(confirmingDocument.id)
                }}
              >
                Delete
              </button>
            </>
          }
        />
      )}
    </Panel>
  )
}

function TranslationProgressMeter({ progress }: { progress: TranslationJobProgress }) {
  return (
    <div className="translation-job-progress">
      <div className="translation-job-progress-header">
        <span>{formatQualityLabel(progress.status)}</span>
        <span>{progress.percent}%</span>
      </div>
      <div
        className="translation-progress-meter"
        aria-label={'Translation job ' + progress.percent + '% complete'}
      >
        <span style={{ width: progress.percent + '%' }} />
      </div>
      <small>
        {progress.completedSegments} of {progress.totalSegments} segments · {progress.completedBatches} of {progress.totalBatches} batches
      </small>
      {progress.cachedSegments > 0 && (
        <small>
          Reused {progress.cachedSegments} cached segment{progress.cachedSegments === 1 ? '' : 's'}
          {progress.translatedSegments > 0 ? ' · Translated ' + progress.translatedSegments + ' fresh' : ''}
          {progress.reusedSegmentsInBatch > 0 ? ' · Current batch reused ' + progress.reusedSegmentsInBatch : ''}
        </small>
      )}
      {progress.preview && <small>Preview: {progress.preview}</small>}
    </div>
  )
}

interface TranslationModelInstallButtonProps {
  disabled: boolean
  model: TranslationModelInfo
  progress: TranslationModelInstallProgress | null
  status?: TranslationModelStatus
  onInstall: (modelId: string) => Promise<void>
}

function TranslationModelInstallButton({
  disabled,
  model,
  progress,
  status,
  onInstall,
}: TranslationModelInstallButtonProps) {
  const installable = model.manifestState === 'pinned-file-manifest'
  const installing = progress !== null || status?.installing
  if (status?.installed) {
    return <span className="translation-model-badge">Installed</span>
  }
  if (!installable) {
    return <span className="translation-model-badge translation-model-badge-muted">Planned</span>
  }
  return (
    <button
      type="button"
      className="translation-model-install-btn"
      disabled={disabled || installing}
      onClick={() => { void onInstall(model.id) }}
    >
      {installing ? 'Installing ' + (progress?.percent ?? 0) + '%' : 'Install'}
    </button>
  )
}

function formatDocumentFormat(format?: string): string {
  if (!format) return 'Document'
  return format.toUpperCase()
}

function formatModelLanguagePair(model: TranslationModelInfo): string {
  return (
    model.sourceLanguages.map(formatLanguageLabel).join(', ') +
    ' → ' +
    model.targetLanguages.map(formatLanguageLabel).join(', ')
  )
}

function formatTierLabel(tier: string): string {
  switch (tier) {
    case 'fast':
      return 'Fast'
    case 'quality':
      return 'High quality'
    case 'context':
      return 'Context-rich'
    default:
      return formatQualityLabel(tier)
  }
}

// One-line state summary for the collapsed models disclosure.
function formatModelsSummary(
  installableModels: TranslationModelInfo[],
  plannedModels: TranslationModelInfo[],
  modelStatuses: Record<string, TranslationModelStatus>,
  installProgress: TranslationModelInstallProgress | null,
): string {
  if (!installableModels.length && !plannedModels.length) {
    return 'No models available in this runtime'
  }
  const pieces: string[] = []
  if (installProgress && installProgress.status !== 'installed') {
    pieces.push('Installing ' + installProgress.percent + '%')
  }
  const installed = installableModels.filter((model) => modelStatuses[model.id]?.installed).length
  const installable = installableModels.length - installed
  if (installed) pieces.push(installed + ' installed')
  if (installable) pieces.push(installable + ' installable')
  if (plannedModels.length) pieces.push(plannedModels.length + ' planned')
  return pieces.join(' · ')
}

// Language names beat raw ISO codes for recognition; fall back to the code
// when the runtime cannot resolve a display name.
function formatLanguageLabel(language: string): string {
  if (language === 'auto') return 'Auto detect'
  try {
    const name = new Intl.DisplayNames(['en'], { type: 'language' }).of(language)
    if (name && name !== language) return name
  } catch {
    // Unknown/invalid code: fall through to the uppercase code.
  }
  return language.toUpperCase()
}

function formatQualityLabel(mode: string): string {
  return mode.charAt(0).toUpperCase() + mode.slice(1)
}

// Size only: installed/installing state is already carried by the row's
// badge or button, so repeating it in text would be redundant.
function formatModelSize(status?: TranslationModelStatus): string {
  if (!status) return ''
  const bytes = status.installed ? status.installedBytes : status.archiveBytes
  return bytes > 0 ? formatBytes(bytes) : ''
}

function firstSentence(text: string): string {
  const index = text.indexOf('. ')
  return index === -1 ? text : text.slice(0, index + 1)
}

function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 MB'
  const mb = bytes / (1024 * 1024)
  if (mb < 1024) return mb.toFixed(mb >= 100 ? 0 : 1) + ' MB'
  const gb = mb / 1024
  return gb.toFixed(gb >= 10 ? 1 : 2) + ' GB'
}

function uniqueOptions(values: string[]): string[] {
  return values.filter((value, index) => value && values.indexOf(value) === index)
}
