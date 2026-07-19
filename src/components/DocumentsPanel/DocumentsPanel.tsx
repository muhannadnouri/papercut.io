import { useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import type { DocumentInfo } from '../../types/search'
import type { AuthorGroup } from '../../hooks/useDocumentFilters'
import type { UploadedLibraryOrganization } from '../../uploads/DocumentUploads'
import { Panel } from '../Panel/Panel'
import { DocumentList } from '../DocumentList/DocumentList'
import { UploadedLibraryTree } from '../UploadedLibraryTree/UploadedLibraryTree'
import { splitDocumentGroupsByUpload } from '../DocumentBrowser/documentGroups'
import '../DocumentBrowser/DocumentBrowser.css'

interface DocumentsPanelStatus {
  status: string
  message: ReactNode
}

export interface DocumentImportOption {
  id: string
  label: string
  detail?: string
  statusLabel?: string
  disabled?: boolean
  future?: boolean
  onSelect?: () => void
}

interface DocumentsPanelProps {
  allDocuments: DocumentInfo[]
  audioSavedOnly?: boolean
  collapsedAuthors: Set<string>
  docFilterLower: string
  documentFilter: string
  documentsLoading: boolean
  groupedDocs: AuthorGroup[]
  importOptions?: DocumentImportOption[]
  importStatuses?: DocumentsPanelStatus[]
  libraryOrganization?: UploadedLibraryOrganization
  documentOpening?: boolean
  openingDocumentUrl?: string
  showDocuments: boolean
  onAudioSavedOnlyChange?: (enabled: boolean) => void
  onCreateLibraryFolder?: (parentId: string | null, name: string) => void | Promise<void>
  onDeleteDocument?: (doc: DocumentInfo) => void | Promise<void>
  onDeleteLibraryFolder?: (folderId: string) => void | Promise<void>
  onFilterChange: (value: string) => void
  onMoveLibraryDocuments?: (documentIds: string[], folderId: string | null) => void | Promise<void>
  onRenameLibraryFolder?: (folderId: string, name: string) => void | Promise<void>
  onToggleAuthor: (author: string) => void
  onToggleShow: () => void
  onViewDocument: (url: string) => void
}

export function DocumentsPanel({
  allDocuments,
  audioSavedOnly = false,
  collapsedAuthors,
  docFilterLower,
  documentFilter,
  documentsLoading,
  groupedDocs,
  importOptions = [],
  importStatuses = [],
  libraryOrganization,
  documentOpening = false,
  openingDocumentUrl,
  showDocuments,
  onAudioSavedOnlyChange,
  onCreateLibraryFolder,
  onDeleteDocument,
  onDeleteLibraryFolder,
  onFilterChange,
  onMoveLibraryDocuments,
  onRenameLibraryFolder,
  onToggleAuthor,
  onToggleShow,
  onViewDocument,
}: DocumentsPanelProps) {
  const { t } = useTranslation()
  const [importMenuOpen, setImportMenuOpen] = useState(false)
  const activeImport = importOptions.find((option) => option.statusLabel)
  const hasImportOptions = importOptions.length > 0
  const operationBusy = importStatuses.some((item) => item.status === 'importing' || item.status === 'deleting')
  const importDisabled = hasImportOptions && importOptions.every((option) => option.disabled || option.future || !option.onSelect)
  const { uploadDocs, nonUploadGroups } = splitDocumentGroupsByUpload(groupedDocs)
  const canShowUploadedTree = Boolean(
    libraryOrganization &&
    onCreateLibraryFolder &&
    onDeleteLibraryFolder &&
    onMoveLibraryDocuments &&
    onRenameLibraryFolder,
  )

  if (documentsLoading) {
    return (
      <div className="documents-panel documents-panel-loading">
        <div className="documents-loading">
          <span className="spinner" aria-hidden="true" />
          <span>{t('library.documents.loading')}</span>
        </div>
      </div>
    )
  }

  return (
    <Panel
      className={'document-browser-panel documents-panel' + (importMenuOpen ? ' document-browser-panel-menu-open documents-panel-menu-open' : '')}
      ariaLabel={t('library.documents.ariaLabel')}
      title={t('library.documents.title', { count: allDocuments.length })}
      open={showDocuments}
      onToggle={onToggleShow}
    >
      <div className="documents-list-header">
        <input
          type="text"
          dir="auto"
          className="document-filter-input"
          placeholder={t('library.documents.filterPlaceholder')}
          value={documentFilter}
          onChange={(e) => onFilterChange(e.target.value)}
        />
        {hasImportOptions && (
          <div className="document-import-menu">
            <button
              className="document-import-btn"
              aria-expanded={importMenuOpen}
              disabled={importDisabled}
              onClick={() => setImportMenuOpen((value) => !value)}
              type="button"
            >
              {activeImport?.statusLabel ?? t('library.documents.import')}
              <span className={`toggle-arrow ${importMenuOpen ? 'open' : ''}`}>&#9662;</span>
            </button>
            {importMenuOpen && (
              <div className="document-import-options">
                {importOptions.map((option) => {
                  const disabled = option.disabled || option.future || !option.onSelect
                  return (
                    <button
                      key={option.id}
                      className="document-import-option"
                      disabled={disabled}
                      onClick={() => {
                        setImportMenuOpen(false)
                        option.onSelect?.()
                      }}
                      type="button"
                    >
                      <span>{option.label}{option.future ? ` (${t('library.documents.future')})` : ''}</span>
                      {option.detail && <small>{option.detail}</small>}
                    </button>
                  )
                })}
              </div>
            )}
          </div>
        )}
        {onAudioSavedOnlyChange && (
          <label className="audio-filter-toggle">
            <input
              type="checkbox"
              checked={audioSavedOnly}
              onChange={(e) => onAudioSavedOnlyChange(e.target.checked)}
            />
            <span>{t('library.documents.savedAudio')}</span>
          </label>
        )}
      </div>

      {importStatuses.map((item, index) => item.message && item.status !== 'idle' ? (
        <div key={item.status + index} className={'document-import-status document-import-' + item.status}>
          {item.message}
        </div>
      ) : null)}

      {canShowUploadedTree && libraryOrganization && (
        <UploadedLibraryTree
          documents={uploadDocs}
          organization={libraryOrganization}
          documentOpening={documentOpening}
          deleteDisabled={operationBusy || documentOpening}
          openingDocumentUrl={openingDocumentUrl}
          onCreateFolder={onCreateLibraryFolder!}
          onDeleteDocument={onDeleteDocument}
          onDeleteFolder={onDeleteLibraryFolder!}
          onMoveDocuments={onMoveLibraryDocuments!}
          onRenameFolder={onRenameLibraryFolder!}
          onViewDocument={onViewDocument}
        />
      )}

      {(!canShowUploadedTree || nonUploadGroups.length > 0 || uploadDocs.length === 0) && (
        <DocumentList
          groupedDocs={canShowUploadedTree ? nonUploadGroups : groupedDocs}
          collapsedAuthors={collapsedAuthors}
          docFilterLower={docFilterLower}
          emptyMessage={
            allDocuments.length === 0
              ? t('library.documents.empty')
              : audioSavedOnly
                ? t('library.documents.emptySavedAudio')
                : documentFilter.trim()
                  ? t('library.documents.emptyFilter')
                  : t('library.documents.empty')
          }
          onToggleAuthor={onToggleAuthor}
          onViewDocument={onViewDocument}
          onDeleteDocument={onDeleteDocument}
          deleteDisabled={operationBusy || documentOpening}
          openingDocumentUrl={openingDocumentUrl}
          viewDisabled={documentOpening}
        />
      )}
    </Panel>
  )
}
