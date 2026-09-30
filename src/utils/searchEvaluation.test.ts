import { expect, test } from 'vitest'
import corpus from '../../scripts/fixtures/search-v2/corpus.json'
import { parseSearchQuery } from './phraseSearch'
import { normalizeForPhraseMatch } from './textUtils'

test('every evaluation query reaches native search with its intended clauses', () => {
  for (const query of corpus.queries) {
    expect(query.mode, query.id).toBe('all')
    const parsed = parseSearchQuery(query.query)
    expect(parsed.unmatchedQuote, query.id).toBe(false)
    expect(parsed.unquotedText.toLowerCase(), query.id).toBe(query.nativeQuery)
    expect(parsed.exactPhrases.map(normalizeForPhraseMatch), query.id).toEqual(query.phrases ?? [])
  }
})
