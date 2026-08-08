import { Trans } from 'react-i18next'
import type { TFunction } from 'i18next'
import type { ReactNode } from 'react'
import type { DocumentImportStatus } from '../../hooks/useUploadedLibrary'
import { PDF_OCR_NO_TEXT } from './recognizePdf'
import type { PdfOcrLanguage } from './tesseractOcr'
import { PDF_OCR_INTERRUPTED } from './pdfRecognitionJob'
import {
  pdfRecognitionActionPageCount,
  pdfRecognitionIssueAction,
} from './pdfRecognitionPromptState'
import './PdfRecognitionStatus.css'

/** Present the shared OCR progress, cancellation, review, and acceptance lifecycle. */
export function PdfRecognitionStatus({
  status,
  t,
  onCancel,
  onRecognize,
  onAccept,
  showDocumentTitle = false,
}: {
  status: DocumentImportStatus
  t: TFunction
  onCancel: () => void | Promise<void>
  onRecognize: (
    documentUrl: string,
    language: PdfOcrLanguage,
    issues?: NonNullable<DocumentImportStatus['recognitionIssues']>,
  ) => void | Promise<boolean>
  onAccept: (documentUrl: string) => void | Promise<boolean>
  showDocumentTitle?: boolean
}) {
  const progress = status.recognitionProgress
  const recognizing = status.status === 'recognizing'
  const title = status.title ?? ''
  const issues = status.recognitionIssues
  const retryDocumentUrl = status.documentUrl
  const issueCount = (issues?.failedPages.length ?? 0) +
    (issues?.unrecognizedPages.length ?? 0) +
    (issues?.lowConfidencePages.length ?? 0)
  const issueAction = pdfRecognitionIssueAction(issues)
  const actionPageCount = pdfRecognitionActionPageCount(issues)
  let message: ReactNode

  if (recognizing && status.cancelRequested) {
    message = t('library.status.stoppingRecognition')
  } else if (recognizing && progress?.phase === 'recognizing') {
    message = <Trans i18nKey="library.status.recognizingPage" values={{ title, current: progress.pageNumber, total: progress.pageCount }} components={{ title: <bdi /> }} />
  } else if (recognizing && progress?.phase === 'indexing') {
    message = <Trans i18nKey="library.status.indexingRecognition" values={{ title }} components={{ title: <bdi /> }} />
  } else if (recognizing) {
    message = <Trans i18nKey="library.status.preparingRecognition" values={{ title }} components={{ title: <bdi /> }} />
  } else if (status.status === 'recognized' && issueAction === 'retry') {
    message = <Trans i18nKey="library.status.recognitionNeedsRetry" values={{ title, count: actionPageCount }} components={{ title: <bdi /> }} />
  } else if (status.status === 'recognized' && issueAction === 'accept') {
    message = (
      <span className="pdf-recognition-outcome">
        <strong>{t('library.status.recognitionReviewComplete')}</strong>
        {showDocumentTitle && <bdi className="pdf-recognition-outcome-title" title={title}>{title}</bdi>}
        <span>{t('library.status.recognitionPagesToReview', { count: actionPageCount })}</span>
      </span>
    )
  } else if (status.status === 'recognized') {
    message = <Trans i18nKey="library.status.recognitionComplete" values={{ title }} components={{ title: <bdi /> }} />
  } else if (status.status === 'cancelled') {
    message = <Trans i18nKey="library.status.recognitionCancelled" values={{ title }} components={{ title: <bdi /> }} />
  } else if (status.message === PDF_OCR_INTERRUPTED) {
    message = <Trans i18nKey="library.status.recognitionInterrupted" values={{ title }} components={{ title: <bdi /> }} />
  } else {
    message = status.message === PDF_OCR_NO_TEXT
      ? t('library.status.recognitionNoText')
      : status.message
  }

  return (
    <div className="document-batch-status">
      <div className="document-batch-status-row">
        <span role="status">{message}</span>
        {recognizing && (
          <button
            type="button"
            className="document-batch-cancel"
            disabled={status.cancelRequested}
            onClick={() => void onCancel()}
          >
            {status.cancelRequested ? t('library.status.stopping') : t('common.cancel')}
          </button>
        )}
      </div>
      {recognizing && (
        <progress
          className="document-batch-progress"
          aria-label={t('library.status.recognitionProgressLabel')}
          max={progress?.pageCount || undefined}
          value={progress?.phase === 'recognizing' ? progress.pageNumber : undefined}
        />
      )}
      {!recognizing && issueCount > 0 && issues && (
        <div className="pdf-recognition-review">
          <details className={`document-batch-failures pdf-recognition-issues pdf-recognition-issues-${issueAction}`}>
            <summary>{t('library.status.recognitionIssues', { count: issueCount })}</summary>
            <ul>
              {issues.failedPages.length > 0 && (
                <li>{t('library.status.recognitionFailedPages', { pages: issues.failedPages.join(', ') })}</li>
              )}
              {issues.unrecognizedPages.length > 0 && (
                <li>{t('library.status.recognitionUnrecognizedPages', { pages: issues.unrecognizedPages.join(', ') })}</li>
              )}
              {issues.lowConfidencePages.length > 0 && (
                <li>{t('library.status.recognitionLowConfidencePages', { pages: issues.lowConfidencePages.join(', ') })}</li>
              )}
            </ul>
          </details>
          {issueAction === 'accept' && !status.recognitionImprovementAttempted && (
            <p className="pdf-recognition-review-help">
              {t('library.status.recognitionRetryHelp')}
            </p>
          )}
          {retryDocumentUrl && (
            <div className="pdf-recognition-actions">
              {issueAction === 'retry' && (
                <button
                  type="button"
                  className="document-batch-cancel pdf-recognition-action pdf-recognition-action-secondary"
                  onClick={() => void onRecognize(retryDocumentUrl, status.recognitionLanguage ?? 'eng')}
                >
                  <RetryIcon />
                  {t('library.status.retryRecognitionPages')}
                </button>
              )}
              {status.status === 'recognized' && issueAction === 'accept' && (
                <>
                  {!status.recognitionImprovementAttempted && (
                    <button
                      type="button"
                      className="document-batch-cancel pdf-recognition-action pdf-recognition-action-secondary"
                      onClick={() => void onRecognize(
                        retryDocumentUrl,
                        status.recognitionLanguage ?? 'eng',
                        issues,
                      )}
                    >
                      <RetryIcon />
                      {t('library.status.improveRecognitionPages')}
                    </button>
                  )}
                  <button
                    type="button"
                    className="document-batch-cancel pdf-recognition-action pdf-recognition-action-primary"
                    onClick={() => void onAccept(retryDocumentUrl)}
                  >
                    <CheckIcon />
                    {t('library.status.useRecognizedText')}
                  </button>
                </>
              )}
            </div>
          )}
          {issueAction === 'accept' && (
            <p className="pdf-recognition-accept-help">
              {t('library.status.recognitionAcceptHelp')}
            </p>
          )}
        </div>
      )}
    </div>
  )
}

function RetryIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M20 11a8 8 0 1 0-2.34 5.66M20 4v7h-7" />
    </svg>
  )
}

function CheckIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="m5 12 4 4L19 6" />
    </svg>
  )
}
