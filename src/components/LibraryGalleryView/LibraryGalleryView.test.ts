import { describe, expect, it } from 'vitest'
import { isBookDocument } from './libraryCategories'

describe('isBookDocument', () => {
  it('keeps EPUB and future PDF files separate from HTML documents', () => {
    expect(isBookDocument({ title: 'Book', url: '/book', format: 'epub' })).toBe(true)
    expect(isBookDocument({ title: 'PDF', url: '/pdf', format: 'pdf' })).toBe(true)
    expect(isBookDocument({ title: 'Page', url: '/page', format: 'html' })).toBe(false)
    expect(isBookDocument({ title: 'Unknown', url: '/unknown' })).toBe(false)
  })
})
