import { useEffect, useId, useRef, type FormEvent, type ReactNode } from 'react'
import './AppDialog.css'

interface AppDialogProps {
  title: ReactNode
  description?: ReactNode
  children?: ReactNode
  actions: ReactNode
  onCancel: () => void
  onSubmit?: (event: FormEvent) => void
}

export function AppDialog({ title, description, children, actions, onCancel, onSubmit }: AppDialogProps) {
  const titleId = useId()
  const descriptionId = useId()
  const dialogRef = useRef<HTMLDivElement | HTMLFormElement | null>(null)
  const setDialogRef = (node: HTMLDivElement | HTMLFormElement | null) => {
    dialogRef.current = node
  }

  useEffect(() => {
    dialogRef.current?.focus()

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === 'Escape') onCancel()
    }

    document.addEventListener('keydown', handleKeyDown)
    return () => document.removeEventListener('keydown', handleKeyDown)
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
          className="app-dialog"
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
          className="app-dialog"
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
