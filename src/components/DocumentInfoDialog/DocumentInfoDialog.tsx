import { useCallback, useEffect, useRef, useState, type FormEvent } from 'react'
import { useTranslation } from 'react-i18next'
import { LIBRARY_OPERATION_IN_PROGRESS } from '../../hooks/useUploadedLibrary'
import type { DocumentInfo } from '../../types/search'
import { formatStorageSize } from '../../utils/formatUtils'
import { AppDialog } from '../AppDialog/AppDialog'
import './DocumentInfoDialog.css'

interface DocumentInfoDialogProps {
  document: DocumentInfo
  onCancel: () => void
  onSave: (document: DocumentInfo, title: string) => Promise<void>
}

const MAX_TITLE_LENGTH = 512

/** Edit Papercut-owned display metadata without implying that source-file
 * metadata, filenames, or saved audiobook snapshots are being rewritten. */
export function DocumentInfoDialog({
  document,
  onCancel,
  onSave,
}: DocumentInfoDialogProps) {
  const { t, i18n } = useTranslation()
  const [title, setTitle] = useState(document.title)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const inputRef = useRef<HTMLInputElement>(null)
  const trimmedTitle = title.trim()
  const locale = i18n.resolvedLanguage ?? i18n.language
  const imported = document.importedAtMs
    ? new Intl.DateTimeFormat(locale, { dateStyle: 'medium', timeStyle: 'short' })
      .format(new Date(document.importedAtMs))
    : null
  const size = formatStorageSize(document.bytes)

  useEffect(() => {
    inputRef.current?.focus()
    inputRef.current?.select()
  }, [])

  const cancel = useCallback(() => {
    if (!busy) onCancel()
  }, [busy, onCancel])

  const submit = async (event: FormEvent) => {
    event.preventDefault()
    if (!trimmedTitle || busy || trimmedTitle === document.title) return
    setBusy(true)
    setError('')
    try {
      await onSave(document, trimmedTitle)
      onCancel()
    } catch (reason) {
      setError(reason instanceof Error && reason.message === LIBRARY_OPERATION_IN_PROGRESS
        ? t('library.documentInfo.operationInProgress')
        : reason instanceof Error ? reason.message : String(reason))
      setBusy(false)
    }
  }

  return (
    <AppDialog
      title={t('library.documentInfo.title')}
      description={t('library.documentInfo.description')}
      onCancel={cancel}
      onSubmit={submit}
      actions={(
        <>
          <button type="button" className="app-dialog-cancel" disabled={busy} onClick={cancel}>
            {t('common.cancel')}
          </button>
          <button
            type="submit"
            className="app-dialog-submit"
            disabled={busy || !trimmedTitle || trimmedTitle === document.title}
          >
            {t('common.save')}
          </button>
        </>
      )}
    >
      <label className="document-info-title-field">
        <span>{t('library.documentInfo.displayTitle')}</span>
        <input
          ref={inputRef}
          type="text"
          dir="auto"
          value={title}
          maxLength={MAX_TITLE_LENGTH}
          disabled={busy}
          onChange={(event) => setTitle(event.target.value)}
        />
      </label>
      <dl className="app-dialog-details">
        <Detail
          label={t('library.documentInfo.originalFileName')}
          value={document.originalFileName || t('library.documentInfo.unavailable')}
        />
        <Detail
          label={t('library.documentInfo.format')}
          value={document.format?.toUpperCase() ?? t('library.documentInfo.unavailable')}
        />
        <Detail
          label={t('library.documentInfo.fileSize')}
          value={size ?? t('library.documentInfo.unavailable')}
        />
        <Detail
          label={t('library.documentInfo.imported')}
          value={imported ?? t('library.documentInfo.unavailable')}
        />
        <Detail
          label={t(document.format === 'pdf'
            ? 'library.documentInfo.pages'
            : 'library.documentInfo.sections')}
          value={document.sections?.toLocaleString(locale) ?? t('library.documentInfo.unavailable')}
        />
      </dl>
      {error && <p className="app-dialog-error" role="alert" dir="auto">{error}</p>}
    </AppDialog>
  )
}

function Detail({ label, value }: { label: string; value: string }) {
  return (
    <div className="app-dialog-detail-row">
      <dt>{label}</dt>
      <dd dir="auto">{value}</dd>
    </div>
  )
}
