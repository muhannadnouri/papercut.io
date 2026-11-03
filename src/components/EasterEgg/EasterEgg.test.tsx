// @vitest-environment jsdom
import {
  act,
  createElement,
  useEffect,
} from 'react'
import { createRoot } from 'react-dom/client'
import {
  afterEach,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from 'vitest'
import {
  EASTER_EGG_DISMISS_GRACE_MS,
  EASTER_EGG_TAP_COUNT,
  useEasterEgg,
} from './useEasterEgg'
import { EasterEggOverlay } from './EasterEggOverlay'

let latest: ReturnType<typeof useEasterEgg> | null = null

function Probe() {
  const egg = useEasterEgg()
  useEffect(() => {
    latest = egg
  })
  return null
}

function OverlayProbe() {
  const egg = useEasterEgg()
  useEffect(() => {
    latest = egg
  })
  return createElement(EasterEggOverlay, {
    open: egg.revealed,
    onDismiss: egg.dismiss,
  })
}

async function renderProbe(element = createElement(Probe)) {
  const root = createRoot(document.createElement('div'))
  await act(async () => {
    root.render(element)
  })
  return root
}

describe('useEasterEgg', () => {
  let root: Awaited<ReturnType<typeof renderProbe>>

  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(async () => {
    await act(async () => {
      root.unmount()
    })
    vi.useRealTimers()
  })

  it('reveals after the required number of quick taps', async () => {
    root = await renderProbe()
    act(() => {
      for (let i = 0; i < EASTER_EGG_TAP_COUNT; i += 1) {
        latest?.registerTap()
      }
    })
    expect(latest?.revealed).toBe(true)
  })

  it('stays hidden before the required number of taps', async () => {
    root = await renderProbe()
    act(() => {
      for (let i = 0; i < EASTER_EGG_TAP_COUNT - 1; i += 1) {
        latest?.registerTap()
      }
    })
    expect(latest?.revealed).toBe(false)
  })

  it('resets the counter when taps stall past the window', async () => {
    root = await renderProbe()
    act(() => {
      for (let i = 0; i < EASTER_EGG_TAP_COUNT - 1; i += 1) {
        latest?.registerTap()
      }
    })
    act(() => {
      vi.advanceTimersByTime(3001)
    })
    // Stale taps are forgotten: a full fresh sequence is now required.
    act(() => {
      for (let i = 0; i < EASTER_EGG_TAP_COUNT - 1; i += 1) {
        latest?.registerTap()
      }
    })
    expect(latest?.revealed).toBe(false)
    act(() => {
      latest?.registerTap()
    })
    expect(latest?.revealed).toBe(true)
  })

  it('dismisses a revealed overlay', async () => {
    root = await renderProbe()
    act(() => {
      for (let i = 0; i < EASTER_EGG_TAP_COUNT; i += 1) {
        latest?.registerTap()
      }
    })
    act(() => {
      latest?.dismiss()
    })
    expect(latest?.revealed).toBe(false)
  })
})

describe('EasterEggOverlay', () => {
  let root: Awaited<ReturnType<typeof renderProbe>>

  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(async () => {
    await act(async () => {
      root.unmount()
    })
    vi.useRealTimers()
  })

  it('ignores trailing taps in the grace window, then dismisses on click', async () => {
    root = await renderProbe(createElement(OverlayProbe))
    act(() => {
      for (let i = 0; i < EASTER_EGG_TAP_COUNT; i += 1) {
        latest?.registerTap()
      }
    })
    const overlay = () => document.body.querySelector('.easter-egg-overlay')
    expect(overlay()).not.toBeNull()

    // A trailing tap right after reveal must not dismiss the egg.
    act(() => {
      overlay()?.dispatchEvent(new MouseEvent('click', { bubbles: true }))
    })
    expect(overlay()).not.toBeNull()

    // Once the grace window passes, a click dismisses it.
    act(() => {
      vi.advanceTimersByTime(EASTER_EGG_DISMISS_GRACE_MS)
    })
    act(() => {
      overlay()?.dispatchEvent(new MouseEvent('click', { bubbles: true }))
    })
    expect(overlay()).toBeNull()
  })
})
