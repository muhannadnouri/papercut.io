import { describe, expect, it } from 'vitest'
import { parseReaderBookmark } from './useReaderBookmark'

describe('parseReaderBookmark', () => {
  it('keeps window scroll data and PDF page coordinates', () => {
    expect(parseReaderBookmark(JSON.stringify({
      scrollRatio: 0.5,
      scrollY: 320,
      updatedAtMs: 10,
      viewerLocation: { pageNumber: 7, left: 12, top: 416 },
    }))).toEqual({
      scrollRatio: 0.5,
      scrollY: 320,
      updatedAtMs: 10,
      viewerLocation: { pageNumber: 7, left: 12, top: 416 },
    })
  })

  it('rejects an invalid viewer location', () => {
    expect(parseReaderBookmark(JSON.stringify({
      scrollRatio: 0,
      scrollY: 0,
      updatedAtMs: 10,
      viewerLocation: { pageNumber: 7, pageOffsetRatio: 0.5 },
    }))).toBeNull()
  })
})
