import { currentAppLocale } from '../i18n'

export function formatStorageSize(bytes: number | undefined): string | null {
  if (!bytes || !Number.isFinite(bytes) || bytes <= 0) return null
  const locale = currentAppLocale()
  if (bytes >= 1024 * 1024 * 1024) {
    const gb = bytes / 1024 / 1024 / 1024
    const digits = gb >= 10 ? 1 : 2
    return new Intl.NumberFormat(locale, {
      minimumFractionDigits: digits,
      maximumFractionDigits: digits,
    }).format(gb) + ' GB'
  }
  return new Intl.NumberFormat(locale).format(Math.max(1, Math.ceil(bytes / 1024 / 1024))) + ' MB'
}
