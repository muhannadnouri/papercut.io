import { describe, expect, it } from 'vitest'
import { parseReaderBookmark, readBookmarkedDocumentUrls } from './useReaderBookmark'

function bookmarkStorage(entries: Record<string, string>) {
  const keys = Object.keys(entries)
  return {
    length: keys.length,
    key: (index: number) => keys[index] ?? null,
    getItem: (key: string) => entries[key] ?? null,
  }
}

describe('readBookmarkedDocumentUrls', () => {
  it('returns only URLs backed by valid current bookmarks', () => {
    const valid = JSON.stringify({
      updatedAtMs: 10,
      viewerLocation: { textOffset: 1842 },
    })
    const urls = readBookmarkedDocumentUrls(bookmarkStorage({
      'papercut:reader-bookmark:/documents/book.html': valid,
      'papercut:reader-bookmark:upload://invalid': '{',
      'unrelated-setting': valid,
    }))

    expect([...urls]).toEqual(['/documents/book.html'])
  })
})

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

  it.each([
    ['malformed JSON', '{'],
    ['JSON null', 'null'],
    ['a non-finite timestamp', '{"updatedAtMs":1e999,"viewerLocation":{"textOffset":1}}'],
  ])('rejects %s', (_case, raw) => {
    expect(parseReaderBookmark(raw)).toBeNull()
  })
})
