import { useRef, useState, type Ref } from 'react'
import { useTranslation } from 'react-i18next'
import { Popover } from 'react-aria-components'
import {
  FONT_FAMILY_OPTIONS,
  READER_PAGE_THEMES,
  READER_SETTING_LIMITS,
  clampReaderNumber,
  type ReaderRangeConfig,
  type ReaderSettingsState,
} from './useReaderSettings'
import { AppSelect } from '../AppSelect/AppSelect'
import './ReaderSettings.css'

interface ReaderSettingsProps {
  applyingFontFamily: string | null
  disabled?: boolean
  settings: ReaderSettingsState
  onChange: (next: Partial<ReaderSettingsState>) => void
  onReset: () => void
}


export function ReaderSettings({
  applyingFontFamily,
  disabled = false,
  settings,
  onChange,
  onReset,
}: ReaderSettingsProps) {
  if (disabled) {
    return (
      <div className="reader-settings">
        <ReaderSettingsButton busy={false} disabled open={false} onClick={() => {}} />
      </div>
    )
  }

  return (
    <EnabledReaderSettings
      applyingFontFamily={applyingFontFamily}
      settings={settings}
      onChange={onChange}
      onReset={onReset}
    />
  )
}

function EnabledReaderSettings({
  applyingFontFamily,
  settings,
  onChange,
  onReset,
}: Omit<ReaderSettingsProps, 'disabled'>) {
  const { t } = useTranslation()
  const buttonRef = useRef<HTMLButtonElement | null>(null)
  const [open, setOpen] = useState(false)
  const applyingFontOption = FONT_FAMILY_OPTIONS.find((option) => option.value === applyingFontFamily)
  const applyingFontLabel = applyingFontOption ? t(applyingFontOption.labelKey) : applyingFontFamily

  return (
    <div className="reader-settings">
      <ReaderSettingsButton
        buttonRef={buttonRef}
        busy={Boolean(applyingFontFamily)}
        open={open}
        onClick={() => setOpen((value) => !value)}
      />
      <Popover
        className="reader-settings-popover"
        isOpen={open}
        onOpenChange={setOpen}
        triggerRef={buttonRef}
        placement="bottom end"
        offset={8}
        containerPadding={8}
        shouldFlip
        aria-label={t('reader.settings.dialogLabel')}
      >
        <div className="reader-setting-row">
          <span id="reader-setting-font">{t('reader.settings.font')}</span>
          <AppSelect
            className="reader-setting-font-select"
            value={settings.fontFamily}
            options={FONT_FAMILY_OPTIONS.map((option) => ({
              value: option.value,
              label: t(option.labelKey),
            }))}
            ariaLabelledBy="reader-setting-font"
            disabled={Boolean(applyingFontFamily)}
            onChange={(fontFamily) => onChange({ fontFamily })}
          />
        </div>

        {applyingFontFamily && (
          <div className="reader-setting-font-status" role="status" aria-live="polite" aria-atomic="true">
            <span className="spinner" aria-hidden="true" />
            <span>{t('reader.settings.applyingFont', {
              font: `\u2068${applyingFontLabel}\u2069`,
            })}</span>
          </div>
        )}

        <fieldset className="reader-setting-page-theme">
          <legend>{t('reader.settings.pageColor')}</legend>
          <div className="reader-page-theme-options">
            {READER_PAGE_THEMES.map((pageTheme) => (
              <label key={pageTheme} className="reader-page-theme-option">
                <input
                  type="radio"
                  name="reader-page-theme"
                  value={pageTheme}
                  checked={settings.pageTheme === pageTheme}
                  onChange={() => onChange({ pageTheme })}
                />
                <span className={`reader-page-theme-swatch reader-page-theme-swatch-${pageTheme}`} aria-hidden="true" />
                <span>{t(`reader.settings.pageThemes.${pageTheme}`)}</span>
              </label>
            ))}
          </div>
        </fieldset>

        <ReaderRange
          id="size"
          label={t('reader.settings.size')}
          value={settings.fontSizePx}
          config={READER_SETTING_LIMITS.fontSizePx}
          onChange={(value) => onChange({ fontSizePx: value })}
        />
        <ReaderRange
          id="line-spacing"
          label={t('reader.settings.lineSpacing')}
          value={settings.lineHeight}
          config={READER_SETTING_LIMITS.lineHeight}
          onChange={(value) => onChange({ lineHeight: value })}
        />
        <ReaderRange
          id="width"
          label={t('reader.settings.width')}
          value={settings.widthCh}
          config={READER_SETTING_LIMITS.widthCh}
          onChange={(value) => onChange({ widthCh: value })}
        />

        <button className="reader-settings-reset" type="button" onClick={onReset}>{t('reader.settings.reset')}</button>
      </Popover>
    </div>
  )
}

function ReaderSettingsButton({
  buttonRef,
  busy,
  disabled = false,
  open,
  onClick,
}: {
  buttonRef?: Ref<HTMLButtonElement>
  busy: boolean
  disabled?: boolean
  open: boolean
  onClick: () => void
}) {
  const { t } = useTranslation()
  return (
    <button
      ref={buttonRef}
      className="reader-settings-btn"
      aria-label={t('reader.settings.button')}
      aria-expanded={!disabled && open}
      aria-busy={busy || undefined}
      title={t('reader.settings.button')}
      disabled={disabled}
      onClick={onClick}
      type="button"
    >
      {busy
        ? <span className="spinner reader-settings-spinner" aria-hidden="true" />
        : <span className="reader-settings-type-icon" aria-hidden="true">Aa</span>}
    </button>
  )
}

function ReaderRange({
  id,
  label,
  value,
  config,
  onChange,
}: {
  id: string
  label: string
  value: number
  config: ReaderRangeConfig
  onChange: (value: number) => void
}) {
  const { t } = useTranslation()
  const labelId = `reader-setting-${id}`

  return (
    <div className="reader-setting-row reader-setting-range" role="group" aria-labelledby={labelId}>
      <span id={labelId}>{label}</span>
      <button
        type="button"
        className="reader-setting-step"
        onClick={() => onChange(stepReaderValue(value, config, -1))}
        disabled={value <= config.min}
        aria-label={t('reader.settings.decrease', { setting: label })}
        title={t('reader.settings.decrease', { setting: label })}
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
        aria-label={t('reader.settings.increase', { setting: label })}
        title={t('reader.settings.increase', { setting: label })}
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
