import type { TextContent, TextItem } from 'pdfjs-dist/types/src/display/api'

const MINIMUM_CHARACTERS = 32
const MINIMUM_WORDS = 4
const MINIMUM_ALPHANUMERIC_RATIO = 0.5
const MAXIMUM_REPLACEMENT_RATIO = 0.1

interface PdfImageOperations {
  paintImageMaskXObject: number
  paintImageMaskXObjectGroup: number
  paintImageMaskXObjectRepeat: number
  paintImageXObject: number
  paintImageXObjectRepeat: number
  paintInlineImageXObject: number
  paintInlineImageXObjectGroup: number
}

export type FinalizedPdfTextStatus = 'ready' | 'recognition-available' | 'recognition-required'

/** Keep searchable hybrids usable while retaining an optional OCR entry point. */
export function finalizedPdfTextStatus(
  hasUsableText: boolean,
  hasRecognitionCandidate: boolean,
): FinalizedPdfTextStatus {
  if (!hasUsableText) return 'recognition-required'
  return hasRecognitionCandidate ? 'recognition-available' : 'ready'
}

/** Treat sparse or damaged extraction as usable only when it resembles prose.
 * Image detection is kept separate so ordinary text pages avoid operator-list work. */
export function hasUsableNativePdfText(content: TextContent): boolean {
  const text = content.items
    .filter((item): item is TextItem => 'str' in item)
    .map((item) => item.str)
    .join(' ')
  return hasUsablePdfText(text)
}

/** Apply the import readiness threshold to already-rendered PDF.js text. */
export function hasUsablePdfText(text: string): boolean {
  const characters = Array.from(text).filter((character) => !/\s/u.test(character))
  if (characters.length < MINIMUM_CHARACTERS) return false

  const words = text.trim().split(/\s+/u)
  if (words.length < MINIMUM_WORDS) return false

  const alphanumeric = characters.filter((character) => /[\p{L}\p{N}]/u.test(character)).length
  const replacements = characters.filter((character) => character === '\uFFFD').length
  return alphanumeric / characters.length >= MINIMUM_ALPHANUMERIC_RATIO
    && replacements / characters.length <= MAXIMUM_REPLACEMENT_RATIO
}

/** Distinguish an image-backed missing-text page from an intentional blank page. */
export function hasPdfPageImages(fnArray: number[], operations: PdfImageOperations): boolean {
  return fnArray.some((operation) =>
    operation === operations.paintImageMaskXObject
    || operation === operations.paintImageMaskXObjectGroup
    || operation === operations.paintImageMaskXObjectRepeat
    || operation === operations.paintImageXObject
    || operation === operations.paintImageXObjectRepeat
    || operation === operations.paintInlineImageXObject
    || operation === operations.paintInlineImageXObjectGroup)
}
