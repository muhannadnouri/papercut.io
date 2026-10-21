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
  TranslationModelRemoveResult,
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
  modelRemoveState: {
    removingModelId: string
    result: TranslationModelRemoveResult | null
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
  onRemoveTranslationModel: (modelId: string) => Promise<void>
  onStartTranslationPreflight: (request: TranslationStartRequest) => Promise<void>
  refresh: () => Promise<void>
}

export function TranslationPanel({
  capabilities,
  deleteState,
  error,
  loading,
  modelInstallState,
  modelRemoveState,
  modelStatuses,
  selectedDocument,
  startState,
  translatedDocuments,
  onCancelTranslation,
  onDeleteTranslatedDocument,
  onOpenTranslatedDocument,
  onInstallTranslationModel,
  onRemoveTranslationModel,
  onStartTranslationPreflight,
  refresh,
}: TranslationPanelProps) {
  const { t, i18n } = useTranslation()
  const locale = i18n.resolvedLanguage ?? i18n.language
  const webviewUnavailable = capabilities?.models.some(model => model.unavailableReason === 'webview') ?? false
  const acceptancePending = capabilities?.models.some(model => model.unavailableReason === 'acceptance-pending') ?? false
  const statusLabel = loading
    ? t('translation.status.checking')
    : capabilities?.available
      ? t('translation.status.available')
      : t(webviewUnavailable || acceptancePending ? 'translation.models.unavailable' : 'translation.status.unavailable')
  const modelOptions = useMemo(() => capabilities?.models ?? [], [capabilities])
  const installableModels = useMemo(
    () => modelOptions.filter((model) => model.manifestState === 'pinned-file-manifest'),
    [modelOptions],
  )
  const plannedModels = useMemo(
    () => modelOptions.filter((model) => model.manifestState !== 'pinned-file-manifest'),
    [modelOptions],
  )
  const runnableModels = useMemo(
    () => installableModels.filter(model => model.available),
    [installableModels],
  )
  const [confirmingDeleteId, setConfirmingDeleteId] = useState('')
  const [confirmingModelId, setConfirmingModelId] = useState('')
  const confirmingDocument = useMemo(
    () => translatedDocuments.find((doc) => doc.id === confirmingDeleteId) ?? null,
    [confirmingDeleteId, translatedDocuments],
  )
  const confirmingModel = useMemo(
    () => installableModels.find(model => model.id === confirmingModelId)
      ?? (confirmingModelId && capabilities?.formatting?.modelId === confirmingModelId
        ? { id: confirmingModelId, name: t('translation.formatting.packName'), sizeNotes: '' } : null),
    [confirmingModelId, installableModels, capabilities, t],
  )
  const [docsOpen, setDocsOpen] = useState(true)
  // Local-only dismissal of finished status messages; a new message (different
  // key) reappears without needing hook state changes.
  const [dismissedStatusKeys, setDismissedStatusKeys] = useState<string[]>([])
  const [modelId, setModelId] = useState('')
  const [sourceLanguage, setSourceLanguage] = useState('')
  const [targetLanguage, setTargetLanguage] = useState('en')
  const [useHardwareAcceleration, setUseHardwareAcceleration] = useState(false)
  // Bind consent to the exact engine/pair; switching away cannot silently carry it forward.
  const [formattingChoice, setFormattingChoice] = useState('')
  // Preserve catalog ordering/defaults, but never fall back to an unavailable engine.
  const activeModelId = runnableModels.some((model) => model.id === modelId)
    ? modelId
    : runnableModels[0]?.id ?? ''
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
  const bergamotSelected = selectedModel?.engine === 'bergamot'
  const formatting = capabilities?.formatting
  const formattingPair = ['opus-mt-fr-en-ctranslate2', 'opus-mt-es-en-ctranslate2'].includes(activeModelId)
    && activeSourceLanguage === (activeModelId.includes('-fr-') ? 'fr' : 'es') && activeTargetLanguage === 'en'
  const formattingKey = [activeModelId, activeSourceLanguage, activeTargetLanguage].join(':')
  const formattingStatus = formatting?.modelId ? modelStatuses[formatting.modelId] : undefined
  const formattingReady = Boolean(
    formattingPair && formatting?.available && formattingStatus?.installed && !formattingStatus.installing,
  )
  const formattingReasonKey = !formattingPair ? 'translation.formatting.unsupported'
    : formatting?.available ? 'translation.formatting.description'
      : formatting?.reason === 'acceptance-pending' ? 'translation.formatting.pending'
        : 'translation.formatting.runtimeUnavailable'
  const formattingRequested = formattingChoice !== ''
  const preserveFormatting = formattingReady && formattingChoice === formattingKey
  const supportedDocument = Boolean(selectedDocument && ['html', 'epub'].includes(selectedDocument.format ?? ''))
  const hardwareAcceleration = selectedModel?.id === 'hy-mt2-1.8b-q8'
    ? capabilities?.hardwareAcceleration ?? null
    : null
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
  const removeStatusKey = 'remove:' + modelRemoveState.message
  const deleteStatusKey = deleteState ? 'delete:' + deleteState.id + ':' + deleteState.message : ''
  const dismissStatus = (key: string) => {
    setDismissedStatusKeys((previous) => (previous.includes(key) ? previous : [...previous, key]))
  }
  const showJobStatus =
    Boolean(startState.message) && !(startState.result && dismissedStatusKeys.includes(jobStatusKey))
  const showInstallStatus =
    Boolean(modelInstallState.message) &&
    !(modelInstallState.result && dismissedStatusKeys.includes(installStatusKey))
  const showRemoveStatus =
    Boolean(modelRemoveState.message) &&
    !(modelRemoveState.result && dismissedStatusKeys.includes(removeStatusKey))
  const showDeleteStatus = Boolean(deleteState) && !dismissedStatusKeys.includes(deleteStatusKey)
  const backendUnavailable = Boolean(capabilities && !capabilities.available)
  const jobFailed = !startState.checking && !startState.result && Boolean(startState.message)
  const jobNeedsReview = startState.result?.status === 'needs-review'
  const installFailed =
    !modelInstallState.installingModelId &&
    !modelInstallState.result &&
    !modelInstallState.progress &&
    Boolean(modelInstallState.message)
  const removeFailed =
    !modelRemoveState.removingModelId &&
    !modelRemoveState.result &&
    Boolean(modelRemoveState.message)
  const jobMessage = startState.result
    ? t(jobNeedsReview ? 'translation.status.translationNeedsReviewMessage' : 'translation.status.translationCompleteMessage')
    : startState.cancelling
      ? t('translation.status.cancellingJob')
      : startState.checking && !startState.progress
        ? t('translation.status.preparingJob')
        : startState.checking && startState.progress && ['loading-alignment', 'aligning-batch', 'reconstructing-batch'].includes(startState.progress.status)
          ? formatProgressStatusLabel(startState.progress, t)
          : startState.message
  const installMessage = modelInstallState.result
    ? t('translation.status.modelInstalledMessage')
    : modelInstallState.installingModelId && !modelInstallState.progress
      ? t('translation.status.preparingModel')
      : modelInstallState.progress
        ? formatInstallProgressMessage(modelInstallState.progress, t)
        : modelInstallState.message
  const removeMessage = modelRemoveState.result
    ? t('translation.status.modelRemovedMessage', {
        size: formatStorageSize(modelRemoveState.result.bytesFreed) ?? '0 B',
      })
    : modelRemoveState.removingModelId
      ? t('translation.status.removingModel')
      : modelRemoveState.message

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
      {(error || backendUnavailable || showJobStatus || showInstallStatus || showRemoveStatus) && (
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
              <span>{t(acceptancePending
                ? 'translation.models.acceptancePending'
                : webviewUnavailable ? 'translation.models.webviewUnavailable'
                : 'translation.status.unavailableReason')}</span>
            </div>
          )}

          {showJobStatus && (
            <div
              className={'translation-alert' + (startState.result ? '' : ' translation-alert-neutral')}
              role="status"
            >
              <strong>
                {startState.result
                  ? t(jobNeedsReview ? 'translation.status.translationNeedsReview' : 'translation.status.translationComplete')
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

          {showRemoveStatus && (
            <div
              className={'translation-alert' + (removeFailed ? ' translation-alert-error' : '')}
              role={removeFailed ? 'alert' : 'status'}
            >
              <strong>
                {modelRemoveState.result
                  ? t('translation.status.modelRemoved')
                  : removeFailed
                    ? t('translation.status.modelRemoveFailed')
                    : t('translation.status.modelRemove')}
              </strong>
              {modelRemoveState.result && (
                <button
                  type="button"
                  className="translation-alert-dismiss"
                  aria-label={t('translation.status.dismissModelRemove')}
                  onClick={() => dismissStatus(removeStatusKey)}
                >
                  ×
                </button>
              )}
              <span className="translation-alert-message">{removeMessage}</span>
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
                disabled={!runnableModels.length || startState.checking}
                onChange={(event) => {
                  const nextModelId = event.target.value
                  const nextModel = installableModels.find((model) => model.id === nextModelId)
                  setFormattingChoice('')
                  setModelId(nextModelId)
                  setSourceLanguage(nextModel?.sourceLanguages[0] ?? '')
                  setTargetLanguage(nextModel?.targetLanguages[0] ?? 'en')
                }}
              >
                {installableModels.length ? installableModels.map((model) => (
                  <option key={model.id} value={model.id} disabled={!model.available}>
                    {model.name}{model.available ? '' : ' · ' + t('translation.models.unavailable')}
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
                onChange={(event) => { setFormattingChoice(''); setSourceLanguage(event.target.value) }}
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
                onChange={(event) => { setFormattingChoice(''); setTargetLanguage(event.target.value) }}
              >
                {targetLanguages.map((language) => (
                  <option key={language} value={language}>
                    {formatLanguageLabel(language, locale, t)}
                  </option>
                ))}
              </select>
            </label>
          </div>
          {selectedModel?.engine === 'ctranslate2' && (
            <p className="translation-resource-note">{t('translation.models.ctranslate2BoundaryNote')}</p>
          )}
          {bergamotSelected && (
            <p className="translation-resource-note">{t('translation.models.bergamotNote')}</p>
          )}
          {!supportedDocument ? (
            <p role="status">{t('translation.workbench.htmlEpubOnly')}</p>
          ) : selectedModel && !modelStatuses[activeModelId]?.installed && (
            <p role="status">{t('translation.errors.modelNotInstalled')}</p>
          )}
          {selectedModel?.id === 'hy-mt2-1.8b-q8' && (
            <p className="translation-resource-note">
              <strong>{t('translation.models.highResourceTitle')}</strong>{' '}
              {t('translation.models.hyMt2ResourceNote')}
            </p>
          )}
          {!bergamotSelected && (
            <section className="translation-resource-note" aria-label={t('translation.formatting.label')}>
              <label className="translation-acceleration-option">
                <input
                  type="checkbox"
                  checked={formattingRequested}
                  disabled={(!formattingReady && !formattingRequested) || startState.checking
                    || Boolean(modelInstallState.installingModelId) || Boolean(modelRemoveState.removingModelId)}
                  onChange={event => setFormattingChoice(event.target.checked ? formattingKey : '')}
                />
                <strong>{t('translation.formatting.label')}</strong>
              </label>
              <p>{t(formattingReasonKey)}</p>
              {formatting?.modelId && ((formattingPair && formatting.available) || formattingStatus?.installed) && (
                <>
                  <strong>{t('translation.formatting.packName')}</strong>
                  <p>{t('translation.formatting.downloadNote', { size: formatStorageSize(formatting.archiveBytes) ?? '' })}</p>
                  <p role="status">{t(formattingStatus?.installed ? 'translation.formatting.packReady' : 'translation.formatting.packMissing')}</p>
                  <TranslationModelInstallButton
                    model={{ id: formatting.modelId, name: t('translation.formatting.packName'), manifestState: 'pinned-file-manifest' }}
                    disabled={startState.checking || Boolean(modelInstallState.installingModelId) || Boolean(modelRemoveState.removingModelId)}
                    status={formattingStatus}
                    progress={modelInstallState.progress?.modelId === formatting.modelId ? modelInstallState.progress : null}
                    onInstall={onInstallTranslationModel}
                    onRequestRemove={setConfirmingModelId}
                    t={t}
                  />
                </>
              )}
            </section>
          )}
          {hardwareAcceleration && (
            <label className="translation-acceleration-option">
              <input
                type="checkbox"
                checked={useHardwareAcceleration}
                disabled={startState.checking}
                onChange={(event) => setUseHardwareAcceleration(event.target.checked)}
              />
              <span>
                <strong>{t('translation.workbench.hardwareAcceleration')}</strong>
                <small>
                  {t('translation.workbench.hardwareAccelerationDescription', {
                    device: hardwareAcceleration.device,
                  })}
                </small>
              </span>
            </label>
          )}
          <div className="translation-action-row">
            <button
              type="button"
              disabled={
                startState.checking ||
                Boolean(modelInstallState.installingModelId) ||
                Boolean(modelRemoveState.removingModelId) ||
                (formattingRequested && !preserveFormatting) ||
                !activeModelId ||
                !supportedDocument ||
                !modelStatuses[activeModelId]?.installed
              }
              title={t('translation.workbench.runTitle')}
              onClick={() => {
                void onStartTranslationPreflight({
                  documentUrl: selectedDocument.url,
                  sourceLanguage: activeSourceLanguage,
                  targetLanguage: activeTargetLanguage,
                  modelId: activeModelId,
                  qualityMode: activeQualityMode,
                  useHardwareAcceleration: Boolean(hardwareAcceleration && useHardwareAcceleration),
                  ...(preserveFormatting ? { preserveFormatting: true } : {}),
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
                  {!model.available && (
                    <small>{t(model.unavailableReason === 'acceptance-pending'
                      ? 'translation.models.acceptancePending'
                      : model.unavailableReason === 'webview' ? 'translation.models.webviewUnavailable'
                      : 'translation.models.engineUnavailable')}</small>
                  )}
                </div>
                <TranslationModelInstallButton
                  model={model}
                  progress={modelInstallState.progress?.modelId === model.id ? modelInstallState.progress : null}
                  status={modelStatuses[model.id]}
                  disabled={Boolean(
                    modelInstallState.installingModelId ||
                    modelRemoveState.removingModelId ||
                    startState.checking || (!model.available && !modelStatuses[model.id]?.installed)
                  )}
                  onInstall={onInstallTranslationModel}
                  onRequestRemove={setConfirmingModelId}
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
      {confirmingModel && (
        <AppDialog
          title={t('translation.confirmModelRemove.title')}
          description={t('translation.confirmModelRemove.description', {
            name: confirmingModel.name,
            size: formatModelSize(modelStatuses[confirmingModel.id]) || confirmingModel.sizeNotes,
          })}
          onCancel={() => setConfirmingModelId('')}
          actions={
            <>
              <button
                type="button"
                className="app-dialog-cancel"
                onClick={() => setConfirmingModelId('')}
              >
                {t('common.cancel')}
              </button>
              <button
                type="button"
                className="app-dialog-danger"
                onClick={() => {
                  setConfirmingModelId('')
                  void onRemoveTranslationModel(confirmingModel.id)
                }}
              >
                {t('translation.models.remove')}
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
  const statusLabel = formatProgressStatusLabel(progress, t)
  const indeterminate = progress.completedSegments === 0 && ![
    'completed',
    'needs-review',
    'validating',
    'stored',
    'cancelled',
    'failed',
  ].includes(progress.status)

  return (
    <div className="translation-job-progress">
      <div className="translation-job-progress-header">
        <span>{statusLabel}</span>
        <span>{progress.percent}%</span>
      </div>
      <div
        className={'translation-progress-meter' + (indeterminate ? ' is-indeterminate' : '')}
        role="progressbar"
        aria-label={t('translation.progress.job', { percent: progress.percent })}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={indeterminate ? undefined : progress.percent}
        aria-valuetext={statusLabel}
      >
        <span style={indeterminate ? undefined : { width: progress.percent + '%' }} />
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
  model: Pick<TranslationModelInfo, 'id' | 'name' | 'manifestState'>
  progress: TranslationModelInstallProgress | null
  status?: TranslationModelStatus
  onInstall: (modelId: string) => Promise<void>
  onRequestRemove: (modelId: string) => void
  t: TFunction
}

function TranslationModelInstallButton({
  disabled,
  model,
  progress,
  status,
  onInstall,
  onRequestRemove,
  t,
}: TranslationModelInstallButtonProps) {
  const installable = model.manifestState === 'pinned-file-manifest'
  const installing = progress !== null || status?.installing
  if (status?.installed) {
    return (
      <div className="translation-model-actions">
        <span className="translation-model-badge">{t('translation.models.installed')}</span>
        <button
          type="button"
          className="translation-model-remove-btn"
          disabled={disabled}
          aria-label={t('translation.models.removeLabel', { name: model.name })}
          onClick={() => onRequestRemove(model.id)}
        >
          {t('translation.models.remove')}
        </button>
      </div>
    )
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
      aria-label={`${t('translation.models.install')}: ${model.name}`}
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
  const installable = installableModels.filter(model => model.available && !modelStatuses[model.id]?.installed).length
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

function formatProgressStatusLabel(progress: TranslationJobProgress, t: TFunction): string {
  const nextBatch = Math.min(progress.completedBatches + 1, progress.totalBatches)
  switch (progress.status) {
    case 'loading-alignment':
      return t('translation.progress.status.loadingAlignment')
    case 'aligning-batch':
      return t('translation.progress.status.aligningBatch')
    case 'reconstructing-batch':
      return t('translation.progress.status.reconstructingBatch')
    case 'loading-model':
      return t('translation.progress.status.loadingModel')
    case 'starting':
      return t('translation.progress.status.starting')
    case 'preparing-batch':
      return t('translation.progress.status.preparingBatch', {
        current: nextBatch,
        total: progress.totalBatches,
      })
    case 'translating-batch':
      return t('translation.progress.status.translatingBatch', {
        current: nextBatch,
        total: progress.totalBatches,
      })
    case 'batch-complete':
      return t('translation.progress.status.batchComplete', {
        current: progress.completedBatches,
        total: progress.totalBatches,
      })
    case 'validating':
      return t('translation.progress.status.validating')
    case 'storing':
      return t('translation.progress.status.storing')
    case 'stored':
    case 'completed':
      return t('translation.progress.status.completed')
    case 'needs-review':
      return t('translation.status.translationNeedsReview')
    case 'failed':
      return t('translation.status.translationFailed')
    case 'cancelled':
      return t('translation.progress.status.cancelled')
    default:
      return progress.status.charAt(0).toUpperCase() + progress.status.slice(1)
  }
}

function formatStatusLabel(status: string, t: TFunction): string {
  const labels: Record<string, string> = {
    'loading-model': t('translation.progress.status.loadingModel'),
    starting: t('translation.progress.status.starting'),
    translating: t('translation.progress.status.translating'),
    validating: t('translation.progress.status.validating'),
    storing: t('translation.progress.status.storing'),
    stored: t('translation.progress.status.completed'),
    completed: t('translation.progress.status.completed'),
    'needs-review': t('translation.status.translationNeedsReview'),
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
