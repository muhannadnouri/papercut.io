import type { ReadableSegmentKind } from './readableSegments'

const READABLE_BLOCK_TAGS = new Set([
  'ADDRESS',
  'ARTICLE',
  'ASIDE',
  'BLOCKQUOTE',
  'CAPTION',
  'DD',
  'DETAILS',
  'DIALOG',
  'DIV',
  'DL',
  'DT',
  'FIELDSET',
  'FIGCAPTION',
  'FIGURE',
  'FOOTER',
  'FORM',
  'H1',
  'H2',
  'H3',
  'H4',
  'H5',
  'H6',
  'HEADER',
  'HGROUP',
  'HR',
  'LI',
  'MAIN',
  'NAV',
  'OL',
  'P',
  'PRE',
  'SECTION',
  'TABLE',
  'TBODY',
  'TD',
  'TFOOT',
  'TH',
  'THEAD',
  'TR',
  'UL',
])

// Review originals stay visible, but must not enter translated narration or its locators.
export const HTML_SKIP_SELECTOR = 'script, style, noscript, svg, [data-papercut-translation-review]'

const INLINE_FOOTNOTE_REFERENCE_RE = /^\[\d+[a-z]?\*?\]$/i

export interface ReadableHtmlSegment {
  owner: Element
  textNodes: Text[]
}

// Centralizes the HTML block vocabulary so extraction and highlighting agree on
// where readable boundaries exist.
export function isReadableHtmlBlock(element: Element): boolean {
  return READABLE_BLOCK_TAGS.has(element.tagName)
}

// Assign each text node to its nearest readable block. Consecutive runs keep
// wrapper-owned text around child blocks without duplicating child content.
export function collectReadableHtmlSegments(root: Element | null): ReadableHtmlSegment[] {
  if (!root) return []

  const segments: ReadableHtmlSegment[] = []
  const fallbackTextNodes: Text[] = []
  const walker = root.ownerDocument.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
    acceptNode(node) {
      const parent = node.parentElement
      if (!parent || parent.closest(HTML_SKIP_SELECTOR) || isInlineFootnoteReferenceText(parent)) {
        return NodeFilter.FILTER_REJECT
      }
      return NodeFilter.FILTER_ACCEPT
    },
  })

  let current: Node | null
  while ((current = walker.nextNode())) {
    const textNode = current as Text
    fallbackTextNodes.push(textNode)
    const owner = findNearestReadableOwner(textNode.parentElement, root)
    if (!owner) continue

    const previous = segments[segments.length - 1]
    if (previous?.owner === owner) {
      previous.textNodes.push(textNode)
    } else {
      segments.push({ owner, textNodes: [textNode] })
    }
  }

  const readableSegments = segments.filter((segment) =>
    segment.textNodes.some((node) => /\S/.test(node.data))
  )
  if (readableSegments.length > 0) return readableSegments

  return fallbackTextNodes.some((node) => /\S/.test(node.data))
    ? [{ owner: root, textNodes: fallbackTextNodes }]
    : []
}

function isInlineFootnoteReferenceText(element: Element): boolean {
  const anchor = element.closest('a')
  if (!anchor) return false

  const label = anchor.textContent?.replace(/\s+/g, '') ?? ''
  return INLINE_FOOTNOTE_REFERENCE_RE.test(label)
}

export function findNearestReadableOwner(element: Element | null, root: Element): Element | null {
  let current = element
  while (current) {
    if (isReadableHtmlBlock(current)) return current
    if (current === root) return null
    current = current.parentElement
  }
  return null
}

// Reduces HTML-specific tags into format-neutral segment kinds used by chunking.
export function htmlSegmentKind(element: Element): ReadableSegmentKind {
  if (/^H[1-6]$/.test(element.tagName)) return 'heading'
  if (element.tagName === 'P') return 'paragraph'
  if (element.tagName === 'LI') return 'listItem'
  return 'block'
}
