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
  onImportEpubDocument: () => void | Promise<void>
  onImportHtmlDocument: () => void | Promise<void>
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
  onImportEpubDocument,
  onImportHtmlDocument,
  onMoveLibraryDocuments,
  onRenameLibraryFolder,
  onToggleAuthor,
  onToggleShow,
  onViewDocument,
}: LibraryTabProps) {
  const { t } = useTranslation()
  const operationBusy = documentImport.status === 'importing' || documentImport.status === 'deleting'
  const statusMessage = documentImportStatusMessage(documentImport, t)

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
            id: 'html',
            label: 'HTML',
            detail: t('library.import.htmlDetail'),
            statusLabel: documentImport.status === 'importing' && documentImport.format === 'html'
              ? t('library.import.importingHtml')
              : undefined,
            disabled: operationBusy,
            onSelect: onImportHtmlDocument,
          },
          {
            id: 'epub',
            label: 'EPUB',
            detail: t('library.import.epubDetail'),
            statusLabel: documentImport.status === 'importing' && documentImport.format === 'epub'
              ? t('library.import.importingEpub')
              : undefined,
            disabled: operationBusy,
            onSelect: onImportEpubDocument,
          },
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
function documentImportStatusMessage(status: DocumentImportStatus, t: TFunction): ReactNode {
  if (status.status === 'idle') return null
  if (status.status === 'importing') {
    return t(status.format === 'epub' ? 'library.status.importingEpub' : 'library.status.importingHtml')
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
