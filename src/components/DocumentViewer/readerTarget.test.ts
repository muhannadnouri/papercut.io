import { describe, expect, it } from 'vitest'
import { readerSectionSelector } from './readerTarget'

describe('reader search-result section targets', () => {
  it('maps persisted section ordinals to generated reader markers', () => {
    expect(readerSectionSelector(12)).toBe('[data-papercut-section="12"]')
  })

  it('rejects values that cannot be persisted section ordinals', () => {
    expect(readerSectionSelector(-1)).toBeNull()
    expect(readerSectionSelector(1.5)).toBeNull()
    expect(readerSectionSelector(undefined)).toBeNull()
  })
})
