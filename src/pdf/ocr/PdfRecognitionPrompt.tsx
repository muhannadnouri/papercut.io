import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { DocumentImportStatus } from '../../hooks/useUploadedLibrary'
import { PdfRecognitionStatus } from './PdfRecognitionStatus'
import { isPdfRecognitionStatusForDocument } from './pdfRecognitionPromptState'
import './PdfRecognitionPrompt.css'

/** Offer OCR where its benefit is visible, while reusing the shared job state. */
export function PdfRecognitionPrompt({
  documentUrl,
  recognitionRequired,
  status,
  onCancel,
  onRecognize,
}: {
  documentUrl: string
  recognitionRequired: boolean
  status: DocumentImportStatus
  onCancel: () => void | Promise<void>
  onRecognize: (documentUrl: string) => void | Promise<boolean>
}) {
  const { t } = useTranslation()
  const [dismissedUrl, setDismissedUrl] = useState<string>()
  const ownsStatus = isPdfRecognitionStatusForDocument(status, documentUrl)
  const recognizing = ownsStatus && status.status === 'recognizing'
  const operationBusy = status.status === 'importing' || status.status === 'recognizing' ||
    status.status === 'deleting'

  if ((!recognitionRequired && !ownsStatus) || (dismissedUrl === documentUrl && !ownsStatus)) {
    return null
  }

  return (
    <section className="pdf-recognition-prompt" aria-labelledby="pdf-recognition-prompt-title">
      <div className="pdf-recognition-prompt-copy">
        <h2 id="pdf-recognition-prompt-title">{t('reader.pdf.makeSearchableTitle')}</h2>
        <p>{t('reader.pdf.makeSearchableDescription')}</p>
      </div>
      {ownsStatus && status.status !== 'idle' && (
        <PdfRecognitionStatus
          status={status}
          t={t}
          onCancel={onCancel}
          onRetry={onRecognize}
        />
      )}
      {recognitionRequired && !recognizing && (
        <div className="pdf-recognition-prompt-actions">
          <button
            type="button"
            className="pdf-recognition-prompt-primary"
            disabled={operationBusy}
            onClick={() => void onRecognize(documentUrl)}
          >
            {t('reader.pdf.makeSearchable')}
          </button>
          <button
            type="button"
            className="pdf-recognition-prompt-secondary"
            onClick={() => setDismissedUrl(documentUrl)}
          >
            {t('reader.pdf.notNow')}
          </button>
        </div>
      )}
    </section>
  )
}
