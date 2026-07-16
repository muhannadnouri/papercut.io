import i18n from 'i18next'
import { initReactI18next } from 'react-i18next'
import en from './locales/en.json'
import ar from './locales/ar.json'

const LOCALE_STORAGE_KEY = 'papercut.locale.v1'

export const APP_LOCALE_OPTIONS = [
  { value: 'en', label: 'English' },
  { value: 'ar', label: 'العربية' },
] as const

export type AppLocale = typeof APP_LOCALE_OPTIONS[number]['value']

const initialLocale = loadInitialLocale()

void i18n
  .use(initReactI18next)
  .init({
    resources: {
      en: { translation: en },
      ar: { translation: ar },
    },
    lng: initialLocale,
    fallbackLng: 'en',
    supportedLngs: APP_LOCALE_OPTIONS.map((option) => option.value),
    interpolation: { escapeValue: false },
    initAsync: false,
  })

applyDocumentLocale(initialLocale)

export function resolveAppLocale(value: string | null | undefined): AppLocale {
  const base = value?.toLowerCase().split('-')[0]
  return base === 'ar' ? 'ar' : 'en'
}

export function currentAppLocale(): AppLocale {
  return resolveAppLocale(i18n.resolvedLanguage ?? i18n.language)
}

export async function changeAppLocale(value: string): Promise<void> {
  const locale = resolveAppLocale(value)
  saveLocale(locale)
  applyDocumentLocale(locale)
  await i18n.changeLanguage(locale)
}

export function applyDocumentLocale(locale: AppLocale): void {
  document.documentElement.lang = locale
  document.documentElement.dir = locale === 'ar' ? 'rtl' : 'ltr'
}

// Prefer an explicit app choice, then the first supported browser language.
// This intentionally avoids a detector dependency for two bundled locales.
function loadInitialLocale(): AppLocale {
  try {
    const stored = window.localStorage.getItem(LOCALE_STORAGE_KEY)
    if (stored) return resolveAppLocale(stored)
  } catch {
    // Browser language fallback remains available when storage is restricted.
  }

  for (const language of navigator.languages ?? [navigator.language]) {
    const base = language.toLowerCase().split('-')[0]
    if (base === 'ar' || base === 'en') return base
  }
  return 'en'
}

function saveLocale(locale: AppLocale): void {
  try {
    window.localStorage.setItem(LOCALE_STORAGE_KEY, locale)
  } catch {
    // The selected locale still applies for this session.
  }
}

export default i18n
