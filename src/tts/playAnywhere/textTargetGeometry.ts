import type { ViewerViewportBounds } from '../../viewers/types'

type CaretDocument = Document & {
  caretPositionFromPoint?: (x: number, y: number) => { offsetNode: Node;
    offset: number } | null
  caretRangeFromPoint?: (x: number, y: number) => Range | null
}

export function clippedBounds(element: Element, viewport: ViewerViewportBounds): ViewerViewportBounds | null {
  let bounds = intersect(viewport, element.getBoundingClientRect())
  const view = element.ownerDocument.defaultView!
  for (let parent = element.parentElement; parent && bounds; parent = parent.parentElement) {
    const style = view.getComputedStyle(parent)
    const rect = parent.getBoundingClientRect()
    bounds = intersect(bounds, {
      top: style.overflowY === 'visible' ? bounds.top : rect.top,
      bottom: style.overflowY === 'visible' ? bounds.bottom : rect.bottom,
      left: style.overflowX === 'visible' ? bounds.left : rect.left,
      right: style.overflowX === 'visible' ? bounds.right : rect.right,
    })
  }
  return bounds
}

/** Use source offsets, including in rotated/RTL fragments; do not sort by screen position. */
export function firstVisibleTextOffset(node: Text, bounds: ViewerViewportBounds): number | null {
  const doc = node.ownerDocument as CaretDocument
  const range = doc.createRange()
  range.selectNodeContents(node)
  let first: number | null = null
  // shortcut: one oversized text node yields all its line rects; optimize if measured paragraph cost exceeds the capture budget.
  for (const rect of range.getClientRects()) {
    const clip = intersect(bounds, rect)
    if (!clip) {
      continue
    }
    const insetX = Math.min(1, (clip.right - clip.left) / 2)
    const insetY = Math.min(1, (clip.bottom - clip.top) / 2)
    for (const y of [clip.top + insetY, (clip.top + clip.bottom) / 2, clip.bottom - insetY]) {
      for (const x of [clip.left + insetX, clip.right - insetX]) {
        const caret = doc.caretPositionFromPoint?.(x, y)
        const fallback = caret ? null : doc.caretRangeFromPoint?.(x, y)
        const target = caret?.offsetNode ?? fallback?.startContainer
        let offset = caret?.offset ?? fallback?.startOffset
        if (target !== node || offset === undefined) {
          continue
        }
        while (offset < node.length && /\s/u.test(node.data[offset])) {
          offset++
        }
        if (offset >= node.length) {
          continue
        }
        const glyph = doc.createRange()
        glyph.setStart(node, offset)
        glyph.setEnd(node, offset + 1)
        if (![...glyph.getClientRects()].some((rect) => intersect(clip, rect))) {
          continue
        }
        if (first === null || offset < first) {
          first = offset
        }
      }
    }
  }
  return first
}

export function visibleText(node: Text, root: HTMLElement): boolean {
  const view = node.ownerDocument.defaultView!
  if (!node.parentElement || view.getComputedStyle(node.parentElement).visibility !== 'visible') {
    return false
  }
  for (let element: HTMLElement | null = node.parentElement; element; element = element.parentElement) {
    const style = view.getComputedStyle(element)
    if (style.display === 'none' || style.opacity === '0') {
      return false
    }
    if (element === root) {
      return true
    }
  }
  return false
}

export function validBounds(bounds: ViewerViewportBounds): boolean {
  return [bounds.top, bounds.bottom, bounds.left, bounds.right].every(Number.isFinite) &&
    bounds.bottom > bounds.top && bounds.right > bounds.left
}

export function intersect(left: ViewerViewportBounds, right: ViewerViewportBounds): ViewerViewportBounds | null {
  const result = {
    top: Math.max(left.top, right.top),
    bottom: Math.min(left.bottom, right.bottom),
    left: Math.max(left.left, right.left),
    right: Math.min(left.right, right.right),
  }
  return validBounds(result) ? result : null
}

export function viewportStamp(root: HTMLElement, bounds: ViewerViewportBounds): string {
  const rect = root.getBoundingClientRect()
  const view = root.ownerDocument.defaultView!
  const style = view.getComputedStyle(root)
  const values: Array<number | string> = [bounds.top, bounds.bottom, bounds.left, bounds.right,
    rect.top, rect.left, rect.width, rect.height, view.innerWidth, view.innerHeight,
    style.fontSize, style.fontFamily, style.lineHeight, style.letterSpacing]
  for (let element: HTMLElement | null = root; element; element = element.parentElement) {
    values.push(element.scrollTop, element.scrollLeft)
  }
  values.push(view.scrollX, view.scrollY)
  return JSON.stringify(values)
}
