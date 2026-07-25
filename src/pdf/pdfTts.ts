import type { ReadableSegment } from '../tts/alignment/readableSegments'

/**
 * Flatten page-ordered PDF.js blocks into the narration segments used by
 * HTML/EPUB. Array positions remain stable so highlighting can later resolve
 * segment indexes against the corresponding persisted coordinate sidecars.
 */
export function pdfBlocksToReadableSegments(
  pages: string[][],
): ReadableSegment[] {
  return pages.flatMap((blocks) => blocks
    .filter((text) => text.trim())
    .map((text) => ({ text, kind: 'paragraph' as const })))
}
