import { describe, expect, it } from 'vitest'
import type { Block, Page } from 'tesseract.js'
import { ocrPageTextLayer } from './tesseractOcr'
import {
  isPdfOcrPageImprovement,
  pdfOcrPageQuality,
  pdfOcrRenderScale,
} from './recognizePdf'

describe('ocrPageTextLayer', () => {
  it('preserves reading order and scales word bounds into PDF coordinates', () => {
    const page = {
      blocks: [{
        paragraphs: [{
          lines: [{
            words: [
              { text: 'Read', confidence: 94, bbox: { x0: 20, y0: 40, x1: 100, y1: 64 } },
              { text: 'offline', confidence: 105, bbox: { x0: 110, y0: 40, x1: 230, y1: 64 } },
            ],
          }],
        }],
      } as Block],
    } as Pick<Page, 'blocks'>

    expect(ocrPageTextLayer(page, 2, 300, 400, 600, 800)).toEqual({
      schemaVersion: 1,
      pageIndex: 2,
      width: 300,
      height: 400,
      blocks: [
        { text: 'Read ', bounds: [10, 20, 40, 12], order: 0, confidence: 0.94 },
        { text: 'offline\n', bounds: [55, 20, 60, 12], order: 1, confidence: 1 },
      ],
    })
  })

  it('rejects invalid render dimensions', () => {
    expect(() => ocrPageTextLayer({ blocks: [] }, 0, 612, 792, 0, 0)).toThrow(
      'OCR page dimensions are invalid',
    )
  })
})

describe('pdfOcrRenderScale', () => {
  it('renders ordinary pages at 300 DPI and caps oversized renders', () => {
    expect(pdfOcrRenderScale(612, 792)).toBeCloseTo(300 / 72)
    const scale = pdfOcrRenderScale(4_000, 4_000)
    expect(scale).toBe(0.75)
    expect(4_000 * scale * 4_000 * scale).toBe(9_000_000)
  })
})

describe('pdfOcrPageQuality', () => {
  it('weights OCR confidence by non-whitespace characters', () => {
    expect(pdfOcrPageQuality({
      schemaVersion: 1,
      pageIndex: 0,
      width: 100,
      height: 100,
      blocks: [
        { text: 'long ', bounds: [0, 0, 10, 10], order: 0, confidence: 0.8 },
        { text: 'x', bounds: [10, 0, 2, 10], order: 1, confidence: 0.3 },
      ],
    })).toEqual({ characters: 5, confidence: 0.7 })
  })

  it('replaces stored OCR only when neither quality metric regresses', () => {
    const layer = (text: string, confidence: number) => ({
      schemaVersion: 1 as const,
      pageIndex: 0,
      width: 100,
      height: 100,
      blocks: [{ text, bounds: [0, 0, 10, 10] as [number, number, number, number], order: 0, confidence }],
    })
    const existing = layer('four', 0.5)

    expect(isPdfOcrPageImprovement(existing, layer('five!', 0.6))).toBe(true)
    expect(isPdfOcrPageImprovement(existing, layer('few', 0.9))).toBe(false)
    expect(isPdfOcrPageImprovement(existing, layer('longer', 0.4))).toBe(false)
  })
})
