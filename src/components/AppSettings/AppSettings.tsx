import { Fragment, useCallback, useEffect, useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { getVersion } from '@tauri-apps/api/app'
import { isTauri } from '@tauri-apps/api/core'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import type { ThemeChoice } from '../../hooks/useTheme'
import {
  APP_LOCALE_OPTIONS,
  changeAppLocale,
  currentAppLocale,
} from '../../i18n'
import { LibraryTransferDialog } from '../../library-transfer/LibraryTransferDialog'
import { openExternalUrl } from '../../utils/openExternalUrl'
import { isMobileUserAgent } from '../../utils/platform'
import { AppDialog } from '../AppDialog/AppDialog'
import { AppSelect } from '../AppSelect/AppSelect'
import aiAudioUseNotice from '../../../AI_AUDIO_USE_NOTICE.md?raw'
import papercutLicense from '../../../LICENSE.md?raw'
import thirdPartyNotices from '../../../THIRD_PARTY_NOTICES.md?raw'
import './AppSettings.css'

const ZOOM_STORAGE_KEY = 'papercut.zoom.v1'
const DEFAULT_ZOOM = 100
const MIN_ZOOM = 70
const MAX_ZOOM = 200
const ZOOM_STEP = 10
const NEXT_THEME: Record<ThemeChoice, ThemeChoice> = {
  system: 'light',
  light: 'dark',
  dark: 'system',
}

type LegalNoticeId = 'ai-audio' | 'papercut' | 'third-party'

const LEGAL_NOTICE_CONTENT: Record<LegalNoticeId, string> = {
  'ai-audio': aiAudioUseNotice,
  papercut: papercutLicense,
  'third-party': thirdPartyNotices,
}

interface AppSettingsProps {
  themeChoice: ThemeChoice
  onThemeChange: (choice: ThemeChoice) => void
  developerMode: boolean
  onDeveloperModeChange: (enabled: boolean) => void
  libraryDocumentCount: number
  onLibraryImported: () => void | Promise<void>
}

export function AppSettings({
  themeChoice,
  onThemeChange,
  developerMode,
  onDeveloperModeChange,
  libraryDocumentCount,
  onLibraryImported,
}: AppSettingsProps) {
  const { t } = useTranslation()
  const tauriRuntime = isTauri()
  const [open, setOpen] = useState(false)
  const [legalNoticesOpen, setLegalNoticesOpen] = useState(false)
  const [selectedLegalNotice, setSelectedLegalNotice] = useState<LegalNoticeId | null>(null)
  const [transferOpen, setTransferOpen] = useState(false)
  const [version, setVersion] = useState<string | null>(() => tauriRuntime ? null : '')
  const zoom = useAppZoom()
  const closeSettings = useCallback(() => {
    setOpen(false)
    setLegalNoticesOpen(false)
    setSelectedLegalNotice(null)
  }, [])
  const themeOption = {
    system: t('settings.themeSystem'),
    light: t('settings.themeLight'),
    dark: t('settings.themeDark'),
  }[themeChoice]

  useEffect(() => {
    if (!tauriRuntime || !open || version !== null) return
    void getVersion()
      .then(setVersion)
      .catch(() => setVersion(''))
  }, [open, tauriRuntime, version])

  const versionLabel = version === null
    ? t('settings.versionLoading')
    : version || (tauriRuntime ? t('settings.versionUnavailable') : t('settings.webBuild'))
  const legalNotices = [
    { id: 'ai-audio' as const, title: t('settings.aiAudioUseNotice') },
    { id: 'papercut' as const, title: t('settings.papercutLicense') },
    { id: 'third-party' as const, title: t('settings.thirdPartyNotices') },
  ]
  const selectedNotice = legalNotices.find(({ id }) => id === selectedLegalNotice)

  return (
    <div className="app-settings">
      <button
        type="button"
        className="app-settings-btn"
        aria-label={t('settings.button')}
        aria-haspopup="dialog"
        title={t('settings.button')}
        onClick={() => setOpen(true)}
      >
        <SettingsIcon />
      </button>

      {open && legalNoticesOpen && (
        <AppDialog
          key={selectedLegalNotice ?? 'legal-notices'}
          className="app-settings-legal-dialog"
          title={selectedNotice?.title ?? t('settings.legalAndNotices')}
          onCancel={closeSettings}
          actions={(
            <button
              type="button"
              className="app-dialog-cancel"
              onClick={() => {
                if (selectedLegalNotice) setSelectedLegalNotice(null)
                else setLegalNoticesOpen(false)
              }}
            >
              {t('common.back')}
            </button>
          )}
        >
          {selectedLegalNotice ? (
            <LegalNoticeContent source={LEGAL_NOTICE_CONTENT[selectedLegalNotice]} />
          ) : (
            <nav className="app-settings-legal-notices" aria-label={t('settings.legalAndNotices')}>
              {legalNotices.map((notice) => (
                <button
                  key={notice.id}
                  type="button"
                  className="app-settings-legal-notice-link"
                  onClick={() => setSelectedLegalNotice(notice.id)}
                >
                  <span>{notice.title}</span>
                  <span className="app-settings-legal-chevron" aria-hidden="true" />
                </button>
              ))}
            </nav>
          )}
        </AppDialog>
      )}

      {open && !legalNoticesOpen && (
        <AppDialog
          title={t('settings.title')}
          onCancel={closeSettings}
          actions={(
            <button type="button" className="app-dialog-submit" onClick={closeSettings}>
              {t('common.done')}
            </button>
          )}
        >
          <section className="app-settings-section" aria-labelledby="app-settings-appearance">
            <h3 id="app-settings-appearance">{t('settings.appearance')}</h3>

            <div className="app-setting">
              <span id="app-setting-language">{t('settings.language')}</span>
              <AppSelect
                className="app-setting-select"
                value={currentAppLocale()}
                options={APP_LOCALE_OPTIONS.map((option) => ({
                  value: option.value,
                  label: option.experimental
                    ? `${option.label} (${t('settings.experimental')})`
                    : option.label,
                }))}
                ariaLabelledBy="app-setting-language"
                onChange={(locale) => { void changeAppLocale(locale) }}
              />
            </div>

            <div className="app-setting">
              <span id="app-setting-theme">{t('settings.theme')}</span>
              <button
                type="button"
                className="app-theme-cycle"
                aria-labelledby="app-setting-theme app-setting-theme-value"
                onClick={() => onThemeChange(NEXT_THEME[themeChoice])}
              >
                <ThemeIcon choice={themeChoice} />
                <span id="app-setting-theme-value">{themeOption}</span>
              </button>
            </div>

            {zoom.supported && (
              <div className="app-setting">
                <span id="app-setting-zoom">{t('settings.zoom')}</span>
                <div className="app-zoom-control" role="group" aria-labelledby="app-setting-zoom">
                  <button
                    type="button"
                    aria-label={t('settings.zoomDecrease')}
                    title={t('settings.zoomDecrease')}
                    disabled={zoom.value <= MIN_ZOOM}
                    onClick={() => zoom.setValue(zoom.value - ZOOM_STEP)}
                  >
                    &minus;
                  </button>
                  <output aria-live="polite">{zoom.value}%</output>
                  <button
                    type="button"
                    aria-label={t('settings.zoomIncrease')}
                    title={t('settings.zoomIncrease')}
                    disabled={zoom.value >= MAX_ZOOM}
                    onClick={() => zoom.setValue(zoom.value + ZOOM_STEP)}
                  >
                    +
                  </button>
                </div>
                {zoom.value !== DEFAULT_ZOOM && (
                  <button className="app-zoom-reset" type="button" onClick={() => zoom.setValue(DEFAULT_ZOOM)}>
                    {t('settings.zoomReset')}
                  </button>
                )}
              </div>
            )}
          </section>

          {tauriRuntime && (
            <section className="app-settings-section app-settings-data" aria-labelledby="app-settings-data">
              <h3 id="app-settings-data">{t('settings.data')}</h3>
              <button
                type="button"
                className="app-settings-data-action"
                onClick={() => {
                  setOpen(false)
                  setTransferOpen(true)
                }}
              >
                <TransferIcon />
                {t('settings.transferLibrary')}
              </button>
            </section>
          )}

          <section className="app-settings-section app-settings-developer" aria-labelledby="app-settings-developer">
            <h3 id="app-settings-developer">{t('settings.developer')}</h3>
            <label className="app-setting app-setting-toggle">
              <span>{t('settings.developerMode')}</span>
              <span className="app-settings-switch">
                <input
                  type="checkbox"
                  checked={developerMode}
                  onChange={(event) => onDeveloperModeChange(event.target.checked)}
                />
                <span aria-hidden="true" />
              </span>
            </label>
          </section>

          <section className="app-settings-section app-settings-about" aria-labelledby="app-settings-about">
            <h3 id="app-settings-about">{t('settings.about')}</h3>
            <button
              type="button"
              className="app-settings-data-action"
              onClick={() => setLegalNoticesOpen(true)}
            >
              {t('settings.legalAndNotices')}
            </button>
          </section>

          <div className="app-settings-version">
            <span>{t('settings.version')}</span>
            <strong>{versionLabel}</strong>
          </div>
        </AppDialog>
      )}

      {transferOpen && (
        <LibraryTransferDialog
          documentCount={libraryDocumentCount}
          onBack={() => {
            setTransferOpen(false)
            setOpen(true)
          }}
          onImported={onLibraryImported}
        />
      )}
    </div>
  )
}

function SettingsIcon() {
  return (
    <svg className="app-settings-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7Z" />
      <path d="M19.4 15a1.7 1.7 0 0 0 .34 1.88l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.88-.34 1.7 1.7 0 0 0-1.03 1.56V21a2 2 0 1 1-4 0v-.08A1.7 1.7 0 0 0 9 19.36a1.7 1.7 0 0 0-1.88.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.7 1.7 0 0 0 4.63 15a1.7 1.7 0 0 0-1.56-1.03H3a2 2 0 1 1 0-4h.08A1.7 1.7 0 0 0 4.64 9a1.7 1.7 0 0 0-.34-1.88l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.7 1.7 0 0 0 9 4.63a1.7 1.7 0 0 0 1-1.56V3a2 2 0 1 1 4 0v.08A1.7 1.7 0 0 0 15 4.64a1.7 1.7 0 0 0 1.88-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.7 1.7 0 0 0 19.36 9a1.7 1.7 0 0 0 1.56 1H21a2 2 0 1 1 0 4h-.08A1.7 1.7 0 0 0 19.4 15Z" />
    </svg>
  )
}

function ThemeIcon({ choice }: { choice: ThemeChoice }) {
  if (choice === 'system') {
    return (
      <svg className="app-theme-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
        <rect x="3" y="4" width="18" height="14" rx="2" />
        <path d="M8 22h8M12 18v4" />
      </svg>
    )
  }
  if (choice === 'light') {
    return (
      <svg className="app-theme-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
        <circle cx="12" cy="12" r="4" />
        <path d="M12 2v2M12 20v2M4.93 4.93l1.42 1.42M17.66 17.66l1.41 1.41M2 12h2M20 12h2M4.93 19.07l1.42-1.42M17.66 6.34l1.41-1.41" />
      </svg>
    )
  }
  return (
    <svg className="app-theme-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M21 12.8A9 9 0 1 1 11.2 3 7 7 0 0 0 21 12.8Z" />
    </svg>
  )
}

function TransferIcon() {
  return (
    <svg className="app-settings-data-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M7 7h13m0 0-3-3m3 3-3 3M17 17H4m0 0 3 3m-3-3 3-3" />
    </svg>
  )
}

function LegalNoticeContent({ source }: { source: string }) {
  const blocks = source.trim().split(/\n\s*\n/).slice(1)

  return (
    <div className="app-settings-legal-content" dir="ltr">
      {blocks.map((block, index) => {
        const heading = block.match(/^##\s+(.+)$/)
        if (heading) return <h3 key={index}>{renderLegalInline(heading[1])}</h3>

        return <p key={index}>{renderLegalInline(block.replace(/\s*\n\s*/g, ' '))}</p>
      })}
    </div>
  )
}

function renderLegalInline(text: string): ReactNode[] {
  return text
    .split(/(\*\*[^*]+\*\*|`[^`]+`|_[^_]+_|https?:\/\/\S+)/g)
    .filter(Boolean)
    .map((part, index) => {
      if (part.startsWith('**') && part.endsWith('**')) {
        return <strong key={index}>{part.slice(2, -2)}</strong>
      }
      if (part.startsWith('`') && part.endsWith('`')) {
        return <code key={index}>{part.slice(1, -1)}</code>
      }
      if (part.startsWith('_') && part.endsWith('_')) {
        return <em key={index}>{part.slice(1, -1)}</em>
      }
      if (part.startsWith('http://') || part.startsWith('https://')) {
        return (
          <button
            key={index}
            type="button"
            className="app-settings-legal-link"
            onClick={() => { void openExternalUrl(part) }}
          >
            {part}
          </button>
        )
      }
      return <Fragment key={index}>{part}</Fragment>
    })
}

function useAppZoom() {
  const supported = isTauri() && !isMobileUserAgent()
  const [value, setValueState] = useState(() => loadZoom())
  const setZoom = useCallback((next: number) => {
    const clamped = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, next))
    setValueState(clamped)
    saveZoom(clamped)
  }, [])

  // Tauri applies WebView zoom outside the DOM, so restore it explicitly on
  // startup and after every preference change.
  useEffect(() => {
    if (!supported) return
    void getCurrentWebview().setZoom(value / 100).catch(() => {})
  }, [supported, value])

  useEffect(() => {
    if (!supported) return

    function handleKeyDown(event: KeyboardEvent) {
      if (!event.ctrlKey && !event.metaKey) return
      if (event.key === '0') {
        event.preventDefault()
        setZoom(DEFAULT_ZOOM)
      } else if (event.key === '+' || event.key === '=') {
        event.preventDefault()
        setZoom(value + ZOOM_STEP)
      } else if (event.key === '-') {
        event.preventDefault()
        setZoom(value - ZOOM_STEP)
      }
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [setZoom, supported, value])

  return { supported, value, setValue: setZoom }
}

function loadZoom(): number {
  try {
    const value = Number(window.localStorage.getItem(ZOOM_STORAGE_KEY))
    return Number.isFinite(value) && value >= MIN_ZOOM && value <= MAX_ZOOM
      ? value
      : DEFAULT_ZOOM
  } catch {
    return DEFAULT_ZOOM
  }
}

function saveZoom(value: number): void {
  try {
    window.localStorage.setItem(ZOOM_STORAGE_KEY, String(value))
  } catch {
    // Zoom still works when preference persistence is unavailable.
  }
}
