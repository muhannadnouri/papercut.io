import { describe, expect, it } from 'vitest'
import { normalizeReaderPageTheme, prepareReaderFontApplication } from './useReaderSettings'

describe('normalizeReaderPageTheme', () => {
  it('keeps supported themes and repairs stale persisted values', () => {
    expect(normalizeReaderPageTheme('gray')).toBe('gray')
    expect(normalizeReaderPageTheme('black')).toBe('black')
    expect(normalizeReaderPageTheme('sepia')).toBe('default')
  })
})

describe('prepareReaderFontApplication', () => {
  it('yields two frames for status paint even when the selected font cannot load', async () => {
    const steps: string[] = []

    await prepareReaderFontApplication(
      'Reader Font',
      async (fontFamily) => {
        steps.push(`load:${fontFamily}`)
        throw new Error('Unavailable font')
      },
      async () => { steps.push('frame') },
    )

    expect(steps).toEqual(['load:Reader Font', 'frame', 'frame'])
  })
})
