import {
  buildReadableDomSegmentIndex,
  normalizeDomTextPoint,
  type ReadableDomSegmentIndex,
} from '../alignment/domTextSegments'
import { normalizeSegmentText } from '../alignment/readableSegments'
import type { ViewerTextTargetApi, ViewerTextTargetResult, ViewerViewportBounds } from '../../viewers/types'

type CaretDocument = Document & {
  caretPositionFromPoint?: (x: number, y: number) => { offsetNode: Node; offset: number } | null
  caretRangeFromPoint?: (x: number, y: number) => Range | null
}

interface CaptureIndex {
  dom: ReadableDomSegmentIndex
  lengths: number[]
  visible: Set<Element>
  owners: Map<Element, number[]>
}

/** Optional, lazy reader capability. This module never loads or starts audio. */
export function createHtmlTextTargetApi(
  root: HTMLElement,
  getViewport: () => ViewerViewportBounds | null,
): { api: ViewerTextTargetApi; dispose: () => void } {
  const doc = root.ownerDocument as CaretDocument
  const view = doc.defaultView!
  let index: CaptureIndex | null = null
  let visibility: IntersectionObserver | null = null
  let revision = 0
  let disposed = false
  let pending: { frame: number; resolve: (result: ViewerTextTargetResult) => void } | null = null

  const invalidate = () => {
    revision += 1
    index = null
    visibility?.disconnect()
    visibility = null
  }
  const mutations = new view.MutationObserver(invalidate)
  mutations.observe(root, { subtree: true, childList: true, characterData: true, attributes: true })
  const isCurrent = (version: number) => {
    if (mutations.takeRecords().length) invalidate()
    return !disposed && root.isConnected && revision === version
  }
  const cancelPending = () => {
    if (!pending) return
    view.cancelAnimationFrame(pending.frame)
    pending.resolve({ status: 'unavailable', reason: 'stale' })
    pending = null
  }
  const onVisibilityChange = () => {
    if (doc.visibilityState === 'hidden') {
      revision += 1
      cancelPending()
    }
  }
  doc.addEventListener('visibilitychange', onVisibilityChange)

  function getIndex(): CaptureIndex {
    if (index) return index
    const dom = buildReadableDomSegmentIndex(root)
    const built: CaptureIndex = {
      dom,
      lengths: dom.segments.map((segment) => normalizeSegmentText(segment.textNodes.map((node) => node.data).join('')).length),
      owners: new Map(),
      visible: new Set(),
    }
    dom.segments.forEach((segment, segmentIndex) => {
      const owners = built.owners.get(segment.owner) ?? []
      owners.push(segmentIndex)
      built.owners.set(segment.owner, owners)
    })
    index = built
    visibility = new view.IntersectionObserver((entries) => {
      if (index !== built) return
      for (const entry of entries) {
        if (entry.isIntersecting) built.visible.add(entry.target)
        else built.visible.delete(entry.target)
      }
    })
    for (const owner of built.owners.keys()) visibility.observe(owner)
    return built
  }

  const api: ViewerTextTargetApi = {
    async captureViewport() {
      cancelPending()
      if (mutations.takeRecords().length) invalidate()
      if (disposed || !root.isConnected) return Promise.resolve({ status: 'unavailable', reason: 'stale' })
      if (doc.visibilityState === 'hidden') return Promise.resolve({ status: 'unavailable', reason: 'not-ready' })
      if (!view.IntersectionObserver) return Promise.resolve({ status: 'unavailable', reason: 'not-ready' })
      const bounds = getViewport()
      if (!bounds || !validBounds(bounds)) return Promise.resolve({ status: 'unavailable', reason: 'no-visible-text' })
      const stamp = viewportStamp(root, bounds)
      const built = getIndex()
      const version = revision

      return new Promise<ViewerTextTargetResult>((resolve, reject) => {
        // Let the browser deliver visibility after layout; reject intervening scroll/reflow.
        const finish = () => {
          pending = null
          const currentBounds = getViewport()
          if (!isCurrent(version) || !currentBounds || viewportStamp(root, currentBounds) !== stamp) {
            resolve({ status: 'unavailable', reason: 'stale' })
            return
          }
          const candidates = [...built.visible.keys()]
            .flatMap((owner) => built.owners.get(owner) ?? [])
            .sort((a, b) => a - b)
          for (const segmentIndex of candidates) {
            const segment = built.dom.segments[segmentIndex]
            const clip = clippedBounds(segment.owner, bounds)
            if (!clip) continue
            for (const node of segment.textNodes) {
              if (!visibleText(node, root)) continue
              const point = firstVisiblePoint(doc, node, segment.textNodes, clip)
              if (!point) continue
              resolve({
                status: 'captured',
                point: { format: 'html', segmentIndex, offset: point.normalizedOffset },
                segmentLengths: built.lengths,
                isCurrent: () => isCurrent(version),
              })
              return
            }
          }
          const canHitTest = typeof doc.caretPositionFromPoint === 'function' || typeof doc.caretRangeFromPoint === 'function'
          resolve({ status: 'unavailable', reason: canHitTest ? 'no-visible-text' : 'unmapped' })
        }
        pending = { resolve, frame: view.requestAnimationFrame(() => {
          if (pending) pending.frame = view.requestAnimationFrame(() => {
            try { finish() } catch (error) { pending = null; reject(error) }
          })
        }) }
      })
    },
  }

  return { api, dispose() {
    disposed = true
    cancelPending()
    mutations.disconnect()
    doc.removeEventListener('visibilitychange', onVisibilityChange)
    invalidate()
  } }
}

/** Reader chrome supplies a conservative unobscured rectangle; PDF has its own container. */
export function readerTextViewport(shell: HTMLElement): ViewerViewportBounds | null {
  const surface = shell.querySelector('.document-view')
  const view = shell.ownerDocument.defaultView
  if (!surface || !view) return null
  const viewport = view.visualViewport
  let bounds = intersect(surface.getBoundingClientRect(), {
    top: viewport?.offsetTop ?? 0, left: viewport?.offsetLeft ?? 0,
    bottom: (viewport?.offsetTop ?? 0) + (viewport?.height ?? view.innerHeight),
    right: (viewport?.offsetLeft ?? 0) + (viewport?.width ?? view.innerWidth),
  })
  if (!bounds) return null
  for (const control of shell.querySelectorAll('.doc-header, .find-bar, .audio-floating-playback')) {
    const rect = control.getBoundingClientRect()
    const style = view.getComputedStyle(control)
    if (!validBounds(rect) || style.visibility !== 'visible' || style.opacity === '0' ||
      rect.left >= bounds.right || rect.right <= bounds.left) continue
    if (rect.top <= bounds.top && rect.bottom > bounds.top) bounds = { ...bounds, top: rect.bottom }
    else if (rect.top > bounds.top && rect.top < bounds.bottom) bounds = { ...bounds, bottom: rect.top }
  }
  return validBounds(bounds) ? bounds : null
}

function clippedBounds(element: Element, viewport: ViewerViewportBounds): ViewerViewportBounds | null {
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

function firstVisiblePoint(doc: CaretDocument, node: Text, nodes: Text[], bounds: ViewerViewportBounds) {
  const range = doc.createRange()
  range.selectNodeContents(node)
  let first: ReturnType<typeof normalizeDomTextPoint> = null
  // shortcut: one oversized text node yields all its line rects; optimize if measured paragraph cost exceeds the capture budget.
  for (const rect of range.getClientRects()) {
    const clip = intersect(bounds, rect)
    if (!clip) continue
    const y = (clip.top + clip.bottom) / 2
    for (const x of [clip.left + Math.min(1, (clip.right - clip.left) / 2), clip.right - Math.min(1, (clip.right - clip.left) / 2)]) {
      const caret = doc.caretPositionFromPoint?.(x, y)
      const fallback = caret ? null : doc.caretRangeFromPoint?.(x, y)
      const target = caret?.offsetNode ?? fallback?.startContainer
      const offset = caret?.offset ?? fallback?.startOffset
      if (target !== node || offset === undefined) continue
      const point = normalizeDomTextPoint(nodes, node, offset)
      if (!point) continue
      const glyph = doc.createRange()
      glyph.setStart(point.node, point.offset)
      glyph.setEnd(point.node, Math.min(point.offset + 1, point.node.length))
      if (![...glyph.getClientRects()].some((rect) => intersect(clip, rect))) continue
      if (!first || point.normalizedOffset < first.normalizedOffset) first = point
    }
  }
  return first
}

function visibleText(node: Text, root: HTMLElement): boolean {
  const view = node.ownerDocument.defaultView!
  if (!node.parentElement || view.getComputedStyle(node.parentElement).visibility !== 'visible') return false
  for (let element: HTMLElement | null = node.parentElement; element; element = element.parentElement) {
    const style = view.getComputedStyle(element)
    if (style.display === 'none' || style.opacity === '0') return false
    if (element === root) return true
  }
  return false
}

function validBounds(bounds: ViewerViewportBounds): boolean {
  return [bounds.top, bounds.bottom, bounds.left, bounds.right].every(Number.isFinite) &&
    bounds.bottom > bounds.top && bounds.right > bounds.left
}

function intersect(left: ViewerViewportBounds, right: ViewerViewportBounds): ViewerViewportBounds | null {
  const result = { top: Math.max(left.top, right.top), bottom: Math.min(left.bottom, right.bottom),
    left: Math.max(left.left, right.left), right: Math.min(left.right, right.right) }
  return validBounds(result) ? result : null
}

function viewportStamp(root: HTMLElement, bounds: ViewerViewportBounds): string {
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
