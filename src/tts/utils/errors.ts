import i18n from '../../i18n'
import { NativeTtsError } from '../api/nativeTts'

const ERROR_KEYS = {
  'native-tts-unavailable': 'tts.errors.nativeUnavailable',
  'unsupported-model': 'tts.errors.unsupportedModel',
  'operation-in-progress': 'tts.errors.operationInProgress',
  'runtime-not-installed': 'tts.errors.runtimeNotInstalled',
  'model-not-installed': 'tts.errors.modelNotInstalled',
  'no-speakable-text': 'tts.errors.noSpeakableText',
  'operation-cancelled': 'tts.errors.operationCancelled',
  'audiobook-cache-mismatch': 'tts.errors.audiobookCacheMismatch',
  'wav-too-large': 'tts.errors.wavTooLarge',
  'invalid-audiobook-bundle': 'tts.errors.invalidBundle',
} as const

export function nativeTtsErrorMessage(error: unknown): string {
  if (error instanceof NativeTtsError) {
    const key = ERROR_KEYS[error.code as keyof typeof ERROR_KEYS]
    if (key) return i18n.t(key)
  }
  return nativeTtsErrorDetail(error)
}

export function nativeTtsErrorDetail(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

export function isNativeTtsCancellation(error: unknown): boolean {
  return error instanceof NativeTtsError
    ? error.code === 'operation-cancelled'
    : nativeTtsErrorDetail(error).toLowerCase().includes('cancelled')
}
