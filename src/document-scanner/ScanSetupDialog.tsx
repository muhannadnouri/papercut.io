import { useId, useState, type FormEvent } from 'react'
import { useTranslation } from 'react-i18next'
import { AppDialog } from '../components/AppDialog/AppDialog'
import { AppSelect } from '../components/AppSelect/AppSelect'
import type { DocumentRecognitionLanguage, DocumentScanSetup } from './documentScanner'
import './ScanSetupDialog.css'

interface ScanSetupDialogProps {
  source: 'camera' | 'photos'
  onCancel: () => void
  onSubmit: (setup: DocumentScanSetup) => void
}

/** Collect the only metadata that changes scanner processing before native UI
 * opens. Unsupported languages stay importable without implying OCR support. */
export function ScanSetupDialog({ source, onCancel, onSubmit }: ScanSetupDialogProps) {
  const { t } = useTranslation()
  const languageLabelId = useId()
  const [title, setTitle] = useState(() => t(
    source === 'camera'
      ? 'library.scanSetup.defaultScanTitle'
      : 'library.scanSetup.defaultPhotoTitle',
  ))
  const [recognitionLanguage, setRecognitionLanguage] = useState<DocumentRecognitionLanguage>('english')
  const trimmedTitle = title.trim()

  const handleSubmit = (event: FormEvent) => {
    event.preventDefault()
    if (!trimmedTitle) return
    onSubmit({ title: trimmedTitle, recognitionLanguage })
  }

  return (
    <AppDialog
      title={t('library.scanSetup.title')}
      description={t('library.scanSetup.description')}
      className="scan-setup-dialog"
      onCancel={onCancel}
      onSubmit={handleSubmit}
      actions={(
        <>
          <button type="button" className="app-dialog-cancel" onClick={onCancel}>
            {t('common.cancel')}
          </button>
          <button type="submit" className="app-dialog-submit" disabled={!trimmedTitle}>
            {t('library.scanSetup.continue')}
          </button>
        </>
      )}
    >
      <label className="scan-setup-field">
        <span>{t('library.scanSetup.displayTitle')}</span>
        <input
          autoFocus
          type="text"
          dir="auto"
          value={title}
          maxLength={512}
          onChange={(event) => setTitle(event.target.value)}
        />
      </label>
      <div className="scan-setup-field">
        <span id={languageLabelId}>{t('library.scanSetup.textLanguage')}</span>
        <AppSelect
          value={recognitionLanguage}
          ariaLabelledBy={languageLabelId}
          options={[
            {
              value: 'english',
              label: t('library.scanSetup.english'),
              description: t('library.scanSetup.englishDetail'),
            },
            {
              value: 'arabic',
              label: t('library.scanSetup.arabic'),
              description: t('library.scanSetup.arabicDetail'),
            },
            {
              value: 'other',
              label: t('library.scanSetup.otherLanguage'),
              description: t('library.scanSetup.otherLanguageDetail'),
            },
          ]}
          onChange={(value) => setRecognitionLanguage(value as DocumentRecognitionLanguage)}
        />
      </div>
    </AppDialog>
  )
}
