import { describe, expect, it } from 'vitest'
import { findTextPartMatches } from './readerTextRanges'

describe('reader text ranges', () => {
  it('matches one phrase across formatted text nodes and collapsed whitespace', () => {
    expect(findTextPartMatches(['The  ', 'quick', '\n fox'], 'the quick fox')).toEqual([[
      { partIndex: 0, startOffset: 0, endOffset: 5 },
      { partIndex: 1, startOffset: 0, endOffset: 5 },
      { partIndex: 2, startOffset: 0, endOffset: 5 },
    ]])
  })

  it('excludes surrounding paragraph text from a cross-node match', () => {
    expect(findTextPartMatches(['Before that ', 'will happen after'], 'that will')).toEqual([[
      { partIndex: 0, startOffset: 7, endOffset: 12 },
      { partIndex: 1, startOffset: 0, endOffset: 4 },
    ]])
  })

  it('matches a PDF phrase split across positioned text items', () => {
    expect(findTextPartMatches(['A late', ' page ', 'result'], 'late page result')).toEqual([[
      { partIndex: 0, startOffset: 2, endOffset: 6 },
      { partIndex: 1, startOffset: 0, endOffset: 6 },
      { partIndex: 2, startOffset: 0, endOffset: 6 },
    ]])
  })

  it('treats an explicit line break as whitespace, not joined text', () => {
    const parts = ['foo', 'bar']
    const breaksBefore = [false, true]

    expect(findTextPartMatches(parts, 'foo bar', Infinity, breaksBefore)).toEqual([[
      { partIndex: 0, startOffset: 0, endOffset: 3 },
      { partIndex: 1, startOffset: 0, endOffset: 3 },
    ]])
    expect(findTextPartMatches(parts, 'foobar', Infinity, breaksBefore)).toEqual([])
  })

  it('treats straight, en, and em dashes as the same visible punctuation', () => {
    for (const query of ['well-being', 'well–being', 'well—being']) {
      expect(findTextPartMatches(['“Well—being”'], `"${query}"`)).toEqual([[
        { partIndex: 0, startOffset: 0, endOffset: 12 },
      ]])
    }
  })

  it('keeps common-query matching bounded when a caller supplies a limit', () => {
    expect(findTextPartMatches(['a'.repeat(10_000)], 'a', 3)).toEqual([
      [{ partIndex: 0, startOffset: 0, endOffset: 1 }],
      [{ partIndex: 0, startOffset: 1, endOffset: 2 }],
      [{ partIndex: 0, startOffset: 2, endOffset: 3 }],
    ])
  })
})
