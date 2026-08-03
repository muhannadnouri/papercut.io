import { describe, expect, it } from 'vitest'
import type { Block, Page } from 'tesseract.js'
import { ocrPageTextLayer } from './tesseractOcr'

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
