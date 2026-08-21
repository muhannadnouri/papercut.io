import { describe, expect, it } from 'vitest'
import { parseSearchQuery } from './phraseSearch'

describe('search query parsing', () => {
  it('keeps broad words and exact phrases distinct in a mixed query', () => {
    expect(parseSearchQuery('Anne “green gables” orchard')).toEqual({
      exactPhrases: ['green gables'],
      providerQuery: 'Anne green gables orchard',
      unquotedText: 'Anne orchard',
      unmatchedQuote: false,
    })
  })

  it('reports an unmatched quote instead of silently broadening the query', () => {
    expect(parseSearchQuery('anne "green gables').unmatchedQuote).toBe(true)
  })
})
