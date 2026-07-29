import { describe, expect, it } from 'vitest'
import { textPointAtOffset } from './htmlBookmark'

describe('HTML bookmark text offsets', () => {
  it('resolves an offset across inline text nodes', () => {
    expect(textPointAtOffset([4, 7, 3], 9)).toEqual({
      partIndex: 1,
      offset: 5,
    })
  })
})
