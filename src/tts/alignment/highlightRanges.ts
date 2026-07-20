import {
  buildReadableDomTextLocatorIndex,
  buildReadableDomSegmentIndex,
  createRangeForSourceSpan,
  createSourceSpanFromTextMatch,
  locatorTextsMatch,
  sourceSpanEndGlobalOffset,
  type ReadableDomSegmentIndex,
  type ReadableDomTextLocatorIndex,
} from './domTextSegments'
import { segmentWordTokens, type WordToken } from './wordTiming'
import { logTtsDiagnostic } from '../diagnostics/TtsDiagnostics'
import type { TtsChunk, TtsChunkSourceSpan } from '../types'

const TTS_HIGHLIGHT_NAME = 'tts-current'
const TTS_WORD_HIGHLIGHT_NAME = 'tts-current-word'
const MAX_CACHED_RANGES = 128

interface CacheRef<T> {
  current: T
}

interface NormalizedTextPoint {
  node: Text
  offset: number
}

interface NormalizedRangePoints {
  start: NormalizedTextPoint
  end: NormalizedTextPoint
}

export interface SegmentIndexCache {
  root: HTMLElement
  version: number
  index: ReadableDomSegmentIndex
}

export interface AlignmentCache {
  root: HTMLElement
  doc: Document
  version: number
  chunks: TtsChunk[]
  allowDomFallback: boolean
  segmentIndex: ReadableDomSegmentIndex
  textLocatorIndex: ReadableDomTextLocatorIndex | null
  fallbackSourceSpans: Map<number, TtsChunkSourceSpan> | null
  ranges: Map<number, Range>
  failedRanges: Set<number>
  highlight: Highlight
  wordHighlight: Highlight
  wordTokens: Map<number, WordToken[]>
  wordRanges: Map<number, Array<Range | null>>
}

// Reuse document/chunk cache when valid, then replace single named Highlight range.
export function highlightTtsChunk(
  root: HTMLElement | null,
  chunkIndex: number,
  chunks: TtsChunk[],
  allowDomFallback: boolean,
  rootVersion: number,
  segmentIndexCacheRef: CacheRef<SegmentIndexCache | null>,
  alignmentCacheRef: CacheRef<AlignmentCache | null>,
): { range: Range } | null {
  const doc = root?.ownerDocument
  const view = doc?.defaultView
  if (!root || !doc || !view) return null

  ensureTtsHighlightStyles(doc)

  const cache = getOrBuildAlignmentCache(
    root,
    doc,
    view,
    chunks,
    allowDomFallback,
    chunkIndex,
    rootVersion,
    segmentIndexCacheRef,
    alignmentCacheRef,
  )

  cache.highlight.clear()
  const range = getChunkRange(cache, chunkIndex)
  if (!range) return null

  cache.highlight.add(range)
  view.CSS.highlights.set(TTS_HIGHLIGHT_NAME, cache.highlight)
  return { range }
}

export function highlightTtsWord(
  root: HTMLElement | null,
  chunkIndex: number,
  chunks: TtsChunk[],
  allowDomFallback: boolean,
  progress: number,
  rootVersion: number,
  segmentIndexCacheRef: CacheRef<SegmentIndexCache | null>,
  alignmentCacheRef: CacheRef<AlignmentCache | null>,
): void {
  const doc = root?.ownerDocument
  const view = doc?.defaultView
  if (!root || !doc || !view) return

  ensureTtsHighlightStyles(doc)
  const cache = getOrBuildAlignmentCache(
    root,
    doc,
    view,
    chunks,
    allowDomFallback,
    chunkIndex,
    rootVersion,
    segmentIndexCacheRef,
    alignmentCacheRef,
  )

  cache.wordHighlight.clear()
  const chunkRange = getChunkRange(cache, chunkIndex)
  const wordRange = chunkRange ? getActiveWordRange(cache, chunkIndex, chunkRange, progress) : null
  if (!wordRange) {
    clearTtsWordHighlight(doc, cache)
    return
  }

  cache.wordHighlight.add(wordRange)
  ;(cache.wordHighlight as Highlight & { priority?: number }).priority = 1
  view.CSS.highlights.set(TTS_WORD_HIGHLIGHT_NAME, cache.wordHighlight)
}

function getOrBuildAlignmentCache(
  root: HTMLElement,
  doc: Document,
  view: Window & typeof globalThis,
  chunks: TtsChunk[],
  allowDomFallback: boolean,
  chunkIndex: number,
  rootVersion: number,
  segmentIndexCacheRef: CacheRef<SegmentIndexCache | null>,
  alignmentCacheRef: CacheRef<AlignmentCache | null>,
): AlignmentCache {
  let cache = alignmentCacheRef.current
  if (!isUsableAlignmentCache(cache, root, chunks, allowDomFallback, chunkIndex, rootVersion)) {
    clearTtsHighlight(doc, cache)
    cache = buildAlignmentCache(root, doc, view, chunks, allowDomFallback, rootVersion, segmentIndexCacheRef)
    alignmentCacheRef.current = cache
  }
  return cache
}

// Alignment cache is tied to both live reader root and exact chunk-array identity.
function buildAlignmentCache(
  root: HTMLElement,
  doc: Document,
  view: Window & typeof globalThis,
  chunks: TtsChunk[],
  allowDomFallback: boolean,
  rootVersion: number,
  segmentIndexCacheRef: CacheRef<SegmentIndexCache | null>,
): AlignmentCache {
  return {
    root,
    doc,
    version: rootVersion,
    chunks,
    allowDomFallback,
    segmentIndex: getOrBuildSegmentIndex(root, rootVersion, segmentIndexCacheRef),
    textLocatorIndex: null,
    fallbackSourceSpans: null,
    ranges: new Map(),
    failedRanges: new Set(),
    highlight: new view.Highlight(),
    wordHighlight: new view.Highlight(),
    wordTokens: new Map(),
    wordRanges: new Map(),
  }
}

// Synchronous fallback handles Play before idle pre-index completes.
export function getOrBuildSegmentIndex(
  root: HTMLElement,
  rootVersion: number,
  cacheRef: CacheRef<SegmentIndexCache | null>,
): ReadableDomSegmentIndex {
  const cached = cacheRef.current
  if (cached?.root === root && cached.version === rootVersion) return cached.index

  const started = performance.now()
  const index = buildReadableDomSegmentIndex(root)
  cacheRef.current = { root, version: rootVersion, index }
  logTtsDiagnostic('[tts-highlight] DOM segment index built', {
    segments: index.segments.length,
    elapsedMs: Math.round(performance.now() - started),
  })
  return index
}

// Resolve/cache one chunk range. Map insertion order acts as small LRU; failed
// mappings are memoized to avoid repeated scans and duplicate diagnostics.
function getChunkRange(cache: AlignmentCache, chunkIndex: number): Range | null {
  const cached = cache.ranges.get(chunkIndex)
  if (cached) {
    cache.ranges.delete(chunkIndex)
    cache.ranges.set(chunkIndex, cached)
    return cached
  }
  if (cache.failedRanges.has(chunkIndex)) return null

  const chunk = cache.chunks[chunkIndex]
  const sourceSpan = chunk?.sourceSpan

  const started = performance.now()
  let range = sourceSpan
    ? createRangeForSourceSpan(cache.doc, cache.segmentIndex, sourceSpan)
    : null
  let strategy: 'source-span' | 'dom-fallback' = 'source-span'
  if (range && !rangeTextMatchesChunk(range, chunk)) {
    logHighlightRangeBuilt(cache, chunkIndex, range, performance.now() - started, 'source-span')
    range = null
  }

  if (!range && cache.allowDomFallback) {
    range = createFallbackRange(cache, chunkIndex)
    if (range) strategy = 'dom-fallback'
  }

  if (!range) {
    cache.failedRanges.add(chunkIndex)
    logTtsDiagnostic('[tts-highlight] chunk range unavailable', {
      chunkIndex,
      reason: sourceSpan ? 'source span does not match reader DOM' : 'missing source span',
      domFallback: cache.allowDomFallback,
    }, 'warn')
    return null
  }

  logHighlightRangeBuilt(cache, chunkIndex, range, performance.now() - started, strategy)
  cache.ranges.set(chunkIndex, range)
  if (cache.ranges.size > MAX_CACHED_RANGES) {
    const oldestIndex = cache.ranges.keys().next().value
    if (oldestIndex !== undefined) cache.ranges.delete(oldestIndex)
  }
  const elapsedMs = performance.now() - started
  if (elapsedMs >= 16) {
    logTtsDiagnostic('[tts-highlight] slow chunk range built', {
      chunkIndex,
      elapsedMs: Math.round(elapsedMs),
    })
  }
  return range
}

// Pick a word inside the active chunk from elapsed playback progress. Papercut
// does not store word timestamps, so this deliberately favors cheap,
// deterministic approximation over expensive audio analysis or manifest churn.
function getActiveWordRange(
  cache: AlignmentCache,
  chunkIndex: number,
  chunkRange: Range,
  progress: number,
): Range | null {
  const tokens = getWordTokens(cache, chunkIndex)
  if (tokens.length === 0) return null

  const tokenIndex = Math.min(Math.floor(progress * tokens.length), tokens.length - 1)
  let ranges = cache.wordRanges.get(chunkIndex)
  if (!ranges?.every((range) => !range || (range.startContainer.isConnected && range.endContainer.isConnected))) {
    ranges = createNormalizedWordRanges(cache.doc, chunkRange, tokens)
    cache.wordRanges.set(chunkIndex, ranges)
  }
  if (cache.wordRanges.size > MAX_CACHED_RANGES) {
    const oldestIndex = cache.wordRanges.keys().next().value
    if (oldestIndex !== undefined) cache.wordRanges.delete(oldestIndex)
  }

  const range = ranges[tokenIndex]
  return range && range.startContainer.isConnected && range.endContainer.isConnected ? range : null
}

function getWordTokens(cache: AlignmentCache, chunkIndex: number): WordToken[] {
  const cached = cache.wordTokens.get(chunkIndex)
  if (cached) return cached

  const text = cache.chunks[chunkIndex]?.text ?? ''
  const tokens = segmentWordTokens(text)
  cache.wordTokens.set(chunkIndex, tokens)
  return tokens
}

function createNormalizedWordRanges(
  doc: Document,
  parentRange: Range,
  tokens: WordToken[],
): Array<Range | null> {
  return findNormalizedWordRangePoints(parentRange, tokens).map((points) => {
    if (!points) return null

    const range = doc.createRange()
    range.setStart(points.start.node, Math.min(points.start.offset, points.start.node.length))
    range.setEnd(points.end.node, Math.min(points.end.offset + 1, points.end.node.length))
    return range
  })
}

// Replay normalized text inside one chunk Range once so word offsets from
// chunk.text can become live DOM points without inserting spans into uploaded
// documents. The returned array is token-aligned, so a failed word mapping does
// not shift later words and make playback highlighting lie about position.
function findNormalizedWordRangePoints(
  range: Range,
  tokens: WordToken[],
): Array<NormalizedRangePoints | null> {
  const pointsByToken = Array<NormalizedRangePoints | null>(tokens.length).fill(null)
  if (tokens.length === 0) return pointsByToken

  const doc = range.startContainer.ownerDocument
  const root = range.commonAncestorContainer
  const walkerRoot = root.nodeType === Node.TEXT_NODE ? root.parentNode : root
  if (!doc || !walkerRoot) return pointsByToken

  const walker = doc.createTreeWalker(walkerRoot, NodeFilter.SHOW_TEXT, {
    acceptNode: (node) => range.intersectsNode(node)
      ? NodeFilter.FILTER_ACCEPT
      : NodeFilter.FILTER_REJECT,
  })

  let normalizedOffset = 0
  let pendingWhitespace: NormalizedTextPoint | null = null
  let start: NormalizedTextPoint | null = null
  let tokenIndex = 0

  const emit = (point: NormalizedTextPoint): boolean => {
    while (tokenIndex < tokens.length && normalizedOffset >= tokens[tokenIndex].endOffset) {
      tokenIndex += 1
      start = null
    }
    const token = tokens[tokenIndex]
    if (!token) return true

    if (normalizedOffset === token.startOffset) start = point
    if (normalizedOffset === token.endOffset - 1) {
      if (start) pointsByToken[tokenIndex] = { start, end: point }
      tokenIndex += 1
      start = null
    }

    normalizedOffset += 1
    return tokenIndex >= tokens.length
  }

  let current: Node | null = walker.currentNode
  while (current) {
    if (current.nodeType === Node.TEXT_NODE && range.intersectsNode(current)) {
      const node = current as Text
      const rawStart = node === range.startContainer ? range.startOffset : 0
      const rawEnd = node === range.endContainer ? range.endOffset : node.data.length

      for (let offset = rawStart; offset < rawEnd; offset++) {
        if (/\s/.test(node.data[offset])) {
          if (normalizedOffset > 0) pendingWhitespace = { node, offset }
          continue
        }

        if (pendingWhitespace) {
          if (emit(pendingWhitespace)) return pointsByToken
          pendingWhitespace = null
        }

        if (emit({ node, offset })) return pointsByToken
      }
    }
    current = walker.nextNode()
  }

  return pointsByToken
}

// Last-resort compatibility path for imported audiobook bundles. Old bundles
// preserve canonical chunk/audio metadata but not durable DOM locators, so a
// restored legacy HTML document may no longer produce trustworthy source spans.
// This recovers a span from the currently rendered reader text, then validates
// the resulting Range before allowing it to drive visible highlighting.
function createFallbackRange(cache: AlignmentCache, chunkIndex: number): Range | null {
  const chunk = cache.chunks[chunkIndex]
  if (!chunk?.text) return null

  const sourceSpan = getOrBuildFallbackSourceSpan(cache, chunkIndex)
  if (!sourceSpan) {
    logTtsDiagnostic('[tts-highlight] DOM fallback unavailable', {
      chunkIndex,
      chunkId: chunk.id,
      reason: 'chunk text not found in reader DOM',
      textLength: chunk.text.length,
    }, 'warn')
    return null
  }

  const range = createRangeForSourceSpan(cache.doc, cache.segmentIndex, sourceSpan)
  if (!range || !rangeTextMatchesChunk(range, chunk)) {
    logTtsDiagnostic('[tts-highlight] DOM fallback mismatch', {
      chunkIndex,
      chunkId: chunk.id,
      sourceSpan: `${sourceSpan.startSegmentIndex}:${sourceSpan.startOffset}-${sourceSpan.endSegmentIndex}:${sourceSpan.endOffset}`,
      chunkPreview: previewDiagnosticText(normalizeDiagnosticText(chunk.text)),
      rangePreview: previewDiagnosticText(normalizeDiagnosticText(range?.toString() ?? '')),
    }, 'warn')
    return null
  }

  logTtsDiagnostic('[tts-highlight] DOM fallback range built', {
    chunkIndex,
    chunkId: chunk.id,
    sourceSpan: `${sourceSpan.startSegmentIndex}:${sourceSpan.startOffset}-${sourceSpan.endSegmentIndex}:${sourceSpan.endOffset}`,
  })
  return range
}

// Return the recovered sourceSpan for any chunk, regardless of playback order.
// This matters for the chunk browser: a user can jump directly to chunk 40, so
// fallback highlighting cannot depend on chunks 1-39 having already played.
function getOrBuildFallbackSourceSpan(cache: AlignmentCache, chunkIndex: number): TtsChunkSourceSpan | undefined {
  if (!cache.fallbackSourceSpans) {
    cache.fallbackSourceSpans = buildFallbackSourceSpans(cache)
  }
  return cache.fallbackSourceSpans.get(chunkIndex)
}

// Build fallback spans in canonical audiobook order using a forward cursor. This
// avoids the classic repeated-text trap where every later chunk containing "the"
// or a common Arabic phrase would otherwise match an earlier occurrence. The
// work is cached per stable reader DOM and only runs when the normal sourceSpan
// path fails for imported bundles.
function buildFallbackSourceSpans(cache: AlignmentCache): Map<number, TtsChunkSourceSpan> {
  const started = performance.now()
  const locator = getOrBuildTextLocatorIndex(cache)
  const sourceSpans = new Map<number, TtsChunkSourceSpan>()
  let cursor = 0

  for (let index = 0; index < cache.chunks.length; index++) {
    const chunk = cache.chunks[index]
    if (!chunk?.text) continue
    const sourceSpan = createSourceSpanFromTextMatch(locator, chunk.text, cursor)
    if (!sourceSpan) continue
    sourceSpans.set(index, sourceSpan)
    const nextCursor = sourceSpanEndGlobalOffset(locator, sourceSpan)
    if (nextCursor >= 0) cursor = nextCursor
  }

  logTtsDiagnostic('[tts-highlight] DOM fallback source spans built', {
    chunks: cache.chunks.length,
    matched: sourceSpans.size,
    elapsedMs: Math.round(performance.now() - started),
  })
  return sourceSpans
}

// Build the normalized text map lazily because very large books should not pay
// this cost merely by opening or starting playback when ordinary spans are valid.
function getOrBuildTextLocatorIndex(cache: AlignmentCache): ReadableDomTextLocatorIndex {
  if (cache.textLocatorIndex) return cache.textLocatorIndex

  const started = performance.now()
  const locator = buildReadableDomTextLocatorIndex(cache.segmentIndex)
  cache.textLocatorIndex = locator
  logTtsDiagnostic('[tts-highlight] DOM text locator index built', {
    characters: locator.text.length,
    // matchCharacters is shorter when compatibility matching drops Arabic
    // visual marks; a big gap here explains why exact imported lookup failed.
    matchCharacters: locator.matchText.length,
    segments: locator.segmentTexts.length,
    elapsedMs: Math.round(performance.now() - started),
  })
  return locator
}

// Diagnostics compare the chunk text to the actual DOM Range text. If they
// match but the visible highlight looks wrong, the issue is likely platform
// rendering/scrolling. If they differ, source-span mapping is the culprit.
function logHighlightRangeBuilt(
  cache: AlignmentCache,
  chunkIndex: number,
  range: Range,
  elapsedMs: number,
  strategy: 'source-span' | 'dom-fallback',
): void {
  const chunk = cache.chunks[chunkIndex]
  const sourceSpan = chunk?.sourceSpan
  const chunkText = normalizeDiagnosticText(chunk?.text ?? '')
  const rangeText = normalizeDiagnosticText(range.toString())
  const rect = range.getBoundingClientRect()
  const matches = chunkText === rangeText

  logTtsDiagnostic(matches ? '[tts-highlight] chunk range built' : '[tts-highlight] chunk range mismatch', {
    chunkIndex,
    chunkId: chunk?.id ?? '',
    matches,
    chunkPreview: previewDiagnosticText(chunkText),
    rangePreview: previewDiagnosticText(rangeText),
    chunkLength: chunkText.length,
    rangeLength: rangeText.length,
    strategy,
    domFallback: cache.allowDomFallback,
    sourceSpan: sourceSpan
      ? `${sourceSpan.startSegmentIndex}:${sourceSpan.startOffset}-${sourceSpan.endSegmentIndex}:${sourceSpan.endOffset}`
      : '',
    segments: cache.segmentIndex.segments.length,
    elapsedMs: Math.round(elapsedMs),
    rectTop: Math.round(rect.top),
    rectHeight: Math.round(rect.height),
    documentLang: cache.doc.documentElement.lang || '',
    documentDir: cache.doc.documentElement.dir || cache.doc.body?.dir || '',
    cssHighlights: Boolean(cache.doc.defaultView?.CSS.highlights),
    userAgent: navigator.userAgent,
  }, matches ? 'info' : 'warn')
}

function normalizeDiagnosticText(text: string): string {
  return text.replace(/\s+/g, ' ').trim()
}

function rangeTextMatchesChunk(range: Range, chunk: TtsChunk | undefined): boolean {
  return locatorTextsMatch(range.toString(), chunk?.text ?? '')
}

function previewDiagnosticText(text: string): string {
  return text.length <= 160 ? text : text.slice(0, 157).trimEnd() + '...'
}

// Detached range endpoints indicate reader navigation/mutation; rebuild cache then.
function isUsableAlignmentCache(
  cache: AlignmentCache | null,
  root: HTMLElement,
  chunks: TtsChunk[],
  allowDomFallback: boolean,
  chunkIndex: number,
  rootVersion: number,
): cache is AlignmentCache {
  if (
    !cache ||
    cache.root !== root ||
    cache.version !== rootVersion ||
    cache.chunks !== chunks ||
    cache.allowDomFallback !== allowDomFallback
  ) return false
  const range = cache.ranges.get(chunkIndex)
  return !range || Boolean(range.startContainer.isConnected && range.endContainer.isConnected)
}

export function invalidateTtsDomCaches(
  doc: Document | undefined,
  segmentIndexCacheRef: CacheRef<SegmentIndexCache | null>,
  alignmentCacheRef: CacheRef<AlignmentCache | null>,
): void {
  segmentIndexCacheRef.current = null
  const cache = alignmentCacheRef.current
  if (cache) clearTtsHighlight(doc ?? cache.doc, cache)
  alignmentCacheRef.current = null
}

// Clear both owned Highlight object and global registry entry, including old docs.
export function clearTtsHighlight(doc: Document, cache: AlignmentCache | null): void {
  cache?.highlight.clear()
  cache?.wordHighlight.clear()
  clearTtsHighlightRegistry(cache?.doc)
  if (cache?.doc !== doc) clearTtsHighlightRegistry(doc)
}

export function clearTtsWordHighlight(doc: Document, cache: AlignmentCache | null): void {
  cache?.wordHighlight.clear()
  clearTtsWordHighlightRegistry(cache?.doc ?? doc)
  if (cache?.doc !== doc) clearTtsWordHighlightRegistry(doc)
}

function clearTtsHighlightRegistry(doc: Document | undefined): void {
  const registry = doc?.defaultView?.CSS.highlights
  if (!registry) return

  registry.get(TTS_HIGHLIGHT_NAME)?.clear()
  registry.delete(TTS_HIGHLIGHT_NAME)
  clearTtsWordHighlightRegistry(doc)
}

function clearTtsWordHighlightRegistry(doc: Document | undefined): void {
  const registry = doc?.defaultView?.CSS.highlights
  if (!registry) return

  registry.get(TTS_WORD_HIGHLIGHT_NAME)?.clear()
  registry.delete(TTS_WORD_HIGHLIGHT_NAME)
}

function ensureTtsHighlightStyles(doc: Document): void {
  if (doc.getElementById('tts-current-styles')) return
  const style = doc.createElement('style')
  style.id = 'tts-current-styles'
  style.textContent = `
    ::highlight(${TTS_HIGHLIGHT_NAME}) {
      background-color: var(--highlight-tts, #c7f9cc);
      color: inherit;
    }

    ::highlight(${TTS_WORD_HIGHLIGHT_NAME}) {
      background-color: var(--highlight-tts-word, #86efac);
      color: inherit;
    }
  `
  doc.head.appendChild(style)
}

// Ranges now live in the app document, so their rects are already window-local.
export function scrollRangeIntoView(range: Range): void {
  const rangeRect = range.getBoundingClientRect()
  if (!Number.isFinite(rangeRect.top)) return

  const top = window.scrollY + rangeRect.top - window.innerHeight / 2
  window.scrollTo({ top: Math.max(top, 0), behavior: 'smooth' })
}
