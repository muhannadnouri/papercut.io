import { useId, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { AppSelect } from '../../components/AppSelect/AppSelect'
import type { DocumentImportStatus } from '../../hooks/useUploadedLibrary'
import { PdfRecognitionStatus } from './PdfRecognitionStatus'
import {
  isPdfRecognitionStatusForDocument,
  pdfRecognitionIssueAction,
} from './pdfRecognitionPromptState'
import type { PdfOcrLanguage } from './tesseractOcr'
import './PdfRecognitionPrompt.css'

/** Offer OCR where its benefit is visible, while reusing the shared job state. */
export function PdfRecognitionPrompt({
  documentUrl,
  recognitionRequired,
  status,
  onCancel,
  onRecognize,
  onAccept,
}: {
  documentUrl: string
  recognitionRequired: boolean
  status: DocumentImportStatus
  onCancel: () => void | Promise<void>
  onRecognize: (
    documentUrl: string,
    language: PdfOcrLanguage,
    improveIssues?: DocumentImportStatus['recognitionIssues'],
  ) => void | Promise<boolean>
  onAccept: (documentUrl: string) => void | Promise<boolean>
}) {
  const { t } = useTranslation()
  const languageLabelId = useId()
  const [dismissedUrl, setDismissedUrl] = useState<string>()
  const [language, setLanguage] = useState<PdfOcrLanguage>('eng')
  const ownsStatus = isPdfRecognitionStatusForDocument(status, documentUrl)
  const recognizing = ownsStatus && status.status === 'recognizing'
  const issueAction = ownsStatus && status.status === 'recognized'
    ? pdfRecognitionIssueAction(status.recognitionIssues)
    : null
  const operationBusy = status.status === 'importing' || status.status === 'recognizing' ||
    status.status === 'deleting'
  const activeLanguage = ownsStatus ? status.recognitionLanguage ?? language : language

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
          onImprove={onRecognize}
          onAccept={onAccept}
        />
      )}
      {recognitionRequired && !recognizing && !issueAction && (
        <>
          <div className="pdf-recognition-prompt-language">
            <span id={languageLabelId}>{t('library.scanSetup.textLanguage')}</span>
            <AppSelect
              value={activeLanguage}
              ariaLabelledBy={languageLabelId}
              disabled={ownsStatus}
              options={[
                { value: 'eng', label: t('library.scanSetup.english') },
                { value: 'ara', label: t('library.scanSetup.arabic') },
              ]}
              onChange={(value) => setLanguage(value as PdfOcrLanguage)}
            />
          </div>
          <div className="pdf-recognition-prompt-actions">
            <button
              type="button"
              className="pdf-recognition-prompt-primary"
              disabled={operationBusy}
              onClick={() => void onRecognize(documentUrl, activeLanguage)}
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
        </>
      )}
    </section>
  )
}
