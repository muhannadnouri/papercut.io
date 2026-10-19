import { describe, expect, it } from 'vitest'
import { isFullscreenToolbarTap } from './fullscreenToolbar'

describe('isFullscreenToolbarTap', () => {
  it('accepts taps but rejects scroll and drag gestures', () => {
    expect(isFullscreenToolbarTap({ x: 40, y: 80 }, { x: 44, y: 75 })).toBe(true)
    expect(isFullscreenToolbarTap({ x: 40, y: 80 }, { x: 42, y: 105 })).toBe(false)
  })
})
