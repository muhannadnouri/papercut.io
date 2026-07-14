import { LIBTASHKEEL_TEXT_PREPROCESSOR, TEXT_PREPROCESSOR_NONE } from '../types'
import { FALLBACK_TTS_MODELS, getTtsModel, getTtsVoiceName } from '../models'

import { formatStorageSize } from '../../utils/formatUtils'
// Re-export so modules importing from '../utils/format' (AudiobooksPanel,
// useAudiobookManager) get it here too, alongside this file's own formatters.
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

// export function formatStorageSize(bytes: number | undefined): string | null {
//   if (!bytes || !Number.isFinite(bytes) || bytes <= 0) return null
//   if (bytes >= 1024 * 1024 * 1024) {
//     const gb = bytes / 1024 / 1024 / 1024
//     return gb.toFixed(gb >= 10 ? 1 : 2) + ' GB'
//   }
//   return Math.max(1, Math.ceil(bytes / 1024 / 1024)) + ' MB'
// }

export function formatAudiobookVoiceMeta(
  modelId: string,
  voice: string,
  speed: number,
  dtype: string,
  textPreprocessor?: string,
  silmaNfeStep?: number,
): string {
  const model = getTtsModel(FALLBACK_TTS_MODELS, modelId)
  const voiceName = getTtsVoiceName(FALLBACK_TTS_MODELS, modelId, voice)
  const parts = ['Voice 🔊 ' + voiceName, '⚡' + formatSpeedLabel(speed), dtype]
  if (model.family === 'silma-f5' && silmaNfeStep) parts.push('NFE ' + silmaNfeStep)
  if (textPreprocessor === LIBTASHKEEL_TEXT_PREPROCESSOR) parts.push('Arabic tashkeel')
  return parts.join(' • ')
}

export function formatSavedAudiobookMeta(
  modelId: string,
  voice: string,
  speed: number,
  textPreprocessor: string | undefined,
  seconds: number | undefined,
  bytes: number | undefined,
): string {
  const model = getTtsModel(FALLBACK_TTS_MODELS, modelId)
  const voiceName = getTtsVoiceName(FALLBACK_TTS_MODELS, modelId, voice)
  const parts = [
    '🤖 ' + model.name,
    '🔊 ' + voiceName,
    '⚡ ' + formatSpeedLabel(speed),
    'AI-generated',
  ]
  if (model.family === 'silma-f5') parts.push('reference voice')
  if (textPreprocessor && textPreprocessor !== TEXT_PREPROCESSOR_NONE) {
    const processingName = model.textPreprocessors.find((item) => item.id === textPreprocessor)?.name
    parts.push('✨ ' + (processingName ?? textPreprocessor))
  }
  if (seconds && seconds > 0) parts.push('⏱ ' + formatDuration(seconds))
  const storage = formatStorageSize(bytes)
  if (storage) parts.push('💾 ' + storage)
  return parts.join(' • ')
}

export function formatDownloadSavedStatus(seconds: number | undefined, percent: number, bytes?: number): string {
  const boundedPercent = Math.min(Math.max(percent, 0), 100)
  const parts = seconds && seconds > 0
    ? ['Saved duration ' + formatDuration(seconds), boundedPercent + '% saved']
    : [boundedPercent + '% saved']
  const storage = formatStorageSize(bytes)
  if (storage) parts.push(storage + ' stored')
  return parts.join(' • ')
}

export function formatAudiobookExportMessage(path: string, format: 'bundle' | 'wav' = 'bundle'): string {
  const label = format === 'wav' ? 'WAV' : 'bundle'
  if (path.startsWith('content://')) {
    return 'Exported ' + label + ' to the selected file.'
  }
  return 'Exported ' + label + ' to ' + path
}
