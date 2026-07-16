import { useCallback, useEffect, useState } from 'react'
import { getVersion } from '@tauri-apps/api/app'
import { isTauri } from '@tauri-apps/api/core'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import type { ThemeChoice } from '../../hooks/useTheme'
import { AppDialog } from '../AppDialog/AppDialog'
import './AppSettings.css'

const ZOOM_STORAGE_KEY = 'papercut.zoom.v1'
const DEFAULT_ZOOM = 100
const MIN_ZOOM = 70
const MAX_ZOOM = 200
const ZOOM_STEP = 10

const THEME_OPTIONS: Array<{ choice: ThemeChoice; label: string }> = [
  { choice: 'system', label: 'System' },
  { choice: 'light', label: 'Light' },
  { choice: 'dark', label: 'Dark' },
]

interface AppSettingsProps {
  themeChoice: ThemeChoice
  onThemeChange: (choice: ThemeChoice) => void
}

export function AppSettings({ themeChoice, onThemeChange }: AppSettingsProps) {
  const [open, setOpen] = useState(false)
  const [version, setVersion] = useState<string | null>(() => isTauri() ? null : 'Web build')
  const zoom = useAppZoom()
  const closeSettings = useCallback(() => setOpen(false), [])

  useEffect(() => {
    if (!open || version !== null) return
    void getVersion()
      .then(setVersion)
      .catch(() => setVersion('Unavailable'))
  }, [open, version])

  return (
    <div className="app-settings">
      <button
        type="button"
        className="app-settings-btn"
        aria-label="App settings"
        aria-haspopup="dialog"
        title="App settings"
        onClick={() => setOpen(true)}
      >
        <SettingsIcon />
      </button>

      {open && (
        <AppDialog
          title="App Settings"
          onCancel={closeSettings}
          actions={(
            <button type="button" className="app-dialog-submit" onClick={closeSettings}>
              Done
            </button>
          )}
        >
          <section className="app-settings-section" aria-labelledby="app-settings-appearance">
            <h3 id="app-settings-appearance">Appearance</h3>

            <div className="app-setting">
              <span id="app-setting-theme">Theme</span>
              <div className="app-theme-options" role="group" aria-labelledby="app-setting-theme">
                {THEME_OPTIONS.map((option) => (
                  <button
                    key={option.choice}
                    type="button"
                    className={themeChoice === option.choice ? 'app-theme-option active' : 'app-theme-option'}
                    aria-pressed={themeChoice === option.choice}
                    onClick={() => onThemeChange(option.choice)}
                  >
                    {option.label}
                  </button>
                ))}
              </div>
            </div>

            {zoom.supported && (
              <div className="app-setting">
                <span id="app-setting-zoom">Zoom</span>
                <div className="app-zoom-control" role="group" aria-labelledby="app-setting-zoom">
                  <button
                    type="button"
                    aria-label="Decrease app zoom"
                    title="Decrease app zoom"
                    disabled={zoom.value <= MIN_ZOOM}
                    onClick={() => zoom.setValue(zoom.value - ZOOM_STEP)}
                  >
                    &minus;
                  </button>
                  <output aria-live="polite">{zoom.value}%</output>
                  <button
                    type="button"
                    aria-label="Increase app zoom"
                    title="Increase app zoom"
                    disabled={zoom.value >= MAX_ZOOM}
                    onClick={() => zoom.setValue(zoom.value + ZOOM_STEP)}
                  >
                    +
                  </button>
                </div>
                {zoom.value !== DEFAULT_ZOOM && (
                  <button className="app-zoom-reset" type="button" onClick={() => zoom.setValue(DEFAULT_ZOOM)}>
                    Reset
                  </button>
                )}
              </div>
            )}
          </section>

          <div className="app-settings-version">
            <span>Version</span>
            <strong>{version ?? 'Loading...'}</strong>
          </div>
        </AppDialog>
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

function useAppZoom() {
  const supported = isTauri() && !isMobilePlatform()
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

// Tauri's WebView zoom API is desktop-only; mobile keeps theme and version
// settings while reader typography remains under Reader Settings.
function isMobilePlatform(): boolean {
  return /Android|iP(ad|hone|od)/.test(navigator.userAgent) ||
    (navigator.platform === 'MacIntel' && navigator.maxTouchPoints > 1)
}
