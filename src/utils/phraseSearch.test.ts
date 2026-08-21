import { describe, expect, it } from 'vitest'
import { parseSearchQuery } from './phraseSearch'
import { sanitizeMarkedExcerpt } from './textUtils'

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

it('keeps only native mark tags in a search excerpt', () => {
  expect(sanitizeMarkedExcerpt('<img src=x onerror="boom"><mark>safe</mark>')).toBe(
    '&lt;img src=x onerror=&quot;boom&quot;&gt;<mark>safe</mark>',
  )
})
