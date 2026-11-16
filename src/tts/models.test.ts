import {
  describe,
  expect,
  it,
} from 'vitest'
import {
  FALLBACK_TTS_MODELS,
  getRecommendedTtsModel,
  getTtsPreviewText,
  suggestTtsModel,
} from './models'
import {
  DEFAULT_TTS_MODEL_ID,
  KOKORO_7M_MODEL_ID,
  NABRA_7M_MODEL_ID,
  LIBTASHKEEL_TEXT_PREPROCESSOR,
  PIPER_KAREEM_MODEL_ID,
  SUPERTONIC_AR_MODEL_ID,
  SUPERTONIC_EN_MODEL_ID,
} from './types'

const LATIN_CHUNKS = [{
  id: 'latin',
  text: 'A Latin-script document does not reveal which supported language it uses.',
}]
const ARABIC_CHUNKS = [{
  id: 'arabic',
  text: 'هذا نص عربي طويل بما يكفي لاقتراح نموذج عربي للقراءة الصوتية.',
}]

const CATALOGS = [
  {
    order: 'fallback',
    models: FALLBACK_TTS_MODELS,
  },
  {
    order: 'reversed',
    models: [...FALLBACK_TTS_MODELS].reverse(),
  },
]

describe('recommended TTS models', () => {
  it.each(CATALOGS)(
    'finds recommendations in $order catalog order', ({ models }) => {
      expect(getRecommendedTtsModel(models, 'en-US')?.id).toBe(DEFAULT_TTS_MODEL_ID)
      expect(getRecommendedTtsModel(models, 'AR-JO')?.id).toBe(PIPER_KAREEM_MODEL_ID)
      expect(getRecommendedTtsModel(models, 'fr-FR')).toBeUndefined()
    },
  )

  it('does not recommend unavailable models', () => {
    const models = FALLBACK_TTS_MODELS.filter((model) => model.id !== PIPER_KAREEM_MODEL_ID)
    expect(getRecommendedTtsModel(models, 'ar')).toBeUndefined()
    expect(suggestTtsModel(models, ARABIC_CHUNKS, DEFAULT_TTS_MODEL_ID)?.id)
      .toBe(SUPERTONIC_AR_MODEL_ID)
  })
})

describe('suggestTtsModel', () => {
  it('preserves the selected model for ambiguous Latin-script text', () => {
    expect(suggestTtsModel(
      FALLBACK_TTS_MODELS,
      LATIN_CHUNKS,
      SUPERTONIC_EN_MODEL_ID,
    )).toBeNull()
  })

  it('preserves an Arabic engine already selected for Arabic text', () => {
    expect(suggestTtsModel(
      FALLBACK_TTS_MODELS,
      ARABIC_CHUNKS,
      SUPERTONIC_AR_MODEL_ID,
    )).toBeNull()
  })

  it.each(CATALOGS)('suggests Kareem when the selected language differs ($order order)', ({ models }) => {
    expect(suggestTtsModel(
      models,
      ARABIC_CHUNKS,
      SUPERTONIC_EN_MODEL_ID,
    )?.id).toBe(PIPER_KAREEM_MODEL_ID)
  })
})

describe('getTtsPreviewText', () => {
  it('uses the selected model language and falls back to English', () => {
    expect(getTtsPreviewText('ar-JO')).toMatch(/[\u0600-\u06ff]/u)
    expect(getTtsPreviewText('unknown')).toBe(getTtsPreviewText('en-US'))
  })
})

describe('paired distill selections', () => {
  it('keeps independent single-voice choices without changing recommendations', () => {
    const english = FALLBACK_TTS_MODELS.find((model) => model.id === KOKORO_7M_MODEL_ID)!
    const arabic = FALLBACK_TTS_MODELS.find((model) => model.id === NABRA_7M_MODEL_ID)!
    expect(english.voices.map((voice) => voice.id)).toEqual(['af_msa'])
    expect(arabic.voices.map((voice) => voice.id)).toEqual(['af_msa'])
    expect(arabic.defaultTextPreprocessor).toBe(LIBTASHKEEL_TEXT_PREPROCESSOR)
    expect(suggestTtsModel(FALLBACK_TTS_MODELS, ARABIC_CHUNKS, arabic.id)).toBeNull()
    expect(suggestTtsModel(FALLBACK_TTS_MODELS, LATIN_CHUNKS, english.id)).toBeNull()
    expect(getRecommendedTtsModel(FALLBACK_TTS_MODELS, 'en')?.id).toBe(DEFAULT_TTS_MODEL_ID)
    expect(getRecommendedTtsModel(FALLBACK_TTS_MODELS, 'ar')?.id).toBe(PIPER_KAREEM_MODEL_ID)
  })
})
