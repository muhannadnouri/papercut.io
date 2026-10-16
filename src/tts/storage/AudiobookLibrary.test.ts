import { describe, expect, it } from 'vitest'
import { createAudiobookId, getSavedAudiobooksForDocument, type SavedAudiobookRecord } from './AudiobookLibrary'
import { DEFAULT_TTS_MODEL_ID } from '../types'

function record(id: string, documentUrl: string): SavedAudiobookRecord {
  return {
    id,
    documentUrl,
    title: id,
    voice: 'voice',
    speed: 1,
    modelId: 'model',
    textPreprocessor: 'none',
    dtype: 'native',
    savedAt: 1,
    chunks: 1,
  }
}

describe('saved audiobook records', () => {
  it('preserves the exact pre-Nabra English audiobook cache identities', () => {
    for (const voice of ['af_heart', 'bf_emma']) {
      expect(createAudiobookId('/documents/example.html', {
        modelId: DEFAULT_TTS_MODEL_ID, voice, speed: 1,
      })).toBe(`sherpa-onnx/kokoro-multi-lang-v1_0|native-save-v4-segmented|native|${voice}|1.00|/documents/example.html`)
    }
  })

  it('returns every saved version for the requested document only', () => {
    const records = [
      record('first-voice', '/book.html'),
      record('other-book', '/other.html'),
      record('second-voice', '/book.html'),
    ]

    expect(getSavedAudiobooksForDocument(records, '/book.html').map((item) => item.id))
      .toEqual(['first-voice', 'second-voice'])
  })
})
