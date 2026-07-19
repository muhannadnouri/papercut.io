import { Trans, useTranslation } from 'react-i18next'
import type { TFunction } from 'i18next'
import type { ReactNode } from 'react'
import type { AuthorGroup } from '../../hooks/useDocumentFilters'
import type { DocumentImportStatus } from '../../hooks/useUploadedLibrary'
import type { DocumentInfo } from '../../types/search'
import type { UploadedLibraryOrganization } from '../../uploads/DocumentUploads'
import { formatStorageSize } from '../../utils/formatUtils'
import { DocumentsPanel } from '../DocumentsPanel/DocumentsPanel'

interface LibraryTabProps {
  allDocuments: DocumentInfo[]
  audioSavedOnly: boolean
  collapsedAuthors: Set<string>
  docFilterLower: string
  documentFilter: string
  documentImport: DocumentImportStatus
  documentOpening: boolean
  documentsLoading: boolean
  groupedDocs: AuthorGroup[]
  libraryOrganization: UploadedLibraryOrganization
  openingDocumentUrl?: string
  showDocuments: boolean
  onAudioSavedOnlyChange: (enabled: boolean) => void
  onCreateLibraryFolder: (parentId: string | null, name: string) => void | Promise<void>
  onDeleteDocument: (doc: DocumentInfo) => void | Promise<void>
  onDeleteLibraryFolder: (folderId: string) => void | Promise<void>
  onFilterChange: (value: string) => void
  onCancelDocumentBatch: () => void | Promise<void>
  onImportDocumentBatch: () => void | Promise<void>
  onImportDocumentFolder: () => void | Promise<void>
  onMoveLibraryDocuments: (documentIds: string[], folderId: string | null) => void | Promise<void>
  onRenameLibraryFolder: (folderId: string, name: string) => void | Promise<void>
  onToggleAuthor: (author: string) => void
  onToggleShow: () => void
  onViewDocument: (url: string) => void | Promise<void>
}

export function LibraryTab({
  allDocuments,
  audioSavedOnly,
  collapsedAuthors,
  docFilterLower,
  documentFilter,
  documentImport,
  documentOpening,
  documentsLoading,
  groupedDocs,
  libraryOrganization,
  openingDocumentUrl,
  showDocuments,
  onAudioSavedOnlyChange,
  onCreateLibraryFolder,
  onDeleteDocument,
  onDeleteLibraryFolder,
  onFilterChange,
  onCancelDocumentBatch,
  onImportDocumentBatch,
  onImportDocumentFolder,
  onMoveLibraryDocuments,
  onRenameLibraryFolder,
  onToggleAuthor,
  onToggleShow,
  onViewDocument,
}: LibraryTabProps) {
  const { t } = useTranslation()
  const operationBusy = documentImport.status === 'importing' || documentImport.status === 'deleting'
  const statusMessage = documentImportStatusMessage(documentImport, t, onCancelDocumentBatch)
  const folderImportSupported = !isMobilePlatform()

  return (
    <section className="tab-panel" role="tabpanel" aria-label={t('library.tabLabel')} data-tab="library">
      <DocumentsPanel
        documentsLoading={documentsLoading}
        showDocuments={showDocuments}
        allDocuments={allDocuments}
        audioSavedOnly={audioSavedOnly}
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
          // { id: 'pdf', label: 'PDF', detail: 'Import PDFs when text extraction support lands', future: true },
        ]}
        importStatuses={statusMessage ? [{ status: documentImport.status, message: statusMessage }] : []}
        libraryOrganization={libraryOrganization}
        documentOpening={documentOpening}
        openingDocumentUrl={openingDocumentUrl}
        collapsedAuthors={collapsedAuthors}
        onToggleShow={onToggleShow}
        onFilterChange={onFilterChange}
        onAudioSavedOnlyChange={onAudioSavedOnlyChange}
        onCreateLibraryFolder={onCreateLibraryFolder}
        onDeleteDocument={onDeleteDocument}
        onDeleteLibraryFolder={onDeleteLibraryFolder}
        onMoveLibraryDocuments={onMoveLibraryDocuments}
        onRenameLibraryFolder={onRenameLibraryFolder}
        onToggleAuthor={onToggleAuthor}
        onViewDocument={onViewDocument}
      />
    </section>
  )
}

/** Render semantic upload state where translations and bidi isolation are available. */
function documentImportStatusMessage(
  status: DocumentImportStatus,
  t: TFunction,
  onCancelBatch: () => void | Promise<void>,
): ReactNode {
  if (status.status === 'idle') return null
  if (status.format === 'batch' || status.format === 'folder') {
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

/** Folder enumeration needs a real desktop filesystem; mobile retains the
 * portable single/multi-file pickers without exposing an unsupported action. */
function isMobilePlatform(): boolean {
  return /Android|iP(ad|hone|od)/.test(navigator.userAgent) ||
    (navigator.platform === 'MacIntel' && navigator.maxTouchPoints > 1)
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
    message = t('library.status.preparingBatch')
  } else if (result) {
    message = t(result.cancelled ? 'library.status.batchCancelled' : 'library.status.batchComplete', {
      imported: result.imported.length,
      failed: failures.length,
    })
  } else {
    message = status.status === 'error' ? status.message : t('library.status.cancelled')
  }

  return (
    <div className="document-batch-status" aria-live="polite">
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
