import {
  useLayoutEffect,
  useRef,
  useState,
} from 'react'
import { useTranslation } from 'react-i18next'
import {
  readerSelection,
  selectedTextStart,
} from './selectionTarget'
import './ReaderPassageAction.css'

interface ReaderPassageActionProps {
  hasPlayed: boolean
  pending: boolean
  onSnapshotSelection: () => boolean
  onPlayFromHere: () => void
  onPlayFromSelection: () => void
}

/**
 * Idle-only shortcut: session history changes its label, never its target or availability.
 * Selection availability is presentation only; action-time source/audio validation remains authoritative.
 */
export function ReaderPassageAction({
  hasPlayed,
  pending,
  onSnapshotSelection,
  onPlayFromHere,
  onPlayFromSelection,
}: ReaderPassageActionProps) {
  const { t } = useTranslation()
  const buttonRef = useRef<HTMLButtonElement | null>(null)
  const selectionIntent = useRef(false)
  const [hasSelection, setHasSelection] = useState(false)

  useLayoutEffect(() => {
    const button = buttonRef.current
    const root = button?.closest('.app-reader')?.querySelector<HTMLElement>('.document-view')
    if (!button || !root) return
    const doc = button.ownerDocument
    const shell = button.closest<HTMLElement>('.app-reader')!
    // Bookmark feedback sits above this rail; account for translated labels wrapping at large text.
    const fitNotice = () => shell.style.setProperty('--reader-passage-action-height', button.getBoundingClientRect().height + 'px')
    fitNotice()
    const resize = typeof ResizeObserver === 'function' ? new ResizeObserver(fitNotice) : undefined
    resize?.observe(button)
    const update = () => {
      // Control focus may collapse native selection; preserve its advertised action until focus leaves.
      if (doc.activeElement === button) return
      const selection = readerSelection(root)
      const available = Boolean(selection && selectedTextStart(selection.range))
      selectionIntent.current = available
      setHasSelection(available)
    }
    update()
    doc.addEventListener('selectionchange', update)
    button.addEventListener('blur', update)
    return () => {
      doc.removeEventListener('selectionchange', update)
      button.removeEventListener('blur', update)
      resize?.disconnect()
      shell.style.removeProperty('--reader-passage-action-height')
      // Starting audio removes this idle shortcut; transfer keyboard focus to the resulting player.
      if (doc.activeElement === button) queueMicrotask(() => {
        if (root.isConnected && doc.activeElement === doc.body) {
          root.closest('.app-reader')?.querySelector<HTMLButtonElement>('.audio-floating-playback .audio-primary-btn')?.focus()
        }
      })
    }
  }, [])

  /** Snapshot before pointer focus; keyboard Tab already snapshots through bindSelectionIntent. */
  const snapshot = () => {
    selectionIntent.current = onSnapshotSelection() || hasSelection
  }
  const play = () => {
    // Never silently fall back to the viewport if an advertised selection becomes stale.
    const selected = selectionIntent.current || onSnapshotSelection()
    if (selected) onPlayFromSelection()
    else onPlayFromHere()
  }
  const label = t(hasSelection ? 'tts.playAnywhere.selection' : 'tts.playAnywhere.here')

  return (
    <button ref={buttonRef} type="button"
      className={'reader-passage-action' + (hasPlayed ? '' : ' reader-passage-action-initial')}
      aria-label={label} title={label}
      disabled={pending} aria-busy={pending} onPointerDown={snapshot} onClick={play}>
      {pending ? <span className="spinner" aria-hidden="true" /> : (
        <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
          <path d="M8 5v14l11-7z" />
        </svg>
      )}
      {hasPlayed && <span>{label}</span>}
    </button>
  )
}
