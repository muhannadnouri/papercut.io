import { describe, expect, it } from 'vitest'
import type { TextContent } from 'pdfjs-dist/types/src/display/api'
import {
  finalizedPdfTextStatus,
  hasPdfPageImages,
  hasUsableNativePdfText,
  hasUsablePdfText,
} from './pdfOcrReadiness'

describe('PDF OCR readiness', () => {
  it('keeps prose native while identifying image-backed missing text', () => {
    expect(hasUsableNativePdfText(textContent(
      'Papercut preserves this complete native sentence for reading and search.',
    ))).toBe(true)
    expect(hasUsableNativePdfText(textContent('IV'))).toBe(false)
    expect(hasUsablePdfText('x x')).toBe(false)

    const operations = imageOperations()
    expect(hasPdfPageImages([operations.paintImageXObject], operations)).toBe(true)
    expect(hasPdfPageImages([1, 2, 3], operations)).toBe(false)
  })

  it('requires OCR only when the document has no usable text', () => {
    expect(finalizedPdfTextStatus(true, false)).toBe('ready')
    expect(finalizedPdfTextStatus(true, true)).toBe('recognition-available')
    expect(finalizedPdfTextStatus(false, true)).toBe('recognition-required')
    expect(finalizedPdfTextStatus(false, false)).toBe('recognition-required')
  })
})

function textContent(text: string): TextContent {
  return {
    items: [{ str: text }],
    styles: {},
    lang: null,
  } as unknown as TextContent
}

function imageOperations() {
  return {
    paintImageMaskXObject: 10,
    paintImageMaskXObjectGroup: 11,
    paintImageMaskXObjectRepeat: 12,
    paintImageXObject: 13,
    paintImageXObjectRepeat: 14,
    paintInlineImageXObject: 15,
    paintInlineImageXObjectGroup: 16,
  }
}
