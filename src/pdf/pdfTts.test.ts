import { describe, expect, it } from 'vitest'
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
})
