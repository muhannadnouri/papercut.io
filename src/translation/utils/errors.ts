import i18n from '../../i18n'
import { TranslationError } from '../api/nativeTranslation'

const ERROR_KEYS = {
  'translation-unavailable': 'translation.errors.unavailable',
  'model-not-installed': 'translation.errors.modelNotInstalled',
  'unsupported-translation-option': 'translation.errors.unsupportedOption',
  'operation-in-progress': 'translation.errors.operationInProgress',
  'operation-cancelled': 'translation.errors.operationCancelled',
  'source-not-found': 'translation.errors.sourceNotFound',
  'no-translatable-text': 'translation.errors.noTranslatableText',
  'quality-check-failed': 'translation.errors.qualityCheckFailed',
  'translated-document-not-found': 'translation.errors.documentNotFound',
} as const

export function translationErrorMessage(error: unknown): string {
  if (error instanceof TranslationError) {
    const key = ERROR_KEYS[error.code as keyof typeof ERROR_KEYS]
    if (key) return i18n.t(key)
  }
  return translationErrorDetail(error)
}

export function translationErrorDetail(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

export function translationLibraryRefreshError(error: unknown): string {
  return i18n.t('translation.errors.libraryRefresh', {
    detail: translationErrorDetail(error),
  })
}
