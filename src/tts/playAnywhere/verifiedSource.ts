import {
  buildTextLocatorIndex, locatorRangeIsIgnorable, locatorTextIsIgnorable, locatorTextsMatch, recoverOrderedSourceSpans,
  sourceSpanEndGlobalOffset, sourceSpanStartGlobalOffset,
} from '../alignment/domTextSegments'
import type { PdfReadableSegment } from '../../pdf/pdfTts'
import { pdfSourceSpansForChunk } from '../../pdf/pdfTts'
import { normalizeSegmentText } from '../alignment/readableSegments'
import type { TtsChunk } from '../types'
import { createChunkSourceResolver } from './sourceChunkResolver'
import { createPdfChunkSourceResolver } from './pdfChunkSourceResolver'

/** Validate text, bounds, order and coverage once; recovered locators never rewrite saved chunks. */
export function verifiedHtmlResolver(chunks: readonly TtsChunk[], texts: readonly string[], allowRecovery: boolean) {
  const locator = buildTextLocatorIndex(texts)
  const lengths = texts.map(text => text.length)
  const validate = (mapped: readonly TtsChunk[]) => {
    const resolver = createChunkSourceResolver(mapped, lengths)
    if (resolver.status !== 'ready') return resolver
    let cursor = 0
    for (const chunk of mapped) {
      if (!chunk.text.trim()) continue
      const span = chunk.sourceSpan!
      const start = sourceSpanStartGlobalOffset(locator, span)
      const end = sourceSpanEndGlobalOffset(locator, span)
      if (!locatorRangeIsIgnorable(locator, cursor, start) ||
        !locatorTextsMatch(locator.text.slice(start, end), chunk.text)) {
        return { status: 'unavailable', reason: 'unmapped' } as const
      }
      cursor = end
    }
    return locatorRangeIsIgnorable(locator, cursor, locator.text.length) ? resolver
      : { status: 'unavailable', reason: 'unmapped' } as const
  }
  const durable = validate(chunks)
  if (durable.status === 'ready' || !allowRecovery) return durable
  const recovered = recoverOrderedSourceSpans(locator, chunks)
  if (chunks.some((chunk, index) => chunk.text.trim() && !recovered.has(index))) return durable
  return validate(chunks.map((chunk, index) => ({ ...chunk, sourceSpan: recovered.get(index) })))
}

/** PDF spans must be the current projection of verified narration spans, not arbitrary runtime locators. */
export function verifiedPdfResolver(chunks: readonly TtsChunk[], segments: PdfReadableSegment[]) {
  for (const segment of segments) {
    if (normalizeSegmentText(segment.text) !== segment.text) return { status: 'unavailable', reason: 'invalid-source' } as const
    let cursor = 0
    for (const run of segment.sourceRuns) {
      if (![run.pageIndex, run.blockOrder, run.startOffset, run.endOffset, run.sourceStartOffset, run.sourceEndOffset]
        .every(value => Number.isSafeInteger(value) && value >= 0) || run.startOffset < cursor ||
        run.endOffset <= run.startOffset || run.endOffset > segment.text.length || run.sourceEndOffset <= run.sourceStartOffset ||
        !locatorTextIsIgnorable(segment.text.slice(cursor, run.startOffset))) {
        return { status: 'unavailable', reason: 'invalid-source' } as const
      }
      cursor = run.endOffset
    }
    if (!locatorTextIsIgnorable(segment.text.slice(cursor))) return { status: 'unavailable', reason: 'invalid-source' } as const
  }
  const narration = verifiedHtmlResolver(chunks, segments.map(segment => segment.text), false)
  if (narration.status !== 'ready') return narration
  for (const chunk of chunks) {
    if (!chunk.text.trim()) continue
    const projected = pdfSourceSpansForChunk(segments, chunk.sourceSpan)
    if (projected.length !== chunk.pdfSourceSpans?.length || projected.some((span, index) => {
      const actual = chunk.pdfSourceSpans![index]
      return span.pageIndex !== actual.pageIndex || span.blockOrder !== actual.blockOrder ||
        span.startOffset !== actual.startOffset || span.endOffset !== actual.endOffset
    })) return { status: 'unavailable', reason: 'invalid-source' } as const
  }
  return createPdfChunkSourceResolver(chunks)
}
