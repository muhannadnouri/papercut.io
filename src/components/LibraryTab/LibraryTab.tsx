import type { AuthorGroup } from '../../hooks/useDocumentFilters'
import type { DocumentInfo } from '../../types/search'
import type { UploadedLibraryOrganization } from '../../uploads/DocumentUploads'
import { DocumentsPanel } from '../DocumentsPanel/DocumentsPanel'

interface DocumentImportStatus {
  status: string
  message: string
}

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
  return (
    <section className="tab-panel" role="tabpanel" aria-label="Library" data-tab="library">
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
            detail: 'Import a local .html or .htm document',
            statusLabel: documentImport.status === 'importing' && documentImport.message.includes('HTML') ? 'Importing HTML' : undefined,
            disabled: documentImport.status === 'importing',
            onSelect: onImportHtmlDocument,
          },
          {
            id: 'epub',
            label: 'EPUB',
            detail: 'Import a local .epub book',
            statusLabel: documentImport.status === 'importing' && documentImport.message.includes('EPUB') ? 'Importing EPUB' : undefined,
            disabled: documentImport.status === 'importing',
            onSelect: onImportEpubDocument,
          },
          // { id: 'pdf', label: 'PDF', detail: 'Import PDFs when text extraction support lands', future: true },
        ]}
        importStatuses={[documentImport]}
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
