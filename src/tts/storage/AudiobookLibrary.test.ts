import {
  describe,
  expect,
  it,
} from 'vitest'
import {
  createAudiobookId,
  getSavedAudiobooksForDocument,
  type SavedAudiobookRecord,
} from './AudiobookLibrary'

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

describe('saved audiobook document lookup', () => {
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

it.each([
  ['sherpa-onnx/kokoro-multi-lang-v1_0', 'af_heart', 'none', 'sherpa-onnx/kokoro-multi-lang-v1_0|native-save-v4-segmented|native|af_heart|1.00|/book.html'],
  ['sherpa-onnx/kokoro-multi-lang-v1_0', 'bf_emma', 'none', 'sherpa-onnx/kokoro-multi-lang-v1_0|native-save-v4-segmented|native|bf_emma|1.00|/book.html'],
  ['sherpa-onnx/vits-piper-ar_JO-kareem-medium', 'kareem', 'libtashkeel-1.5.0', 'sherpa-onnx/vits-piper-ar_JO-kareem-medium|native-save-v4-segmented|native|kareem|1.00|libtashkeel-1.5.0|/book.html'],
])('preserves the historical audiobook ID for %s / %s', (modelId, voice, textPreprocessor, expected) => {
  expect(createAudiobookId('/book.html', {
    modelId,
    voice,
    textPreprocessor,
    speed: 1,
  })).toBe(expected)
})
