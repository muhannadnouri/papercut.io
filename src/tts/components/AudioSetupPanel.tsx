import { useCallback, useEffect, useRef, useState, type ReactNode } from 'react'
import type { TFunction } from 'i18next'
import { useTranslation } from 'react-i18next'
import type { NativeTtsModelInstallProgress, NativeTtsModelStatus } from '../api/nativeTts'
import { previewNativeTtsVoice } from '../api/nativeTts'
import { getTtsPreviewText } from '../models'
import { nativeTtsErrorMessage } from '../utils/errors'
import {
  DEFAULT_SILMA_NFE_STEP,
  LIBTASHKEEL_TEXT_PREPROCESSOR,
  SILMA_NFE_STEP_OPTIONS,
  TEXT_PREPROCESSOR_NONE,
  type TextPreprocessorInfo,
  type TtsModelInfo,
  type TtsVoice,
  type TtsVoiceInfo,
} from '../types'

const HIGH_THREAD_COUNT_WARNING_THRESHOLD = 4

interface SelectOption {
  label: string
  value: string | number
}

const LANGUAGE_LABELS: Record<string, { label: string; sortKey: string }> = {
  ar: { label: 'العربية (Arabic)', sortKey: 'Arabic' },
  en: { label: 'English', sortKey: 'English' },
  es: { label: 'Español (Spanish)', sortKey: 'Spanish' },
  fr: { label: 'Français (French)', sortKey: 'French' },
  hi: { label: 'हिन्दी (Hindi)', sortKey: 'Hindi' },
  it: { label: 'Italiano (Italian)', sortKey: 'Italian' },
  pt: { label: 'Português (Brasil) - Portuguese (Brazil)', sortKey: 'Portuguese' },
  zh: { label: '中文（普通话） - Chinese (Mandarin)', sortKey: 'Chinese' },
}

// The model metadata can be locale-specific (`ar-JO`) even when the UI should
// present one human language. Keep the full model id intact and group only the
// language dropdown by base language; the model dropdown carries dialect/engine detail.
function getLanguageOption(model: TtsModelInfo): { label: string; value: string } {
  const value = model.language.split('-')[0].toLowerCase() || model.language
  const languageLabel = LANGUAGE_LABELS[value]?.label
    ?? model.languageLabel
  return {
    label: languageLabel,
    value,
  }
}

export interface AudioSetupPanelProps {
  appliedThreadCount: number | null
  debugEnabled?: boolean
  defaultThreadCount: number
  maxThreadCount: number
  modelId: string
  models: TtsModelInfo[]
  modelInstallProgress: NativeTtsModelInstallProgress | null
  modelStatus: NativeTtsModelStatus | null
  onDiagnosticsChange?: (enabled: boolean) => void
  onInstallModel: () => void
  onModelChange: (modelId: string) => void
  onProbeSilmaSidecar?: () => void
  onPreviewStart: () => void
  onSilmaNfeStepChange: (nfeStep: number) => void
  onTextPreprocessorChange: (textPreprocessor: string) => void
  onThreadCountChange: (threadCount: number) => void
  onVoiceChange: (voice: TtsVoice) => void
  silmaProbeRunning?: boolean
  silmaNfeStep: number
  textPreprocessor: string
  textPreprocessors: TextPreprocessorInfo[]
  threadCount: number
  voice: TtsVoice
  voices: TtsVoiceInfo[]
}

export function AudioSetupPanel({
  appliedThreadCount,
  debugEnabled = false,
  defaultThreadCount,
  maxThreadCount,
  modelId,
  models,
  modelInstallProgress,
  modelStatus,
  onDiagnosticsChange,
  onInstallModel,
  onModelChange,
  onProbeSilmaSidecar,
  onPreviewStart,
  onSilmaNfeStepChange,
  onTextPreprocessorChange,
  onThreadCountChange,
  onVoiceChange,
  silmaProbeRunning = false,
  silmaNfeStep,
  textPreprocessor,
  textPreprocessors,
  threadCount,
  voice,
  voices,
}: AudioSetupPanelProps) {
  const { t } = useTranslation()
  const advancedDetailsRef = useRef<HTMLDetailsElement | null>(null)
  const previewAudioRef = useRef<HTMLAudioElement | null>(null)
  const previewRequestRef = useRef(0)
  const previewUrlRef = useRef<string | null>(null)
  const [previewState, setPreviewState] = useState<{
    selection: string
    status: 'idle' | 'loading' | 'playing'
  }>({ selection: '', status: 'idle' })
  const [previewError, setPreviewError] = useState<{ selection: string, message: string } | null>(null)
  const modelInstalling = modelStatus?.installing || (
    modelInstallProgress !== null &&
    modelInstallProgress.status !== 'installed' &&
    modelInstallProgress.status !== 'error'
  )
  const modelInstalled = Boolean(modelStatus?.installed || modelInstallProgress?.status === 'installed')
  const modelPercent = modelInstallProgress?.percent ?? 0
  const modelProgressMessage = formatModelInstallProgressMessage(modelInstallProgress, t)
  const modelSize = formatModelSize(modelStatus?.archiveBytes ?? modelInstallProgress?.totalBytes ?? 0)
  const threadOptions = Array.from({ length: maxThreadCount }, (_, index) => index + 1)
  const showHighThreadWarning = threadCount > HIGH_THREAD_COUNT_WARNING_THRESHOLD
  const hasTextProcessing = textPreprocessors.length > 1
  const selectedModel = models.find((model) => model.id === modelId) ?? models[0]
  const modelInstallSupported = modelStatus?.installSupported ?? (selectedModel?.family !== 'silma-f5')
  const isSilmaModel = selectedModel?.family === 'silma-f5'
  const silmaRuntimeMissing = isSilmaModel && modelStatus?.runtimeInstalled === false
  const installButtonLabel = silmaRuntimeMissing
    ? t('tts.setup.installSilma')
    : isSilmaModel ? t('tts.setup.downloadSilma') : t('tts.setup.downloadModel')
  const installingButtonLabel = silmaRuntimeMissing
    ? t('tts.setup.installingSilma')
    : isSilmaModel ? t('tts.setup.downloadingSilma') : t('tts.setup.downloadingModel')
  const sourceAssetLabel = isSilmaModel ? t('tts.setup.sourceHuggingFace') : t('tts.setup.sourceGitHub')
  const silmaInstallNote = isSilmaModel
    ? [
        silmaRuntimeMissing
          ? t('tts.setup.installRuntimeNote')
          : modelInstalled
            ? t('tts.setup.silmaInstalledNote')
            : t('tts.setup.silmaDownloadNote'),
        modelSize ? t('tts.setup.downloadSize', { size: modelSize }) : '',
        t('tts.setup.largeDownloadNote'),
      ].filter(Boolean).join(' ')
    : null
  const selectedLanguage = selectedModel ? getLanguageOption(selectedModel).value : ''
  const languageOptions = models.reduce<SelectOption[]>((options, model) => {
    const languageOption = getLanguageOption(model)
    const languageAlreadyAdded = options.some((option) => option.value === languageOption.value)
    if (!languageAlreadyAdded) {
      options.push(languageOption)
    }
    return options
  }, []).sort((a, b) => {
    const aSortKey = LANGUAGE_LABELS[String(a.value)]?.sortKey ?? a.label
    const bSortKey = LANGUAGE_LABELS[String(b.value)]?.sortKey ?? b.label
    return aSortKey.localeCompare(bSortKey)
  })
  const modelsForLanguage = selectedLanguage
    ? models.filter((model) => getLanguageOption(model).value === selectedLanguage)
    : models
  const showModelInstallDetails = !modelInstalled || silmaRuntimeMissing || modelInstallProgress !== null || modelInstalling
  const recommendedVoice = voices.find((item) => item.id === selectedModel?.defaultVoice)
  const previewSelection = [modelId, voice, textPreprocessor, silmaNfeStep].join('\0')
  const previewStatus = previewState.selection === previewSelection ? previewState.status : 'idle'
  const previewErrorMessage = previewError?.selection === previewSelection ? previewError.message : null
  // Keep active non-default behavior visible while Advanced is collapsed.
  const advancedSummary = [
    hasTextProcessing && textPreprocessor !== selectedModel?.defaultTextPreprocessor
      ? localizeTextPreprocessor(
          textPreprocessors.find((item) => item.id === textPreprocessor),
          t,
        ).name
      : '',
    threadCount !== defaultThreadCount
      ? `${showHighThreadWarning ? '⚠ ' : ''}${t('tts.setup.threadCount', { count: threadCount })}`
      : '',
    debugEnabled ? `${t('tts.setup.diagnostics')}: ${t('tts.setup.on')}` : '',
    isSilmaModel && silmaNfeStep !== DEFAULT_SILMA_NFE_STEP
      ? silmaNfeStepLabel(silmaNfeStep, t)
      : '',
  ].filter(Boolean).join(' · ')

  useEffect(() => {
    if (showHighThreadWarning && advancedDetailsRef.current) {
      advancedDetailsRef.current.open = true
    }
  }, [showHighThreadWarning])

  const clearPreviewAudio = useCallback(() => {
    previewRequestRef.current += 1
    previewAudioRef.current?.pause()
    previewAudioRef.current = null
    if (previewUrlRef.current) URL.revokeObjectURL(previewUrlRef.current)
    previewUrlRef.current = null
  }, [])
  const stopPreview = useCallback(() => {
    clearPreviewAudio()
    setPreviewState({ selection: '', status: 'idle' })
  }, [clearPreviewAudio])

  useEffect(() => clearPreviewAudio, [clearPreviewAudio, previewSelection])

  const handlePreview = async () => {
    if (previewStatus === 'playing') {
      stopPreview()
      return
    }
    onPreviewStart()
    setPreviewState({ selection: previewSelection, status: 'loading' })
    setPreviewError(null)
    const requestId = previewRequestRef.current + 1
    previewRequestRef.current = requestId
    try {
      const wav = await previewNativeTtsVoice({
        modelId,
        textPreprocessor,
        voice,
        text: getTtsPreviewText(selectedModel?.language ?? 'en'),
        speed: 1,
        threadCount,
        silmaNfeStep,
      })
      if (previewRequestRef.current !== requestId) return
      const url = URL.createObjectURL(new Blob([wav], { type: 'audio/wav' }))
      const audio = new Audio(url)
      previewUrlRef.current = url
      previewAudioRef.current = audio
      audio.onended = stopPreview
      await audio.play()
      if (previewRequestRef.current !== requestId) return
      setPreviewState({ selection: previewSelection, status: 'playing' })
    } catch (error) {
      stopPreview()
      setPreviewError({
        selection: previewSelection,
        message: t('tts.setup.previewError', { message: nativeTtsErrorMessage(error) }),
      })
    }
  }

  return (
    <div className="audio-setup-panel">
      <section className="audio-setup-group" aria-label={t('tts.setup.voiceSettings')}>
        <h4 className="audio-setup-group-title">{t('tts.setup.voiceSection')}</h4>
        <div className="audio-settings-grid audio-settings-grid-main">
          <SelectField
            className="audio-field-language"
            label={'🌐 ' + t('tts.setup.language')}
            title={t('tts.setup.speechLanguage')}
            value={selectedLanguage}
            options={languageOptions}
            onChange={(language) => {
              const nextModel = models.find((model) => getLanguageOption(model).value === language)
              if (nextModel) onModelChange(nextModel.id)
            }}
          />

          <div className="audio-field audio-field-model">
            <div className="audio-field-heading">
              <span>
                {'🤖 ' + t('tts.setup.model')}
                {modelInstalled && (
                  <span className="audio-model-state audio-model-state-installed">
                    (<CheckIcon /><span>{t('tts.setup.installed')}</span>)
                  </span>
                )}
              </span>
            </div>
            <select
              className="tts-select"
              value={modelId}
              onChange={(event) => onModelChange(event.target.value)}
              title={t('tts.setup.speechModel')}
            >
              {modelsForLanguage.map((model) => (
                <option key={model.id} value={model.id}>
                  {model.name}
                </option>
              ))}
            </select>
            {debugEnabled && (
              <div className="audio-model-source" title={modelStatus?.sourceUrl}>
                <span>{modelStatus?.sourceLabel ?? t('tts.setup.sourceSherpa')}</span>
                <span>{modelSize ? modelSize + ' ' + sourceAssetLabel : sourceAssetLabel}</span>
              </div>
            )}
          </div>

          {showModelInstallDetails && (
            <div className="audio-model-install">
              {!isSilmaModel && !modelInstalled && modelSize && (
                <span className="audio-thread-meta" dir="auto">
                  {t('tts.setup.downloadSize', { size: modelSize })}
                </span>
              )}
              {(!modelInstalled || silmaRuntimeMissing) && modelInstallSupported && (
                <button
                  type="button"
                  className="tts-btn tts-save-btn"
                  onClick={onInstallModel}
                  disabled={modelInstalling}
                  title={silmaRuntimeMissing
                    ? t('tts.setup.installRuntimeTitle')
                    : isSilmaModel ? t('tts.setup.downloadSilmaTitle') : t('tts.setup.downloadModelTitle')}
                >
                  <DownloadIcon />
                  <span>{modelInstalling ? installingButtonLabel : installButtonLabel}</span>
                </button>
              )}
              {silmaInstallNote && <span className="audio-thread-meta" dir="auto">{silmaInstallNote}</span>}
              {!modelInstalled && !modelInstallSupported && (
                <div className="audiobook-status audiobook-status-error" aria-live="polite">
                  <div className="audiobook-status-row">
                    <span dir="auto">{modelStatus?.message ?? t('tts.setup.manualInstall')}</span>
                  </div>
                </div>
              )}
              {(modelInstallProgress || modelInstalling) && (
                <div
                  className={'audiobook-status audiobook-status-' + (modelInstallProgress?.status === 'error' ? 'error' : modelInstalled ? 'saved' : 'saving')}
                  aria-live="polite"
                >
                  <div className="audiobook-status-row">
                    <span dir="auto">{modelProgressMessage ?? modelStatus?.message ?? t('tts.setup.preparingDownload')}</span>
                    <span>{modelPercent}%</span>
                  </div>
                  {!modelInstalled && modelInstallProgress?.status !== 'error' && (
                    <div
                      className="audio-progress-meter"
                      role="progressbar"
                      aria-label={installingButtonLabel}
                      aria-valuemin={0}
                      aria-valuemax={100}
                      aria-valuenow={modelPercent}
                    >
                      <span style={{ width: modelPercent + '%' }} />
                    </div>
                  )}
                </div>
              )}
              {silmaRuntimeMissing && (
                <div className="audiobook-status audiobook-status-error" aria-live="polite">
                  <div className="audiobook-status-row">
                    <span dir="auto">{modelStatus?.runtimeMessage ?? t('tts.setup.runtimeMissing')}</span>
                  </div>
                  {modelStatus?.runtimeDir && (
                    <div className="audiobook-status-row">
                      <span dir="ltr">{modelStatus.runtimeDir}</span>
                    </div>
                  )}
                </div>
              )}
            </div>
          )}

          <div className="audio-field audio-field-voice">
            <span>{'🔊 ' + t('tts.setup.voice')}</span>
            <select
              className="tts-select"
              value={voice}
              onChange={(event) => onVoiceChange(event.target.value as TtsVoice)}
              title={t('tts.setup.voice')}
            >
              {voices.map((item) => (
                <option key={item.id} value={item.id}>
                  {item.name}{item.id === selectedModel?.defaultVoice ? ' · ' + t('tts.setup.recommended') : ''}
                </option>
              ))}
            </select>
            {recommendedVoice && (
              <span className="audio-thread-meta" dir="auto">
                {t('tts.setup.recommendedVoice', { voice: recommendedVoice.name })}
              </span>
            )}
            {isSilmaModel && (
              <span className="audio-thread-meta">
                {t('tts.setup.silmaVoiceHelp')}
              </span>
            )}
            <button
              type="button"
              className="tts-btn audio-preview-button"
              onClick={() => void handlePreview()}
              disabled={!modelInstalled || silmaRuntimeMissing || modelInstalling || previewStatus === 'loading'}
              aria-busy={previewStatus === 'loading'}
              aria-pressed={previewStatus === 'playing'}
              title={!modelInstalled || silmaRuntimeMissing ? t('tts.setup.previewModelRequired') : t('tts.setup.previewVoice')}
            >
              {previewStatus === 'loading'
                ? <span className="spinner audio-preview-spinner" aria-hidden="true" />
                : <PreviewIcon playing={previewStatus === 'playing'} />}
              <span aria-live="polite" aria-atomic="true">
                {previewStatus === 'loading'
                  ? t('tts.setup.generatingPreview')
                  : previewStatus === 'playing'
                    ? t('tts.setup.stopPreview')
                    : t('tts.setup.previewVoice')}
              </span>
            </button>
            {previewErrorMessage && <span className="audio-preview-error" role="alert" dir="auto">{previewErrorMessage}</span>}
          </div>

        </div>
      </section>

      <details ref={advancedDetailsRef} className="audio-setup-group audio-setup-advanced">
        <summary className="audio-setup-advanced-summary">
          <span className="audio-setup-group-title">{t('tts.setup.advanced')}</span>
          {advancedSummary && (
            <span className="audio-setup-advanced-state" dir="auto">{advancedSummary}</span>
          )}
        </summary>
        <div className="audio-setup-advanced-grid">
          {hasTextProcessing && (
            <SelectField
              className="audio-field-text-processing"
              label={'✨ ' + t('tts.setup.textProcessing')}
              title={t('tts.setup.textProcessingTitle')}
              value={textPreprocessor}
              options={textPreprocessors.map((item) => ({
                label: localizeTextPreprocessor(item, t).name,
                value: item.id,
              }))}
              onChange={onTextPreprocessorChange}
            >
              <span className="audio-thread-meta">
                {localizeTextPreprocessor(
                  textPreprocessors.find((item) => item.id === textPreprocessor),
                  t,
                ).description}
              </span>
            </SelectField>
          )}
          <SelectField
            className="audio-field-threads"
            label={'🧵 ' + t('tts.setup.threads')}
            selectClassName="tts-threads"
            title={t('tts.setup.threadsTitle')}
            value={threadCount}
            options={threadOptions.map((count) => ({
              label: t('tts.setup.threadCount', { count }),
              value: count,
            }))}
            onChange={(value) => onThreadCountChange(Number(value))}
          >
            <span className="audio-thread-meta">
              {appliedThreadCount !== null
                ? t('tts.setup.threadSummaryApplied', {
                    default: defaultThreadCount,
                    max: maxThreadCount,
                    applied: appliedThreadCount,
                  })
                : t('tts.setup.threadSummary', {
                    default: defaultThreadCount,
                    max: maxThreadCount,
                  })}
            </span>
            {showHighThreadWarning && (
              <span className="audio-thread-warning" role="alert">
                {t('tts.setup.threadWarning')}
              </span>
            )}
          </SelectField>
          <label className="audio-field audio-field-diagnostics" title={t('tts.setup.diagnosticsTitle')}>
            <span>{'🧪 ' + t('tts.setup.diagnostics')}</span>
            <span className="audio-diagnostics-control">
              <span className="audio-diagnostics-value">{debugEnabled ? t('tts.setup.on') : t('tts.setup.off')}</span>
              <input
                type="checkbox"
                checked={debugEnabled}
                onChange={(event) => onDiagnosticsChange?.(event.target.checked)}
                disabled={!onDiagnosticsChange}
              />
              <span className="audio-diagnostics-switch" aria-hidden="true" />
            </span>
          </label>
          {isSilmaModel && (
            <SelectField
              className="audio-field-silma-quality"
              label={'🎚️ ' + t('tts.setup.silmaQuality')}
              title={t('tts.setup.silmaQualityTitle')}
              value={silmaNfeStep}
              options={SILMA_NFE_STEP_OPTIONS.map((step) => ({
                label: silmaNfeStepLabel(step, t),
                value: step,
              }))}
              onChange={(value) => onSilmaNfeStepChange(Number(value))}
            >
              <span className="audio-thread-meta">
                {t('tts.setup.qualityHelp')}
              </span>
            </SelectField>
          )}
          {isSilmaModel && onProbeSilmaSidecar && (
            <div className="audio-field audio-field-silma-probe">
              <span><span aria-hidden="true">🚗</span> {t('tts.setup.silmaSidecar')}</span>
              <button
                type="button"
                className="audio-probe-button"
                onClick={onProbeSilmaSidecar}
                disabled={silmaProbeRunning}
                title={t('tts.setup.probeTitle')}
              >
                {silmaProbeRunning ? t('tts.setup.probing') : t('tts.setup.probeSidecar')}
              </button>
            </div>
          )}
        </div>
      </details>
    </div>
  )
}

function silmaNfeStepLabel(step: number, t: TFunction): string {
  if (step === 64) return t('tts.setup.qualityHighest', { step })
  if (step === 32) return t('tts.setup.qualityHigh', { step })
  if (step === 16) return t('tts.setup.qualityBalanced', { step })
  if (step === 12) return t('tts.setup.qualityFast', { step })
  if (step === 8) return t('tts.setup.qualityFaster', { step })
  return t('tts.setup.qualityFastest', { step })
}

function localizeTextPreprocessor(
  item: TextPreprocessorInfo | undefined,
  t: TFunction,
): Pick<TextPreprocessorInfo, 'name' | 'description'> {
  if (!item) return { name: '', description: '' }
  if (item.id === TEXT_PREPROCESSOR_NONE) {
    return {
      name: t('tts.setup.preprocessorOriginal'),
      description: item.description.includes('Arabic')
        ? t('tts.setup.preprocessorArabicOriginalDescription')
        : t('tts.setup.preprocessorOriginalDescription'),
    }
  }
  if (item.id === LIBTASHKEEL_TEXT_PREPROCESSOR) {
    return {
      name: t('tts.setup.preprocessorTashkeel'),
      description: t('tts.setup.preprocessorTashkeelDescription'),
    }
  }
  if (item.id === 'silma-default') {
    return {
      name: t('tts.setup.preprocessorSilma'),
      description: t('tts.setup.preprocessorSilmaDescription'),
    }
  }
  return item
}

function formatModelInstallProgressMessage(
  progress: NativeTtsModelInstallProgress | null,
  t: TFunction,
): string | undefined {
  if (!progress) return undefined
  if (progress.status === 'error') return progress.message
  if (progress.status === 'installed') return t('tts.setup.installed')
  if (progress.status === 'extracting') return t('tts.audiobooks.extracting')
  if (progress.status === 'downloading') {
    return t('tts.audiobooks.downloadingPercent', { percent: progress.percent })
  }
  return t('tts.audiobooks.startingDownload')
}

function SelectField({
  children,
  className,
  label,
  onChange,
  options,
  selectClassName,
  title,
  value,
}: {
  children?: ReactNode
  className: string
  label: string
  onChange: (value: string) => void
  options: SelectOption[]
  selectClassName?: string
  title: string
  value: string | number
}) {
  return (
    <label className={'audio-field ' + className}>
      <span>{label}</span>
      <select
        className={'tts-select' + (selectClassName ? ' ' + selectClassName : '')}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        title={title}
      >
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
      {children}
    </label>
  )
}

function DownloadIcon() {
  return (
    <svg className="audio-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M12 3v10m0 0 4-4m-4 4-4-4M5 17v3h14v-3" fill="none" stroke="currentcolor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  )
}

function CheckIcon() {
  return (
    <svg className="audio-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="m4 12 5 5L20 6" fill="none" stroke="currentcolor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  )
}

function PreviewIcon({ playing }: { playing: boolean }) {
  return (
    <svg className="audio-preview-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d={playing ? 'M7 7h10v10H7z' : 'M8 5v14l11-7z'} />
    </svg>
  )
}

function formatModelSize(bytes: number): string | null {
  if (!Number.isFinite(bytes) || bytes <= 0) return null
  if (bytes >= 1024 * 1024 * 1024) return (bytes / 1024 / 1024 / 1024).toFixed(2) + ' GB'
  return Math.round(bytes / 1024 / 1024) + ' MB'
}
