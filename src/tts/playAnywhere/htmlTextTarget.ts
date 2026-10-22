import {
  buildReadableDomSegmentIndex,
  normalizeDomTextPoint,
  type ReadableDomSegmentIndex,
} from '../alignment/domTextSegments'
import { normalizeSegmentText } from '../alignment/readableSegments'
import { clippedBounds, firstVisibleTextOffset, intersect, validBounds, viewportStamp, visibleText } from './textTargetGeometry'
import type { ViewerTextTargetApi, ViewerTextTargetResult, ViewerViewportBounds } from '../../viewers/types'

interface CaptureIndex {
  dom: ReadableDomSegmentIndex
  lengths: number[]
  visible: Set<Element>
  pendingVisibility: Set<Element>
  owners: Map<Element, number[]>
}

/** Optional, lazy reader capability. This module never loads or starts audio. */
export function createHtmlTextTargetApi(
  root: HTMLElement,
  getViewport: () => ViewerViewportBounds | null,
): { api: ViewerTextTargetApi; dispose: () => void } {
  const doc = root.ownerDocument
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
      pendingVisibility: new Set(),
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
        built.pendingVisibility.delete(entry.target)
        if (entry.isIntersecting) built.visible.add(entry.target)
        else built.visible.delete(entry.target)
      }
    })
    for (const owner of built.owners.keys()) {
      built.pendingVisibility.add(owner)
      visibility.observe(owner)
    }
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
      const visibilityDeadline = view.performance.now() + 500

      return new Promise<ViewerTextTargetResult>((resolve, reject) => {
        // Let the browser deliver visibility after layout; reject intervening scroll/reflow.
        const finish = () => {
          const currentBounds = getViewport()
          if (!isCurrent(version) || !currentBounds || viewportStamp(root, currentBounds) !== stamp) {
            pending = null
            resolve({ status: 'unavailable', reason: 'stale' })
            return
          }
          if (built.pendingVisibility.size) {
            if (view.performance.now() < visibilityDeadline && pending) {
              pending.frame = view.requestAnimationFrame(runFinish)
            } else {
              pending = null
              resolve({ status: 'unavailable', reason: 'not-ready' })
            }
            return
          }
          pending = null
          const candidates = [...built.visible.keys()]
            .flatMap((owner) => built.owners.get(owner) ?? [])
            .sort((a, b) => a - b)
          for (const segmentIndex of candidates) {
            const segment = built.dom.segments[segmentIndex]
            const clip = clippedBounds(segment.owner, bounds)
            if (!clip) continue
            for (const node of segment.textNodes) {
              if (!visibleText(node, root)) continue
              const offset = firstVisibleTextOffset(node, clip)
              const point = offset === null ? null : normalizeDomTextPoint(segment.textNodes, node, offset)
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
        const runFinish = () => {
          try { finish() } catch (error) { pending = null; reject(error) }
        }
        pending = { resolve, frame: view.requestAnimationFrame(() => {
          if (pending) pending.frame = view.requestAnimationFrame(runFinish)
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
