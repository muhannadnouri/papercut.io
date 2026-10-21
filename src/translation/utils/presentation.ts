import type { TFunction } from 'i18next'

// Language names beat raw ISO codes for recognition; fall back to the code
// when the runtime cannot resolve a display name.
export function formatLanguageLabel(language: string, locale: string, t: TFunction): string {
  if (language === 'auto') return t('translation.languages.autoDetect')
  try {
    const name = new Intl.DisplayNames([locale], { type: 'language' }).of(language)
    if (name && name !== language) return name
  } catch {
    // Unknown/invalid code: fall through to the uppercase code.
  }
  return language.toUpperCase()
}

// Saved rows use ready; job results use stored. Both share the localized completion label.
export function formatStatusLabel(status: string, t: TFunction): string {
  const labels: Record<string, string> = {
    'loading-model': t('translation.progress.status.loadingModel'),
    starting: t('translation.progress.status.starting'),
    translating: t('translation.progress.status.translating'),
    validating: t('translation.progress.status.validating'),
    storing: t('translation.progress.status.storing'),
    ready: t('translation.progress.status.completed'),
    stored: t('translation.progress.status.completed'),
    completed: t('translation.progress.status.completed'),
    'needs-review': t('translation.status.translationNeedsReview'),
    cancelled: t('translation.progress.status.cancelled'),
  }
  return labels[status] ?? status.charAt(0).toUpperCase() + status.slice(1)
}
