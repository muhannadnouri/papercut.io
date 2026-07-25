import { describe, expect, it } from 'vitest'
import { parseReaderBookmark } from './useReaderBookmark'

describe('parseReaderBookmark', () => {
  it('keeps legacy scroll data and clamps a PDF page offset', () => {
    expect(parseReaderBookmark(JSON.stringify({
      scrollRatio: 0.5,
      scrollY: 320,
      updatedAtMs: 10,
      viewerLocation: { pageNumber: 7, pageOffsetRatio: 1.4 },
    }))).toEqual({
      scrollRatio: 0.5,
      scrollY: 320,
      updatedAtMs: 10,
      viewerLocation: { pageNumber: 7, pageOffsetRatio: 1 },
    })
  })
})
