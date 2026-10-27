import {
  describe,
  expect,
  it,
} from 'vitest'
import { findTextPartMatches } from './readerTextRanges'

describe('reader text ranges', () => {
  it('matches one phrase across formatted text nodes and collapsed whitespace', () => {
    expect(findTextPartMatches(['The  ', 'quick', '\n fox'], 'the quick fox')).toEqual([[
      {
        partIndex: 0,
        startOffset: 0,
        endOffset: 5,
      },
      {
        partIndex: 1,
        startOffset: 0,
        endOffset: 5,
      },
      {
        partIndex: 2,
        startOffset: 0,
        endOffset: 5,
      },
    ]])
  })

  it('excludes surrounding paragraph text from a cross-node match', () => {
    expect(findTextPartMatches(['Before that ', 'will happen after'], 'that will')).toEqual([[
      {
        partIndex: 0,
        startOffset: 7,
        endOffset: 12,
      },
      {
        partIndex: 1,
        startOffset: 0,
        endOffset: 4,
      },
    ]])
  })

  it('matches a PDF phrase split across positioned text items', () => {
    expect(findTextPartMatches(['A late', ' page ', 'result'], 'late page result')).toEqual([[
      {
        partIndex: 0,
        startOffset: 2,
        endOffset: 6,
      },
      {
        partIndex: 1,
        startOffset: 0,
        endOffset: 6,
      },
      {
        partIndex: 2,
        startOffset: 0,
        endOffset: 6,
      },
    ]])
  })

  it('treats an explicit line break as whitespace, not joined text', () => {
    const parts = ['foo', 'bar']
    const breaksBefore = [false, true]

    expect(findTextPartMatches(parts, 'foo bar', Infinity, breaksBefore)).toEqual([[
      {
        partIndex: 0,
        startOffset: 0,
        endOffset: 3,
      },
      {
        partIndex: 1,
        startOffset: 0,
        endOffset: 3,
      },
    ]])
    expect(findTextPartMatches(parts, 'foobar', Infinity, breaksBefore)).toEqual([])
  })

  it('treats straight, en, and em dashes as the same visible punctuation', () => {
    for (const query of ['well-being', 'well–being', 'well—being']) {
      expect(findTextPartMatches(['“Well—being”'], `"${query}"`)).toEqual([[
        {
          partIndex: 0,
          startOffset: 0,
          endOffset: 12,
        },
      ]])
    }
  })

  it('keeps common-query matching bounded when a caller supplies a limit', () => {
    expect(findTextPartMatches(['a'.repeat(10_000)], 'a', 3)).toEqual([
      [{
        partIndex: 0,
        startOffset: 0,
        endOffset: 1,
      }],
      [{
        partIndex: 0,
        startOffset: 1,
        endOffset: 2,
      }],
      [{
        partIndex: 0,
        startOffset: 2,
        endOffset: 3,
      }],
    ])
  })

  it.each(['-', '\u2010', '\u2011'])('maps a PDF %s wrap back to the original nodes and UTF-16 offsets', (hyphen) => {
    const parts = [`😀 High${hyphen}`, 'lights guide ', `fer${hyphen}`, 'ries. Later high‐', 'lights.']
    const breaks = [false, true, false, true, true]
    expect(findTextPartMatches(parts, 'highlights guide ferries', Infinity, breaks, true)).toEqual([[
      {
        partIndex: 0,
        startOffset: 3,
        endOffset: 8,
      },
      {
        partIndex: 1,
        startOffset: 0,
        endOffset: 13,
      },
      {
        partIndex: 2,
        startOffset: 0,
        endOffset: 4,
      },
      {
        partIndex: 3,
        startOffset: 0,
        endOffset: 4,
      },
    ]])
    expect(findTextPartMatches(parts, 'highlights', Infinity, breaks, true)).toEqual([
      [{
        partIndex: 0,
        startOffset: 3,
        endOffset: 8,
      }, {
        partIndex: 1,
        startOffset: 0,
        endOffset: 6,
      }],
      [{
        partIndex: 3,
        startOffset: 12,
        endOffset: 17,
      }, {
        partIndex: 4,
        startOffset: 0,
        endOffset: 6,
      }],
    ])
    expect(findTextPartMatches(parts, 'highlights', 1, breaks, true)).toHaveLength(1)
    expect(findTextPartMatches(parts, 'highlights', Infinity, breaks)).toEqual([])
  })

  it('preserves same-line compounds and non-continuing PDF lines', () => {
    for (const [before, after, hasBreak] of [
      ['high‐', 'lights', false], ['high—', 'lights', true],
      ['high–', 'lights', true], ['high‐', 'Lights', true],
    ] as const) {
      expect(findTextPartMatches([before, after], 'highlights', Infinity, [false, hasBreak], true)).toEqual([])
    }
    expect(findTextPartMatches(['state‐owned ferries'], 'state-owned', Infinity, [], true)).toEqual([[
      {
        partIndex: 0,
        startOffset: 0,
        endOffset: 11,
      },
    ]])
    expect(findTextPartMatches(['foo', 'bar'], 'foobar', Infinity, [false, true], true)).toEqual([])
  })

  it('maps wraps inside a text node and across a separate hyphen node', () => {
    expect(findTextPartMatches(['HTTP‐', 'Server'], 'HTTPServer', Infinity, [false, true], true)).toEqual([[
      {
        partIndex: 0,
        startOffset: 0,
        endOffset: 5,
      },
      {
        partIndex: 1,
        startOffset: 0,
        endOffset: 6,
      },
    ]])
    expect(findTextPartMatches(['😀 high‐\nlights'], 'highlights', Infinity, [], true)).toEqual([[
      {
        partIndex: 0,
        startOffset: 3,
        endOffset: 15,
      },
    ]])
    expect(findTextPartMatches(['high', '‐', 'lights'], 'highlights', Infinity, [false, false, true], true)).toEqual([[
      {
        partIndex: 0,
        startOffset: 0,
        endOffset: 4,
      },
      {
        partIndex: 1,
        startOffset: 0,
        endOffset: 1,
      },
      {
        partIndex: 2,
        startOffset: 0,
        endOffset: 6,
      },
    ]])
  })
})
