import { useEffect, useRef, useState, type FormEvent, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { AppDialog } from '../AppDialog/AppDialog'
import './TextInputDialog.css'

interface TextInputDialogProps {
  title: ReactNode
  label: string
  description?: ReactNode
  initialValue?: string
  confirmLabel?: string
  busy?: boolean
  error?: string
  maxLength?: number
  onCancel: () => void
  onSubmit: (value: string) => void
}

export function TextInputDialog({
  title,
  label,
  description,
  initialValue = '',
  confirmLabel,
  busy = false,
  error,
  maxLength,
  onCancel,
  onSubmit,
}: TextInputDialogProps) {
  const { t } = useTranslation()
  const [value, setValue] = useState(initialValue)
  const inputRef = useRef<HTMLInputElement>(null)
  const trimmedValue = value.trim()

  useEffect(() => {
    setValue(initialValue)
  }, [initialValue])

  useEffect(() => {
    inputRef.current?.focus()
    inputRef.current?.select()
  }, [])

  const handleSubmit = (event: FormEvent) => {
    event.preventDefault()
    if (!trimmedValue || busy) return
    onSubmit(trimmedValue)
  }

  return (
    <AppDialog
      title={title}
      description={description}
      onCancel={onCancel}
      onSubmit={handleSubmit}
      actions={(
        <>
          <button type="button" className="app-dialog-cancel" disabled={busy} onClick={onCancel}>
            {t('common.cancel')}
          </button>
          <button type="submit" className="app-dialog-submit" disabled={busy || !trimmedValue}>
            {confirmLabel ?? t('common.save')}
          </button>
        </>
      )}
    >
      <label className="text-input-dialog-field">
        <span>{label}</span>
        <input
          ref={inputRef}
          type="text"
          value={value}
          maxLength={maxLength}
          disabled={busy}
          onChange={(event) => setValue(event.target.value)}
        />
      </label>
      {error && <p className="text-input-dialog-error">{error}</p>}
    </AppDialog>
  )
}
