import type { ReadableSegment } from '../tts/alignment/readableSegments'
import type { PdfTtsSourceSpan, TtsChunkSourceSpan } from '../tts/types'
import type { PdfNarrationSegment } from '../uploads/DocumentUploads'

export type PdfReadableSegment = ReadableSegment & PdfNarrationSegment

/**
 * Add the shared structural kind without discarding PDF source runs. Segment
 * indexes and UTF-16 offsets can therefore resolve through the persisted
 * page/block sidecars during the next highlighting stage.
 */
export function pdfNarrationToReadableSegments(
  segments: PdfNarrationSegment[],
): PdfReadableSegment[] {
  return segments
    .filter((segment) => segment.text.trim())
    .map((segment) => ({ ...segment, kind: 'paragraph' as const }))
}

/**
 * Resolve one normalized narration chunk back to PDF.js text-item offsets.
 *
 * Inserted layout spaces have no source run and are skipped. A ligature run is
 * highlighted as one source glyph when a chunk boundary intersects it because
 * its expanded narration length cannot be split into meaningful PDF offsets.
 */
export function pdfSourceSpansForChunk(
  segments: PdfReadableSegment[],
  chunkSpan: TtsChunkSourceSpan | undefined,
): PdfTtsSourceSpan[] {
  if (!chunkSpan) return []

  const spans: PdfTtsSourceSpan[] = []
  for (
    let segmentIndex = chunkSpan.startSegmentIndex;
    segmentIndex <= chunkSpan.endSegmentIndex;
    segmentIndex++
  ) {
    const segment = segments[segmentIndex]
    if (!segment) continue
    const segmentStart = segmentIndex === chunkSpan.startSegmentIndex ? chunkSpan.startOffset : 0
    const segmentEnd = segmentIndex === chunkSpan.endSegmentIndex
      ? chunkSpan.endOffset
      : segment.text.length

    for (const run of segment.sourceRuns) {
      const overlapStart = Math.max(segmentStart, run.startOffset)
      const overlapEnd = Math.min(segmentEnd, run.endOffset)
      if (overlapStart >= overlapEnd) continue

      const narrationLength = run.endOffset - run.startOffset
      const sourceLength = run.sourceEndOffset - run.sourceStartOffset
      const offsetsAreLinear = narrationLength === sourceLength
      spans.push({
        pageIndex: run.pageIndex,
        blockOrder: run.blockOrder,
        startOffset: offsetsAreLinear
          ? run.sourceStartOffset + overlapStart - run.startOffset
          : run.sourceStartOffset,
        endOffset: offsetsAreLinear
          ? run.sourceStartOffset + overlapEnd - run.startOffset
          : run.sourceEndOffset,
      })
    }
  }
  return spans
}
