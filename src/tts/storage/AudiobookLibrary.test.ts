import { describe, expect, it } from 'vitest'
import { getSavedAudiobooksForDocument, type SavedAudiobookRecord } from './AudiobookLibrary'

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
