import { describe, expect, it } from 'vitest'
import type { PdfPageTextLayer } from '../../uploads/DocumentUploads'
import { hasPdfOcrText, pdfOcrLayerScale } from './pdfOcrTextLayer'

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
