import {
  useEffect,
  useRef,
} from 'react'
import { createPortal } from 'react-dom'
import papercutIcon from '../../assets/papercut-icon.png'
import { EASTER_EGG_DISMISS_GRACE_MS } from './useEasterEgg'
import './EasterEgg.css'

interface EasterEggOverlayProps {
  open: boolean
  onDismiss: () => void
}

export function EasterEggOverlay({ open, onDismiss }: EasterEggOverlayProps) {
  const openedAt = useRef(0)

  useEffect(() => {
    if (!open) {
      return
    }
    openedAt.current = Date.now()
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        onDismiss()
      }
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [
    open,
    onDismiss,
  ])

  const handleClick = () => {
    if (Date.now() - openedAt.current < EASTER_EGG_DISMISS_GRACE_MS) {
      return
    }
    onDismiss()
  }

  if (!open) {
    return null
  }

  // Portal to body so the overlay escapes AppDialog stacking and focus scoping.
  return createPortal(
    <div className="easter-egg-overlay" onClick={handleClick} role="presentation">
      <img className="easter-egg-logo" src={papercutIcon} alt="" aria-hidden="true" />
    </div>,
    document.body,
  )
}
