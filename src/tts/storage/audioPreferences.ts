import { FALLBACK_TTS_MODELS, getTtsModel, resolveModelTextPreprocessor } from '../models'
import {
  DEFAULT_TTS_MODEL_ID,
  DEFAULT_TTS_SPEED,
  DEFAULT_SILMA_NFE_STEP,
  NATIVE_TTS_DTYPE,
  TEXT_PREPROCESSOR_NONE,
  resolveSilmaNfeStep,
  type TtsDtype,
  type TtsModelId,
  type TtsVoice,
} from '../types'

const STORAGE_KEY = 'papercut.audioPreferences.v1'

export interface AudioPreferences {
  modelId: TtsModelId
  voice: TtsVoice
  speed: number
  playbackRate: number
  wordHighlightEnabled: boolean
  dtype: TtsDtype
  textPreprocessor: string
  silmaNfeStep: number
  audioSavedOnly: boolean
}

export const DEFAULT_AUDIO_PREFERENCES: AudioPreferences = {
  modelId: DEFAULT_TTS_MODEL_ID,
  voice: getTtsModel(FALLBACK_TTS_MODELS, DEFAULT_TTS_MODEL_ID).defaultVoice,
  speed: DEFAULT_TTS_SPEED,
  playbackRate: 1,
  wordHighlightEnabled: true,
  dtype: NATIVE_TTS_DTYPE,
  textPreprocessor: TEXT_PREPROCESSOR_NONE,
  silmaNfeStep: DEFAULT_SILMA_NFE_STEP,
  audioSavedOnly: false,
}

export function getAudioPreferences(): AudioPreferences {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return DEFAULT_AUDIO_PREFERENCES

    const parsed = JSON.parse(raw) as Partial<AudioPreferences>
    const model = getTtsModel(
      FALLBACK_TTS_MODELS,
      typeof parsed.modelId === 'string' ? parsed.modelId : DEFAULT_TTS_MODEL_ID,
    )
    const voice = typeof parsed.voice === 'string' && model.voices.some((item) => item.id === parsed.voice)
      ? parsed.voice
      : model.defaultVoice
    return {
      modelId: model.id,
      voice,
      speed: isValidSpeed(parsed.speed) ? parsed.speed : DEFAULT_AUDIO_PREFERENCES.speed,
      playbackRate: isValidPlaybackRate(parsed.playbackRate) ? parsed.playbackRate : DEFAULT_AUDIO_PREFERENCES.playbackRate,
      wordHighlightEnabled: typeof parsed.wordHighlightEnabled === 'boolean'
        ? parsed.wordHighlightEnabled
        : DEFAULT_AUDIO_PREFERENCES.wordHighlightEnabled,
      dtype: parsed.dtype === NATIVE_TTS_DTYPE ? parsed.dtype : DEFAULT_AUDIO_PREFERENCES.dtype,
      textPreprocessor: resolveModelTextPreprocessor(model, parsed.textPreprocessor),
      silmaNfeStep: resolveSilmaNfeStep({ silmaNfeStep: parsed.silmaNfeStep }),
      audioSavedOnly: typeof parsed.audioSavedOnly === 'boolean'
        ? parsed.audioSavedOnly
        : DEFAULT_AUDIO_PREFERENCES.audioSavedOnly,
    }
  } catch {
    return DEFAULT_AUDIO_PREFERENCES
  }
}

export function saveAudioPreferences(preferences: Partial<AudioPreferences>): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify({
      ...getAudioPreferences(),
      ...preferences,
    }))
  } catch {
    // Preferences are optional; audio generation still works without storage.
  }
}

function isValidSpeed(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && value > 0
}

function isValidPlaybackRate(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0.5 && value <= 3
}
