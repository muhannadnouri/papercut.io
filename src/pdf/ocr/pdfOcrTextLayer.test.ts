import { describe, expect, it } from 'vitest'
import type { PdfPageTextLayer } from '../../uploads/DocumentUploads'
import {
  hasPdfOcrText,
  pdfOcrLayerScale,
  pdfOcrTextLines,
  pdfOcrTextScale,
} from './pdfOcrTextLayer'

describe('pdfOcrLayerScale', () => {
  it('maps stored OCR coordinates onto the rendered PDF.js text layer', () => {
    expect(pdfOcrLayerScale(1_500, 2_000, 750, 1_000)).toEqual({ x: 0.5, y: 0.5 })
    expect(pdfOcrLayerScale(0, 2_000, 750, 1_000)).toEqual({ x: 1, y: 1 })
  })

  it('prefers persisted OCR over a sparse native text layer', () => {
    const layer: PdfPageTextLayer = {
      schemaVersion: 1,
      pageIndex: 3,
      width: 612,
      height: 792,
      blocks: [{
        text: 'recognized',
        bounds: [10, 20, 80, 16],
        order: 0,
        confidence: 0.94,
      }],
    }

    expect(hasPdfOcrText(layer)).toBe(true)
    expect(hasPdfOcrText({
      ...layer,
      blocks: [{ ...layer.blocks[0], confidence: null }],
    })).toBe(false)
  })
})

describe('OCR selection geometry', () => {
  it('joins ordered words into bounded lines and fits measured text', () => {
    const lines = pdfOcrTextLines([
      { text: 'Second\n', bounds: [40, 50, 60, 12], order: 3, confidence: 0.9 },
      { text: 'Readable ', bounds: [10, 20, 55, 10], order: 1, confidence: 0.9 },
      { text: 'line\n', bounds: [70, 19, 30, 12], order: 2, confidence: 0.9 },
      { text: 'ignored', bounds: [0, 0, 0, 10], order: 0, confidence: 0.9 },
    ])

    expect(lines).toEqual([
      { text: 'Readable line', bounds: [10, 19, 90, 12] },
      { text: 'Second', bounds: [40, 50, 60, 12] },
    ])
    expect(pdfOcrTextScale(90, 120)).toBe(0.75)
    expect(pdfOcrTextScale(90, 0)).toBe(1)
  })
})
