import { describe, expect, it } from 'vitest'
import {
  AUDIOBOOK_SAVE_CHUNK_PROFILE,
  chunkAudiobookSaveSegmentsWithSpans,
} from '../tts/utils/text'
import { pdfNarrationToReadableSegments } from './pdfTts'

describe('pdfNarrationToReadableSegments', () => {
  it('keeps reconstructed text and source runs in stable order', () => {
    expect(pdfNarrationToReadableSegments([
      {
        text: 'First paragraph.',
        sourceRuns: [{
          pageIndex: 0,
          blockOrder: 2,
          startOffset: 0,
          endOffset: 16,
          sourceStartOffset: 0,
          sourceEndOffset: 16,
        }],
      },
      { text: '  ', sourceRuns: [] },
      { text: 'Second paragraph.', sourceRuns: [] },
    ])).toEqual([
      {
        text: 'First paragraph.',
        kind: 'paragraph',
        sourceRuns: [{
          pageIndex: 0,
          blockOrder: 2,
          startOffset: 0,
          endOffset: 16,
          sourceStartOffset: 0,
          sourceEndOffset: 16,
        }],
      },
      { text: 'Second paragraph.', kind: 'paragraph', sourceRuns: [] },
    ])
  })

  it('keeps reconstructed prose within audiobook save limits', () => {
    const text = `${'A long reconstructed PDF sentence keeps every source word intact, '.repeat(14)}then ends.`
    const chunks = chunkAudiobookSaveSegmentsWithSpans(
      pdfNarrationToReadableSegments([{ text, sourceRuns: [] }]),
    )

    expect(chunks.length).toBeGreaterThan(1)
    expect(chunks.every((chunk) => chunk.text.length <= AUDIOBOOK_SAVE_CHUNK_PROFILE.maxChunkLength)).toBe(true)
    expect(chunks.map((chunk) => chunk.text).join(' ')).toBe(text)
  })
})
