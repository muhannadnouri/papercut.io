import { isIOSWebKit } from '../../utils/platform'

const SEARCH_TARGET_HIGHLIGHT_NAME = 'search-target'

export function decodeReaderHash(value: string): string {
  try {
    return decodeURIComponent(value)
  } catch {
    return value
  }
}

export function clearSearchTargetHighlight(root: HTMLElement): void {
  const doc = root.ownerDocument
  clearSearchTargetRegistry(doc)
  root.querySelectorAll('mark[data-search-target]').forEach((mark) => {
    const parent = mark.parentNode
    if (!parent) return
    parent.replaceChild(doc.createTextNode(mark.textContent ?? ''), mark)
    parent.normalize()
  })
}

// Search-result jumps deliberately avoid rewriting large reader DOMs. The old
// <mark> insertion path could destabilize iOS/WebKit after deep EPUB jumps, so
// the normal path is now: find a live Range, register a named CSS Highlight,
// then scroll to the Range rect. The DOM fallback is only for non-iOS WebViews
// that do not expose CSS.highlights.
export function highlightFirstSearchTarget(root: HTMLElement, text: string): Range | null {
  const range = findFirstSearchTargetRange(root, text)
  if (!range) return null
  if (setSearchTargetRegistryHighlight(root.ownerDocument, range)) return range

  if (!isIOSWebKit()) {
    const mark = markRangeSearchTarget(range)
    return mark ? rangeForElement(mark) : range
  }

  return range
}

// Finds the first visible text-node occurrence that matches the result snippet.
// This is intentionally a simple first-match locator because search cards are
// document-level summaries; durable section/page locators are the later scalable
// fix for repeated phrases in very large books.
function findFirstSearchTargetRange(root: HTMLElement, text: string): Range | null {
  const query = text.trim().toLowerCase()
  if (!query) return null

  const doc = root.ownerDocument
  const walker = doc.createTreeWalker(root, NodeFilter.SHOW_TEXT)
  let node: Node | null
  while ((node = walker.nextNode())) {
    if (node.parentElement?.closest('script, style, noscript, svg')) continue
    const value = node.textContent ?? ''
    const at = value.toLowerCase().indexOf(query)
    if (at < 0) continue

    const range = doc.createRange()
    range.setStart(node, at)
    range.setEnd(node, at + query.length)
    return range
  }

  return null
}

// Own one CSS Highlight registry entry. Do not clear the whole registry here:
// audiobook playback owns "tts-current", and Find may own its own marks/ranges.
function setSearchTargetRegistryHighlight(doc: Document, range: Range): boolean {
  const view = doc.defaultView
  const registry = view?.CSS?.highlights
  if (!view || !registry) return false

  const highlight = new view.Highlight(range)
  registry.set(SEARCH_TARGET_HIGHLIGHT_NAME, highlight)
  return true
}

function clearSearchTargetRegistry(doc: Document): void {
  const registry = doc.defaultView?.CSS?.highlights
  if (!registry) return
  registry.get(SEARCH_TARGET_HIGHLIGHT_NAME)?.clear()
  registry.delete(SEARCH_TARGET_HIGHLIGHT_NAME)
}

// Last-resort compatibility for older non-iOS WebViews. It only handles the
// single-text-node ranges created above; cross-node wrapping is avoided because
// the search-result target is a convenience jump, not a full document highlighter.
function markRangeSearchTarget(range: Range): HTMLElement | null {
  const doc = range.startContainer.ownerDocument
  if (!doc) return null
  if (range.startContainer !== range.endContainer || range.startContainer.nodeType !== Node.TEXT_NODE) return null

  const text = range.startContainer.textContent ?? ''
  const start = range.startOffset
  const end = range.endOffset
  const mark = doc.createElement('mark')
  mark.dataset.searchTarget = 'true'
  mark.textContent = text.slice(start, end)

  const fragment = doc.createDocumentFragment()
  if (start > 0) fragment.appendChild(doc.createTextNode(text.slice(0, start)))
  fragment.appendChild(mark)
  if (end < text.length) {
    fragment.appendChild(doc.createTextNode(text.slice(end)))
  }
  range.startContainer.parentNode?.replaceChild(fragment, range.startContainer)
  return mark
}

function rangeForElement(element: Element): Range {
  const range = element.ownerDocument.createRange()
  range.selectNodeContents(element)
  return range
}

export function scrollToReaderRange(range: Range): void {
  const rect = range.getBoundingClientRect()
  const targetTop = window.scrollY + rect.top
  window.scrollTo({ top: Math.max(targetTop - window.innerHeight / 2, 0), behavior: readerScrollBehavior() })
}

export function readerScrollBehavior(): ScrollBehavior {
  return isIOSWebKit() ? 'auto' : 'smooth'
}
