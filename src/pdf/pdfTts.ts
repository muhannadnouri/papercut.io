import type { ReadableSegment } from '../tts/alignment/readableSegments'
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
