import type { TFunction } from 'i18next'
import { LIBTASHKEEL_TEXT_PREPROCESSOR, TEXT_PREPROCESSOR_NONE } from '../types'
import { FALLBACK_TTS_MODELS, getTtsModel, getTtsVoiceName } from '../models'

import { formatStorageSize } from '../../utils/formatUtils'
export { formatStorageSize }

export function formatDuration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return '0:00'
  const rounded = Math.round(seconds)
  const hours = Math.floor(rounded / 3600)
  const minutes = Math.floor((rounded % 3600) / 60)
  const remainingSeconds = rounded % 60
  if (hours > 0) {
    return hours + ':' + String(minutes).padStart(2, '0') + ':' + String(remainingSeconds).padStart(2, '0')
  }
  return minutes + ':' + String(remainingSeconds).padStart(2, '0')
}

export function formatSpeedLabel(speed: number): string {
  if (!Number.isFinite(speed)) return '1x'
  return speed.toFixed(speed % 1 === 0 ? 0 : 2).replace(/0$/, '').replace(/\.$/, '') + 'x'
}

export function formatAudiobookVoiceMeta(
  t: TFunction,
  modelId: string,
  voice: string,
  speed: number,
  dtype: string,
  textPreprocessor?: string,
  silmaNfeStep?: number,
): string {
  const model = getTtsModel(FALLBACK_TTS_MODELS, modelId)
  const voiceName = getTtsVoiceName(FALLBACK_TTS_MODELS, modelId, voice)
  const parts = [t('tts.audiobooks.voiceMeta', { voice: voiceName }), '⚡' + formatSpeedLabel(speed), dtype]
  if (model.family === 'silma-f5' && silmaNfeStep) parts.push('NFE ' + silmaNfeStep)
  if (textPreprocessor === LIBTASHKEEL_TEXT_PREPROCESSOR) parts.push(t('tts.audiobooks.arabicTashkeel'))
  return parts.join(' • ')
}

export function formatSavedAudiobookMeta(
  t: TFunction,
  modelId: string,
  voice: string,
  _speed: number,
  textPreprocessor: string | undefined,
  seconds: number | undefined,
  bytes: number | undefined,
): string {
  return formatSavedAudiobookMetaParts(
    t,
    modelId,
    voice,
    _speed,
    textPreprocessor,
    seconds,
    bytes,
  ).join(' • ')
}

export function formatSavedAudiobookMetaParts(
  t: TFunction,
  modelId: string,
  voice: string,
  _speed: number,
  textPreprocessor: string | undefined,
  seconds: number | undefined,
  bytes: number | undefined,
): string[] {
  const model = getTtsModel(FALLBACK_TTS_MODELS, modelId)
  const voiceName = getTtsVoiceName(FALLBACK_TTS_MODELS, modelId, voice)
  const parts = [
    '🤖 ' + model.name,
    '🔊 ' + voiceName,
    t('tts.audiobooks.aiGenerated'),
  ]
  if (textPreprocessor && textPreprocessor !== TEXT_PREPROCESSOR_NONE) {
    const processingName = model.textPreprocessors.find((item) => item.id === textPreprocessor)?.name
    parts.push('✨ ' + formatTextPreprocessorLabel(t, textPreprocessor, processingName))
  }
  if (seconds && seconds > 0) parts.push('⏱\u00a0' + formatDuration(seconds))
  const storage = formatStorageSize(bytes)
  if (storage) parts.push('💾 ' + storage)
  return parts
}

export function formatTextPreprocessorLabel(
  t: TFunction,
  id: string,
  fallback?: string,
): string {
  if (id === TEXT_PREPROCESSOR_NONE) return t('tts.setup.preprocessorOriginal')
  if (id === LIBTASHKEEL_TEXT_PREPROCESSOR) return t('tts.setup.preprocessorTashkeel')
  if (id === 'silma-default') return t('tts.setup.preprocessorSilma')
  return fallback ?? id
}

export function formatDownloadSavedStatus(
  t: TFunction,
  seconds: number | undefined,
  percent: number,
  bytes?: number,
): string {
  const boundedPercent = Math.min(Math.max(percent, 0), 100)
  const parts = seconds && seconds > 0
    ? [
        t('tts.audiobooks.savedDuration', { duration: formatDuration(seconds) }),
        t('tts.audiobooks.percentSaved', { percent: boundedPercent }),
      ]
    : [t('tts.audiobooks.percentSaved', { percent: boundedPercent })]
  const storage = formatStorageSize(bytes)
  if (storage) parts.push(t('tts.audiobooks.storageStored', { storage }))
  return parts.join(' • ')
}

export function formatAudiobookExportMessage(
  t: TFunction,
  path: string,
  format: 'bundle' | 'wav' = 'bundle',
): string {
  const label = format === 'wav' ? 'WAV' : t('tts.audiobooks.exportBundle')
  if (path.startsWith('content://')) {
    return t('tts.audiobooks.exportedSelected', { format: label })
  }
  return t('tts.audiobooks.exportedPath', { format: label, path })
}
