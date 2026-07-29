import i18n from 'i18next'
import { initReactI18next } from 'react-i18next'
import en from './locales/en.json'
import ar from './locales/ar.json'
import es from './locales/es.json'
import fr from './locales/fr.json'
import hi from './locales/hi.json'
import it from './locales/it.json'
import ptBR from './locales/pt-BR.json'
import zhCN from './locales/zh-CN.json'

const LOCALE_STORAGE_KEY = 'papercut.locale.v1'

export const APP_LOCALE_OPTIONS = [
  { value: 'en', label: 'English', experimental: false },
  { value: 'ar', label: 'العربية', experimental: false },
  { value: 'zh-CN', label: '中文（简体）', experimental: true },
  { value: 'fr', label: 'Français', experimental: true },
  { value: 'hi', label: 'हिन्दी', experimental: true },
  { value: 'it', label: 'Italiano', experimental: true },
  { value: 'pt-BR', label: 'Português (Brasil)', experimental: true },
  { value: 'es', label: 'Español', experimental: true },
] as const

export type AppLocale = typeof APP_LOCALE_OPTIONS[number]['value']

const resources = {
  en: { translation: en },
  ar: { translation: ar },
  es: { translation: es },
  fr: { translation: fr },
  hi: { translation: hi },
  it: { translation: it },
  'pt-BR': { translation: ptBR },
  'zh-CN': { translation: zhCN },
}

const initialLocale = loadInitialLocale()

void i18n
  .use(initReactI18next)
  .init({
    resources,
    lng: initialLocale,
    fallbackLng: 'en',
    supportedLngs: APP_LOCALE_OPTIONS.map((option) => option.value),
    interpolation: { escapeValue: false },
    initAsync: false,
  })

applyDocumentLocale(initialLocale)

export function resolveAppLocale(value: string | null | undefined): AppLocale {
  const normalized = value?.toLowerCase().replace('_', '-')
  if (normalized === 'pt' || normalized?.startsWith('pt-br')) return 'pt-BR'
  if (
    normalized === 'zh'
    || normalized?.startsWith('zh-cn')
    || normalized?.startsWith('zh-hans')
    || normalized?.startsWith('zh-sg')
  ) return 'zh-CN'
  const base = normalized?.split('-')[0]
  const locale = APP_LOCALE_OPTIONS.find((option) => option.value.toLowerCase() === base)
  return locale?.value ?? 'en'
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
// This intentionally avoids a detector dependency for a small bundled list.
function loadInitialLocale(): AppLocale {
  try {
    const stored = window.localStorage.getItem(LOCALE_STORAGE_KEY)
    if (stored) return resolveAppLocale(stored)
  } catch {
    // Browser language fallback remains available when storage is restricted.
  }

  for (const language of navigator.languages ?? [navigator.language]) {
    const locale = resolveAppLocale(language)
    if (locale !== 'en' || language.toLowerCase().startsWith('en')) return locale
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
