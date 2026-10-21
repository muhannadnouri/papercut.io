// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { findReaderTextMatches } from '../../components/DocumentViewer/readerTextRanges'
import { chunkAudiobookSaveHtmlWithSpans, extractReadableTextFromHtml } from '../utils/text'
import { buildReadableDomSegmentIndex, buildReadableDomTextLocatorIndex } from './domTextSegments'
import { highlightTtsChunk, highlightTtsWord, type AlignmentCache, type SegmentIndexCache } from './highlightRanges'
import { segmentWordTokens } from './wordTiming'

vi.mock('../diagnostics/TtsDiagnostics', () => ({ logTtsDiagnostic: vi.fn() }))

const original = '<p data-papercut-translation-review="original" lang="es"><span data-papercut-translation-review="label">Original — formatting retained<br></span>Una <strong>casa</strong> roja.<a href="#note">[1]</a></p>'
const label = '<p data-papercut-translation-review="label">Translation — formatting needs review</p>'
// Generated HTML has no whitespace between sibling blocks; narration still needs a word separator.
const reviewHtml = original + label + '<p>A red house.</p>' + original + label + '<p>I do not know.</p>'
let root: HTMLDivElement

beforeEach(() => {
  root = document.createElement('div')
  document.body.append(root)
  vi.stubGlobal('Highlight', class extends Set<Range> {})
  vi.stubGlobal('CSS', { highlights: new Map<string, Set<Range>>() })
  Object.defineProperty(Range.prototype, 'getBoundingClientRect', {
    configurable: true, value: () => ({ top: 0, height: 1 }),
  })
})
afterEach(() => {
  root.remove()
  document.getElementById('tts-current-styles')?.remove()
  Reflect.deleteProperty(Range.prototype, 'getBoundingClientRect')
  vi.unstubAllGlobals()
})

describe('retained translation originals stay outside the narrated and indexed text', () => {
  it('agrees across narration, DOM locators and reader search while keeping source links and styles', () => {
    root.innerHTML = reviewHtml
    expect(extractReadableTextFromHtml(root.innerHTML)).toBe('A red house.\n\nI do not know.')
    const locator = buildReadableDomTextLocatorIndex(buildReadableDomSegmentIndex(root))
    expect(locator.text).toBe('A red house. I do not know.')
    expect(findReaderTextMatches(root, 'red house')).toHaveLength(1)
    expect(findReaderTextMatches(root, 'casa')).toHaveLength(0)
    expect(findReaderTextMatches(root, 'formatting needs review')).toHaveLength(0)
    expect(root.querySelector('[data-papercut-translation-review="original"] strong')?.textContent).toBe('casa')
    expect(root.querySelector('[data-papercut-translation-review="original"] a')?.getAttribute('href')).toBe('#note')
  })

  it.each([
    ['review', reviewHtml],
    ['ordinary', '<p>A <strong>red</strong> house.</p><p>I do <em>not</em> know.</p>'],
  ])('keeps %s cross-block chunk and word highlights on the narrated text', (_name, html) => {
    root.innerHTML = html
    const before = root.innerHTML
    const chunks = chunkAudiobookSaveHtmlWithSpans(html, { minChunkLength: 1000, maxChunkLength: 2000 })
      .map((chunk, index) => ({ ...chunk, id: String(index) }))
    expect(chunks).toHaveLength(1)
    expect(chunks[0].text).toBe('A red house. I do not know.')
    const segments = { current: null as SegmentIndexCache | null }
    const alignment = { current: null as AlignmentCache | null }
    expect(highlightTtsChunk(root, 0, chunks, false, 0, segments, alignment)).not.toBeNull()
    const ranges = [...CSS.highlights.get('tts-current')!]
    expect(ranges.map(range => range.toString()).join(' ')).toContain('house.')
    for (const range of ranges) {
      expect(range.startContainer.parentElement?.closest('[data-papercut-translation-review]')).toBeNull()
      expect(range.endContainer.parentElement?.closest('[data-papercut-translation-review]')).toBeNull()
    }
    const tokens = segmentWordTokens(chunks[0].text)
    tokens.forEach((token, index) => {
      highlightTtsWord(root, 0, chunks, false, (index + 0.1) / tokens.length, 0, segments, alignment)
      const word = [...CSS.highlights.get('tts-current-word')!][0]
      expect(word?.toString()).toBe(chunks[0].text.slice(token.startOffset, token.endOffset))
      expect(word.startContainer.parentElement?.closest('[data-papercut-translation-review]')).toBeNull()
    })
    expect(root.innerHTML).toBe(before)
  })

  it('never falls back to narrating a document containing only review originals or labels', () => {
    root.innerHTML = original + label
    expect(extractReadableTextFromHtml(root.innerHTML)).toBe('')
    expect(buildReadableDomSegmentIndex(root).segments).toHaveLength(0)
    expect(findReaderTextMatches(root, 'casa')).toHaveLength(0)
    expect(root.querySelector('strong')?.textContent).toBe('casa')
  })
})
