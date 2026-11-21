import {
  useId,
  useMemo,
  useState,
} from 'react'
import {
  createPortal,
  flushSync,
} from 'react-dom'
import { useTranslation } from 'react-i18next'
import type { OutlineEntry } from '../../viewers/types'
import { AppDialog } from '../AppDialog/AppDialog'
import { ChevronIcon } from '../ChevronIcon/ChevronIcon'
import { initialOutlineExpansionDepth } from './outlineExpansion'
import { OutlineIcon } from '../icons'

interface DocumentOutlineProps {
  entries: OutlineEntry[]
  onNavigate: (target: string) => HTMLElement | undefined
}

/** Portal the list outside the reader article and header so Find, narration and header layout ignore it. */
export function DocumentOutline({ entries, onNavigate }: DocumentOutlineProps) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const panelId = useId()
  const title = t('reader.contents')

  const expandedDepth = useMemo(() => initialOutlineExpansionDepth(entries), [entries])
  const [toggled, setToggled] = useState<Set<string>>(() => new Set())

  function toggle(path: string) {
    setToggled(previous => {
      const next = new Set(previous)
      if (next.has(path)) {
        next.delete(path)
      } else {
        next.add(path)
      }
      return next
    })
  }

  /** Position identifies a branch even when labels repeat or a publisher group has no destination. */
  function renderList(items: OutlineEntry[], parentPath: string, depth: number) {
    return <ol>
      {items.map(({ label, target, children }, index) => {
        const path = parentPath + '/' + index
        const expanded = (depth < expandedDepth) !== toggled.has(path)
        return <li key={path}>
          {children.length > 0 ? <button type="button" aria-expanded={expanded}
            aria-label={t(expanded ? 'reader.collapseSection' : 'reader.expandSection', { label })}
            onClick={() => toggle(path)}>
            <ChevronIcon /><bdi>{label}</bdi>
          </button> : target
            ? <button type="button" onClick={() => select(target)}><bdi>{label}</bdi></button>
            : <strong><bdi>{label}</bdi></strong>}
          {children.length > 0 && expanded && renderList(children, path, depth + 1)}
        </li>
      })}
    </ol>
  }

  /** Finish dialog focus restoration and release its scroll lock before jumping.
   * Focusing the passage without scrolling leaves header clearance to the reader's navigation code. */
  function select(id: string) {
    flushSync(() => setOpen(false))
    const target = onNavigate(id)
    if (target) {
      target.focus({ preventScroll: true })
      // A named anchor without href can report tabIndex 0 yet ignore focus().
      if (target.ownerDocument.activeElement !== target && !target.hasAttribute('tabindex')) {
        target.tabIndex = -1
        target.focus({ preventScroll: true })
      }
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
        <nav id={panelId} aria-label={title}>{renderList(entries, '', 0)}</nav>
      </AppDialog>, document.body,
    )}
  </>
}
