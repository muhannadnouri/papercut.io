import { useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import type { TFunction } from 'i18next'
import { Button, Menu, MenuItem, MenuTrigger, Popover } from 'react-aria-components'
import type { DocumentInfo } from '../../types/search'
import type { AuthorGroup } from '../../hooks/useDocumentFilters'
import type { UploadedDocumentDeleteBatchResult, UploadedLibraryOrganization } from '../../uploads/DocumentUploads'
import { Panel } from '../Panel/Panel'
import { DocumentList } from '../DocumentList/DocumentList'
import { UploadedLibraryTree } from '../UploadedLibraryTree/UploadedLibraryTree'
import { LibraryGalleryView, type LibraryGalleryCategory } from '../LibraryGalleryView/LibraryGalleryView'
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
  developerMode?: boolean
  docFilterLower: string
  documentFilter: string
  documentsLoading: boolean
  groupedDocs: AuthorGroup[]
  importOptions?: DocumentImportOption[]
  importStatuses?: DocumentsPanelStatus[]
  libraryOrganization?: UploadedLibraryOrganization
  documentOpening?: boolean
  openingDocumentUrl?: string
  savedAudiobookDocumentUrls?: ReadonlySet<string>
  showDocuments: boolean
  onAudioSavedOnlyChange?: (enabled: boolean) => void
  onCreateLibraryFolder?: (parentId: string | null, name: string) => void | Promise<void>
  onDeleteDocument?: (doc: DocumentInfo) => void | Promise<void>
  onDeleteDocuments?: (docs: DocumentInfo[]) => Promise<UploadedDocumentDeleteBatchResult | null>
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
  developerMode = false,
  docFilterLower,
  documentFilter,
  documentsLoading,
  groupedDocs,
  importOptions = [],
  importStatuses = [],
  libraryOrganization,
  documentOpening = false,
  openingDocumentUrl,
  savedAudiobookDocumentUrls = new Set(),
  showDocuments,
  onAudioSavedOnlyChange,
  onCreateLibraryFolder,
  onDeleteDocument,
  onDeleteDocuments,
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
  const [preferredView, setPreferredView] = useState<LibraryView>(loadView)
  const [galleryCategory, setGalleryCategory] = useState<LibraryGalleryCategory>(
    loadCategory,
  )
  const view = developerMode ? preferredView : 'list'
  const activeImport = importOptions.find((option) => option.statusLabel)
  const hasImportOptions = importOptions.length > 0
  const importBusy = importStatuses.some((item) => item.status === 'importing')
  const operationBusy = importStatuses.some((item) => item.status === 'importing' || item.status === 'deleting')
  const importDisabled = hasImportOptions && importOptions.every((option) => option.disabled || option.future || !option.onSelect)
  const { uploadDocs, nonUploadGroups } = splitDocumentGroupsByUpload(groupedDocs)
  const canShowUploadedTree = Boolean(
    libraryOrganization &&
    onCreateLibraryFolder &&
    onDeleteDocuments &&
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
      className="document-browser-panel documents-panel"
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
            <MenuTrigger isOpen={importMenuOpen} onOpenChange={setImportMenuOpen}>
              <Button className="document-import-btn" isDisabled={importDisabled}>
                {activeImport?.statusLabel ?? t('library.documents.import')}
                <span className={`toggle-arrow ${importMenuOpen ? 'open' : ''}`} aria-hidden="true">&#9662;</span>
              </Button>
              <Popover
                className="document-import-popover"
                placement="bottom end"
                offset={6}
                containerPadding={8}
                shouldFlip
              >
                <Menu className="document-import-options" aria-label={t('library.documents.import')}>
                  {importOptions.map((option) => {
                    const disabled = option.disabled || option.future || !option.onSelect
                    const label = option.label + (option.future ? ` (${t('library.documents.future')})` : '')
                    return (
                      <MenuItem
                        key={option.id}
                        id={option.id}
                        className="document-import-option"
                        isDisabled={disabled}
                        textValue={label}
                        aria-label={option.detail ? `${label}. ${option.detail}` : label}
                        onAction={option.onSelect}
                      >
                        <span>{label}</span>
                        {option.detail && <small>{option.detail}</small>}
                      </MenuItem>
                    )
                  })}
                </Menu>
              </Popover>
            </MenuTrigger>
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
        {developerMode && (
          <div className="library-view-options" role="group" aria-label={t('library.documents.viewLabel')}>
            <button
              type="button"
              className={view === 'gallery' ? 'library-view-option active' : 'library-view-option'}
              aria-pressed={view === 'gallery'}
              onClick={() => {
                setPreferredView('gallery')
                savePreference(VIEW_STORAGE_KEY, 'gallery')
              }}
            >
              ▦ {t('library.documents.galleryView')}
            </button>
            <button
              type="button"
              className={view === 'list' ? 'library-view-option active' : 'library-view-option'}
              aria-pressed={view === 'list'}
              onClick={() => {
                setPreferredView('list')
                savePreference(VIEW_STORAGE_KEY, 'list')
              }}
            >
              ☰ {t('library.documents.listView')}
            </button>
          </div>
        )}
      </div>

      {importStatuses.map((item, index) => item.message && item.status !== 'idle' ? (
        <div key={item.status + index} className={'document-import-status document-import-' + item.status}>
          {item.message}
        </div>
      ) : null)}

      {view === 'gallery' ? (
        <LibraryGalleryView
          category={galleryCategory}
          collapsedAuthors={collapsedAuthors}
          docFilterLower={docFilterLower}
          groupedDocs={groupedDocs}
          savedAudiobookDocumentUrls={savedAudiobookDocumentUrls}
          documentOpening={documentOpening}
          mutationDisabled={operationBusy}
          openingDocumentUrl={openingDocumentUrl}
          emptyMessage={emptyMessage(allDocuments.length, audioSavedOnly, documentFilter, t)}
          onCategoryChange={(category) => {
            setGalleryCategory(category)
            savePreference(CATEGORY_STORAGE_KEY, category)
          }}
          onDeleteDocument={onDeleteDocument}
          onToggleAuthor={onToggleAuthor}
          onViewDocument={onViewDocument}
        />
      ) : (
        <>
          {canShowUploadedTree && libraryOrganization && (
            <UploadedLibraryTree
              documents={uploadDocs}
              organization={libraryOrganization}
              documentOpening={documentOpening}
              mutationDisabled={operationBusy}
              resetEditing={importBusy}
              openingDocumentUrl={openingDocumentUrl}
              onCreateFolder={onCreateLibraryFolder!}
              onDeleteDocuments={onDeleteDocuments!}
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
              emptyMessage={emptyMessage(allDocuments.length, audioSavedOnly, documentFilter, t)}
              onToggleAuthor={onToggleAuthor}
              onViewDocument={onViewDocument}
              onDeleteDocument={onDeleteDocument}
              deleteDisabled={operationBusy || documentOpening}
              openingDocumentUrl={openingDocumentUrl}
              viewDisabled={documentOpening}
            />
          )}
        </>
      )}
    </Panel>
  )
}

type LibraryView = 'gallery' | 'list'

const VIEW_STORAGE_KEY = 'papercut.library-view.v1'
const CATEGORY_STORAGE_KEY = 'papercut.library-gallery-category.v1'

function emptyMessage(documentCount: number, audioSavedOnly: boolean, filter: string, t: TFunction): string {
  if (documentCount === 0) return t('library.documents.empty')
  if (audioSavedOnly) return t('library.documents.emptySavedAudio')
  return filter.trim() ? t('library.documents.emptyFilter') : t('library.documents.empty')
}

function loadView(): LibraryView {
  try {
    return window.localStorage.getItem(VIEW_STORAGE_KEY) === 'gallery' ? 'gallery' : 'list'
  } catch {
    return 'list'
  }
}

function loadCategory(): LibraryGalleryCategory {
  try {
    return window.localStorage.getItem(CATEGORY_STORAGE_KEY) === 'documents' ? 'documents' : 'books'
  } catch {
    return 'books'
  }
}

function savePreference(key: string, value: string): void {
  try {
    window.localStorage.setItem(key, value)
  } catch {
    // The view still changes when preference persistence is unavailable.
  }
}
