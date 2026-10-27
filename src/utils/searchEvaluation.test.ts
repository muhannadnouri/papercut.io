/// <reference types="node" />
import {
  expect,
  test,
} from 'vitest'
import corpus from '../../scripts/fixtures/search-v2/corpus.json'
import { parseSearchQuery } from './phraseSearch'
import { normalizeForPhraseMatch } from './textUtils'
import {
  readFileSync,
  writeFileSync,
} from 'node:fs'

test('every evaluation query reaches native search with its intended clauses', () => {
  for (const query of corpus.queries) {
    expect(['all', 'broader'], query.id).toContain(query.mode)
    const parsed = parseSearchQuery(query.query)
    expect(parsed.unmatchedQuote, query.id).toBe(false)
    expect(parsed.unquotedText.toLowerCase(), query.id).toBe(query.nativeQuery)
    expect(parsed.exactPhrases.map(normalizeForPhraseMatch), query.id).toEqual(query.phrases ?? [])
  }
})

test.skipIf(!process.env.PAPERCUT_SEARCH_EVAL_QUERIES)('prepares private evaluation requests with the production query parser', () => {
  const input = process.env.PAPERCUT_SEARCH_EVAL_QUERIES
  if (!input) {
    throw new Error('missing private query input path')
  }
  const output = process.env.PAPERCUT_SEARCH_EVAL_REQUESTS
  if (!output) {
    throw new Error('missing private request output path')
  }
  const queries: unknown = JSON.parse(readFileSync(input, 'utf8'))
  if (!Array.isArray(queries) || queries.length === 0) {
    throw new Error('expected nonempty query array')
  }
  const seen = new Set<string>()
  const requests = queries.map((query) => {
    if (!query || typeof query.id !== 'string' || typeof query.query !== 'string' ||
      !query.id.trim() || seen.has(query.id) || !['all', 'broader'].includes(query.mode ?? 'all') ||
      (query.documentUrls !== undefined && (!Array.isArray(query.documentUrls) || !query.documentUrls.length ||
        !query.documentUrls.every((url: unknown) => typeof url === 'string' && url.trim())))) {
      throw new Error('invalid or duplicate private query')
    }
    seen.add(query.id)
    const parsed = parseSearchQuery(query.query)
    if (parsed.unmatchedQuote || !parsed.providerQuery) {
      throw new Error('invalid private query syntax')
    }
    return {
      id: query.id,
      request: {
        query: parsed.unquotedText,
        exactPhrases: parsed.exactPhrases.map(normalizeForPhraseMatch),
        mode: query.mode ?? 'all',
        documentUrls: query.documentUrls,
        limit: query.documentUrls ? 100 : 50,
      },
    }
  })
  writeFileSync(output, JSON.stringify(requests))
})
