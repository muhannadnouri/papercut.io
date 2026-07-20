import { describe, expect, it } from 'vitest'
import { segmentWordTokens } from './wordTiming'

function tokenTexts(text: string): string[] {
  return segmentWordTokens(text).map((token) => text.slice(token.startOffset, token.endOffset))
}

describe('word timing helpers', () => {
  it('keeps offsets aligned for Latin text and punctuation', () => {
    expect(tokenTexts('Hello, world!')).toEqual(['Hello', 'world'])
  })

  it('keeps offsets aligned for Arabic text with diacritics', () => {
    expect(tokenTexts('مَرْحَبًا بالعالم')).toEqual(['مَرْحَبًا', 'بالعالم'])
  })

  it('ignores whitespace and punctuation-only chunks', () => {
    expect(segmentWordTokens('...   ')).toEqual([])
  })
})
