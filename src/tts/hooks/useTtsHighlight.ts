import { useEffect, useRef, type RefObject } from 'react'
import {
  clearTtsHighlight,
  clearTtsWordHighlight,
  getOrBuildSegmentIndex,
  highlightTtsChunk,
  highlightTtsWord,
  invalidateTtsDomCaches,
  scrollRangeIntoView,
  type AlignmentCache,
  type SegmentIndexCache,
} from '../alignment/highlightRanges'
import type { TtsChunk } from '../types'

const SCROLL_SETTLE_MS = 120
// Note - Recommended first value for testing:
// - 0.18s lead while playing.
// - If it still feels behind, bump to 0.25s;
// - If it feels jumpy/ahead, drop to 0.12s.
const WORD_HIGHLIGHT_LEAD_SECONDS = 0.25

interface UseTtsHighlightOptions {
  enabled: boolean
  currentChunkIndex: number | null
  chunks: TtsChunk[]
  allowDomFallback?: boolean
  currentChunkTime?: number
  currentChunkDuration?: number
  isPlaying?: boolean
  wordHighlightEnabled?: boolean
}

// Highlights the current saved-audiobook chunk inside the rendered reader DOM.
export function useTtsHighlight(
  rootRef: RefObject<HTMLElement | null>,
  {
    enabled,
    currentChunkIndex,
    chunks,
    allowDomFallback = false,
    currentChunkTime = 0,
    currentChunkDuration = 0,
    isPlaying = false,
    wordHighlightEnabled = true,
  }: UseTtsHighlightOptions,
): void {
  const segmentIndexCacheRef = useRef<SegmentIndexCache | null>(null)
  const alignmentCacheRef = useRef<AlignmentCache | null>(null)
  const rootVersionRef = useRef(0)
  const observedRootRef = useRef<HTMLElement | null>(null)
  const mutationObserverRef = useRef<MutationObserver | null>(null)

  // Find highlights and reader content swaps replace Text nodes under the same
  // article root. Versioning invalidates cached DOM node indexes without
  // rescanning the whole book on every mutation.
  useEffect(() => {
    const root = rootRef.current
    if (observedRootRef.current === root) return

    mutationObserverRef.current?.disconnect()
    mutationObserverRef.current = null
    observedRootRef.current = root
    rootVersionRef.current += 1
    invalidateTtsDomCaches(root?.ownerDocument, segmentIndexCacheRef, alignmentCacheRef)

    if (!root) return

    const observer = new MutationObserver(() => {
      rootVersionRef.current += 1
      invalidateTtsDomCaches(root.ownerDocument, segmentIndexCacheRef, alignmentCacheRef)
    })
    observer.observe(root, { childList: true, subtree: true, characterData: true })
    mutationObserverRef.current = observer
  })

  // Pre-index only when highlight playback is active. Huge uploaded books should
  // not pay a full DOM text walk merely because the reader opened.
  useEffect(() => {
    let idleHandle: number | null = null
    let timeoutHandle: number | null = null

    const cancelScheduledBuild = () => {
      if (idleHandle !== null) {
        window.cancelIdleCallback(idleHandle)
        idleHandle = null
      }
      if (timeoutHandle !== null) {
        window.clearTimeout(timeoutHandle)
        timeoutHandle = null
      }
    }

    const buildIndex = () => {
      idleHandle = null
      timeoutHandle = null
      const root = rootRef.current
      if (!root?.isConnected) return
      getOrBuildSegmentIndex(root, rootVersionRef.current, segmentIndexCacheRef)
    }

    const scheduleBuild = () => {
      cancelScheduledBuild()
      const root = rootRef.current
      if (
        segmentIndexCacheRef.current?.root !== root
        || segmentIndexCacheRef.current?.version !== rootVersionRef.current
      ) {
        segmentIndexCacheRef.current = null
      }
      if (window.requestIdleCallback) {
        idleHandle = window.requestIdleCallback(buildIndex, { timeout: 1000 })
      } else {
        timeoutHandle = window.setTimeout(buildIndex, 0)
      }
    }

    if (enabled && currentChunkIndex !== null) scheduleBuild()
    return () => {
      cancelScheduledBuild()
      segmentIndexCacheRef.current = null
    }
  }, [currentChunkIndex, enabled, rootRef])

  // CSS Highlight ranges retain DOM nodes; clear registry/cache on unmount.
  useEffect(
    () => () => {
      mutationObserverRef.current?.disconnect()
      mutationObserverRef.current = null
      observedRootRef.current = null
      const cache = alignmentCacheRef.current
      if (cache) clearTtsHighlight(cache.doc, cache)
      alignmentCacheRef.current = null
    },
    [],
  )

  // Update only active range. RAF coalesces rapid chunk changes; delayed scroll
  // prevents many smooth-scroll animations from competing during button spam.
  useEffect(() => {
    const root = rootRef.current
    const doc = root?.ownerDocument

    if (!enabled || currentChunkIndex === null) {
      if (doc) clearTtsHighlight(doc, alignmentCacheRef.current)
      alignmentCacheRef.current = null
      return
    }

    let frame: number | null = null
    let scrollTimer: number | null = null
    const attemptHighlight = () => {
      if (frame !== null) window.cancelAnimationFrame(frame)
      if (scrollTimer !== null) window.clearTimeout(scrollTimer)

      frame = window.requestAnimationFrame(() => {
        frame = null
        try {
          const result = highlightTtsChunk(
            rootRef.current,
            currentChunkIndex,
            chunks,
            allowDomFallback,
            rootVersionRef.current,
            segmentIndexCacheRef,
            alignmentCacheRef,
          )
          if (!result) return

          scrollTimer = window.setTimeout(() => {
            scrollTimer = null
            scrollRangeIntoView(result.range)
          }, SCROLL_SETTLE_MS)
        } catch (err) {
          console.warn('Unable to highlight current TTS chunk:', err)
        }
      })
    }

    attemptHighlight()

    return () => {
      if (frame !== null) window.cancelAnimationFrame(frame)
      if (scrollTimer !== null) window.clearTimeout(scrollTimer)
    }
  }, [allowDomFallback, chunks, currentChunkIndex, enabled, rootRef])

  // Word highlighting is an intentionally cheap approximation: saved manifests
  // only know chunk timings, so the active word is inferred from chunk progress.
  // The small lead compensates for coarse desktop timeupdate/native poll cadence.
  // ponytail: replace this with real word timestamps only if the TTS backend
  // exposes them without changing saved-audiobook compatibility.
  useEffect(() => {
    const root = rootRef.current
    const doc = root?.ownerDocument
    if (
      !enabled ||
      !wordHighlightEnabled ||
      currentChunkIndex === null ||
      currentChunkDuration <= 0 ||
      !Number.isFinite(currentChunkTime)
    ) {
      if (doc) clearTtsWordHighlight(doc, alignmentCacheRef.current)
      return
    }

    let frame: number | null = window.requestAnimationFrame(() => {
      frame = null
      try {
        highlightTtsWord(
          rootRef.current,
          currentChunkIndex,
          chunks,
          allowDomFallback,
          Math.min(
            Math.max(
              (currentChunkTime + (isPlaying ? WORD_HIGHLIGHT_LEAD_SECONDS : 0)) / currentChunkDuration,
              0,
            ),
            1,
          ),
          rootVersionRef.current,
          segmentIndexCacheRef,
          alignmentCacheRef,
        )
      } catch (err) {
        console.warn('Unable to highlight current TTS word:', err)
      }
    })

    return () => {
      if (frame !== null) window.cancelAnimationFrame(frame)
    }
  }, [
    allowDomFallback,
    chunks,
    currentChunkDuration,
    currentChunkIndex,
    currentChunkTime,
    enabled,
    isPlaying,
    rootRef,
    wordHighlightEnabled,
  ])
}
