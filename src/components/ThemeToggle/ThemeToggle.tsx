import type { ReactNode } from 'react'
import type { ThemeChoice } from '../../hooks/useTheme'
import './ThemeToggle.css'

const THEME_OPTIONS: Array<{ choice: ThemeChoice; icon: ReactNode; label: string }> = [
  { choice: 'system', icon: <SystemIcon />, label: 'Use system theme' },
  { choice: 'light', icon: <SunIcon />, label: 'Light theme' },
  { choice: 'dark', icon: <MoonIcon />, label: 'Dark theme' },
]

interface ThemeToggleProps {
  choice: ThemeChoice
  onChange: (choice: ThemeChoice) => void
}

export function ThemeToggle({ choice, onChange }: ThemeToggleProps) {
  const currentIndex = THEME_OPTIONS.findIndex((option) => option.choice === choice)
  const current = THEME_OPTIONS[currentIndex] ?? THEME_OPTIONS[0]
  const next = THEME_OPTIONS[(currentIndex + 1) % THEME_OPTIONS.length] ?? THEME_OPTIONS[0]

  return (
    <button
      type="button"
      className={`theme-toggle-btn theme-toggle-btn-${current.choice}`}
      aria-label={`Theme: ${current.label}. Switch to ${next.label}.`}
      title={`Theme: ${current.label}. Click for ${next.label}.`}
      onClick={() => onChange(next.choice)}
    >
      <span className="theme-toggle-icon" aria-hidden="true">{current.icon}</span>
    </button>
  )
}

function SystemIcon() {
  return (
    <svg viewBox="0 0 24 24" focusable="false">
      <path d="M4 5.75A2.75 2.75 0 0 1 6.75 3h10.5A2.75 2.75 0 0 1 20 5.75v7.5A2.75 2.75 0 0 1 17.25 16H6.75A2.75 2.75 0 0 1 4 13.25v-7.5Z" />
      <path d="M9.25 20h5.5" />
      <path d="M12 16v4" />
    </svg>
  )
}

function SunIcon() {
  return (
    <svg viewBox="0 0 24 24" focusable="false">
      <circle cx="12" cy="12" r="4" />
      <path d="M12 2.75v2.5" />
      <path d="M12 18.75v2.5" />
      <path d="m5.46 5.46 1.77 1.77" />
      <path d="m16.77 16.77 1.77 1.77" />
      <path d="M2.75 12h2.5" />
      <path d="M18.75 12h2.5" />
      <path d="m5.46 18.54 1.77-1.77" />
      <path d="m16.77 7.23 1.77-1.77" />
    </svg>
  )
}

function MoonIcon() {
  return (
    <svg viewBox="0 0 24 24" focusable="false">
      <path d="M19.15 15.4A7.45 7.45 0 0 1 8.6 4.85 8.3 8.3 0 1 0 19.15 15.4Z" />
    </svg>
  )
}
