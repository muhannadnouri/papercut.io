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

    expect(lines.map(({ text, bounds }) => ({ text, bounds }))).toEqual([
      { text: 'Readable line', bounds: [10, 19, 90, 12] },
      { text: 'Second', bounds: [40, 50, 60, 12] },
    ])
    expect(lines[0].sourceRuns).toEqual([
      { startOffset: 0, endOffset: 9, blockOrder: 1, sourceStartOffset: 0, sourceText: 'Readable ' },
      { startOffset: 9, endOffset: 13, blockOrder: 2, sourceStartOffset: 0, sourceText: 'line\n' },
    ])
    expect(pdfOcrTextScale(90, 120)).toBe(0.75)
    expect(pdfOcrTextScale(90, 0)).toBe(1)
  })
})

it('keeps UTF-16 offsets through removed newlines, inserted spaces and trailing trim', () => {
  const [line] = pdfOcrTextLines([
    { text: '😀a', order: 7, bounds: [0, 0, 20, 10], confidence: 1 },
    { text: 'b\r\nc  ', order: 9, bounds: [22, 0, 20, 10], confidence: 1 },
  ])
  expect(line.text).toBe('😀a bc')
  expect(line.sourceRuns).toEqual([
    { startOffset: 0, endOffset: 3, blockOrder: 7, sourceStartOffset: 0, sourceText: '😀a' },
    { startOffset: 4, endOffset: 5, blockOrder: 9, sourceStartOffset: 0, sourceText: 'b\r\nc  ' },
    { startOffset: 5, endOffset: 8, blockOrder: 9, sourceStartOffset: 3, sourceText: 'b\r\nc  ' },
  ])
})
