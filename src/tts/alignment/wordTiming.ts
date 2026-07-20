export interface WordToken {
  startOffset: number
  endOffset: number
}

interface IntlSegment {
  segment: string
  index: number
  isWordLike?: boolean
}

type IntlSegmenterCtor = new (
  locale?: string | string[],
  options?: { granularity: 'word' },
) => { segment(input: string): Iterable<IntlSegment> }

/** Segment multilingual text while retaining offsets into the original chunk. */
export function segmentWordTokens(text: string): WordToken[] {
  const Segmenter = (Intl as typeof Intl & { Segmenter?: IntlSegmenterCtor }).Segmenter
  if (Segmenter) {
    const segmenter = new Segmenter(undefined, { granularity: 'word' })
    return Array.from(segmenter.segment(text))
      .filter((part) => part.isWordLike)
      .map((part) => ({
        startOffset: part.index,
        endOffset: part.index + part.segment.length,
      }))
  }

  const tokens: WordToken[] = []
  const wordPattern = /[\p{L}\p{N}_]+(?:[-'][\p{L}\p{N}_]+)*/gu
  for (const match of text.matchAll(wordPattern)) {
    if (match.index === undefined) continue
    tokens.push({
      startOffset: match.index,
      endOffset: match.index + match[0].length,
    })
  }
  return tokens
}
