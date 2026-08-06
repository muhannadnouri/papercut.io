import { Trans, useTranslation } from 'react-i18next'
import type { TFunction } from 'i18next'
import { useState, type ReactNode } from 'react'
import type { AuthorGroup } from '../../hooks/useDocumentFilters'
import type { DocumentImportStatus } from '../../hooks/useUploadedLibrary'
import { PdfRecognitionStatus } from '../../pdf/ocr/PdfRecognitionStatus'
import type { DocumentInfo } from '../../types/search'
import type { UploadedDocumentDeleteBatchResult, UploadedLibraryOrganization } from '../../uploads/DocumentUploads'
import { formatStorageSize } from '../../utils/formatUtils'
import { isMobileUserAgent } from '../../utils/platform'
import { DocumentsPanel } from '../DocumentsPanel/DocumentsPanel'
import { DocumentInfoDialog } from '../DocumentInfoDialog/DocumentInfoDialog'

interface LibraryTabProps {
  allDocuments: DocumentInfo[]
  audioSavedOnly: boolean
  bookmarkedDocumentUrls: ReadonlySet<string>
  collapsedAuthors: Set<string>
  docFilterLower: string
  documentFilter: string
  documentImport: DocumentImportStatus
  documentScannerSupported: boolean
  documentPhotoImportSupported: boolean
  documentOpening: boolean
  documentsLoading: boolean
  groupedDocs: AuthorGroup[]
  libraryOrganization: UploadedLibraryOrganization
  openingDocumentUrl?: string
  savedAudiobookDocumentUrls: ReadonlySet<string>
  showDocuments: boolean
  onAudioSavedOnlyChange: (enabled: boolean) => void
  onAcceptRecognizedDocument: (documentUrl: string) => void | Promise<boolean>
  onCreateLibraryFolder: (parentId: string | null, name: string) => void | Promise<void>
  onDeleteDocument: (doc: DocumentInfo) => void | Promise<void>
  onDeleteDocuments: (docs: DocumentInfo[]) => Promise<UploadedDocumentDeleteBatchResult | null>
  onDeleteLibraryFolder: (folderId: string) => void | Promise<void>
  onDismissDocumentImportStatus: () => void
  onFilterChange: (value: string) => void
  onCancelDocumentBatch: () => void | Promise<void>
  onImportDocumentBatch: () => void | Promise<void>
  onImportDocumentFolder: () => void | Promise<void>
  onImportDocumentPhotos: () => void | Promise<void>
  onScanDocument: () => void | Promise<void>
  onMoveLibraryDocuments: (documentIds: string[], folderId: string | null) => void | Promise<void>
  onRecognizeDocument: (documentUrl: string) => void | Promise<boolean>
  onRenameLibraryFolder: (folderId: string, name: string) => void | Promise<void>
  onToggleAuthor: (author: string) => void
  onToggleShow: () => void
  onUpdateDocumentTitle: (documentUrl: string, title: string) => Promise<void>
  onViewAudiobooks: () => void
  onViewDocument: (url: string) => void | Promise<void>
}

export function LibraryTab({
  allDocuments,
  audioSavedOnly,
  bookmarkedDocumentUrls,
  collapsedAuthors,
  docFilterLower,
  documentFilter,
  documentImport,
  documentScannerSupported,
  documentPhotoImportSupported,
  documentOpening,
  documentsLoading,
  groupedDocs,
  libraryOrganization,
  openingDocumentUrl,
  savedAudiobookDocumentUrls,
  showDocuments,
  onAudioSavedOnlyChange,
  onAcceptRecognizedDocument,
  onCreateLibraryFolder,
  onDeleteDocument,
  onDeleteDocuments,
  onDeleteLibraryFolder,
  onDismissDocumentImportStatus,
  onFilterChange,
  onCancelDocumentBatch,
  onImportDocumentBatch,
  onImportDocumentFolder,
  onImportDocumentPhotos,
  onScanDocument,
  onMoveLibraryDocuments,
  onRecognizeDocument,
  onRenameLibraryFolder,
  onToggleAuthor,
  onToggleShow,
  onUpdateDocumentTitle,
  onViewAudiobooks,
  onViewDocument,
}: LibraryTabProps) {
  const { t } = useTranslation()
  const [infoDocument, setInfoDocument] = useState<DocumentInfo | null>(null)
  const operationBusy = documentImport.status === 'importing' ||
    documentImport.status === 'recognizing' || documentImport.status === 'deleting'
  const statusMessage = documentImportStatusMessage(
    documentImport,
    t,
    onCancelDocumentBatch,
    onRecognizeDocument,
    onAcceptRecognizedDocument,
    allDocuments,
  )
  const folderImportSupported = !isMobileUserAgent()

  return (
    <section className="tab-panel" role="tabpanel" aria-label={t('library.tabLabel')} data-tab="library">
      <DocumentsPanel
        documentsLoading={documentsLoading}
        showDocuments={showDocuments}
        allDocuments={allDocuments}
        audioSavedOnly={audioSavedOnly}
        bookmarkedDocumentUrls={bookmarkedDocumentUrls}
        documentFilter={documentFilter}
        groupedDocs={groupedDocs}
        docFilterLower={docFilterLower}
        importOptions={[
          {
            id: 'batch',
            label: t('library.import.files'),
            detail: t('library.import.filesDetail'),
            statusLabel: documentImport.status === 'importing' && documentImport.format === 'batch'
              ? t('library.import.importingBatch')
              : undefined,
            disabled: operationBusy,
            onSelect: onImportDocumentBatch,
          },
          ...(folderImportSupported ? [{
            id: 'folder',
            label: t('library.import.folder'),
            detail: t('library.import.folderDetail'),
            statusLabel: documentImport.status === 'importing' && documentImport.format === 'folder'
              ? t('library.import.importingFolder')
              : undefined,
            disabled: operationBusy,
            onSelect: onImportDocumentFolder,
          }] : []),
          ...(documentScannerSupported ? [{
            id: 'scan',
            label: t('library.import.scanPages'),
            detail: t('library.import.scanPagesDetail'),
            statusLabel: documentImport.status === 'importing' && documentImport.format === 'scan'
              ? t('library.import.scanningPages')
              : undefined,
            disabled: operationBusy,
            onSelect: onScanDocument,
          }] : []),
          ...(documentPhotoImportSupported ? [{
            id: 'photos',
            label: t('library.import.photos'),
            detail: t('library.import.photosDetail'),
            statusLabel: documentImport.status === 'importing' && documentImport.format === 'photos'
              ? t('library.import.importingPhotos')
              : undefined,
            disabled: operationBusy,
            onSelect: onImportDocumentPhotos,
          }] : []),
          // { id: 'pdf', label: 'PDF', detail: 'Import PDFs when text extraction support lands', future: true },
        ]}
        importStatuses={statusMessage ? [{
          status: documentImport.status,
          message: statusMessage,
          onDismiss: operationBusy ? undefined : onDismissDocumentImportStatus,
        }] : []}
        libraryOrganization={libraryOrganization}
        documentOpening={documentOpening}
        openingDocumentUrl={openingDocumentUrl}
        savedAudiobookDocumentUrls={savedAudiobookDocumentUrls}
        collapsedAuthors={collapsedAuthors}
        onToggleShow={onToggleShow}
        onFilterChange={onFilterChange}
        onAudioSavedOnlyChange={onAudioSavedOnlyChange}
        onCreateLibraryFolder={onCreateLibraryFolder}
        onDeleteDocument={onDeleteDocument}
        onDeleteDocuments={onDeleteDocuments}
        onDeleteLibraryFolder={onDeleteLibraryFolder}
        onMoveLibraryDocuments={onMoveLibraryDocuments}
        onRenameLibraryFolder={onRenameLibraryFolder}
        onToggleAuthor={onToggleAuthor}
        onViewAudiobooks={onViewAudiobooks}
        onViewDocumentInfo={setInfoDocument}
        onViewDocument={onViewDocument}
      />
      {infoDocument && (
        <DocumentInfoDialog
          document={infoDocument}
          onCancel={() => setInfoDocument(null)}
          onSave={async (document, title) => {
            await onUpdateDocumentTitle(document.url, title)
          }}
        />
      )}
    </section>
  )
}

/** Render semantic upload state where translations and bidi isolation are available. */
function documentImportStatusMessage(
  status: DocumentImportStatus,
  t: TFunction,
  onCancelBatch: () => void | Promise<void>,
  onRecognizeDocument: (documentUrl: string) => void | Promise<boolean>,
  onAcceptRecognizedDocument: (documentUrl: string) => void | Promise<boolean>,
  documents: DocumentInfo[],
): ReactNode {
  if (status.status === 'idle') return null
  if (status.format === 'delete-batch') {
    return <DocumentBatchDeleteStatus status={status} t={t} documents={documents} />
  }
  if (status.format === 'pdf-ocr') {
    return <PdfRecognitionStatus
      status={status}
      t={t}
      onCancel={onCancelBatch}
      onRetry={onRecognizeDocument}
      onAccept={onAcceptRecognizedDocument}
    />
  }
  if (status.format === 'batch' || status.format === 'folder' || status.format === 'scan' ||
      status.format === 'photos') {
    return <DocumentBatchImportStatus status={status} t={t} onCancel={onCancelBatch} />
  }
  if (status.status === 'cancelled') return t('library.status.cancelled')
  if (status.status === 'error') return status.message ?? null

  const title = status.title ?? ''
  if (status.status === 'imported') {
    return <Trans i18nKey="library.status.imported" values={{ title }} components={{ title: <bdi /> }} />
  }
  if (status.status === 'deleting') {
    return <Trans i18nKey="library.status.deleting" values={{ title }} components={{ title: <bdi /> }} />
  }

  const storage = formatStorageSize(status.bytesFreed)
  return storage
    ? <Trans i18nKey="library.status.deletedWithStorage" values={{ title, storage }} components={{ title: <bdi /> }} />
    : <Trans i18nKey="library.status.deleted" values={{ title }} components={{ title: <bdi /> }} />
}

/** Present native delete progress and retain readable titles for partial failures. */
function DocumentBatchDeleteStatus({
  status,
  t,
  documents,
}: {
  status: DocumentImportStatus
  t: TFunction
  documents: DocumentInfo[]
}) {
  const progress = status.deleteProgress
  const result = status.deleteResult
  const failures = result?.failures ?? []
  const deleting = status.status === 'deleting'
  const total = progress?.total ?? 0
  const processed = progress?.processed ?? 0
  const titlesByUrl = new Map(documents.map((doc) => [doc.url, doc.title]))

  const message = deleting
    ? total > 0
      ? t('library.status.deleteBatchProgress', { processed, total })
      : t('library.status.preparingDeleteBatch')
    : result
      ? t('library.status.deleteBatchComplete', {
          deleted: result.deleted.length,
          failed: failures.length,
        })
      : status.message

  return (
    <div className="document-batch-status">
      <div className="document-batch-status-row">
        <span>{message}</span>
      </div>
      {deleting && (
        <progress
          className="document-batch-progress"
          aria-label={t('library.status.deleteBatchProgressLabel')}
          max={total || undefined}
          value={total ? processed : undefined}
        />
      )}
      {failures.length > 0 && (
        <details className="document-batch-failures">
          <summary>{t('library.status.failedDeletes', { count: failures.length })}</summary>
          <ul>
            {failures.map((failure) => (
              <li key={failure.documentUrl}>
                <bdi>{titlesByUrl.get(failure.documentUrl) ?? failure.documentUrl}</bdi>:{' '}
                <span dir="auto">{failure.error}</span>
              </li>
            ))}
          </ul>
        </details>
      )}
    </div>
  )
}

/** Keep the long-running batch status in the existing library status row while
 * exposing determinate counts, cooperative cancellation, and per-file errors. */
function DocumentBatchImportStatus({
  status,
  t,
  onCancel,
}: {
  status: DocumentImportStatus
  t: TFunction
  onCancel: () => void | Promise<void>
}) {
  const progress = status.batchProgress
  const result = status.batchResult
  const failures = result?.failures ?? []
  const importing = status.status === 'importing'
  const total = progress?.total ?? 0
  const processed = progress?.processed ?? 0
  const current = progress?.fileName ? Math.min(processed + 1, total) : processed

  let message: ReactNode
  if (importing && status.cancelRequested) {
    message = t('library.status.stoppingBatch')
  } else if (importing && total > 0) {
    message = (
      <>
        {t(progress?.fileName ? 'library.status.importingBatchProgress' : 'library.status.batchProgress', {
          current,
          processed,
          total,
        })}
        {progress?.fileName ? <> · <bdi>{progress.fileName}</bdi></> : null}
      </>
    )
  } else if (importing) {
    message = t(status.format === 'scan'
      ? 'library.status.capturingPages'
      : status.format === 'photos'
        ? 'library.status.preparingPhotos'
        : 'library.status.preparingBatch')
  } else if (result) {
    message = t(result.cancelled ? 'library.status.batchCancelled' : 'library.status.batchComplete', {
      imported: result.imported.length,
      failed: failures.length,
    })
  } else {
    message = status.status === 'error' ? status.message : t('library.status.cancelled')
  }

  return (
    <div className="document-batch-status">
      <div className="document-batch-status-row">
        <span>{message}</span>
        {importing && (
          <button
            type="button"
            className="document-batch-cancel"
            disabled={status.cancelRequested}
            onClick={() => void onCancel()}
          >
            {t(status.cancelRequested ? 'library.status.stopping' : 'common.cancel')}
          </button>
        )}
      </div>
      {importing && (
        <progress
          className="document-batch-progress"
          aria-label={t('library.status.batchProgressLabel')}
          max={total || undefined}
          value={total ? processed : undefined}
        />
      )}
      {failures.length > 0 && (
        <details className="document-batch-failures">
          <summary>{t('library.status.failedFiles', { count: failures.length })}</summary>
          <ul>
            {failures.map((failure, index) => (
              <li key={`${failure.fileName}-${index}`}>
                <bdi>{failure.fileName}</bdi>: <span dir="auto">{failure.error}</span>
              </li>
            ))}
          </ul>
        </details>
      )}
    </div>
  )
}
