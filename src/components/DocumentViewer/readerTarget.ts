import { isIOSWebKit } from '../../utils/platform'
import {
  findReaderTextMatches,
  type ReaderTextMatch,
} from './readerTextRanges'

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
  return highlightSearchTarget(root, text, 0)
}

/** Highlight one occurrence without rebuilding or mutating the surrounding
 * reader DOM. OCR Find uses the occurrence index returned by its page counts. */
export function highlightSearchTarget(
  root: HTMLElement,
  text: string,
  occurrenceIndex: number,
): Range | null {
  const match = findReaderTextMatches(root, text, occurrenceIndex + 1)[occurrenceIndex]
  if (!match) return null
  const { range } = match
  if (setSearchTargetRegistryHighlight(root.ownerDocument, range)) return range

  if (!isIOSWebKit()) {
    const mark = markSearchTargetMatch(match)
    return mark ? rangeForElement(mark) : range
  }

  return range
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

// Older non-iOS WebViews keep the existing DOM fallback. Each Text node is
// wrapped independently so inline formatting remains intact across the match.
function markSearchTargetMatch(match: ReaderTextMatch): HTMLElement | null {
  let firstMark: HTMLElement | null = null
  for (const part of match.parts) {
    const parent = part.node.parentNode
    if (!parent) continue
    const doc = part.node.ownerDocument
    const text = part.node.data
    const mark = doc.createElement('mark')
    mark.dataset.searchTarget = 'true'
    mark.textContent = text.slice(part.startOffset, part.endOffset)

    const fragment = doc.createDocumentFragment()
    if (part.startOffset > 0) {
      fragment.appendChild(doc.createTextNode(text.slice(0, part.startOffset)))
    }
    fragment.appendChild(mark)
    if (part.endOffset < text.length) {
      fragment.appendChild(doc.createTextNode(text.slice(part.endOffset)))
    }
    parent.replaceChild(fragment, part.node)
    firstMark ??= mark
  }
  return firstMark
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
