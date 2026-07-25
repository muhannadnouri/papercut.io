import { describe, expect, it } from 'vitest'
import { pdfBlocksToReadableSegments } from './pdfTts'

describe('pdfBlocksToReadableSegments', () => {
  it('keeps readable PDF blocks in stable page order', () => {
    expect(pdfBlocksToReadableSegments([
      ['First', 'Second', '  '],
      ['Third', 'Fourth'],
    ])).toEqual([
      { text: 'First', kind: 'paragraph' },
      { text: 'Second', kind: 'paragraph' },
      { text: 'Third', kind: 'paragraph' },
      { text: 'Fourth', kind: 'paragraph' },
    ])
  })
})
