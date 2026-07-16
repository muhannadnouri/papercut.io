import { I18nProvider } from 'react-aria-components'
import { useTranslation } from 'react-i18next'
import { useEffect, type ReactNode } from 'react'
import { applyDocumentLocale, resolveAppLocale } from '.'

export function LocaleProvider({ children }: { children: ReactNode }) {
  const { i18n } = useTranslation()
  const locale = resolveAppLocale(i18n.resolvedLanguage ?? i18n.language)

  useEffect(() => {
    applyDocumentLocale(locale)
  }, [locale])

  return <I18nProvider locale={locale}>{children}</I18nProvider>
}
