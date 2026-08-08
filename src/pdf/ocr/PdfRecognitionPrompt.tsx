import { useId, useState } from 'react'
import { Button, Dialog, DialogTrigger, Popover } from 'react-aria-components'
import { useTranslation } from 'react-i18next'
import { AppSelect } from '../../components/AppSelect/AppSelect'
import type { DocumentImportStatus } from '../../hooks/useUploadedLibrary'
import type { UploadedDocumentTextStatus } from '../../uploads/DocumentUploads'
import { PdfRecognitionStatus } from './PdfRecognitionStatus'
import {
  isPdfRecognitionStatusForDocument,
  pdfRecognitionIndicatorState,
  pdfRecognitionIssueAction,
} from './pdfRecognitionPromptState'
import type { PdfOcrLanguage } from './tesseractOcr'
import { PDF_OCR_INTERRUPTED } from './pdfRecognitionJob'
import './PdfRecognitionPrompt.css'

/** Offer OCR where its benefit is visible, while reusing the shared job state. */
export function PdfRecognitionPrompt({
  documentUrl,
  textStatus,
  status,
  onCancel,
  onRecognize,
  onAccept,
}: {
  documentUrl: string
  textStatus?: UploadedDocumentTextStatus
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
  const titleId = useId()
  const languageLabelId = useId()
  const [language, setLanguage] = useState<PdfOcrLanguage>('eng')
  const recognitionRequired = textStatus === 'recognition-required'
  const recognitionAvailable = textStatus === 'recognition-available'
  const ownsStatus = isPdfRecognitionStatusForDocument(status, documentUrl)
  const recognizing = ownsStatus && status.status === 'recognizing'
  const issueAction = ownsStatus && status.status === 'recognized'
    ? pdfRecognitionIssueAction(status.recognitionIssues)
    : null
  const operationBusy = status.status === 'importing' || status.status === 'recognizing' ||
    status.status === 'deleting'
  const activeLanguage = ownsStatus ? status.recognitionLanguage ?? language : language
  const indicator = pdfRecognitionIndicatorState(status, documentUrl, textStatus)
  const interrupted = ownsStatus && status.message === PDF_OCR_INTERRUPTED

  if (!recognitionRequired && !recognitionAvailable && !ownsStatus) return null

  const label = recognizing
    ? t('library.status.recognitionProgressLabel')
    : t('reader.pdf.textRecognition')
  return (
    <span className="pdf-recognition-control" title={label}>
      <DialogTrigger>
        <Button
          className="pdf-recognition-trigger"
          aria-label={label}
          aria-busy={recognizing || undefined}
        >
          <TextRecognitionIcon />
          {indicator === 'running' && (
            <span className="spinner pdf-recognition-activity" aria-hidden="true" />
          )}
          {indicator === 'attention' && (
            <span className="pdf-recognition-attention" aria-hidden="true" />
          )}
          {indicator === 'error' && (
            <span className="pdf-recognition-attention pdf-recognition-error" aria-hidden="true" />
          )}
        </Button>
        <Popover
          className="pdf-recognition-popover"
          placement="bottom end"
          offset={6}
          containerPadding={8}
          shouldFlip
        >
          <Dialog className="pdf-recognition-prompt" aria-labelledby={titleId}>
            <div className="pdf-recognition-prompt-copy">
              <h2 id={titleId}>{t('reader.pdf.textRecognition')}</h2>
              {!ownsStatus && (
                <p>{t(recognitionAvailable
                  ? 'reader.pdf.improveSearchableDescription'
                  : 'reader.pdf.makeSearchableDescription')}</p>
              )}
            </div>
            {ownsStatus && status.status !== 'idle' && (
              <PdfRecognitionStatus
                status={status}
                t={t}
                onCancel={onCancel}
                onRecognize={onRecognize}
                onAccept={onAccept}
              />
            )}
            {(recognitionRequired || recognitionAvailable) && !recognizing && !issueAction && (
              <div className="pdf-recognition-prompt-form">
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
                    {t(interrupted
                      ? 'library.status.resumeRecognition'
                      : recognitionAvailable
                        ? 'reader.pdf.improveSearchable'
                        : 'reader.pdf.makeSearchable')}
                  </button>
                </div>
              </div>
            )}
          </Dialog>
        </Popover>
      </DialogTrigger>
    </span>
  )
}

function TextRecognitionIcon() {
  return (
    <svg className="pdf-recognition-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M8 3H5a2 2 0 0 0-2 2v3m13-5h3a2 2 0 0 1 2 2v3M8 21H5a2 2 0 0 1-2-2v-3m13 5h3a2 2 0 0 0 2-2v-3M7 12h10M9 9h6m-5 6h4" />
    </svg>
  )
}
