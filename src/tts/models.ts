import {
  DEFAULT_TTS_MODEL_ID,
  LIBTASHKEEL_TEXT_PREPROCESSOR,
  PIPER_KAREEM_MODEL_ID,
  SILMA_MODEL_ID,
  SUPERTONIC_AR_MODEL_ID,
  SUPERTONIC_EN_MODEL_ID,
  TEXT_PREPROCESSOR_NONE,
  type TtsChunk,
  type TtsModelInfo,
} from './types'

// Native capabilities are authoritative. This fallback keeps browser/stub UI
// deterministic and preserves existing preferences before Tauri responds.
export const FALLBACK_TTS_MODELS: TtsModelInfo[] = [
  {
    id: DEFAULT_TTS_MODEL_ID,
    name: 'Kokoro v1.0',
    family: 'kokoro',
    language: 'en-US',
    languageLabel: 'English',
    defaultVoice: 'af_heart',
    defaultTextPreprocessor: TEXT_PREPROCESSOR_NONE,
    textPreprocessors: [{
      id: TEXT_PREPROCESSOR_NONE,
      name: 'Original text',
      description: 'Synthesize source text without language preprocessing.',
    }],
    voices: [
      ['af_heart', 'Heart'], ['af_bella', 'Bella'], ['af_nicole', 'Nicole'],
      ['af_sarah', 'Sarah'], ['af_sky', 'Sky'], ['af_nova', 'Nova'],
      ['af_alloy', 'Alloy'], ['af_aoede', 'Aoede'], ['af_kore', 'Kore'],
      ['af_jessica', 'Jessica'], ['af_river', 'River'], ['am_fenrir', 'Fenrir'],
      ['am_michael', 'Michael'], ['am_puck', 'Puck'], ['am_liam', 'Liam'],
      ['am_onyx', 'Onyx'], ['am_echo', 'Echo'], ['am_eric', 'Eric'],
      ['am_santa', 'Santa'], ['bf_emma', 'Emma'], ['bf_isabella', 'Isabella'],
      ['bf_alice', 'Alice'], ['bf_lily', 'Lily'], ['bm_george', 'George'],
      ['bm_lewis', 'Lewis'], ['bm_daniel', 'Daniel'], ['bm_fable', 'Fable'],
    ].map(([id, name]) => ({ id, name })),
  },
  {
    id: PIPER_KAREEM_MODEL_ID,
    name: 'Piper Kareem Medium',
    family: 'vits',
    language: 'ar-JO',
    languageLabel: 'Arabic (Jordan)',
    defaultVoice: 'kareem',
    defaultTextPreprocessor: LIBTASHKEEL_TEXT_PREPROCESSOR,
    textPreprocessors: [
      {
        id: TEXT_PREPROCESSOR_NONE,
        name: 'Original text',
        description: 'Synthesize Arabic source text without automatic diacritization.',
      },
      {
        id: LIBTASHKEEL_TEXT_PREPROCESSOR,
        name: 'Auto diacritization',
        description: 'Restore Arabic tashkeel with Libtashkeel before Piper synthesis.',
      },
    ],
    voices: [{ id: 'kareem', name: 'Kareem' }],
  },
  {
    id: SUPERTONIC_EN_MODEL_ID,
    name: 'Supertonic 3 English',
    family: 'supertonic',
    language: 'en-US',
    languageLabel: 'English',
    defaultVoice: 'speaker_6',
    defaultTextPreprocessor: TEXT_PREPROCESSOR_NONE,
    textPreprocessors: [{
      id: TEXT_PREPROCESSOR_NONE,
      name: 'Original text',
      description: 'Synthesize source text without language preprocessing.',
    }],
    voices: [{ id: 'speaker_6', name: 'Speaker 6' }],
  },
  {
    id: SUPERTONIC_AR_MODEL_ID,
    name: 'Supertonic 3 Arabic',
    family: 'supertonic',
    language: 'ar',
    languageLabel: 'Arabic',
    defaultVoice: 'speaker_6',
    defaultTextPreprocessor: TEXT_PREPROCESSOR_NONE,
    textPreprocessors: [{
      id: TEXT_PREPROCESSOR_NONE,
      name: 'Original text',
      description: 'Synthesize source text without language preprocessing.',
    }],
    voices: [{ id: 'speaker_6', name: 'Speaker 6' }],
  },
  {
    id: SILMA_MODEL_ID,
    name: 'SILMA Arabic TTS',
    family: 'silma-f5',
    language: 'ar',
    languageLabel: 'Arabic',
    defaultVoice: 'silma-ar-default',
    defaultTextPreprocessor: 'silma-default',
    textPreprocessors: [{
      id: 'silma-default',
      name: 'SILMA default',
      description: "Use SILMA's default Arabic text processing before synthesis.",
    }],
    voices: [{ id: 'silma-ar-default', name: 'SILMA Arabic Reference' }],
  },
]

export function getTtsModel(models: TtsModelInfo[], modelId: string): TtsModelInfo {
  return models.find((model) => model.id === modelId)
    ?? models.find((model) => model.id === DEFAULT_TTS_MODEL_ID)
    ?? FALLBACK_TTS_MODELS[0]
}

export function getTtsVoiceName(models: TtsModelInfo[], modelId: string, voiceId: string): string {
  return getTtsModel(models, modelId).voices.find((voice) => voice.id === voiceId)?.name ?? voiceId
}

export function resolveModelTextPreprocessor(
  model: TtsModelInfo,
  requested: string | undefined,
): string {
  return model.textPreprocessors.some((item) => item.id === requested)
    ? requested as string
    : model.defaultTextPreprocessor
}

export function suggestTtsModel(models: TtsModelInfo[], chunks: TtsChunk[]): TtsModelInfo {
  let arabic = 0
  let latin = 0
  for (const chunk of chunks) {
    for (const char of chunk.text) {
      if (/[\u0600-\u06ff]/u.test(char)) arabic += 1
      else if (/[A-Za-z]/.test(char)) latin += 1
    }
  }

  if (arabic > latin && arabic >= 20) {
    return models.find((model) => model.language.toLowerCase().startsWith('ar'))
      ?? getTtsModel(models, DEFAULT_TTS_MODEL_ID)
  }
  return getTtsModel(models, DEFAULT_TTS_MODEL_ID)
}
