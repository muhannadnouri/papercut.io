// @vitest-environment jsdom
import {
  afterEach,
  describe,
  expect,
  it,
} from 'vitest'
import {
  buildReadableDomSegmentIndex,
  createRangeForSourceSpan,
  normalizeDomTextPoint,
} from './domTextSegments'
import { normalizeSegmentText } from './readableSegments'

afterEach(() => {
  document.body.replaceChildren()
})

function fixture(html: string) {
  document.body.innerHTML = `<article>${html}</article>`
  return buildReadableDomSegmentIndex(document.querySelector('article')!)
}

describe('inverse narration DOM points', () => {
  it('round-trips every narrated character through the existing source-span mapper', () => {
    const index = fixture('<p>  One <b>two</b>\n three&nbsp;😀.</p><p dir="rtl">بِدايَةٌ <em>واضحةٌ</em>.</p>')
    index.segments.forEach((segment, segmentIndex) => {
      const text = normalizeSegmentText(segment.textNodes.map((node) => node.data).join(''))
      for (let offset = 0; offset < text.length; offset++) {
        if (/\s/.test(text[offset])) {
          continue
        }
        const range = createRangeForSourceSpan(document, index, {
          startSegmentIndex: segmentIndex,
          endSegmentIndex: segmentIndex,
          startOffset: offset,
          endOffset: offset + 1,
        })!
        const point = normalizeDomTextPoint(segment.textNodes, range.startContainer as Text, range.startOffset)
        expect(point?.normalizedOffset).toBe(offset)
        expect(point?.node).toBe(range.startContainer)
        expect(point?.offset).toBe(range.startOffset)
      }
    })
  })

  it('advances whitespace and inline node boundaries within the same narration segment', () => {
    const segment = fixture('<p> \t One \n <b>two</b>   </p>').segments[0]
    const [first, second, last] = segment.textNodes
    expect(normalizeDomTextPoint(segment.textNodes, first, 0)?.normalizedOffset).toBe(0)
    expect(normalizeDomTextPoint(segment.textNodes, first, first.data.indexOf('One') + 3))
      .toEqual({
        node: second,
        offset: 0,
        normalizedOffset: 4,
      })
    expect(normalizeDomTextPoint(segment.textNodes, first, first.length)?.normalizedOffset).toBe(4)
    expect(normalizeDomTextPoint(segment.textNodes, last, 0)).toBeNull()
    expect(normalizeDomTextPoint(segment.textNodes, last, last.length)).toBeNull()
  })

  it('keeps excluded footnote references out of source offsets', () => {
    const segment = fixture('<p>Alpha<a href="#note">[1]</a> <mark>beta</mark>.</p>').segments[0]
    const excluded = document.querySelector('a')!.firstChild as Text
    expect(normalizeDomTextPoint(segment.textNodes, excluded, 0)).toBeNull()
    const beta = document.querySelector('mark')!.firstChild as Text
    expect(normalizeDomTextPoint(segment.textNodes, beta, 0)?.normalizedOffset).toBe(6)
  })

  it.each([-1, 0.5, NaN, Infinity, 999])('rejects invalid raw offsets: %s', (offset) => {
    const segment = fixture('<p>One</p>').segments[0]
    expect(normalizeDomTextPoint(segment.textNodes, segment.textNodes[0], offset)).toBeNull()
  })
})
