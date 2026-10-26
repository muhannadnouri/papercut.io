import {
  useEffect,
  useEffectEvent,
  useId,
  useRef,
  type FormEvent,
  type ReactNode,
  type RefObject,
} from 'react'
import { useTranslation } from 'react-i18next'
import './AppDialog.css'

const FOCUSABLE_SELECTOR = [
  'a[href]',
  'button:not([disabled])',
  'textarea:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  'summary',
  '[tabindex]:not([tabindex="-1"])',
].join(',')

interface AppDialogProps {
  title: ReactNode
  description?: ReactNode
  children?: ReactNode
  actions?: ReactNode
  className?: string
  showCloseButton?: boolean
  navigationDisabled?: boolean
  fallbackFocusRef?: RefObject<HTMLElement | null>
  onBack?: () => void
  onCancel: () => void
  onSubmit?: (event: FormEvent) => void
}

export function AppDialog({ title, description, children, actions, className, showCloseButton = false, navigationDisabled = false, fallbackFocusRef, onBack, onCancel, onSubmit }: AppDialogProps) {
  const { t } = useTranslation()
  const titleId = useId()
  const descriptionId = useId()
  const dialogRef = useRef<HTMLDivElement | HTMLFormElement | null>(null)
  // Use one busy guard for Close, Escape and backdrop dismissal, including updated transfer state.
  const dismiss = () => {
    if (!navigationDisabled) {
      onCancel()
    }
  }
  const cancelDialog = useEffectEvent(dismiss)
  const restoreDialogFocus = useEffectEvent((previousFocus: HTMLElement | null) => {
    const focusTarget = previousFocus?.isConnected ? previousFocus : fallbackFocusRef?.current
    if (focusTarget?.isConnected) {
      focusTarget.focus()
    }
  })
  const dialogClassName = 'app-dialog' + (showCloseButton || onBack ? ' app-dialog-navigable' : '') + (className ? ` ${className}` : '')
  const setDialogRef = (node: HTMLDivElement | HTMLFormElement | null) => {
    dialogRef.current = node
  }

  useEffect(() => {
    const dialog = dialogRef.current
    if (!dialog) {
      return
    }
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
        cancelDialog()
        return
      }

      if (event.key !== 'Tab') {
        return
      }

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
      restoreDialogFocus(previousFocus)
    }
  }, [])

  const content = (
    <>
      <header className="app-dialog-header">
        {(onBack || showCloseButton) && <div className="app-dialog-navigation">
          {onBack && <button type="button" className="app-dialog-cancel app-dialog-back" disabled={navigationDisabled}
            aria-label={t('common.back')} title={t('common.back')} onClick={() => {
              if (!navigationDisabled) {
                onBack()
              }
            }}>
            <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false"><path d="m12 5-7 7 7 7M5 12h14" /></svg>
          </button>}
          {showCloseButton && <button type="button" className="app-dialog-cancel app-dialog-close"
            disabled={navigationDisabled} aria-label={t('common.close')} title={t('common.close')} onClick={dismiss}>
            <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false"><path d="m6 6 12 12M6 18 18 6" /></svg>
          </button>}
        </div>}
        <h2 id={titleId}>{title}</h2>
        {description && <p id={descriptionId}>{description}</p>}
      </header>
      {children && <div className="app-dialog-body">{children}</div>}
      {actions && <div className="app-dialog-actions">{actions}</div>}
    </>
  )

  return (
    <div className="app-dialog-backdrop" role="presentation" onClick={dismiss}>
      {onSubmit ? (
        <form
          ref={setDialogRef}
          className={dialogClassName}
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
          className={dialogClassName}
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
