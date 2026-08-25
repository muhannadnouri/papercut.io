import { isReadableHtmlBlock } from '../../tts/alignment/htmlStructure'
import {
  normalizeSearchCharacter,
  normalizeSearchPunctuation,
} from '../../utils/textUtils'

const READER_TEXT_SKIP_SELECTOR = 'script, style, noscript, svg'

export interface ReaderTextMatchPart {
  node: Text
  startOffset: number
  endOffset: number
}

export interface ReaderTextMatch {
  range: Range
  parts: ReaderTextMatchPart[]
}

export interface TextPartMatch {
  partIndex: number
  startOffset: number
  endOffset: number
}

interface SearchTextPoint {
  partIndex: number
  startOffset: number
  endOffset: number
}

interface ReaderTextSegment {
  nodes: Text[]
  breaksBefore: boolean[]
}

interface SearchTextIndex {
  text: string
  partIndexes: Uint32Array
  startOffsets: Uint32Array
  endOffsets: Uint32Array
}

interface IndexedReaderTextSegment {
  nodes: Text[]
  search: SearchTextIndex
}

export interface ReaderTextIndex {
  root: HTMLElement
  segments: IndexedReaderTextSegment[]
}

export interface ReaderTextIndexMatch {
  segmentIndex: number
  start: number
  end: number
}

// Match the reader's rendered text inside each readable block. Inline markup
// remains transparent, while block boundaries prevent unrelated paragraphs
// from becoming one accidental match.
export function findReaderTextMatches(
  root: HTMLElement,
  query: string,
  limit = Number.POSITIVE_INFINITY,
): ReaderTextMatch[] {
  const index = createReaderTextIndex(root)
  return findReaderTextIndexMatches(index, query, limit)
    .map((match) => materializeReaderTextMatch(index, match))
}

// Build the DOM-to-text map once per rendered document. Find can then search
// plain strings on each keystroke without repeatedly walking the reader DOM.
export function createReaderTextIndex(root: HTMLElement): ReaderTextIndex {
  return {
    root,
    segments: collectReaderTextSegments(root).map((segment) => ({
      nodes: segment.nodes,
      search: buildSearchText(
        segment.nodes.map((node) => node.data),
        segment.breaksBefore,
      ),
    })),
  }
}

export function findReaderTextIndexMatches(
  index: ReaderTextIndex,
  query: string,
  limit = Number.POSITIVE_INFINITY,
): ReaderTextIndexMatch[] {
  const needle = normalizeSearchQuery(query)
  if (!needle || limit <= 0) return []

  const matches: ReaderTextIndexMatch[] = []
  for (let segmentIndex = 0; segmentIndex < index.segments.length; segmentIndex++) {
    const segment = index.segments[segmentIndex]
    const remaining = limit - matches.length
    if (remaining <= 0) break

    for (const match of findSearchTextMatches(segment.search.text, needle, remaining)) {
      matches.push({ segmentIndex, ...match })
    }
  }
  return matches
}

export function rangeForReaderTextIndexMatch(
  index: ReaderTextIndex,
  match: ReaderTextIndexMatch,
): Range {
  const segment = index.segments[match.segmentIndex]
  const endIndex = match.end - 1
  const startPartIndex = segment.search.partIndexes[match.start]
  const endPartIndex = segment.search.partIndexes[endIndex]
  const startOffset = segment.search.startOffsets[match.start]
  const endOffset = segment.search.endOffsets[endIndex]
  const range = index.root.ownerDocument.createRange()

  if (
    startPartIndex === undefined || endPartIndex === undefined ||
    startOffset === undefined || endOffset === undefined
  ) {
    range.selectNodeContents(index.root)
    range.collapse(true)
    return range
  }

  range.setStart(segment.nodes[startPartIndex], startOffset)
  range.setEnd(segment.nodes[endPartIndex], endOffset)
  return range
}

// Keep the matching core independent of browser globals so node-boundary and
// whitespace behavior has a small, fast regression test.
export function findTextPartMatches(
  parts: readonly string[],
  query: string,
  limit = Number.POSITIVE_INFINITY,
  breaksBefore: readonly boolean[] = [],
): TextPartMatch[][] {
  const needle = normalizeSearchQuery(query)
  if (!needle || limit <= 0) return []

  const search = buildSearchText(parts, breaksBefore)
  return findSearchTextMatches(search.text, needle, limit)
    .map((match) => partsForIndexedMatch(parts, search, match))
}

function collectReaderTextSegments(root: HTMLElement): ReaderTextSegment[] {
  const segments: Array<ReaderTextSegment & { owner: Element }> = []
  const fallback: Text[] = []
  const fallbackBreaksBefore: boolean[] = []
  const walker = root.ownerDocument.createTreeWalker(
    root,
    NodeFilter.SHOW_ELEMENT | NodeFilter.SHOW_TEXT,
  )
  let pendingBreakOwner: Element | null = null
  let pendingFallbackBreak = false
  let current: Node | null

  while ((current = walker.nextNode())) {
    if (current instanceof Element) {
      if (
        current.tagName === 'BR'
        && !current.closest(READER_TEXT_SKIP_SELECTOR)
      ) {
        pendingBreakOwner = current.parentElement
          ? nearestReadableOwner(current.parentElement, root)
          : null
        pendingFallbackBreak = true
      }
      continue
    }

    const node = current as Text
    const parent = node.parentElement
    if (!parent || parent.closest(READER_TEXT_SKIP_SELECTOR)) continue
    fallback.push(node)
    fallbackBreaksBefore.push(pendingFallbackBreak)
    pendingFallbackBreak = false

    const owner = nearestReadableOwner(parent, root)
    if (!owner) continue
    const previous = segments[segments.length - 1]
    if (previous?.owner === owner) {
      previous.breaksBefore.push(pendingBreakOwner === owner)
      previous.nodes.push(node)
    } else {
      segments.push({ owner, nodes: [node], breaksBefore: [false] })
    }
    if (pendingBreakOwner === owner) pendingBreakOwner = null
  }

  const readable = segments
    .filter((segment) => segment.nodes.some((node) => /\S/.test(node.data)))
    .map(({ nodes, breaksBefore }) => ({ nodes, breaksBefore }))
  if (readable.length > 0) return readable
  return fallback.some((node) => /\S/.test(node.data))
    ? [{ nodes: fallback, breaksBefore: fallbackBreaksBefore }]
    : []
}

function nearestReadableOwner(element: Element, root: HTMLElement): Element | null {
  let current: Element | null = element
  while (current) {
    if (isReadableHtmlBlock(current)) return current
    if (current === root) return null
    current = current.parentElement
  }
  return null
}

function buildSearchText(
  parts: readonly string[],
  breaksBefore: readonly boolean[],
): SearchTextIndex {
  let text = ''
  const partIndexes: number[] = []
  const startOffsets: number[] = []
  const endOffsets: number[] = []
  let pendingWhitespace: SearchTextPoint | null = null

  const append = (value: string, point: SearchTextPoint) => {
    text += value
    for (let index = 0; index < value.length; index++) {
      partIndexes.push(point.partIndex)
      startOffsets.push(point.startOffset)
      endOffsets.push(point.endOffset)
    }
  }

  parts.forEach((part, partIndex) => {
    // A <br> has no Text node of its own. Map its searchable space to a
    // zero-width point; trimmed queries ensure it can never become a range end.
    if (breaksBefore[partIndex] && text && !pendingWhitespace) {
      pendingWhitespace = { partIndex, startOffset: 0, endOffset: 0 }
    }
    for (let offset = 0; offset < part.length;) {
      const codePoint = part.codePointAt(offset)
      if (codePoint === undefined) break
      const character = String.fromCodePoint(codePoint)
      const point = { partIndex, startOffset: offset, endOffset: offset + character.length }
      offset += character.length

      if (/\s/u.test(character)) {
        if (text) pendingWhitespace = point
        continue
      }
      if (pendingWhitespace) {
        append(' ', pendingWhitespace)
        pendingWhitespace = null
      }
      append(normalizeSearchCharacter(character).toLowerCase(), point)
    }
  })

  return {
    text,
    partIndexes: Uint32Array.from(partIndexes),
    startOffsets: Uint32Array.from(startOffsets),
    endOffsets: Uint32Array.from(endOffsets),
  }
}

function normalizeSearchQuery(query: string): string {
  return Array.from(normalizeSearchPunctuation(query).replace(/\s+/gu, ' ').trim())
    .map((character) => character.toLowerCase())
    .join('')
}

function findSearchTextMatches(
  text: string,
  needle: string,
  limit: number,
): Array<{ start: number; end: number }> {
  const matches: Array<{ start: number; end: number }> = []
  let at = text.indexOf(needle)
  while (at >= 0 && matches.length < limit) {
    matches.push({ start: at, end: at + needle.length })
    at = text.indexOf(needle, at + needle.length)
  }
  return matches
}

function materializeReaderTextMatch(
  index: ReaderTextIndex,
  match: ReaderTextIndexMatch,
): ReaderTextMatch {
  const segment = index.segments[match.segmentIndex]
  const textParts = segment.nodes.map((node) => node.data)
  const indexedParts = partsForIndexedMatch(textParts, segment.search, match)
  const parts = indexedParts.map((part) => ({
    node: segment.nodes[part.partIndex],
    startOffset: part.startOffset,
    endOffset: part.endOffset,
  }))
  return { range: rangeForReaderTextIndexMatch(index, match), parts }
}

function partsForIndexedMatch(
  parts: readonly string[],
  search: SearchTextIndex,
  indexedMatch: { start: number; end: number },
): TextPartMatch[] {
  const endIndex = indexedMatch.end - 1
  const startPartIndex = search.partIndexes[indexedMatch.start]
  const endPartIndex = search.partIndexes[endIndex]
  if (startPartIndex === undefined || endPartIndex === undefined) return []

  const partsMatch: TextPartMatch[] = []
  for (let partIndex = startPartIndex; partIndex <= endPartIndex; partIndex++) {
    const startOffset = partIndex === startPartIndex ? search.startOffsets[indexedMatch.start] : 0
    const endOffset = partIndex === endPartIndex ? search.endOffsets[endIndex] : parts[partIndex].length
    if (startOffset !== undefined && endOffset !== undefined && endOffset > startOffset) {
      partsMatch.push({ partIndex, startOffset, endOffset })
    }
  }
  return partsMatch
}
