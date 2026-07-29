import { useMemo, useState } from 'react'
import type { TFunction } from 'i18next'
import { useTranslation } from 'react-i18next'
import { AppDialog } from '../../components/AppDialog/AppDialog'
import { Panel } from '../../components/Panel/Panel'
import { formatStorageSize } from '../../utils/formatUtils'
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
  const { t, i18n } = useTranslation()
  const locale = i18n.resolvedLanguage ?? i18n.language
  const statusLabel = loading
    ? t('translation.status.checking')
    : capabilities?.available
      ? t('translation.status.available')
      : t('translation.status.unavailable')
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
    t,
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
  const jobFailed = !startState.checking && !startState.result && Boolean(startState.message)
  const installFailed =
    !modelInstallState.installingModelId &&
    !modelInstallState.result &&
    !modelInstallState.progress &&
    Boolean(modelInstallState.message)
  const jobMessage = startState.result
    ? t('translation.status.translationCompleteMessage')
    : startState.cancelling
      ? t('translation.status.cancellingJob')
      : startState.checking && !startState.progress
        ? t('translation.status.preparingJob')
        : startState.message
  const installMessage = modelInstallState.result
    ? t('translation.status.modelInstalledMessage')
    : modelInstallState.installingModelId && !modelInstallState.progress
      ? t('translation.status.preparingModel')
      : modelInstallState.progress
        ? formatInstallProgressMessage(modelInstallState.progress, t)
        : modelInstallState.message

  return (
    <Panel
      className="translation-panel"
      ariaLabel={t('translation.ariaLabel')}
      title={t('translation.title')}
      meta={statusLabel + (translatedDocuments.length
        ? ' · ' + t('translation.status.translatedCount', { count: translatedDocuments.length })
        : '')}
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
                {t('translation.retry')}
              </button>
            </div>
          )}

          {backendUnavailable && capabilities && (
            <div className="translation-alert">
              <span>{t('translation.status.unavailableReason')}</span>
            </div>
          )}

          {showJobStatus && (
            <div
              className={'translation-alert' + (startState.result ? '' : ' translation-alert-neutral')}
              role="status"
            >
              <strong>
                {startState.result
                  ? t('translation.status.translationComplete')
                  : jobFailed
                    ? t('translation.status.translationFailed')
                    : t('translation.status.translationInProgress')}
              </strong>
              {startState.result && (
                <button
                  type="button"
                  className="translation-alert-dismiss"
                  aria-label={t('translation.status.dismissTranslation')}
                  onClick={() => dismissStatus(jobStatusKey)}
                >
                  ×
                </button>
              )}
              {(startState.result || !startState.progress || jobFailed) && (
                <span className="translation-alert-message" title={jobMessage}>
                  {jobMessage}
                </span>
              )}
              {startState.progress && (
                <TranslationProgressMeter progress={startState.progress} t={t} />
              )}
            </div>
          )}

          {showInstallStatus && (
            <div
              className={'translation-alert' + (modelInstallState.result ? '' : ' translation-alert-neutral')}
              role="status"
            >
              <strong>
                {modelInstallState.result
                  ? t('translation.status.modelInstalled')
                  : installFailed
                    ? t('translation.status.modelInstallFailed')
                    : t('translation.status.modelInstall')}
              </strong>
              {modelInstallState.result && (
                <button
                  type="button"
                  className="translation-alert-dismiss"
                  aria-label={t('translation.status.dismissModelInstall')}
                  onClick={() => dismissStatus(installStatusKey)}
                >
                  ×
                </button>
              )}
              <span className="translation-alert-message" title={installMessage}>
                {installMessage}
              </span>
              {modelInstallState.progress && modelInstallState.progress.status !== 'installed' && (
                <div
                  className="translation-progress-meter"
                  aria-label={t('translation.progress.modelDownload', {
                    percent: modelInstallState.progress.percent,
                  })}
                >
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
          <span className="translation-kicker">{t('translation.workbench.selectedDocument')}</span>
          <strong>{selectedDocument.title}</strong>
          <span>
            {t('translation.workbench.documentCopy', {
              format: formatDocumentFormat(selectedDocument.format, t),
            })}
          </span>
          <div
            className="translation-preflight-controls"
            aria-label={t('translation.workbench.readinessOptions')}
          >
            <label>
              <span>{t('translation.workbench.model')}</span>
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
                  <option value="">{t('translation.workbench.noSupportedModels')}</option>
                )}
              </select>
            </label>
            <label>
              <span>{t('translation.workbench.source')}</span>
              <select
                value={activeSourceLanguage}
                disabled={startState.checking || sourceLanguages.length <= 1}
                onChange={(event) => setSourceLanguage(event.target.value)}
              >
                {sourceLanguages.map((language) => (
                  <option key={language} value={language}>
                    {formatLanguageLabel(language, locale, t)}
                  </option>
                ))}
              </select>
            </label>
            <label>
              <span>{t('translation.workbench.target')}</span>
              <select
                value={activeTargetLanguage}
                disabled={startState.checking}
                onChange={(event) => setTargetLanguage(event.target.value)}
              >
                {targetLanguages.map((language) => (
                  <option key={language} value={language}>
                    {formatLanguageLabel(language, locale, t)}
                  </option>
                ))}
              </select>
            </label>
          </div>
          <div className="translation-action-row">
            <button
              type="button"
              disabled={startState.checking || !activeModelId}
              title={t('translation.workbench.runTitle')}
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
              {startState.checking
                ? t('translation.workbench.translating')
                : t('translation.workbench.run')}
            </button>
            {startState.checking && (
              <button
                type="button"
                className="translation-cancel-btn"
                disabled={startState.cancelling}
                onClick={() => { void onCancelTranslation() }}
              >
                {startState.cancelling ? t('translation.workbench.cancelling') : t('common.cancel')}
              </button>
            )}
          </div>
        </div>
      ) : (
        <div className="translation-empty-state">
          <p>{t('translation.workbench.noDocument')}</p>
        </div>
      )}

      <Panel
        className="translation-subpanel"
        ariaLabel={t('translation.models.ariaLabel')}
        title={t('translation.models.title')}
        meta={modelsSummary}
      >
        <div className="translation-model-list">
          {installableModels.map((model) => {
            const size = formatModelSize(modelStatuses[model.id])
            return (
              <div key={model.id} className="translation-model-item">
                <div>
                  <strong title={model.notes}>{model.name}</strong>
                  <span>{formatModelLanguagePair(model, locale, t)}{size ? ' · ' + size : ''}</span>
                </div>
                <TranslationModelInstallButton
                  model={model}
                  progress={modelInstallState.progress?.modelId === model.id ? modelInstallState.progress : null}
                  status={modelStatuses[model.id]}
                  disabled={Boolean(modelInstallState.installingModelId)}
                  onInstall={onInstallTranslationModel}
                  t={t}
                />
              </div>
            )
          })}
          {plannedModels.length > 0 && (
            <div className="translation-planned-models">
              <span className="translation-kicker">{t('translation.models.planned')}</span>
              {plannedModels.map((model) => (
                <div key={model.id} className="translation-planned-model-row" title={model.notes}>
                  <strong>{model.name}</strong>
                  <span>
                    {formatModelLanguagePair(model, locale, t)} · {formatTierLabel(model.tier, t)}
                  </span>
                </div>
              ))}
            </div>
          )}
          {!modelOptions.length && (
            <p className="translation-section-empty">{t('translation.models.noMetadata')}</p>
          )}
        </div>
      </Panel>

      <Panel
        className="translation-subpanel"
        ariaLabel={t('translation.documents.ariaLabel')}
        title={t('translation.documents.title')}
        meta={t('translation.documents.savedCount', { count: translatedDocuments.length })}
        open={docsOpen}
        onToggle={() => setDocsOpen((value) => !value)}
      >
        {deleteState && showDeleteStatus && (
          <div className={'translation-alert' + (deleteState.deleted ? '' : ' translation-alert-error')} role="status">
            <button
              type="button"
              className="translation-alert-dismiss"
              aria-label={t('translation.status.dismissDelete')}
              onClick={() => dismissStatus(deleteStatusKey)}
            >
              ×
            </button>
            <span className="translation-alert-message">
              {deleteState.deleted
                ? t('translation.status.documentDeleted')
                : deleteState.message}
            </span>
          </div>
        )}
        {translatedDocuments.length > 0 ? (
          <div className="translation-document-list">
            {translatedDocuments.map((doc) => (
              <div key={doc.id} className="translation-document-item">
                <div>
                  <strong>{doc.title}</strong>
                  <span>
                    {formatLanguageLabel(doc.sourceLanguage, locale, t)}
                    {' → '}
                    {formatLanguageLabel(doc.targetLanguage, locale, t)}
                    {' · '}{modelNameById.get(doc.modelId) ?? doc.modelId}
                    {' · '}{formatStatusLabel(doc.status, t)}
                  </span>
                </div>
                <div className="translation-document-actions">
                  <button
                    type="button"
                    className="translation-document-view-btn"
                    aria-label={t('translation.documents.viewLabel', { title: doc.title })}
                    onClick={() => { void onOpenTranslatedDocument(doc.documentUrl) }}
                  >
                    {t('common.view')}
                  </button>
                  <button
                    type="button"
                    className="translation-document-delete-btn"
                    aria-label={t('translation.documents.deleteLabel', { title: doc.title })}
                    onClick={() => setConfirmingDeleteId(doc.id)}
                  >
                    {t('common.delete')}
                  </button>
                </div>
              </div>
            ))}
          </div>
        ) : (
          <p className="translation-section-empty">{t('translation.documents.empty')}</p>
        )}
      </Panel>
      </div>

      {confirmingDocument && (
        <AppDialog
          title={t('translation.confirmDelete.title')}
          description={t('translation.confirmDelete.description', { title: confirmingDocument.title })}
          onCancel={() => setConfirmingDeleteId('')}
          actions={
            <>
              <button
                type="button"
                className="app-dialog-cancel"
                onClick={() => setConfirmingDeleteId('')}
              >
                {t('translation.confirmDelete.keep')}
              </button>
              <button
                type="button"
                className="app-dialog-danger"
                onClick={() => {
                  setConfirmingDeleteId('')
                  void onDeleteTranslatedDocument(confirmingDocument.id)
                }}
              >
                {t('common.delete')}
              </button>
            </>
          }
        />
      )}
    </Panel>
  )
}

function TranslationProgressMeter({
  progress,
  t,
}: {
  progress: TranslationJobProgress
  t: TFunction
}) {
  return (
    <div className="translation-job-progress">
      <div className="translation-job-progress-header">
        <span>{formatStatusLabel(progress.status, t)}</span>
        <span>{progress.percent}%</span>
      </div>
      <div
        className="translation-progress-meter"
        aria-label={t('translation.progress.job', { percent: progress.percent })}
      >
        <span style={{ width: progress.percent + '%' }} />
      </div>
      <small>
        {t('translation.progress.counts', {
          completedSegments: progress.completedSegments,
          totalSegments: progress.totalSegments,
          completedBatches: progress.completedBatches,
          totalBatches: progress.totalBatches,
        })}
      </small>
      {progress.cachedSegments > 0 && (
        <small>
          {t('translation.progress.reusedCached', { count: progress.cachedSegments })}
          {progress.translatedSegments > 0
            ? ' · ' + t('translation.progress.translatedFresh', { count: progress.translatedSegments })
            : ''}
          {progress.reusedSegmentsInBatch > 0
            ? ' · ' + t('translation.progress.currentBatchReused', {
                count: progress.reusedSegmentsInBatch,
              })
            : ''}
        </small>
      )}
      {progress.preview && <small>{t('translation.progress.preview', { preview: progress.preview })}</small>}
    </div>
  )
}

interface TranslationModelInstallButtonProps {
  disabled: boolean
  model: TranslationModelInfo
  progress: TranslationModelInstallProgress | null
  status?: TranslationModelStatus
  onInstall: (modelId: string) => Promise<void>
  t: TFunction
}

function TranslationModelInstallButton({
  disabled,
  model,
  progress,
  status,
  onInstall,
  t,
}: TranslationModelInstallButtonProps) {
  const installable = model.manifestState === 'pinned-file-manifest'
  const installing = progress !== null || status?.installing
  if (status?.installed) {
    return <span className="translation-model-badge">{t('translation.models.installed')}</span>
  }
  if (!installable) {
    return (
      <span className="translation-model-badge translation-model-badge-muted">
        {t('translation.models.planned')}
      </span>
    )
  }
  return (
    <button
      type="button"
      className="translation-model-install-btn"
      disabled={disabled || installing}
      onClick={() => { void onInstall(model.id) }}
    >
      {installing
        ? t('translation.models.installing', { percent: progress?.percent ?? 0 })
        : t('translation.models.install')}
    </button>
  )
}

function formatDocumentFormat(format: string | undefined, t: TFunction): string {
  if (!format) return t('translation.workbench.document')
  return format.toUpperCase()
}

function formatModelLanguagePair(
  model: TranslationModelInfo,
  locale: string,
  t: TFunction,
): string {
  return (
    model.sourceLanguages.map((language) => formatLanguageLabel(language, locale, t)).join(', ') +
    ' → ' +
    model.targetLanguages.map((language) => formatLanguageLabel(language, locale, t)).join(', ')
  )
}

function formatTierLabel(tier: string, t: TFunction): string {
  switch (tier) {
    case 'fast':
      return t('translation.tiers.fast')
    case 'quality':
      return t('translation.tiers.highQuality')
    case 'context':
      return t('translation.tiers.contextRich')
    default:
      return formatStatusLabel(tier, t)
  }
}

// One-line state summary for the collapsed models disclosure.
function formatModelsSummary(
  installableModels: TranslationModelInfo[],
  plannedModels: TranslationModelInfo[],
  modelStatuses: Record<string, TranslationModelStatus>,
  installProgress: TranslationModelInstallProgress | null,
  t: TFunction,
): string {
  if (!installableModels.length && !plannedModels.length) {
    return t('translation.models.none')
  }
  const pieces: string[] = []
  if (installProgress && installProgress.status !== 'installed') {
    pieces.push(t('translation.models.installing', { percent: installProgress.percent }))
  }
  const installed = installableModels.filter((model) => modelStatuses[model.id]?.installed).length
  const installable = installableModels.length - installed
  if (installed) pieces.push(t('translation.models.installedCount', { count: installed }))
  if (installable) pieces.push(t('translation.models.installableCount', { count: installable }))
  if (plannedModels.length) {
    pieces.push(t('translation.models.plannedCount', { count: plannedModels.length }))
  }
  return pieces.join(' · ')
}

// Language names beat raw ISO codes for recognition; fall back to the code
// when the runtime cannot resolve a display name.
function formatLanguageLabel(language: string, locale: string, t: TFunction): string {
  if (language === 'auto') return t('translation.languages.autoDetect')
  try {
    const name = new Intl.DisplayNames([locale], { type: 'language' }).of(language)
    if (name && name !== language) return name
  } catch {
    // Unknown/invalid code: fall through to the uppercase code.
  }
  return language.toUpperCase()
}

function formatStatusLabel(status: string, t: TFunction): string {
  const labels: Record<string, string> = {
    starting: t('translation.progress.status.starting'),
    translating: t('translation.progress.status.translating'),
    validating: t('translation.progress.status.validating'),
    storing: t('translation.progress.status.storing'),
    completed: t('translation.progress.status.completed'),
    cancelled: t('translation.progress.status.cancelled'),
  }
  return labels[status] ?? status.charAt(0).toUpperCase() + status.slice(1)
}

// Size only: installed/installing state is already carried by the row's
// badge or button, so repeating it in text would be redundant.
function formatModelSize(status?: TranslationModelStatus): string {
  if (!status) return ''
  const bytes = status.installed ? status.installedBytes : status.archiveBytes
  return bytes > 0 ? formatStorageSize(bytes) ?? '' : ''
}

function formatInstallProgressMessage(
  progress: TranslationModelInstallProgress,
  t: TFunction,
): string {
  switch (progress.status) {
    case 'starting':
      return t('translation.status.preparingModel')
    case 'downloading':
      return t('translation.status.downloadingModel')
    case 'installed':
      return t('translation.status.modelInstalledMessage')
    default:
      return progress.message
  }
}

function uniqueOptions(values: string[]): string[] {
  return values.filter((value, index) => value && values.indexOf(value) === index)
}
