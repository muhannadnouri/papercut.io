import { describe, expect, it } from 'vitest'
import { parseReaderBookmark } from './useReaderBookmark'

describe('parseReaderBookmark', () => {
  it('keeps PDF page coordinates', () => {
    expect(parseReaderBookmark(JSON.stringify({
      updatedAtMs: 10,
      viewerLocation: { pageNumber: 7, left: 12, top: 416 },
    }))).toEqual({
      updatedAtMs: 10,
      viewerLocation: { pageNumber: 7, left: 12, top: 416 },
    })
  })

  it('keeps semantic HTML and EPUB text offsets', () => {
    expect(parseReaderBookmark(JSON.stringify({
      updatedAtMs: 10,
      viewerLocation: { textOffset: 1842 },
    }))).toEqual({
      updatedAtMs: 10,
      viewerLocation: { textOffset: 1842 },
    })
  })

  it('rejects legacy window-scroll bookmarks', () => {
    expect(parseReaderBookmark(JSON.stringify({
      scrollRatio: 0.5,
      scrollY: 320,
      updatedAtMs: 10,
    }))).toBeNull()
  })

  it('rejects an invalid viewer location', () => {
    expect(parseReaderBookmark(JSON.stringify({
      updatedAtMs: 10,
      viewerLocation: { pageNumber: 7, pageOffsetRatio: 0.5 },
    }))).toBeNull()
  })
})
