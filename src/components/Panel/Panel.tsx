import { useState, type ReactNode } from 'react'
import './Panel.css'

interface PanelProps {
  title: ReactNode
  /** Inline-end summary/count shown in the header. */
  meta?: ReactNode
  /** Controlled open state. Omit to let the panel manage its own. */
  open?: boolean
  /** Initial open state when uncontrolled. */
  defaultOpen?: boolean
  /** Called on header click. Required when `open` is controlled. */
  onToggle?: () => void
  className?: string
  /** Accessible label for the section landmark. */
  ariaLabel?: string
  children: ReactNode
}

/**
 * Collapsible section with a consistent header (title + optional meta + chevron).
 * Works controlled (`open` + `onToggle`) or uncontrolled (`defaultOpen`).
 */
export function Panel({
  title,
  meta,
  open,
  defaultOpen = false,
  onToggle,
  className = '',
  ariaLabel,
  children,
}: PanelProps) {
  const [internalOpen, setInternalOpen] = useState(defaultOpen)
  const isControlled = open !== undefined
  const isOpen = isControlled ? open : internalOpen

  const handleToggle = () => {
    if (isControlled) onToggle?.()
    else setInternalOpen((value) => !value)
  }

  return (
    <section className={'panel ' + className} aria-label={ariaLabel}>
      <button
        type="button"
        className="panel-toggle"
        aria-expanded={isOpen}
        onClick={handleToggle}
      >
        <span className="panel-title">{title}</span>
        {meta != null && <span className="panel-meta" dir="auto">{meta}</span>}
        <span className={'toggle-arrow ' + (isOpen ? 'open' : '')}>&#9662;</span>
      </button>
      {isOpen && <div className="panel-body">{children}</div>}
    </section>
  )
}
