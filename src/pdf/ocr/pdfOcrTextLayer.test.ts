import { describe, expect, it } from 'vitest'
import { pdfOcrLayerScale } from './pdfOcrTextLayer'

describe('pdfOcrLayerScale', () => {
  it('maps stored OCR coordinates onto the rendered PDF.js text layer', () => {
    expect(pdfOcrLayerScale(1_500, 2_000, 750, 1_000)).toEqual({ x: 0.5, y: 0.5 })
    expect(pdfOcrLayerScale(0, 2_000, 750, 1_000)).toEqual({ x: 1, y: 1 })
  })
})
