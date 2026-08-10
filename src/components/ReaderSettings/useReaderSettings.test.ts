import { describe, expect, it } from 'vitest'
import { normalizeReaderPageTheme } from './useReaderSettings'

describe('normalizeReaderPageTheme', () => {
  it('keeps supported themes and repairs stale persisted values', () => {
    expect(normalizeReaderPageTheme('gray')).toBe('gray')
    expect(normalizeReaderPageTheme('black')).toBe('black')
    expect(normalizeReaderPageTheme('sepia')).toBe('default')
  })
})
