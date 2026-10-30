// @vitest-environment jsdom
import {
  describe,
  expect,
  it,
} from 'vitest'
import {
  verifiedHtmlResolver,
  verifiedPdfResolver,
} from './verifiedSource'
import {
  buildTextLocatorIndex,
  createSourceSpanFromTextMatch,
  locatorTextsMatch,
  recoverOrderedSourceSpans,
} from '../alignment/domTextSegments'
import type { TtsChunk } from '../types'
import {
  chunkAudiobookSaveHtmlWithSpans,
  chunkAudiobookSaveSegmentsWithSpans,
} from '../utils/text'
import {
  pdfNarrationToReadableSegments,
  pdfSourceSpansForChunk,
} from '../../pdf/pdfTts'

const span = (start: number, end: number) => ({
  startSegmentIndex: 0,
  endSegmentIndex: 0,
  startOffset: start,
  endOffset: end,
})

describe('verified source lookup', () => {
  it('validates every span against actual text and refuses same-length replacements, gaps and overlaps', () => {
    const chunks: TtsChunk[] = [{
      id: 'first',
      text: 'One.',
      sourceSpan: span(0, 4),
    }, {
      id: 'second',
      text: 'Two.',
      sourceSpan: span(5, 9),
    }]
    const resolver = verifiedHtmlResolver(chunks, ['One. Two.'], false)
    expect(resolver.status).toBe('ready')
    if (resolver.status === 'ready') {
      expect(resolver.resolve({
        segmentIndex: 0,
        offset: 6,
      })).toEqual({
        status: 'resolved',
        chunkIndex: 1,
        chunkId: 'second',
      })
    }
    expect(verifiedHtmlResolver(chunks, ['One. Bad.'], false).status).toBe('ready')
    // The unverifiable span degrades to a localized gap: its own text cannot
    // start audio, but the rest of the book keeps playing.
    const degraded = verifiedHtmlResolver(chunks, ['One. Bad.'], false)
    if (degraded.status === 'ready') {
      expect(degraded.resolve({
        segmentIndex: 0,
        offset: 1,
      })).toEqual({
        status: 'resolved',
        chunkIndex: 0,
        chunkId: 'first',
      })
      expect(degraded.resolve({
        segmentIndex: 0,
        offset: 6,
      })).toEqual({
        status: 'unavailable',
        reason: 'unmapped',
      })
    }
    expect(verifiedHtmlResolver([chunks[1]], ['One. Two.'], false).status).toBe('unavailable')
    expect(verifiedHtmlResolver([chunks[0], {
      ...chunks[1],
      sourceSpan: span(0, 4),
    }], ['One. Two.'], false)).toMatchObject({ reason: 'ambiguous' })
  })

  it('reuses ordered recovery for repeated legacy phrases while preserving all authoritative metadata', () => {
    const chunks: TtsChunk[] = [{
      id: 'old-a',
      text: 'Repeat.',
      textHash: 'keep-a',
    }, {
      id: 'empty',
      text: ' ',
    }, {
      id: 'old-b',
      text: 'Repeat.',
      textHash: 'keep-b',
    }]
    const original = structuredClone(chunks)
    const resolver = verifiedHtmlResolver(chunks, ['Repeat.', 'Repeat.'], true)
    expect(resolver.status).toBe('ready')
    if (resolver.status === 'ready') {
      expect(resolver.resolve({
        segmentIndex: 1,
        offset: 2,
      })).toEqual({
        status: 'resolved',
        chunkIndex: 1,
        chunkId: 'old-b',
      })
    }
    expect(chunks).toEqual(original)
    expect(verifiedHtmlResolver(chunks, ['Repeat.', 'Repeat.', 'Repeat.'], true).status).toBe('unavailable')
    expect(verifiedHtmlResolver(chunks, ['Repeat.'], true)).toMatchObject({ reason: 'not-ready' })
    expect(verifiedHtmlResolver(chunks, ['Repeat.', 'Other.', 'Repeat.'], true).status).toBe('unavailable')
    expect(verifiedHtmlResolver(chunks, ['Repeat.', 'Repeat.'], false)).toMatchObject({ reason: 'not-ready' })
  })

  it('recovers stale locators only for allowed legacy bundles', () => {
    const chunks = [{
      id: 'a',
      text: 'First.',
      sourceSpan: span(99, 100),
    }, {
      id: 'b',
      text: 'Second.',
      sourceSpan: span(0, 1),
    }]
    expect(verifiedHtmlResolver(chunks, ['First.', 'Second.'], true).status).toBe('ready')
    expect(verifiedHtmlResolver(chunks, ['First.', 'Second.'], false).status).toBe('unavailable')
  })

  it('validates actual production decimal, initialism and supplementary Unicode chunk spans', () => {
    const html = '<p>😀 A value of 3.14 with U.S.A. initials. Another sentence.</p><h2>End</h2>'
    const chunks = chunkAudiobookSaveHtmlWithSpans(html).map((chunk, i) => ({
      ...chunk,
      id: String(i),
    }))
    expect(verifiedHtmlResolver(chunks, ['😀 A value of 3.14 with U.S.A. initials. Another sentence.', 'End'], false).status).toBe('ready')
    expect(verifiedHtmlResolver(chunks.map(({ id, text }) => ({
      id,
      text,
    })), ['😀 A value of 3.14 with U.S.A. initials. Another sentence.', 'End'], true).status).toBe('ready')
    expect(locatorTextsMatch('another', 'an other')).toBe(false)
  })

  it.each([',', ';', ':', '،', '؛', '，', '；', '：', '、'])('accepts production clause spacing after %s without changing imported chunks', separator => {
    const text = 'A narrated clause with repeated words '.repeat(4).trim() + separator +
      '“Another clause follows without a source space”' + separator + ' ' +
      'A final long clause keeps the sentence above the splitting limit '.repeat(8).trim() + '.'
    const chunks = chunkAudiobookSaveHtmlWithSpans(`<p>${text}</p>`).map((chunk, i) => ({
      ...chunk,
      id: 'saved-' + i,
      textHash: 'original-' + i,
    }))
    expect(chunks.some(chunk => chunk.text.includes(separator + ' “'))).toBe(true)
    const original = structuredClone(chunks)
    for (const mapped of [chunks, chunks.map(({ id, text, textHash }) => ({
      id,
      text,
      textHash,
    }))]) {
      const resolver = verifiedHtmlResolver(mapped, [text], mapped !== chunks)
      expect(resolver.status).toBe('ready')
      if (resolver.status === 'ready') {
        expect(resolver.resolve({
          segmentIndex: 0,
          offset: text.indexOf('Another'),
        }))
          .toMatchObject({
            status: 'resolved',
            chunkIndex: 0,
            chunkId: 'saved-0',
          })
      }
    }
    expect(chunks).toEqual(original)
    expect(locatorTextsMatch('another', 'an other')).toBe(false)
    expect(locatorTextsMatch('one,two', 'one two')).toBe(false)
    expect(locatorTextsMatch('one-two', 'one- two')).toBe(false)
    expect(locatorTextsMatch('Keep,all words.', 'Keep, other words.')).toBe(false)
    // The one stale span becomes a localized gap; the same-length replacement
    // keeps every later span aligned, so only the changed word cannot play.
    const driftedText = text.replace('Another', 'Changed')
    const drifted = verifiedHtmlResolver(chunks, [driftedText], true)
    expect(drifted.status).toBe('ready')
    if (drifted.status === 'ready') {
      expect(drifted.resolve({
        segmentIndex: 0,
        offset: driftedText.indexOf('Changed'),
      })).toEqual({
        status: 'unavailable',
        reason: 'unmapped',
      })
    }
  })

  it.each(['.', '?', '!', '؟', '。', '！', '？', '।', '॥'])('matches production sentence punctuation after a closer: %s', terminator => {
    for (const suffix of [' Next passage continues.', '']) {
      const text = `A saved parenthetical (with an abbreviation${terminator})${terminator}${suffix}`
      const chunks = chunkAudiobookSaveHtmlWithSpans(`<p>${text}</p>`).map((chunk, i) => ({
        ...chunk,
        id: 'saved-' + i,
      }))
      expect(chunks[0].text).not.toContain(')' + terminator)
      const original = structuredClone(chunks)
      for (const mapped of [chunks, chunks.map(({ id, text }) => ({
        id,
        text,
      }))]) {
        const resolver = verifiedHtmlResolver(mapped, [text], mapped !== chunks)
        expect(resolver.status).toBe('ready')
        if (resolver.status === 'ready') {
          expect(resolver.resolve({
            segmentIndex: 0,
            offset: text.indexOf('abbreviation'),
          }))
            .toMatchObject({
              status: 'resolved',
              chunkId: 'saved-0',
            })
          if (!suffix) {
            expect(resolver.resolve({
              segmentIndex: 0,
              offset: text.length - 1,
            })).toMatchObject({ reason: 'unmapped' })
          }
        }
      }
      expect(chunks).toEqual(original)
    }
    expect(locatorTextsMatch('Same.) .', 'Same.)')).toBe(false)
    expect(locatorTextsMatch('Same. ).', 'Same. )')).toBe(false)
    expect(locatorTextsMatch('Value 1.2.', 'Value 12.')).toBe(false)
    expect(locatorTextsMatch('Same.) Next.', 'Same.) Other.')).toBe(false)
  })

  it.each([')', ']', '"', '”', '’', '»', '）', '】', '」', '』', '》'])('keeps UTF-16 source offsets through consumed closer %s', closer => {
    const text = `😀 Abbreviation.${closer}. Next phrase.`
    const chunks = chunkAudiobookSaveHtmlWithSpans(`<p>${text}</p>`).map(({ text }, i) => ({
      id: 'saved-' + i,
      text,
    }))
    const resolver = verifiedHtmlResolver(chunks, [text], true)
    expect(resolver.status).toBe('ready')
    if (resolver.status === 'ready') {
      expect(resolver.resolve({
        segmentIndex: 0,
        offset: text.indexOf('Next'),
      }))
        .toMatchObject({
          status: 'resolved',
          chunkId: 'saved-0',
        })
    }
  })

  it.each(['\u200f', '\u0640', '\u064e'])('does not invent adjacent sentence punctuation through display mark %j', mark => {
    for (const text of [`A.${mark}). Next words.`, `A.)${mark}. Next words.`]) {
      const chunks = chunkAudiobookSaveHtmlWithSpans(`<p>${text}</p>`).map((chunk, i) => ({
        ...chunk,
        id: 'saved-' + i,
      }))
      expect(verifiedHtmlResolver(chunks, [text], false).status).toBe('ready')
      expect(verifiedHtmlResolver(chunks.map(({ id, text }) => ({
        id,
        text,
      })), [text], true).status).toBe('ready')
    }
  })

  it('keeps exact-first matching and UTF-16 offsets through Arabic marks, NFKC expansions and emoji', () => {
    const texts = ['😀 بِدايَةٌ واضحةٌ.', 'ﻻ 😀', '𝔸 😀']
    const locator = buildTextLocatorIndex(texts)
    const spans = recoverOrderedSourceSpans(locator, [{
      id: 'a',
      text: '😀 بداية واضحة.',
    }, {
      id: 'b',
      text: 'لا 😀',
    }, {
      id: 'c',
      text: 'A 😀',
    }])
    expect([...spans.values()]).toEqual(texts.map((text, i) => ({
      startSegmentIndex: i,
      endSegmentIndex: i,
      startOffset: 0,
      endOffset: text.length,
    })))
    const repeated = buildTextLocatorIndex(['بِداية بداية'])
    expect(createSourceSpanFromTextMatch(repeated, 'بداية', 0)?.startOffset).toBe(7)
  })

  it('checks PDF narration provenance and source projection including one glyph shared by adjacent chunks', () => {
    const segments = pdfNarrationToReadableSegments([{
      text: 'fi',
      sourceRuns: [{
        pageIndex: 0,
        blockOrder: 4,
        startOffset: 0,
        endOffset: 2,
        sourceStartOffset: 0,
        sourceEndOffset: 1,
      }],
    }])
    const chunks = ['f', 'i'].map((text, i) => ({
      id: String(i),
      text,
      sourceSpan: span(i, i + 1),
      pdfSourceSpans: pdfSourceSpansForChunk(segments, span(i, i + 1)),
    }))
    const resolver = verifiedPdfResolver(chunks, segments)
    expect(resolver.status).toBe('ready')
    if (resolver.status === 'ready') {
      expect(resolver.resolve({
        pageIndex: 0,
        blockOrder: 4,
        offset: 0,
      }, 1)).toEqual({
        status: 'resolved',
        chunkIndex: 0,
        chunkId: '0',
      })
      expect(resolver.resolve({
        pageIndex: 0,
        blockOrder: 4,
        offset: 0,
      }, 0)).toMatchObject({ reason: 'invalid-point' })
    }
    expect(verifiedPdfResolver([{
      ...chunks[0],
      pdfSourceSpans: [{
        pageIndex: 0,
        blockOrder: 5,
        startOffset: 0,
        endOffset: 1,
      }],
    }, chunks[1]], segments)).toMatchObject({ reason: 'invalid-source' })
    expect(verifiedPdfResolver(chunks, [{
      ...segments[0],
      sourceRuns: [{
        ...segments[0].sourceRuns[0],
        endOffset: 999,
      }],
    }])).toMatchObject({ reason: 'invalid-source' })
    expect(verifiedPdfResolver(chunks, [{
      ...segments[0],
      sourceRuns: [],
    }])).toMatchObject({ reason: 'invalid-source' })
  })
})

it('never treats the end-of-index sentinel as permission to discard independent final punctuation', () => {
  expect(verifiedHtmlResolver([{
    id: 'saved',
    text: 'First',
    sourceSpan: {
      startSegmentIndex: 0,
      endSegmentIndex: 0,
      startOffset: 0,
      endOffset: 5,
    },
  }], ['First.'], false).status).toBe('unavailable')
})

it('drops one unverifiable span without cascading into its neighbours', () => {
  const texts = ['Alpha passage. Beta passage. Gamma passage.']
  const chunks: TtsChunk[] = [{
    id: 'first',
    text: 'Alpha passage.',
    sourceSpan: span(0, 14),
  }, {
    id: 'second',
    text: 'Stale narration.',
    sourceSpan: span(15, 28),
  }, {
    id: 'third',
    text: 'Gamma passage.',
    sourceSpan: span(29, 43),
  }]
  const resolver = verifiedHtmlResolver(chunks, texts, false)
  expect(resolver.status).toBe('ready')
  if (resolver.status === 'ready') {
    expect(resolver.resolve({
      segmentIndex: 0,
      offset: 2,
    })).toEqual({
      status: 'resolved',
      chunkIndex: 0,
      chunkId: 'first',
    })
    // The dropped chunk's own range never guesses audio.
    expect(resolver.resolve({
      segmentIndex: 0,
      offset: 20,
    })).toEqual({
      status: 'unavailable',
      reason: 'unmapped',
    })
    // The cursor advanced past the dropped span, so the chunk after it stays playable.
    expect(resolver.resolve({
      segmentIndex: 0,
      offset: 33,
    })).toEqual({
      status: 'resolved',
      chunkIndex: 1,
      chunkId: 'third',
    })
  }
  // Fully stale spans still fail: nothing left to play from.
  expect(verifiedHtmlResolver(chunks, ['Nothing narrated here matches.'], false))
    .toEqual({
      status: 'unavailable',
      reason: 'not-ready',
    })
})

it('verifies merged PDF paragraphs using generation grammar at each source boundary', () => {
  const texts = ['Reader handbook', '! Chapter overview', 'This passage explains the first topic.']
  const segments = pdfNarrationToReadableSegments(texts.map((text, blockOrder) => ({
    text,
    sourceRuns: [{
      pageIndex: 0,
      blockOrder,
      startOffset: 0,
      endOffset: text.length,
      sourceStartOffset: 0,
      sourceEndOffset: text.length,
    }],
  })))
  const chunks = chunkAudiobookSaveSegmentsWithSpans(segments).map((chunk, index) => ({
    ...chunk,
    id: `saved-${index}`,
    pdfSourceSpans: pdfSourceSpansForChunk(segments, chunk.sourceSpan),
  }))
  expect(chunks).toHaveLength(1)
  expect(chunks[0].text).not.toContain('!')
  const original = structuredClone(chunks)
  // Exact source text remains valid even when a prior producer retained the leading glyph.
  expect(verifiedPdfResolver([{
    ...chunks[0],
    text: texts.join(' '),
  }], segments).status).toBe('ready')
  expect(verifiedHtmlResolver(chunks, texts, false).status).toBe('ready')
  const resolver = verifiedPdfResolver(chunks, segments)
  expect(resolver.status).toBe('ready')
  if (resolver.status === 'ready') {
    for (let blockOrder = 0; blockOrder < texts.length; blockOrder++) {
      expect(resolver.resolve({
        pageIndex: 0,
        blockOrder,
        offset: blockOrder === 1 ? 2 : 0,
      }, texts[blockOrder].length))
        .toEqual({
          status: 'resolved',
          chunkIndex: 0,
          chunkId: 'saved-0',
        })
    }
  }
  expect(chunks).toEqual(original)
  expect(verifiedPdfResolver([{
    ...chunks[0],
    text: chunks[0].text.replace('first', 'wrong'),
  }], segments).status).toBe('unavailable')
  const source = 'Reader handbook ! Chapter overview'
  const withinParagraph = [{
    id: 'saved',
    text: 'Reader handbook Chapter overview',
    sourceSpan: span(0, source.length),
  }]
  expect(verifiedHtmlResolver(withinParagraph, [source], false).status).toBe('unavailable')
})
