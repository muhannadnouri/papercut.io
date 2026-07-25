import { describe, expect, it } from 'vitest'
import { clampPdfPage, clampPdfZoom } from './PdfControls'

describe('PDF controls', () => {
  it('keeps typed page and zoom values within supported bounds', () => {
    expect(clampPdfPage('0', 3, 10)).toBe(1)
    expect(clampPdfPage('99', 3, 10)).toBe(10)
    expect(clampPdfPage('', 3, 10)).toBe(3)
    expect(clampPdfZoom('10', 100)).toBe(25)
    expect(clampPdfZoom('900', 100)).toBe(400)
    expect(clampPdfZoom('', 125)).toBe(125)
  })
})
