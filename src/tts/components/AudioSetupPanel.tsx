import type { ReactNode } from 'react'
import type { NativeTtsModelInstallProgress, NativeTtsModelStatus } from '../api/nativeTts'
import type { TextPreprocessorInfo, TtsModelInfo, TtsVoice, TtsVoiceInfo } from '../types'
import { formatSpeedLabel } from '../utils/format'

const HIGH_THREAD_COUNT_WARNING_THRESHOLD = 4

const SPEED_MIN = 0.5
const SPEED_MAX = 2
const SPEED_STEP = 0.05

interface SelectOption {
  label: string
  value: string | number
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
  return {
    label: model.languageLabel.replace(/\s*\([^)]*\)$/, ''),
    value: model.language.split('-')[0].toLowerCase() || model.language,
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
  onSpeedChange: (speed: number) => void
  onTextPreprocessorChange: (textPreprocessor: string) => void
  onThreadCountChange: (threadCount: number) => void
  onVoiceChange: (voice: TtsVoice) => void
  speed: number
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
  onSpeedChange,
  onTextPreprocessorChange,
  onThreadCountChange,
  onVoiceChange,
  speed,
  textPreprocessor,
  textPreprocessors,
  threadCount,
  voice,
  voices,
}: AudioSetupPanelProps) {
  const modelInstalling = modelStatus?.installing || (
    modelInstallProgress !== null &&
    modelInstallProgress.status !== 'installed' &&
    modelInstallProgress.status !== 'error'
  )
  const modelInstalled = Boolean(modelStatus?.installed || modelInstallProgress?.status === 'installed')
  const modelPercent = modelInstallProgress?.percent ?? 0
  const modelSize = formatModelSize(modelStatus?.archiveBytes ?? modelInstallProgress?.totalBytes ?? 0)
  const threadOptions = Array.from({ length: maxThreadCount }, (_, index) => index + 1)
  const showHighThreadWarning = threadCount > HIGH_THREAD_COUNT_WARNING_THRESHOLD
  const hasTextProcessing = textPreprocessors.length > 1
  const selectedModel = models.find((model) => model.id === modelId) ?? models[0]
  const selectedLanguage = selectedModel ? getLanguageOption(selectedModel).value : ''
  const languageOptions = models.reduce<SelectOption[]>((options, model) => {
    const languageOption = getLanguageOption(model)
    const languageAlreadyAdded = options.some((option) => option.value === languageOption.value)
    if (!languageAlreadyAdded) {
      options.push(languageOption)
    }
    return options
  }, [])
  const modelsForLanguage = selectedLanguage
    ? models.filter((model) => getLanguageOption(model).value === selectedLanguage)
    : models

  return (
    <div className="audio-setup-panel">
      <section className="audio-setup-group" aria-label="Voice settings">
        <h4 className="audio-setup-group-title">Voice</h4>
        <div className="audio-settings-grid audio-settings-grid-main">
          <SelectField
            className="audio-field-language"
            label="🌐 Language"
            title="Speech language"
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
                🤖 Model
                {modelInstalled && (
                  <span className="audio-model-state audio-model-state-installed">
                    (<CheckIcon /><span>Installed</span>)
                  </span>
                )}
              </span>
            </div>
            <select
              className="tts-select"
              value={modelId}
              onChange={(event) => onModelChange(event.target.value)}
              title="Speech model"
            >
              {modelsForLanguage.map((model) => (
                <option key={model.id} value={model.id}>
                  {model.name}
                </option>
              ))}
            </select>
            {debugEnabled && (
              <div className="audio-model-source" title={modelStatus?.sourceUrl}>
                <span>{modelStatus?.sourceLabel ?? 'sherpa-onnx offline TTS'}</span>
                <span>{modelSize ? modelSize + ' GitHub release' : 'GitHub release asset'}</span>
              </div>
            )}
            {!modelInstalled && (
              <button
                type="button"
                className="tts-btn tts-save-btn"
                onClick={onInstallModel}
                disabled={modelInstalling}
                title="Download selected offline voice model"
              >
                <DownloadIcon />
                <span>{modelInstalling ? 'Downloading Model...' : 'Download Voice Model'}</span>
              </button>
            )}
            {(modelInstallProgress || modelInstalling) && (
              <div className={'audiobook-status audiobook-status-' + (modelInstallProgress?.status === 'error' ? 'error' : modelInstalled ? 'saved' : 'saving')}>
                <div className="audiobook-status-row">
                  <span>{modelInstallProgress?.message ?? modelStatus?.message ?? 'Preparing model download'}</span>
                  <span>{modelPercent}%</span>
                </div>
                {!modelInstalled && modelInstallProgress?.status !== 'error' && (
                  <div className="audio-progress-meter" aria-label={'Voice model download ' + modelPercent + '% complete'}>
                    <span style={{ width: modelPercent + '%' }} />
                  </div>
                )}
              </div>
            )}
          </div>

          <SelectField
            className="audio-field-voice"
            label="🔊 Voice"
            title="Voice"
            value={voice}
            options={voices.map((item) => ({ label: item.name, value: item.id }))}
            onChange={(value) => onVoiceChange(value as TtsVoice)}
          />

          <div className="audio-field audio-field-speed">
            <span id="tts-speed-label">⚡ Generated Speed</span>
            <div className="audio-speed-row">
              <button
                type="button"
                className="audio-speed-step"
                onClick={() => onSpeedChange(snapSpeed(speed - SPEED_STEP))}
                disabled={speed <= SPEED_MIN}
                aria-label="Decrease Speed"
                title="Decrease Speed"
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
                title="Generated audio speed"
                aria-labelledby="tts-speed-label"
              />
              <button
                type="button"
                className="audio-speed-step"
                onClick={() => onSpeedChange(snapSpeed(speed + SPEED_STEP))}
                disabled={speed >= SPEED_MAX}
                aria-label="Increase Speed"
                title="Increase Speed"
              >
                +
              </button>
              <span className="audio-speed-value">{formatSpeedLabel(speed)}</span>
            </div>
          </div>
        </div>
      </section>

      <section className="audio-setup-group audio-setup-advanced" aria-label="Advanced audio settings">
        <div className="audio-setup-group-heading">
          <h4 className="audio-setup-group-title">Advanced</h4>
        </div>
        {hasTextProcessing && (
          <SelectField
            className="audio-field-text-processing"
            label="✨ Text Processing"
            title="Optional language preprocessing before speech synthesis"
            value={textPreprocessor}
            options={textPreprocessors.map((item) => ({ label: item.name, value: item.id }))}
            onChange={onTextPreprocessorChange}
          >
            <span className="audio-thread-meta">
              {textPreprocessors.find((item) => item.id === textPreprocessor)?.description}
            </span>
          </SelectField>
        )}
        <SelectField
          className="audio-field-threads"
          label="🧵 Threads"
          selectClassName="tts-threads"
          title="Native TTS threads"
          value={threadCount}
          options={threadOptions.map((count) => ({
            label: count + ' ' + (count === 1 ? 'thread' : 'threads'),
            value: count,
          }))}
          onChange={(value) => onThreadCountChange(Number(value))}
        >
          <span className="audio-thread-meta">
            Default {defaultThreadCount}, detected max {maxThreadCount}
            {appliedThreadCount !== null ? `, save applied ${appliedThreadCount}` : ''}
          </span>
          {showHighThreadWarning && (
            <span className="audio-thread-warning" role="alert">
              High thread counts can increase memory use, heat, battery drain, and thermal throttling. More threads may be slower.
            </span>
          )}
        </SelectField>
        <label className="audio-field audio-field-diagnostics" title="Show TTS diagnostic events and model source details">
          <span>🧪 Diagnostics</span>
          <span className="audio-diagnostics-control">
            <span className="audio-diagnostics-value">{debugEnabled ? 'On' : 'Off'}</span>
            <input
              type="checkbox"
              checked={debugEnabled}
              onChange={(event) => onDiagnosticsChange?.(event.target.checked)}
              disabled={!onDiagnosticsChange}
            />
            <span className="audio-diagnostics-switch" aria-hidden="true" />
          </span>
        </label>
      </section>
    </div>
  )
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
