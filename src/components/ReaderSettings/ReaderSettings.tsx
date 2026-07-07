import { useEffect, useRef, useState } from 'react'
import {
  FONT_FAMILY_OPTIONS,
  READER_SETTING_LIMITS,
  clampReaderNumber,
  type ReaderRangeConfig,
  type ReaderSettingsState,
} from './useReaderSettings'
import './ReaderSettings.css'

interface ReaderSettingsProps {
  disabled?: boolean
  settings: ReaderSettingsState
  onChange: (next: Partial<ReaderSettingsState>) => void
  onReset: () => void
}


export function ReaderSettings({
  disabled = false,
  settings,
  onChange,
  onReset,
}: ReaderSettingsProps) {
  if (disabled) {
    return (
      <div className="reader-settings">
        <ReaderSettingsButton disabled open={false} onClick={() => {}} />
      </div>
    )
  }

  return (
    <EnabledReaderSettings
      settings={settings}
      onChange={onChange}
      onReset={onReset}
    />
  )
}

function EnabledReaderSettings({
  settings,
  onChange,
  onReset,
}: Omit<ReaderSettingsProps, 'disabled'>) {
  const rootRef = useRef<HTMLDivElement | null>(null)
  const [open, setOpen] = useState(false)

  useEffect(() => {
    if (!open) return

    function handlePointerDown(event: PointerEvent) {
      const root = rootRef.current
      if (!root || root.contains(event.target as Node)) return
      setOpen(false)
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === 'Escape') setOpen(false)
    }

    document.addEventListener('pointerdown', handlePointerDown)
    document.addEventListener('keydown', handleKeyDown)
    return () => {
      document.removeEventListener('pointerdown', handlePointerDown)
      document.removeEventListener('keydown', handleKeyDown)
    }
  }, [open])

  return (
    <div className="reader-settings" ref={rootRef}>
      <ReaderSettingsButton
        open={open}
        onClick={() => setOpen((value) => !value)}
      />
      {open && (
        <div className="reader-settings-popover" role="dialog" aria-label="Reader settings">
          <label className="reader-setting-row">
            <span>Font</span>
            <select
              className="reader-setting-select"
              value={settings.fontFamily}
              onChange={(event) => onChange({ fontFamily: event.target.value })}
            >
              {FONT_FAMILY_OPTIONS.map((option) => (
                <option key={option.value} value={option.value}>{option.label}</option>
              ))}
            </select>
          </label>

          <ReaderRange
            label="Size"
            value={settings.fontSizePx}
            config={READER_SETTING_LIMITS.fontSizePx}
            onChange={(value) => onChange({ fontSizePx: value })}
          />
          <ReaderRange
            label="Line"
            value={settings.lineHeight}
            config={READER_SETTING_LIMITS.lineHeight}
            onChange={(value) => onChange({ lineHeight: value })}
          />
          <ReaderRange
            label="Width"
            value={settings.widthCh}
            config={READER_SETTING_LIMITS.widthCh}
            onChange={(value) => onChange({ widthCh: value })}
          />

          <button className="reader-settings-reset" type="button" onClick={onReset}>Reset</button>
        </div>
      )}
    </div>
  )
}

function ReaderSettingsButton({
  disabled = false,
  open,
  onClick,
}: {
  disabled?: boolean
  open: boolean
  onClick: () => void
}) {
  return (
    <button
      className="reader-settings-btn"
      aria-label="Reader settings"
      aria-expanded={!disabled && open}
      title="Reader settings"
      disabled={disabled}
      onClick={onClick}
      type="button"
    >
      <svg className="reader-settings-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
        <path d="M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7Z" />
        <path d="M19.4 15a1.7 1.7 0 0 0 .34 1.88l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.88-.34 1.7 1.7 0 0 0-1.03 1.56V21a2 2 0 1 1-4 0v-.08A1.7 1.7 0 0 0 9 19.36a1.7 1.7 0 0 0-1.88.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.7 1.7 0 0 0 4.63 15a1.7 1.7 0 0 0-1.56-1.03H3a2 2 0 1 1 0-4h.08A1.7 1.7 0 0 0 4.64 9a1.7 1.7 0 0 0-.34-1.88l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.7 1.7 0 0 0 9 4.63a1.7 1.7 0 0 0 1-1.56V3a2 2 0 1 1 4 0v.08A1.7 1.7 0 0 0 15 4.64a1.7 1.7 0 0 0 1.88-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.7 1.7 0 0 0 19.36 9a1.7 1.7 0 0 0 1.56 1H21a2 2 0 1 1 0 4h-.08A1.7 1.7 0 0 0 19.4 15Z" />
      </svg>
    </button>
  )
}

function ReaderRange({
  label,
  value,
  config,
  onChange,
}: {
  label: string
  value: number
  config: ReaderRangeConfig
  onChange: (value: number) => void
}) {
  const labelId = `reader-setting-${label.toLowerCase()}`

  return (
    <div className="reader-setting-row reader-setting-range" role="group" aria-labelledby={labelId}>
      <span id={labelId}>{label}</span>
      <button
        type="button"
        className="reader-setting-step"
        onClick={() => onChange(stepReaderValue(value, config, -1))}
        disabled={value <= config.min}
        aria-label={`Decrease ${label}`}
        title={`Decrease ${label}`}
      >
        &minus;
      </button>
      <input
        className="reader-setting-slider"
        type="range"
        min={config.min}
        max={config.max}
        step={config.step}
        value={value}
        aria-labelledby={labelId}
        onChange={(event) => onChange(Number(event.target.value))}
      />
      <button
        type="button"
        className="reader-setting-step"
        onClick={() => onChange(stepReaderValue(value, config, 1))}
        disabled={value >= config.max}
        aria-label={`Increase ${label}`}
        title={`Increase ${label}`}
      >
        +
      </button>
      <output>{formatReaderValue(value, config.suffix)}</output>
    </div>
  )
}

// Buttons use the same step as sliders, with decimal cleanup so line-height
// clicks do not accumulate floating point tails such as 0.8500000001.
function stepReaderValue(value: number, config: ReaderRangeConfig, direction: -1 | 1): number {
  const stepped = value + config.step * direction
  const clamped = clampReaderNumber(stepped, config.min, config.max, value)
  return Number.parseFloat(clamped.toFixed(decimalPlaces(config.step)))
}

function decimalPlaces(value: number): number {
  const [, fraction = ''] = String(value).split('.')
  return fraction.length
}

// Keep range outputs readable across integer and decimal controls without
// showing noisy values like 1.50 for line height.
function formatReaderValue(value: number, suffix: string): string {
  const rounded = Number.isInteger(value) ? String(value) : String(Number.parseFloat(value.toFixed(2)))
  return rounded + suffix
}
