import type { ReactNode } from 'react'
import type { TFunction } from 'i18next'
import { useTranslation } from 'react-i18next'
import type { NativeTtsModelInstallProgress, NativeTtsModelStatus } from '../api/nativeTts'
import {
  LIBTASHKEEL_TEXT_PREPROCESSOR,
  SILMA_NFE_STEP_OPTIONS,
  TEXT_PREPROCESSOR_NONE,
  type TextPreprocessorInfo,
  type TtsModelInfo,
  type TtsVoice,
  type TtsVoiceInfo,
} from '../types'
import { formatSpeedLabel } from '../utils/format'

const HIGH_THREAD_COUNT_WARNING_THRESHOLD = 4

const SPEED_MIN = 0.5
const SPEED_MAX = 2
const SPEED_STEP = 0.05

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

// Snap to the slider step and clamp to range. The saved-audiobook cache id buckets
// speed to 2 decimals on both the JS and Rust side, so values must round-trip cleanly
// at that precision; this also avoids float drift breaking equality checks on reload.
function snapSpeed(value: number): number {
  if (!Number.isFinite(value)) return 1
  const snapped = Math.round(value / SPEED_STEP) * SPEED_STEP
  const clamped = Math.min(SPEED_MAX, Math.max(SPEED_MIN, snapped))
  return Number(clamped.toFixed(2))
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
  onSilmaNfeStepChange: (nfeStep: number) => void
  onSpeedChange: (speed: number) => void
  onTextPreprocessorChange: (textPreprocessor: string) => void
  onThreadCountChange: (threadCount: number) => void
  onVoiceChange: (voice: TtsVoice) => void
  speed: number
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
  onSilmaNfeStepChange,
  onSpeedChange,
  onTextPreprocessorChange,
  onThreadCountChange,
  onVoiceChange,
  speed,
  silmaProbeRunning = false,
  silmaNfeStep,
  textPreprocessor,
  textPreprocessors,
  threadCount,
  voice,
  voices,
}: AudioSetupPanelProps) {
  const { t } = useTranslation()
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
                    <div className="audio-progress-meter" aria-label={'Voice model download ' + modelPercent + '% complete'}>
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

          <SelectField
            className="audio-field-voice"
            label={'🔊 ' + t('tts.setup.voice')}
            title={t('tts.setup.voice')}
            value={voice}
            options={voices.map((item) => ({ label: item.name, value: item.id }))}
            onChange={(value) => onVoiceChange(value as TtsVoice)}
          >
            {isSilmaModel && (
              <span className="audio-thread-meta">
                {t('tts.setup.silmaVoiceHelp')}
              </span>
            )}
          </SelectField>

          <div className="audio-field audio-field-speed audio-field-disabled">
            <span id="tts-speed-label">{'⚡ ' + t('tts.setup.generatedSpeed')}</span>
            <div className="audio-speed-row">
              <button
                type="button"
                className="audio-speed-step"
                onClick={() => onSpeedChange(snapSpeed(speed - SPEED_STEP))}
                disabled
                aria-label={t('tts.setup.decreaseSpeed')}
                title={t('tts.setup.fixedSpeed')}
              >
                &minus;
              </button>
              <input
                type="range"
                className="tts-speed-slider"
                min={SPEED_MIN}
                max={SPEED_MAX}
                step={SPEED_STEP}
                value={speed}
                onChange={(event) => onSpeedChange(snapSpeed(Number(event.target.value)))}
                disabled
                title={t('tts.setup.fixedSpeed')}
                aria-labelledby="tts-speed-label"
                aria-describedby="tts-speed-help"
              />
              <button
                type="button"
                className="audio-speed-step"
                onClick={() => onSpeedChange(snapSpeed(speed + SPEED_STEP))}
                disabled
                aria-label={t('tts.setup.increaseSpeed')}
                title={t('tts.setup.fixedSpeed')}
              >
                +
              </button>
              <span className="audio-speed-value">{formatSpeedLabel(speed)}</span>
            </div>
            <span id="tts-speed-help" className="audio-thread-meta">
              {t('tts.setup.speedHelp')}
            </span>
          </div>
        </div>
      </section>

      <section className="audio-setup-group audio-setup-advanced" aria-label={t('tts.setup.advancedSettings')}>
        <div className="audio-setup-group-heading">
          <h4 className="audio-setup-group-title">{t('tts.setup.advanced')}</h4>
        </div>
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
            <span>{t('tts.setup.silmaSidecar')}</span>
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
      </section>
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

function formatModelSize(bytes: number): string | null {
  if (!Number.isFinite(bytes) || bytes <= 0) return null
  if (bytes >= 1024 * 1024 * 1024) return (bytes / 1024 / 1024 / 1024).toFixed(2) + ' GB'
  return Math.round(bytes / 1024 / 1024) + ' MB'
}
