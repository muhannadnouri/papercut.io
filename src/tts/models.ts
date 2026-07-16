import {
  DEFAULT_TTS_MODEL_ID,
  KOKORO_ZH_MODEL_ID,
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
      ['af_heart', '🇺🇸 Heart (A)'], ['af_bella', '🇺🇸 Bella (A-)'],
      ['af_nicole', '🇺🇸 Nicole (B-)'], ['af_sarah', '🇺🇸 Sarah (C+)'],
      ['af_sky', '🇺🇸 Sky (C-)'], ['af_nova', '🇺🇸 Nova (C)'],
      ['af_alloy', '🇺🇸 Alloy (C)'], ['af_aoede', '🇺🇸 Aoede (C+)'],
      ['af_kore', '🇺🇸 Kore (C+)'], ['af_jessica', '🇺🇸 Jessica (D)'],
      ['af_river', '🇺🇸 River (D)'], ['am_fenrir', '🇺🇸 Fenrir (C+)'],
      ['am_michael', '🇺🇸 Michael (C+)'], ['am_puck', '🇺🇸 Puck (C+)'],
      ['am_liam', '🇺🇸 Liam (D)'], ['am_onyx', '🇺🇸 Onyx (D)'],
      ['am_echo', '🇺🇸 Echo (D)'], ['am_eric', '🇺🇸 Eric (D)'],
      ['am_santa', '🇺🇸 Santa (D-)'], ['bf_emma', '🇬🇧 Emma (B-)'],
      ['bf_isabella', '🇬🇧 Isabella (C)'], ['bf_alice', '🇬🇧 Alice (D)'],
      ['bf_lily', '🇬🇧 Lily (D)'], ['bm_george', '🇬🇧 George (C)'],
      ['bm_lewis', '🇬🇧 Lewis (D+)'], ['bm_daniel', '🇬🇧 Daniel (D)'],
      ['bm_fable', '🇬🇧 Fable (C)'],
    ].map(([id, name]) => ({ id, name })),
  },
  {
    id: KOKORO_ZH_MODEL_ID,
    name: 'Kokoro v1.0 Mandarin (Experimental)',
    family: 'kokoro',
    language: 'zh-CN',
    languageLabel: 'Chinese (Mandarin)',
    defaultVoice: 'zf_xiaobei',
    defaultTextPreprocessor: TEXT_PREPROCESSOR_NONE,
    textPreprocessors: [{
      id: TEXT_PREPROCESSOR_NONE,
      name: 'Original text',
      description: 'Synthesize source text without language preprocessing.',
    }],
    voices: [
      ['zf_xiaobei', '🇨🇳 Xiaobei (D)'], ['zf_xiaoni', '🇨🇳 Xiaoni (D)'],
      ['zf_xiaoxiao', '🇨🇳 Xiaoxiao (D)'], ['zf_xiaoyi', '🇨🇳 Xiaoyi (D)'],
      ['zm_yunjian', '🇨🇳 Yunjian (D)'], ['zm_yunxi', '🇨🇳 Yunxi (D)'],
      ['zm_yunxia', '🇨🇳 Yunxia (D)'], ['zm_yunyang', '🇨🇳 Yunyang (D)'],
    ].map(([id, name]) => ({ id, name })),
  },
  {
    id: 'sherpa-onnx/kokoro-multi-lang-v1_0-es',
    name: 'Kokoro v1.0 Spanish (Experimental)',
    family: 'kokoro',
    language: 'es-ES',
    languageLabel: 'Spanish',
    defaultVoice: 'ef_dora',
    defaultTextPreprocessor: TEXT_PREPROCESSOR_NONE,
    textPreprocessors: [{
      id: TEXT_PREPROCESSOR_NONE,
      name: 'Original text',
      description: 'Synthesize source text without language preprocessing.',
    }],
    voices: [
      { id: 'ef_dora', name: '🇪🇸 Dora' },
      { id: 'em_alex', name: '🇪🇸 Alex' },
    ],
  },
  {
    id: 'sherpa-onnx/kokoro-multi-lang-v1_0-fr',
    name: 'Kokoro v1.0 French (Experimental)',
    family: 'kokoro',
    language: 'fr-FR',
    languageLabel: 'French',
    defaultVoice: 'ff_siwis',
    defaultTextPreprocessor: TEXT_PREPROCESSOR_NONE,
    textPreprocessors: [{
      id: TEXT_PREPROCESSOR_NONE,
      name: 'Original text',
      description: 'Synthesize source text without language preprocessing.',
    }],
    voices: [{ id: 'ff_siwis', name: '🇫🇷 Siwis (B-)' }],
  },
  {
    id: 'sherpa-onnx/kokoro-multi-lang-v1_0-hi',
    name: 'Kokoro v1.0 Hindi (Experimental)',
    family: 'kokoro',
    language: 'hi-IN',
    languageLabel: 'Hindi',
    defaultVoice: 'hf_alpha',
    defaultTextPreprocessor: TEXT_PREPROCESSOR_NONE,
    textPreprocessors: [{
      id: TEXT_PREPROCESSOR_NONE,
      name: 'Original text',
      description: 'Synthesize source text without language preprocessing.',
    }],
    voices: [
      ['hf_alpha', '🇮🇳 Alpha (C)'], ['hf_beta', '🇮🇳 Beta (C)'],
      ['hm_omega', '🇮🇳 Omega (C)'], ['hm_psi', '🇮🇳 Psi (C)'],
    ].map(([id, name]) => ({ id, name })),
  },
  {
    id: 'sherpa-onnx/kokoro-multi-lang-v1_0-it',
    name: 'Kokoro v1.0 Italian (Experimental)',
    family: 'kokoro',
    language: 'it-IT',
    languageLabel: 'Italian',
    defaultVoice: 'if_sara',
    defaultTextPreprocessor: TEXT_PREPROCESSOR_NONE,
    textPreprocessors: [{
      id: TEXT_PREPROCESSOR_NONE,
      name: 'Original text',
      description: 'Synthesize source text without language preprocessing.',
    }],
    voices: [
      { id: 'if_sara', name: '🇮🇹 Sara (C)' },
      { id: 'im_nicola', name: '🇮🇹 Nicola (C)' },
    ],
  },
  {
    id: 'sherpa-onnx/kokoro-multi-lang-v1_0-pt-br',
    name: 'Kokoro v1.0 Brazilian Portuguese (Experimental)',
    family: 'kokoro',
    language: 'pt-BR',
    languageLabel: 'Portuguese (Brazil)',
    defaultVoice: 'pf_dora',
    defaultTextPreprocessor: TEXT_PREPROCESSOR_NONE,
    textPreprocessors: [{
      id: TEXT_PREPROCESSOR_NONE,
      name: 'Original text',
      description: 'Synthesize source text without language preprocessing.',
    }],
    voices: [
      { id: 'pf_dora', name: '🇧🇷 Dora' },
      { id: 'pm_alex', name: '🇧🇷 Alex' },
      { id: 'pm_santa', name: '🇧🇷 Santa' },
    ],
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
  let han = 0
  let kana = 0
  let latin = 0
  for (const chunk of chunks) {
    for (const char of chunk.text) {
      if (/[\u0600-\u06ff]/u.test(char)) arabic += 1
      else if (/\p{Script=Han}/u.test(char)) han += 1
      else if (/[\u3040-\u30ff]/u.test(char)) kana += 1
      else if (/[A-Za-z]/.test(char)) latin += 1
    }
  }

  // Han characters dominate Mandarin prose, while kana prevents Japanese text
  // from being incorrectly suggested as Chinese.
  if (han > latin && han > arabic && han >= 20 && kana < 5) {
    return models.find((model) => model.language.toLowerCase().startsWith('zh'))
      ?? getTtsModel(models, DEFAULT_TTS_MODEL_ID)
  }
  if (arabic > latin && arabic >= 20) {
    return models.find((model) => model.language.toLowerCase().startsWith('ar'))
      ?? getTtsModel(models, DEFAULT_TTS_MODEL_ID)
  }
  return getTtsModel(models, DEFAULT_TTS_MODEL_ID)
}
