import { describe, expect, it } from 'vitest'
import {
  DEFAULT_READER_SETTINGS,
  FONT_FAMILY_OPTIONS,
  defaultReaderFontFamily,
  normalizeReaderPageTheme,
  prepareReaderFontApplication,
  resolveReaderFontPreference,
} from './useReaderSettings'

describe('reader font defaults', () => {
  const readexPro = FONT_FAMILY_OPTIONS.find((option) => option.labelKey.endsWith('readexPro'))!.value
  const naskhArabic = FONT_FAMILY_OPTIONS.find((option) => option.labelKey.endsWith('naskhArabic'))!.value

  it('uses Readex Pro for Arabic without replacing explicit font choices', () => {
    expect(defaultReaderFontFamily('ar')).toBe(readexPro)
    expect(defaultReaderFontFamily('ar-JO')).toBe(readexPro)
    expect(defaultReaderFontFamily('en')).toBe(DEFAULT_READER_SETTINGS.fontFamily)
    expect(resolveReaderFontPreference(DEFAULT_READER_SETTINGS.fontFamily, undefined, readexPro)).toEqual({
      fontFamily: readexPro,
      explicit: false,
    })
    expect(resolveReaderFontPreference(naskhArabic, undefined, readexPro)).toEqual({
      fontFamily: naskhArabic,
      explicit: true,
    })
    expect(resolveReaderFontPreference(DEFAULT_READER_SETTINGS.fontFamily, true, readexPro)).toEqual({
      fontFamily: DEFAULT_READER_SETTINGS.fontFamily,
      explicit: true,
    })
  })
})

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
