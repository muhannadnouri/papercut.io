import type {
  HtmlBookmarkLocation,
  ViewerBookmarkApi,
  ViewerBookmarkLocation,
} from './types'

const BOOKMARK_VISIBILITY_MARGIN_PX = 8
const READER_TEXT_SKIP_SELECTOR = 'script, style, noscript, svg'

type CaretDocument = Document & {
  caretPositionFromPoint?: (x: number, y: number) => { offsetNode: Node;
    offset: number } | null
  caretRangeFromPoint?: (x: number, y: number) => Range | null
}

/**
 * Anchor HTML and generated EPUB bookmarks to rendered text instead of page
 * height, so typography and viewport changes do not move the saved passage.
 */
export function createHtmlBookmarkApi(root: HTMLElement): ViewerBookmarkApi {
  const view = root.ownerDocument.defaultView ?? window
  let cached: { location: HtmlBookmarkLocation;
    range: Range } | null = null
  let nodeIndex: Text[] | null = null
  let nodeIndexPromise: Promise<Text[]> | null = null

  // The full-book text walk blocks long enough to freeze the reader on large
  // documents, so build it in yielding slices and consume the finished index
  // from resolve/restore. Capture keeps its own synchronous walk.
  const buildNodeIndex = (): Promise<Text[]> => {
    if (nodeIndexPromise) {
      return nodeIndexPromise
    }
    nodeIndexPromise = new Promise((resolve) => {
      const walker = root.ownerDocument.createTreeWalker(root, NodeFilter.SHOW_TEXT)
      const nodes: Text[] = []
      const step = () => {
        const sliceStartedAt = view.performance.now()
        let current: Node | null
        while ((current = walker.nextNode())) {
          const node = current as Text
          const parent = node.parentElement
          if (
            parent &&
            !parent.closest(READER_TEXT_SKIP_SELECTOR) &&
            /\S/u.test(node.data)
          ) {
            nodes.push(node)
          }
          if (view.performance.now() - sliceStartedAt > 12) {
            view.setTimeout(step, 0)
            return
          }
        }
        nodeIndex = nodes
        resolve(nodes)
        indexListeners.forEach((listener) => listener())
        indexListeners.clear()
      }
      step()
    })
    return nodeIndexPromise
  }
  const indexListeners = new Set<() => void>()

  const resolveWithNodes = (nodes: Text[], location: ViewerBookmarkLocation): Range | null => {
    if (!isHtmlLocation(location)) {
      return null
    }
    if (
      cached?.location.textOffset === location.textOffset &&
      root.contains(cached.range.startContainer)
    ) {
      return cached.range
    }
    const point = textPointAtOffset(
      nodes.map((node) => node.data.length),
      location.textOffset,
    )
    if (!point) {
      return null
    }
    const node = nodes[point.partIndex]
    if (!node || !root.contains(node)) {
      return null
    }
    const range = root.ownerDocument.createRange()
    range.setStart(node, point.offset)
    range.setEnd(node, Math.min(node.length, point.offset + 1))
    cached = {
      location,
      range,
    }
    return range
  }

  void buildNodeIndex()

  return {
    capture: () => captureTextLocation(root),
    isCurrent: (location) => {
      if (!nodeIndex) {
        return false
      }
      const range = resolveWithNodes(nodeIndex, location)
      if (!range) {
        return false
      }
      const bounds = range.getBoundingClientRect()
      return bounds.bottom >= -BOOKMARK_VISIBILITY_MARGIN_PX &&
        bounds.top <= view.innerHeight + BOOKMARK_VISIBILITY_MARGIN_PX
    },
    isPastStart: () => view.scrollY > 180,
    restore: (location) => {
      // The yielding index build keeps the reader responsive on large books;
      // the restoring surface stays up until the seek completes.
      return buildNodeIndex().then((nodes) => {
        const range = resolveWithNodes(nodes, location)
        if (!range) {
          return
        }
        const bounds = range.getBoundingClientRect()
        view.scrollTo({
          top: Math.max(0, view.scrollY + bounds.top),
          behavior: 'auto',
        })
      })
    },
    scrollToTop: () => view.scrollTo({
      top: 0,
      behavior: 'smooth',
    }),
    subscribe: (listener) => {
      indexListeners.add(listener)
      view.addEventListener('scroll', listener, { passive: true })
      view.addEventListener('resize', listener)
      const observer = typeof ResizeObserver === 'undefined'
        ? null
        : new ResizeObserver(listener)
      observer?.observe(root)
      return () => {
        indexListeners.delete(listener)
        view.removeEventListener('scroll', listener)
        view.removeEventListener('resize', listener)
        observer?.disconnect()
      }
    },
  }
}

function captureTextLocation(root: HTMLElement): HtmlBookmarkLocation | null {
  const view = root.ownerDocument.defaultView ?? window
  const nodes = readerTextNodes(root)
  const starts = new Map<Text, number>()
  let total = 0
  for (const node of nodes) {
    starts.set(node, total)
    total += node.data.length
  }

  let nearest: { distance: number;
    textOffset: number } | null = null
  for (const node of nodes) {
    const start = starts.get(node) ?? 0
    const range = root.ownerDocument.createRange()
    range.selectNodeContents(node)
    for (const bounds of range.getClientRects()) {
      const distance = bounds.bottom < 0
        ? -bounds.bottom
        : bounds.top > view.innerHeight
          ? bounds.top - view.innerHeight
          : 0
      if (!nearest || distance < nearest.distance) {
        nearest = {
          distance,
          textOffset: start,
        }
      }
      if (distance > 0) {
        continue
      }

      const point = caretTextPoint(root, node, bounds)
      const pointStart = point ? starts.get(point.node) : undefined
      return {
        textOffset: point && pointStart !== undefined
          ? pointStart + Math.min(point.node.length - 1, Math.max(0, point.offset))
          : start,
      }
    }
  }
  return nearest ? { textOffset: nearest.textOffset } : null
}

function caretTextPoint(
  root: HTMLElement,
  textNode: Text,
  bounds: DOMRect,
): { node: Text;
  offset: number } | null {
  const doc = root.ownerDocument as CaretDocument
  const view = doc.defaultView ?? window
  const direction = view.getComputedStyle(textNode.parentElement ?? root).direction
  const x = direction === 'rtl'
    ? Math.max(bounds.left, bounds.right - 1)
    : Math.min(bounds.right, bounds.left + 1)
  const y = Math.min(bounds.bottom - 1, Math.max(bounds.top + 1, 1))
  const position = doc.caretPositionFromPoint?.(x, y)
  const node = position?.offsetNode
  if (position && node instanceof Text && root.contains(node)) {
    return {
      node,
      offset: position.offset,
    }
  }
  const range = doc.caretRangeFromPoint?.(x, y)
  return range?.startContainer instanceof Text && root.contains(range.startContainer)
    ? {
      node: range.startContainer,
      offset: range.startOffset,
    }
    : null
}

function readerTextNodes(root: HTMLElement): Text[] {
  const nodes: Text[] = []
  const walker = root.ownerDocument.createTreeWalker(root, NodeFilter.SHOW_TEXT)
  let current: Node | null
  while ((current = walker.nextNode())) {
    const node = current as Text
    const parent = node.parentElement
    if (
      parent &&
      !parent.closest(READER_TEXT_SKIP_SELECTOR) &&
      /\S/u.test(node.data)
    ) {
      nodes.push(node)
    }
  }
  return nodes
}

function isHtmlLocation(location: ViewerBookmarkLocation): location is HtmlBookmarkLocation {
  return 'textOffset' in location
}

export function textPointAtOffset(
  lengths: readonly number[],
  textOffset: number,
): { partIndex: number;
  offset: number } | null {
  if (!Number.isInteger(textOffset) || textOffset < 0) {
    return null
  }
  let remaining = textOffset
  for (let partIndex = 0; partIndex < lengths.length; partIndex++) {
    const length = lengths[partIndex]
    if (length <= 0) {
      continue
    }
    if (remaining < length) {
      return {
        partIndex,
        offset: remaining,
      }
    }
    remaining -= length
  }
  return null
}
