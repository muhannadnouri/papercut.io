import { useEffect, useId, useRef } from 'react'
import './ExternalLinkPrompt.css'

interface ExternalLinkPromptProps {
  url: string
  onCancel: () => void
  onOpen: () => void
}

export function ExternalLinkPrompt({ url, onCancel, onOpen }: ExternalLinkPromptProps) {
  const titleId = useId()
  const descriptionId = useId()
  const dialogRef = useRef<HTMLDivElement | null>(null)

  useEffect(() => {
    dialogRef.current?.focus()

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === 'Escape') onCancel()
    }

    document.addEventListener('keydown', handleKeyDown)
    return () => document.removeEventListener('keydown', handleKeyDown)
  }, [onCancel])

  return (
    <div className="external-link-modal-backdrop" role="presentation" onClick={onCancel}>
      <div
        ref={dialogRef}
        className="external-link-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        aria-describedby={descriptionId}
        tabIndex={-1}
        onClick={(event) => event.stopPropagation()}
      >
        <h2 id={titleId}>⚠️ Open External Link?</h2>
        <p id={descriptionId}>This link will open outside Papercut.</p>
        <code>{url}</code>
        <div className="external-link-actions">
          <button type="button" className="external-link-cancel" onClick={onCancel}>Cancel</button>
          <button type="button" className="external-link-open" onClick={onOpen}>Open</button>
        </div>
      </div>
    </div>
  )
}
