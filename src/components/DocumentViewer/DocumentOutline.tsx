import {
  useId,
  useState,
} from 'react'
import {
  createPortal,
  flushSync,
} from 'react-dom'
import { useTranslation } from 'react-i18next'
import type { OutlineEntry } from '../../viewers/types'
import { AppDialog } from '../AppDialog/AppDialog'
import { OutlineIcon } from '../icons'

interface DocumentOutlineProps {
  entries: OutlineEntry[]
  onNavigate: (hash: string) => HTMLElement | undefined
}

/** Preserve publisher order and grouping. Only entries with a destination take keyboard focus. */
function OutlineList({ entries, onSelect }: {
  entries: OutlineEntry[]
  onSelect: (id: string) => void
}) {
  return <ol>
    {entries.map(({ label, target, children }, index) => (
      <li key={index}>
        {target ? <button type="button" onClick={() => onSelect(target)}><bdi>{label}</bdi></button> : <strong><bdi>{label}</bdi></strong>}
        {children.length > 0 && <OutlineList entries={children} onSelect={onSelect} />}
      </li>
    ))}
  </ol>
}

/** Portal the list outside the reader article and header so Find, narration and header layout ignore it. */
export function DocumentOutline({ entries, onNavigate }: DocumentOutlineProps) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const panelId = useId()
  const title = t('reader.contents')

  /** Finish dialog focus restoration and release its scroll lock before jumping.
   * Focusing the passage without scrolling leaves header clearance to the reader's navigation code. */
  function select(id: string) {
    flushSync(() => setOpen(false))
    const target = onNavigate('#' + id)
    if (target) {
      if (target.tabIndex < 0 && !target.hasAttribute('tabindex')) {
        target.tabIndex = -1
      }
      target.focus({ preventScroll: true })
    }
  }

  return <>
    <button type="button" className="document-outline-trigger"
      aria-label={title} aria-haspopup="dialog" aria-expanded={open} aria-controls={open ? panelId : undefined}
      onClick={event => {
        // Safari may leave focus on the passage after a pointer click. Cancel should return to Contents.
        event.currentTarget.focus({ preventScroll: true })
        setOpen(true)
      }}>
      <OutlineIcon className="find-icon" />
      <span className="document-outline-tip" aria-hidden="true">{title}</span>
    </button>
    {open && createPortal(
      <AppDialog title={title} className="document-outline-dialog" showCloseButton onCancel={() => setOpen(false)}>
        <nav id={panelId} aria-label={title}><OutlineList entries={entries} onSelect={select} /></nav>
      </AppDialog>, document.body,
    )}
  </>
}
