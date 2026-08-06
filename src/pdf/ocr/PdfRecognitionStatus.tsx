import { Trans } from 'react-i18next'
import type { TFunction } from 'i18next'
import type { ReactNode } from 'react'
import type { DocumentImportStatus } from '../../hooks/useUploadedLibrary'
import { PDF_OCR_NO_TEXT } from './recognizePdf'

/** Present the shared OCR progress, cancellation, and retry lifecycle. */
export function PdfRecognitionStatus({
  status,
  t,
  onCancel,
  onRetry,
}: {
  status: DocumentImportStatus
  t: TFunction
  onCancel: () => void | Promise<void>
  onRetry: (documentUrl: string) => void | Promise<boolean>
}) {
  const progress = status.recognitionProgress
  const recognizing = status.status === 'recognizing'
  const title = status.title ?? ''
  const issues = status.recognitionIssues
  const retryDocumentUrl = status.documentUrl
  const issueCount = (issues?.failedPages.length ?? 0) + (issues?.lowConfidencePages.length ?? 0)
  let message: ReactNode

  if (recognizing && status.cancelRequested) {
    message = t('library.status.stoppingRecognition')
  } else if (recognizing && progress?.phase === 'recognizing') {
    message = <Trans i18nKey="library.status.recognizingPage" values={{ title, current: progress.pageNumber, total: progress.pageCount }} components={{ title: <bdi /> }} />
  } else if (recognizing && progress?.phase === 'indexing') {
    message = <Trans i18nKey="library.status.indexingRecognition" values={{ title }} components={{ title: <bdi /> }} />
  } else if (recognizing) {
    message = <Trans i18nKey="library.status.preparingRecognition" values={{ title }} components={{ title: <bdi /> }} />
  } else if (status.status === 'recognized' && issueCount > 0) {
    message = <Trans i18nKey="library.status.recognitionNeedsReview" values={{ title, count: issueCount }} components={{ title: <bdi /> }} />
  } else if (status.status === 'recognized') {
    message = <Trans i18nKey="library.status.recognitionComplete" values={{ title }} components={{ title: <bdi /> }} />
  } else if (status.status === 'cancelled') {
    message = <Trans i18nKey="library.status.recognitionCancelled" values={{ title }} components={{ title: <bdi /> }} />
  } else {
    message = status.message === PDF_OCR_NO_TEXT
      ? t('library.status.recognitionNoText')
      : status.message
  }

  return (
    <div className="document-batch-status">
      <div className="document-batch-status-row">
        <span>{message}</span>
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
        <details className="document-batch-failures">
          <summary>{t('library.status.recognitionIssues', { count: issueCount })}</summary>
          <ul>
            {issues.failedPages.length > 0 && (
              <li>{t('library.status.recognitionFailedPages', { pages: issues.failedPages.join(', ') })}</li>
            )}
            {issues.lowConfidencePages.length > 0 && (
              <li>{t('library.status.recognitionLowConfidencePages', { pages: issues.lowConfidencePages.join(', ') })}</li>
            )}
          </ul>
          {retryDocumentUrl && (
            <button
              type="button"
              className="document-batch-cancel"
              onClick={() => void onRetry(retryDocumentUrl)}
            >
              {t('library.status.retryRecognitionPages')}
            </button>
          )}
        </details>
      )}
    </div>
  )
}
