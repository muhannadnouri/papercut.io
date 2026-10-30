import {
  textLocatorIndexSteps,
  locatorRangeIsIgnorable,
  locatorTextIsIgnorable,
  locatorTextsMatch,
  orderedSourceSpanSteps,
  sourceSpanEndGlobalOffset,
  sourceSpanStartGlobalOffset,
} from '../alignment/domTextSegments'
import type { PdfReadableSegment } from '../../pdf/pdfTts'
import { pdfSourceSpansForChunk } from '../../pdf/pdfTts'
import { normalizeSegmentText } from '../alignment/readableSegments'
import {
  finishNarrationWork,
  splitNarrationSentences,
} from '../alignment/narrationText'
import type {
  TtsChunk,
  TtsChunkSourceSpan,
} from '../types'
import { createChunkSourceResolver } from './sourceChunkResolver'
import { createPdfChunkSourceResolver } from './pdfChunkSourceResolver'

/** Validate text, bounds, order and coverage once; recovered locators never rewrite saved chunks. */
type HtmlResolver = ReturnType<typeof createChunkSourceResolver>
type PdfResolver = ReturnType<typeof createPdfChunkSourceResolver>
export function verifiedHtmlResolver(chunks: readonly TtsChunk[], texts: readonly string[], allowRecovery: boolean): HtmlResolver {
  return finishNarrationWork(htmlResolverSteps(chunks, texts, allowRecovery))
}

export function* htmlResolverSteps(chunks: readonly TtsChunk[], texts: readonly string[], allowRecovery: boolean): Generator<void, HtmlResolver> {
  const locator = yield* textLocatorIndexSteps(texts)
  yield* locator.prepareMatchIndex()
  const lengths = texts.map(text => text.length)
  function* validate(mapped: readonly TtsChunk[], strict: boolean): Generator<void, HtmlResolver> {
    const resolver = createChunkSourceResolver(mapped, lengths)
    if (resolver.status !== 'ready') {
      return resolver
    }
    let cursor = 0
    let dropped = false
    const accepted: TtsChunk[] = []
    for (const chunk of mapped) {
      yield
      if (!chunk.text.trim()) {
        continue
      }
      const span = chunk.sourceSpan!
      const start = sourceSpanStartGlobalOffset(locator, span)
      const end = sourceSpanEndGlobalOffset(locator, span)
      const matches = locatorTextsMatch(locator.text.slice(start, end), chunk.text) ||
        (span.startSegmentIndex !== span.endSegmentIndex && locatorTextsMatch(narratedSpanText(texts, span), chunk.text))
      if (strict && (!locatorRangeIsIgnorable(locator, cursor, start) || !matches)) {
        return {
          status: 'unavailable',
          reason: 'unmapped',
        } as const
      }
      if (!matches || !locatorRangeIsIgnorable(locator, cursor, start)) {
        // One unverifiable span drops only its own chunk from play-from-here;
        // the cursor still advances so a single bad span cannot cascade into
        // its neighbours. Recovery has already had its chance to repair the
        // book; fully recovered spans stay strict because their failure means
        // the audio does not belong to this text at all.
        dropped = true
        cursor = end
        continue
      }
      cursor = end
      accepted.push(chunk)
    }
    if (dropped) {
      // Degraded coverage: the trailing guard is meaningless once spans were
      // distrusted, so ship the accepted prefix as-is.
      return createChunkSourceResolver(accepted, lengths)
    }
    return locatorRangeIsIgnorable(locator, cursor, locator.text.length) ? resolver
      : {
        status: 'unavailable',
        reason: 'unmapped',
      } as const
  }
  const durable = yield* validate(chunks, false)
  if (durable.status === 'ready' || !allowRecovery) {
    return durable
  }
  const recovered = yield* orderedSourceSpanSteps(locator, chunks)
  if (chunks.some((chunk, index) => chunk.text.trim() && !recovered.has(index))) {
    return durable
  }
  return yield* validate(chunks.map((chunk, index) => ({
    ...chunk,
    sourceSpan: recovered.get(index),
  })), true)
}

/** Generation splits paragraphs before merging; a leading terminator may be omitted in each paragraph. */
function narratedSpanText(texts: readonly string[], span: TtsChunkSourceSpan): string {
  const parts: string[] = []
  for (let index = span.startSegmentIndex; index <= span.endSegmentIndex; index++) {
    const start = index === span.startSegmentIndex ? span.startOffset : 0
    const end = index === span.endSegmentIndex ? span.endOffset : texts[index].length
    parts.push(...splitNarrationSentences(texts[index].slice(start, end)))
  }
  return parts.join(' ')
}

/** PDF spans must be the current projection of verified narration spans, not arbitrary runtime locators. */
export function verifiedPdfResolver(chunks: readonly TtsChunk[], segments: PdfReadableSegment[]): PdfResolver {
  return finishNarrationWork(pdfResolverSteps(chunks, segments))
}

export function* pdfResolverSteps(chunks: readonly TtsChunk[], segments: PdfReadableSegment[]): Generator<void, PdfResolver> {
  for (const segment of segments) {
    yield
    if (normalizeSegmentText(segment.text) !== segment.text) {
      return {
        status: 'unavailable',
        reason: 'invalid-source',
      } as const
    }
    let cursor = 0
    for (const run of segment.sourceRuns) {
      yield
      if (![run.pageIndex, run.blockOrder, run.startOffset, run.endOffset, run.sourceStartOffset, run.sourceEndOffset]
        .every(value => Number.isSafeInteger(value) && value >= 0) || run.startOffset < cursor ||
        run.endOffset <= run.startOffset || run.endOffset > segment.text.length || run.sourceEndOffset <= run.sourceStartOffset ||
        !locatorTextIsIgnorable(segment.text.slice(cursor, run.startOffset))) {
        return {
          status: 'unavailable',
          reason: 'invalid-source',
        } as const
      }
      cursor = run.endOffset
    }
    if (!locatorTextIsIgnorable(segment.text.slice(cursor))) {
      return {
        status: 'unavailable',
        reason: 'invalid-source',
      } as const
    }
  }
  const narration = yield* htmlResolverSteps(chunks, segments.map(segment => segment.text), false)
  if (narration.status !== 'ready') {
    return narration
  }
  for (const chunk of chunks) {
    yield
    if (!chunk.text.trim()) {
      continue
    }
    const projected = pdfSourceSpansForChunk(segments, chunk.sourceSpan)
    if (projected.length !== chunk.pdfSourceSpans?.length || projected.some((span, index) => {
      const actual = chunk.pdfSourceSpans![index]
      return span.pageIndex !== actual.pageIndex || span.blockOrder !== actual.blockOrder ||
        span.startOffset !== actual.startOffset || span.endOffset !== actual.endOffset
    })) {
      return {
        status: 'unavailable',
        reason: 'invalid-source',
      } as const
    }
  }
  return createPdfChunkSourceResolver(chunks)
}
