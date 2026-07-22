import { isReadableHtmlBlock } from '../../tts/alignment/htmlStructure'

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

// Match the reader's rendered text inside each readable block. Inline markup
// remains transparent, while block boundaries prevent unrelated paragraphs
// from becoming one accidental match.
export function findReaderTextMatches(
  root: HTMLElement,
  query: string,
  limit = Number.POSITIVE_INFINITY,
): ReaderTextMatch[] {
  if (limit <= 0) return []

  const matches: ReaderTextMatch[] = []
  for (const textNodes of collectReaderTextSegments(root)) {
    const remaining = limit - matches.length
    if (remaining <= 0) break

    const partMatches = findTextPartMatches(
      textNodes.map((node) => node.data),
      query,
      remaining,
    )
    for (const partMatch of partMatches) {
      const parts = partMatch.map((part) => ({
        node: textNodes[part.partIndex],
        startOffset: part.startOffset,
        endOffset: part.endOffset,
      }))
      const first = parts[0]
      const last = parts[parts.length - 1]
      if (!first?.node || !last?.node) continue

      const range = root.ownerDocument.createRange()
      range.setStart(first.node, first.startOffset)
      range.setEnd(last.node, last.endOffset)
      matches.push({ range, parts })
    }
  }
  return matches
}

// Keep the matching core independent of browser globals so node-boundary and
// whitespace behavior has a small, fast regression test.
export function findTextPartMatches(
  parts: readonly string[],
  query: string,
  limit = Number.POSITIVE_INFINITY,
): TextPartMatch[][] {
  const needle = normalizeSearchQuery(query)
  if (!needle || limit <= 0) return []

  const { text, points } = buildSearchText(parts)
  const matches: TextPartMatch[][] = []
  let at = text.indexOf(needle)
  while (at >= 0 && matches.length < limit) {
    const start = points[at]
    const end = points[at + needle.length - 1]
    if (start && end) matches.push(partsForMatch(parts, start, end))
    at = text.indexOf(needle, at + needle.length)
  }
  return matches
}

function collectReaderTextSegments(root: HTMLElement): Text[][] {
  const segments: Array<{ owner: Element; nodes: Text[] }> = []
  const fallback: Text[] = []
  const walker = root.ownerDocument.createTreeWalker(root, NodeFilter.SHOW_TEXT)
  let current: Node | null

  while ((current = walker.nextNode())) {
    const node = current as Text
    const parent = node.parentElement
    if (!parent || parent.closest(READER_TEXT_SKIP_SELECTOR)) continue
    fallback.push(node)

    const owner = nearestReadableOwner(parent, root)
    if (!owner) continue
    const previous = segments[segments.length - 1]
    if (previous?.owner === owner) previous.nodes.push(node)
    else segments.push({ owner, nodes: [node] })
  }

  const readable = segments
    .map((segment) => segment.nodes)
    .filter((nodes) => nodes.some((node) => /\S/.test(node.data)))
  if (readable.length > 0) return readable
  return fallback.some((node) => /\S/.test(node.data)) ? [fallback] : []
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

function buildSearchText(parts: readonly string[]): {
  text: string
  points: SearchTextPoint[]
} {
  let text = ''
  const points: SearchTextPoint[] = []
  let pendingWhitespace: SearchTextPoint | null = null

  const append = (value: string, point: SearchTextPoint) => {
    text += value
    for (let index = 0; index < value.length; index++) points.push(point)
  }

  parts.forEach((part, partIndex) => {
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
      append(character.toLowerCase(), point)
    }
  })

  return { text, points }
}

function normalizeSearchQuery(query: string): string {
  return Array.from(query.replace(/\s+/gu, ' ').trim())
    .map((character) => character.toLowerCase())
    .join('')
}

function partsForMatch(
  parts: readonly string[],
  start: SearchTextPoint,
  end: SearchTextPoint,
): TextPartMatch[] {
  const match: TextPartMatch[] = []
  for (let partIndex = start.partIndex; partIndex <= end.partIndex; partIndex++) {
    const startOffset = partIndex === start.partIndex ? start.startOffset : 0
    const endOffset = partIndex === end.partIndex ? end.endOffset : parts[partIndex].length
    if (endOffset > startOffset) match.push({ partIndex, startOffset, endOffset })
  }
  return match
}
