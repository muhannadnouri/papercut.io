import {
  useCallback,
  useEffect,
  useRef,
  useState,
} from 'react'

// Android-style: N quick taps on a quiet element reveal the full-screen logo.
export const EASTER_EGG_TAP_COUNT = 7
const EASTER_EGG_TAP_WINDOW_MS = 3000
// Ignore overlay clicks for a beat after reveal: the tap cadence that opened
// the egg often trails one or two extra clicks straight into the overlay.
export const EASTER_EGG_DISMISS_GRACE_MS = 600

export function useEasterEgg() {
  const [revealed, setRevealed] = useState(false)
  const taps = useRef(0)
  const stallTimer = useRef<number | null>(null)

  const registerTap = useCallback(() => {
    window.clearTimeout(stallTimer.current ?? undefined)
    taps.current += 1
    if (taps.current >= EASTER_EGG_TAP_COUNT) {
      taps.current = 0
      setRevealed(true)
      return
    }
    stallTimer.current = window.setTimeout(() => {
      taps.current = 0
    }, EASTER_EGG_TAP_WINDOW_MS)
  }, [])

  const dismiss = useCallback(() => setRevealed(false), [])

  useEffect(() => () => window.clearTimeout(stallTimer.current ?? undefined), [])

  return {
    revealed,
    registerTap,
    dismiss,
  }
}
