import {
  useCallback,
  useEffect,
  useMemo,
  useState,
  useSyncExternalStore,
} from 'react'

const THEME_STORAGE_KEY = 'papercut.theme.v1'
const DARK_QUERY = '(prefers-color-scheme: dark)'

export type ThemeChoice = 'light' | 'system' | 'dark'
export type ResolvedTheme = 'light' | 'dark'

export interface ThemeState {
  choice: ThemeChoice
  resolved: ResolvedTheme
  setChoice: (choice: ThemeChoice) => void
}

export function useTheme(): ThemeState {
  const [choice, setChoiceState] = useState<ThemeChoice>(() => loadThemeChoice())
  const systemDark = useSyncExternalStore(
    subscribeSystemTheme,
    getSystemDark,
    () => false,
  )

  const resolved = choice === 'system'
    ? (systemDark ? 'dark' : 'light')
    : choice

  useEffect(() => {
    if (typeof document === 'undefined') {
      return
    }
    document.documentElement.dataset.theme = resolved
    document.documentElement.dataset.themeChoice = choice
  }, [choice, resolved])

  const setChoice = useCallback((next: ThemeChoice) => {
    setChoiceState(next)
    saveThemeChoice(next)
  }, [])

  return useMemo(() => ({
    choice,
    resolved,
    setChoice,
  }), [choice, resolved, setChoice])
}

function loadThemeChoice(): ThemeChoice {
  if (typeof window === 'undefined') {
    return 'system'
  }
  try {
    return parseThemeChoice(window.localStorage.getItem(THEME_STORAGE_KEY))
  } catch {
    return 'system'
  }
}

function saveThemeChoice(choice: ThemeChoice): void {
  if (typeof window === 'undefined') {
    return
  }
  try {
    window.localStorage.setItem(THEME_STORAGE_KEY, choice)
  } catch {
    // Non-critical preference persistence can fail in restricted previews.
  }
}

function parseThemeChoice(value: string | null): ThemeChoice {
  return value === 'light' || value === 'dark' || value === 'system' ? value : 'system'
}

function getSystemDark(): boolean {
  return typeof window !== 'undefined' && window.matchMedia(DARK_QUERY).matches
}

function subscribeSystemTheme(onChange: () => void): () => void {
  if (typeof window === 'undefined') {
    return () => {}
  }
  const media = window.matchMedia(DARK_QUERY)
  media.addEventListener('change', onChange)
  return () => media.removeEventListener('change', onChange)
}
