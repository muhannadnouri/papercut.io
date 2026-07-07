import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import { AppDialog } from './AppDialog'

type ConfirmationTone = 'default' | 'danger'

interface ConfirmationDetail {
  label: string
  value: ReactNode
}

interface ConfirmationOptions {
  title: ReactNode
  description?: ReactNode
  details?: ConfirmationDetail[]
  confirmLabel?: string
  cancelLabel?: string | null
  tone?: ConfirmationTone
}

export function useAppConfirmation() {
  const [request, setRequest] = useState<ConfirmationOptions | null>(null)
  const resolverRef = useRef<((confirmed: boolean) => void) | null>(null)

  const close = useCallback((confirmed: boolean) => {
    resolverRef.current?.(confirmed)
    resolverRef.current = null
    setRequest(null)
  }, [])

  const confirm = useCallback((options: ConfirmationOptions) => {
    resolverRef.current?.(false)

    return new Promise<boolean>((resolve) => {
      resolverRef.current = resolve
      setRequest(options)
    })
  }, [])

  useEffect(() => () => {
    resolverRef.current?.(false)
    resolverRef.current = null
  }, [])

  const dialog = useMemo(() => {
    if (!request) return null

    const confirmClassName = 'app-dialog-submit' +
      (request.tone === 'danger' ? ' app-dialog-submit-danger' : '')

    return (
      <AppDialog
        title={request.title}
        description={request.description}
        onCancel={() => close(false)}
        actions={
          <>
            {request.cancelLabel !== null && (
              <button type="button" className="app-dialog-cancel" onClick={() => close(false)}>
                {request.cancelLabel ?? 'Cancel'}
              </button>
            )}
            <button type="button" className={confirmClassName} onClick={() => close(true)}>
              {request.confirmLabel ?? 'Confirm'}
            </button>
          </>
        }
      >
        {request.details && request.details.length > 0 && (
          <dl className="app-dialog-details">
            {request.details.map((detail) => (
              <div key={detail.label} className="app-dialog-detail-row">
                <dt>{detail.label}</dt>
                <dd>{detail.value}</dd>
              </div>
            ))}
          </dl>
        )}
      </AppDialog>
    )
  }, [close, request])

  return { confirm, dialog }
}
