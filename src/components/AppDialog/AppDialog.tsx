import { useEffect, useId, useRef, type FormEvent, type ReactNode } from 'react'
import './AppDialog.css'

const FOCUSABLE_SELECTOR = [
  'a[href]',
  'button:not([disabled])',
  'textarea:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(',')

interface AppDialogProps {
  title: ReactNode
  description?: ReactNode
  children?: ReactNode
  actions: ReactNode
  className?: string
  onCancel: () => void
  onSubmit?: (event: FormEvent) => void
}

export function AppDialog({ title, description, children, actions, className, onCancel, onSubmit }: AppDialogProps) {
  const titleId = useId()
  const descriptionId = useId()
  const dialogRef = useRef<HTMLDivElement | HTMLFormElement | null>(null)
  const setDialogRef = (node: HTMLDivElement | HTMLFormElement | null) => {
    dialogRef.current = node
  }

  useEffect(() => {
    const dialog = dialogRef.current
    if (!dialog) return
    const dialogElement = dialog
    const previousFocus = document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null

    if (!dialogElement.contains(document.activeElement)) {
      const firstFocusable = getFocusableElements(dialogElement)[0]
      ;(firstFocusable ?? dialogElement).focus()
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === 'Escape') {
        event.preventDefault()
        onCancel()
        return
      }

      if (event.key !== 'Tab') return

      const focusable = getFocusableElements(dialogElement)
      if (focusable.length === 0) {
        event.preventDefault()
        dialogElement.focus()
        return
      }

      const first = focusable[0]
      const last = focusable[focusable.length - 1]
      const active = document.activeElement
      const activeInside = active instanceof Node && dialogElement.contains(active)
      if (!activeInside || active === dialogElement) {
        event.preventDefault()
        ;(event.shiftKey ? last : first).focus()
      } else if (event.shiftKey && active === first) {
        event.preventDefault()
        last.focus()
      } else if (!event.shiftKey && active === last) {
        event.preventDefault()
        first.focus()
      }
    }

    document.addEventListener('keydown', handleKeyDown)
    return () => {
      document.removeEventListener('keydown', handleKeyDown)
      if (previousFocus?.isConnected) previousFocus.focus()
    }
  }, [onCancel])

  const content = (
    <>
      <header className="app-dialog-header">
        <h2 id={titleId}>{title}</h2>
        {description && <p id={descriptionId}>{description}</p>}
      </header>
      {children && <div className="app-dialog-body">{children}</div>}
      <div className="app-dialog-actions">{actions}</div>
    </>
  )

  return (
    <div className="app-dialog-backdrop" role="presentation" onClick={onCancel}>
      {onSubmit ? (
        <form
          ref={setDialogRef}
          className={'app-dialog' + (className ? ` ${className}` : '')}
          role="dialog"
          aria-modal="true"
          aria-labelledby={titleId}
          aria-describedby={description ? descriptionId : undefined}
          tabIndex={-1}
          onClick={(event) => event.stopPropagation()}
          onSubmit={onSubmit}
        >
          {content}
        </form>
      ) : (
        <div
          ref={setDialogRef}
          className={'app-dialog' + (className ? ` ${className}` : '')}
          role="dialog"
          aria-modal="true"
          aria-labelledby={titleId}
          aria-describedby={description ? descriptionId : undefined}
          tabIndex={-1}
          onClick={(event) => event.stopPropagation()}
        >
          {content}
        </div>
      )}
    </div>
  )
}

function getFocusableElements(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR))
    .filter((element) => element.getClientRects().length > 0)
}
