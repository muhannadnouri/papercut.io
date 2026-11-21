import {
  describe,
  expect,
  it,
} from 'vitest'
import {
  looksLikeAudiobookBundlePath,
  partitionOpenDocumentSources,
} from './nativeTts'

describe('looksLikeAudiobookBundlePath', () => {
  it('matches the bundle extension case-insensitively', () => {
    expect(looksLikeAudiobookBundlePath('/downloads/book.papercut-audiobook')).toBe(true)
    expect(looksLikeAudiobookBundlePath('/downloads/Book.PAPERCUT-AudioBook')).toBe(true)
  })

  it('leaves document sources alone', () => {
    expect(looksLikeAudiobookBundlePath('/documents/book.epub')).toBe(false)
    expect(looksLikeAudiobookBundlePath('/documents/papercut-audiobook.html')).toBe(false)
    expect(looksLikeAudiobookBundlePath('content://provider/document/42')).toBe(false)
  })
})

describe('partitionOpenDocumentSources', () => {
  it('routes by extension without probing named sources', async () => {
    const probed: string[] = []
    const { bundles, documents } = await partitionOpenDocumentSources(
      [
        '/downloads/book.papercut-audiobook',
        '/documents/book.epub',
        '/documents/notes.md',
      ],
      (source) => {
        probed.push(source)
        return Promise.resolve(false)
      },
    )

    expect(bundles).toEqual(['/downloads/book.papercut-audiobook'])
    expect(documents).toEqual(['/documents/book.epub', '/documents/notes.md'])
    expect(probed).toEqual([])
  })

  it('probes only extensionless sources and falls back on probe failure', async () => {
    const { bundles, documents } = await partitionOpenDocumentSources(
      [
        'content://provider/document/42',
        'content://provider/document/43',
        '/documents/book.pdf',
      ],
      (source) => source.endsWith('42')
        ? Promise.resolve(true)
        : Promise.reject(new Error('probe unavailable')),
    )

    expect(bundles).toEqual(['content://provider/document/42'])
    expect(documents).toEqual(['content://provider/document/43', '/documents/book.pdf'])
  })
})
