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
})
