import { describe, expect, it } from 'vitest'
import { parseSearchQuery, searchQueryWithinLimits } from './phraseSearch'
import { normalizeForPhraseMatch, sanitizeMarkedExcerpt } from './textUtils'

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

  it.each([
    'archive "The guide calls it “silver compass” for navigation" orchard',
    'archive “The guide calls it "silver compass" for navigation” orchard',
    'archive “The guide calls it “silver compass” for navigation” orchard',
  ])('keeps nested quotes inside one required source phrase: %s', (query) => {
    expect(parseSearchQuery(query)).toEqual({
      exactPhrases: ['The guide calls it "silver compass" for navigation'],
      providerQuery: 'archive The guide calls it silver compass for navigation orchard',
      unquotedText: 'archive orchard',
      unmatchedQuote: false,
    })
  })

  it.each(['"guide “silver compass"', '“guide "silver compass”', 'guide”'])(
    'rejects mismatched nested quote delimiters: %s',
    (query) => expect(parseSearchQuery(query).unmatchedQuote).toBe(true),
  )
})

it('normalizes typographic quotes and dashes for literal phrase matching', () => {
  expect(normalizeForPhraseMatch('“Well—Being”')).toBe('"well-being"')
  expect(normalizeForPhraseMatch('“Well–Being”')).toBe('"well-being"')
})

it('keeps only native mark tags in a search excerpt', () => {
  expect(sanitizeMarkedExcerpt('<img src=x onerror="boom"><mark>safe</mark>')).toBe(
    '&lt;img src=x onerror=&quot;boom&quot;&gt;<mark>safe</mark>',
  )
})

it('bounds native Unicode characters and normalized terms across broad and quoted clauses', () => {
  expect(searchQueryWithinLimits('🙂'.repeat(4096), [])).toBe(true)
  expect(searchQueryWithinLimits('🙂'.repeat(4097), [])).toBe(false)
  expect(searchQueryWithinLimits('x'.repeat(4096), ['y'])).toBe(false)
  expect(searchQueryWithinLimits('word '.repeat(128).trim(), [])).toBe(true)
  expect(searchQueryWithinLimits('word '.repeat(128).trim(), ['another'])).toBe(false)
  expect(searchQueryWithinLimits('', ['word '.repeat(129).trim()])).toBe(false)
  expect(searchQueryWithinLimits('é‐\nclair '.repeat(128).trim(), [])).toBe(true)
  expect(searchQueryWithinLimits('!!! '.repeat(129).trim(), [])).toBe(true)
  expect(searchQueryWithinLimits('a\u0085'.repeat(129), [])).toBe(false)
})
