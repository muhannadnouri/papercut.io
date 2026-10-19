import { useCallback, useState, type FormEvent } from 'react'
import { useTranslation } from 'react-i18next'
import { AppDialog } from '../AppDialog/AppDialog'
import './PasteTextDialog.css'

interface PasteTextDialogProps {
  onCancel: () => void
  onSubmit: (title: string, text: string) => Promise<void>
}

/** Preserve pasted input until native storage succeeds so a validation or disk
 * error never forces the user to reconstruct text that may not exist elsewhere. */
export function PasteTextDialog({ onCancel, onSubmit }: PasteTextDialogProps) {
  const { t } = useTranslation()
  const [title, setTitle] = useState(() => t('library.pasteText.defaultTitle'))
  const [text, setText] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string>()
  const hasText = text.trim().length > 0
  const displayTitle = title.trim() || t('library.pasteText.defaultTitle')

  const handleCancel = useCallback(() => {
    if (!busy) onCancel()
  }, [busy, onCancel])

  const handleSubmit = async (event: FormEvent) => {
    event.preventDefault()
    if (!hasText || busy) return
    setBusy(true)
    setError(undefined)
    try {
      await onSubmit(displayTitle, text)
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
      setBusy(false)
    }
  }

  return (
    <AppDialog
      title={t('library.pasteText.title')}
      description={t('library.pasteText.description')}
      className="paste-text-dialog"
      onCancel={handleCancel}
      onSubmit={handleSubmit}
      actions={(
        <>
          <button type="button" className="app-dialog-cancel" disabled={busy} onClick={handleCancel}>
            {t('common.cancel')}
          </button>
          <button type="submit" className="app-dialog-submit" disabled={busy || !hasText}>
            {busy ? t('library.pasteText.saving') : t('common.save')}
          </button>
        </>
      )}
    >
      <label className="paste-text-field">
        <span>{t('library.pasteText.displayTitle')}</span>
        <input
          type="text"
          dir="auto"
          value={title}
          maxLength={512}
          disabled={busy}
          onChange={(event) => setTitle(event.target.value)}
        />
      </label>
      <label className="paste-text-field">
        <span>{t('library.pasteText.content')}</span>
        <textarea
          autoFocus
          dir="auto"
          rows={10}
          value={text}
          disabled={busy}
          onChange={(event) => setText(event.target.value)}
        />
      </label>
      {error && <p className="app-dialog-error" role="alert" dir="auto">{error}</p>}
    </AppDialog>
  )
}
