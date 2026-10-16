import { describe, expect, it } from 'vitest'
import { FALLBACK_TTS_MODELS, getTtsPreviewText, suggestTtsModel } from './models'
import {
  LIBTASHKEEL_TEXT_PREPROCESSOR,
  NABRA_MODEL_ID,
  PIPER_KAREEM_MODEL_ID,
  SUPERTONIC_AR_MODEL_ID,
  SUPERTONIC_EN_MODEL_ID,
  TEXT_PREPROCESSOR_NONE,
} from './types'

const LATIN_CHUNKS = [{
  id: 'latin',
  text: 'A Latin-script document does not reveal which supported language it uses.',
}]
const ARABIC_CHUNKS = [{
  id: 'arabic',
  text: 'هذا نص عربي طويل بما يكفي لاقتراح نموذج عربي للقراءة الصوتية.',
}]

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

  it('still suggests an Arabic model when the selected language differs', () => {
    expect(suggestTtsModel(
      FALLBACK_TTS_MODELS,
      ARABIC_CHUNKS,
      SUPERTONIC_EN_MODEL_ID,
    )?.language).toMatch(/^ar/i)
  })

  it('preserves selected Nabra and the existing Arabic suggestion before native capabilities arrive', () => {
    const nabra = FALLBACK_TTS_MODELS.find((model) => model.id === NABRA_MODEL_ID)!
    expect(nabra.defaultVoice).toBe('af_msa')
    expect(nabra.voices.map((voice) => voice.id)).toEqual(['af_msa'])
    expect(nabra.defaultTextPreprocessor).toBe(LIBTASHKEEL_TEXT_PREPROCESSOR)
    expect(nabra.textPreprocessors.map((item) => item.id)).toEqual([
      TEXT_PREPROCESSOR_NONE, LIBTASHKEEL_TEXT_PREPROCESSOR,
    ])
    expect(suggestTtsModel(FALLBACK_TTS_MODELS, ARABIC_CHUNKS, NABRA_MODEL_ID)).toBeNull()
    expect(suggestTtsModel(FALLBACK_TTS_MODELS, ARABIC_CHUNKS, SUPERTONIC_EN_MODEL_ID)?.id)
      .toBe(PIPER_KAREEM_MODEL_ID)
  })
})

describe('getTtsPreviewText', () => {
  it('uses the selected model language and falls back to English', () => {
    expect(getTtsPreviewText('ar-JO')).toMatch(/[\u0600-\u06ff]/u)
    expect(getTtsPreviewText('unknown')).toBe(getTtsPreviewText('en-US'))
  })
})
