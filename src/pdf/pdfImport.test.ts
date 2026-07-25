import { describe, expect, it } from 'vitest'
import type { TextContent } from 'pdfjs-dist/types/src/display/api'
import { pageTextLayer } from './pdfImport'

describe('pageTextLayer', () => {
  it('preserves item order, line boundaries, and finite page coordinates', () => {
    const content = {
      items: [
        {
          str: 'Inline',
          dir: 'ltr',
          transform: [1, 0, 0, 12, 10, 30],
          width: 35,
          height: 12,
          fontName: 'f1',
          hasEOL: false,
        },
        {
          str: ' formatting',
          dir: 'ltr',
          transform: [1, 0, 0, 12, 45, 30],
          width: 60,
          height: 12,
          fontName: 'f2',
          hasEOL: true,
        },
      ],
      styles: {},
      lang: null,
    } as unknown as TextContent

    const layer = pageTextLayer(
      (_viewport, item) => item,
      content,
      2,
      612,
      792,
      [1, 0, 0, 1, 0, 0],
    )

    expect(layer.pageIndex).toBe(2)
    expect(layer.blocks.map((block) => block.text).join('')).toBe('Inline formatting\n')
    expect(layer.blocks[0]).toMatchObject({
      bounds: [10, 18, 35, 12],
      order: 0,
    })
  })
})
