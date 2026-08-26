import { useState, type ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import type { TFunction } from 'i18next'
import { Button, Menu, MenuItem, MenuTrigger, Popover } from 'react-aria-components'
import type { DocumentInfo } from '../../types/search'
import type { AuthorGroup } from '../../hooks/useDocumentFilters'
import type { UploadedDocumentDeleteBatchResult, UploadedLibraryOrganization } from '../../uploads/DocumentUploads'
import { BookmarkIcon } from '../BookmarkIndicator/BookmarkIndicator'
import { filterBookmarkedGroups } from '../BookmarkIndicator/bookmarkFilters'
import { BundledDocumentTree } from '../BundledDocumentTree/BundledDocumentTree'
import { Panel } from '../Panel/Panel'
import { DocumentList } from '../DocumentList/DocumentList'
import { UploadedLibraryTree } from '../UploadedLibraryTree/UploadedLibraryTree'
import { LibraryGalleryView, type LibraryGalleryCategory } from '../LibraryGalleryView/LibraryGalleryView'
import { splitDocumentGroupsBySource } from '../DocumentBrowser/documentGroups'
import '../DocumentBrowser/DocumentBrowser.css'

interface DocumentsPanelStatus {
  status: string
  message: ReactNode
  onDismiss?: () => void
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
  bookmarkedDocumentUrls?: ReadonlySet<string>
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
  onViewAudiobooks?: () => void
  onViewDocumentInfo?: (doc: DocumentInfo) => void
  onViewDocument: (url: string) => void
}

export function DocumentsPanel({
  allDocuments,
  audioSavedOnly = false,
  bookmarkedDocumentUrls = new Set(),
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
  onViewAudiobooks,
  onViewDocumentInfo,
  onViewDocument,
}: DocumentsPanelProps) {
  const { t } = useTranslation()
  const [importMenuOpen, setImportMenuOpen] = useState(false)
  const [bookmarkedOnly, setBookmarkedOnly] = useState(false)
  const [preferredView, setPreferredView] = useState<LibraryView>(loadView)
  const [galleryCategory, setGalleryCategory] = useState<LibraryGalleryCategory>(
    loadCategory,
  )
  const view = preferredView
  const activeImport = importOptions.find((option) => option.statusLabel)
  const hasImportOptions = importOptions.length > 0
  const importBusy = importStatuses.some((item) => item.status === 'importing')
  const operationBusy = importStatuses.some((item) =>
    item.status === 'importing' || item.status === 'recognizing' || item.status === 'deleting')
  const importDisabled = hasImportOptions && importOptions.every((option) => option.disabled || option.future || !option.onSelect)
  const visibleGroups = filterBookmarkedGroups(groupedDocs, bookmarkedDocumentUrls, bookmarkedOnly)
  const {
    uploadDocs,
    bundledDocs,
    nonBundledGroups,
    otherGroups,
  } = splitDocumentGroupsBySource(visibleGroups)
  // Every content filter must prune the organization tree, not just typed text;
  // otherwise Saved Audio and Bookmarks leave unrelated empty folders visible.
  const contentFilterActive = docFilterLower.length > 0 || audioSavedOnly || bookmarkedOnly
  const canShowUploadedTree = Boolean(
    libraryOrganization &&
    onCreateLibraryFolder &&
    onDeleteDocuments &&
    onDeleteLibraryFolder &&
    onMoveLibraryDocuments &&
    onRenameLibraryFolder,
  )
  const documentListGroups = canShowUploadedTree ? otherGroups : nonBundledGroups
  const showUploadedTree = canShowUploadedTree && (
    !contentFilterActive || uploadDocs.length > 0
  )
  const hasFolderTree = bundledDocs.length > 0 ||
    (showUploadedTree && uploadDocs.length > 0)

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
      className={`document-browser-panel documents-panel${view === 'gallery' ? ' documents-panel-gallery' : ''}`}
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
              <Button
                className={`document-import-btn${importBusy ? ' document-import-btn-busy' : ''}`}
                isDisabled={importDisabled}
                aria-label={importBusy
                  ? activeImport?.statusLabel ?? t('library.import.importingBatch')
                  : undefined}
              >
                <span className="document-import-btn-label" aria-hidden={importBusy ? true : undefined}>
                  {t('library.documents.import')}
                  <span className={`toggle-arrow ${importMenuOpen ? 'open' : ''}`} aria-hidden="true">&#9662;</span>
                </span>
                {importBusy && <span className="spinner document-import-btn-spinner" aria-hidden="true" />}
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
        <div className="document-view-options">
          <button
            type="button"
            className="library-view-toggle"
            aria-label={view === 'gallery' ? t('library.documents.listView') : t('library.documents.galleryView')}
            title={view === 'gallery' ? t('library.documents.listView') : t('library.documents.galleryView')}
            onClick={() => {
              const nextView = view === 'gallery' ? 'list' : 'gallery'
              setPreferredView(nextView)
              savePreference(VIEW_STORAGE_KEY, nextView)
            }}
          >
            <ViewIcon view={view === 'gallery' ? 'list' : 'gallery'} />
          </button>
          <button
            type="button"
            className="audio-filter-toggle bookmark-filter-toggle"
            aria-label={t('library.documents.bookmarkedOnly')}
            title={t('library.documents.bookmarkedOnly')}
            aria-pressed={bookmarkedOnly}
            onClick={() => setBookmarkedOnly(!bookmarkedOnly)}
          >
            <BookmarkIcon />
          </button>
          {onAudioSavedOnlyChange && (
            <button
              type="button"
              className="audio-filter-toggle"
              aria-pressed={audioSavedOnly}
              onClick={() => onAudioSavedOnlyChange(!audioSavedOnly)}
            >
              <SavedAudioIcon />
              <span>{t('library.documents.savedAudio')}</span>
            </button>
          )}
        </div>
      </div>

      {importStatuses.map((item, index) => item.message && item.status !== 'idle' ? (
        <div
          key={item.status + index}
          className={'document-import-status document-import-' + item.status}
          role={item.status === 'error' ? 'alert' : 'status'}
          aria-live={item.status === 'error' ? 'assertive' : 'polite'}
        >
          <div className="document-import-status-content">{item.message}</div>
          {item.onDismiss && (
            <button
              type="button"
              className="document-import-dismiss"
              aria-label={t('library.status.dismissNotice')}
              title={t('common.close')}
              onClick={item.onDismiss}
            >
              &times;
            </button>
          )}
        </div>
      ) : null)}

      {view === 'gallery' ? (
        <LibraryGalleryView
          category={galleryCategory}
          collapsedAuthors={collapsedAuthors}
          docFilterLower={docFilterLower}
          groupedDocs={visibleGroups}
          bookmarkedDocumentUrls={bookmarkedDocumentUrls}
          savedAudiobookDocumentUrls={savedAudiobookDocumentUrls}
          documentOpening={documentOpening}
          mutationDisabled={operationBusy}
          openingDocumentUrl={openingDocumentUrl}
          emptyMessage={emptyMessage(allDocuments.length, audioSavedOnly, bookmarkedOnly, documentFilter, t)}
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
          {showUploadedTree && libraryOrganization && (
            <UploadedLibraryTree
              documents={uploadDocs}
              organization={libraryOrganization}
              filterActive={contentFilterActive}
              documentOpening={documentOpening}
              mutationDisabled={operationBusy}
              resetEditing={importBusy}
              openingDocumentUrl={openingDocumentUrl}
              savedAudiobookDocumentUrls={savedAudiobookDocumentUrls}
              onCreateFolder={onCreateLibraryFolder!}
              onDeleteDocuments={onDeleteDocuments!}
              onDeleteFolder={onDeleteLibraryFolder!}
              onMoveDocuments={onMoveLibraryDocuments!}
              onRenameFolder={onRenameLibraryFolder!}
              onViewAudiobooks={onViewAudiobooks}
              onViewDocumentInfo={onViewDocumentInfo}
              onViewDocument={onViewDocument}
            />
          )}

          {bundledDocs.length > 0 && (
            <BundledDocumentTree
              documents={bundledDocs}
              filterActive={contentFilterActive}
              documentOpening={documentOpening}
              openingDocumentUrl={openingDocumentUrl}
              onViewDocument={onViewDocument}
            />
          )}

          {(documentListGroups.length > 0 || !hasFolderTree) && (
            <DocumentList
              groupedDocs={documentListGroups}
              collapsedAuthors={collapsedAuthors}
              docFilterLower={docFilterLower}
              emptyMessage={emptyMessage(allDocuments.length, audioSavedOnly, bookmarkedOnly, documentFilter, t)}
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

function ViewIcon({ view }: { view: LibraryView }) {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      {view === 'list' ? (
        <>
          <path d="M3 6h.01M3 12h.01M3 18h.01" />
          <path d="M8 6h13M8 12h13M8 18h13" />
        </>
      ) : (
        <>
          <rect x="3" y="3" width="7" height="7" rx="1" />
          <rect x="14" y="3" width="7" height="7" rx="1" />
          <rect x="3" y="14" width="7" height="7" rx="1" />
          <rect x="14" y="14" width="7" height="7" rx="1" />
        </>
      )}
    </svg>
  )
}

function SavedAudioIcon() {
  return (
    <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
      <path d="M4 14v-2a8 8 0 0 1 16 0v2M4 14h4v7H6a2 2 0 0 1-2-2zm16 0h-4v7h2a2 2 0 0 0 2-2z" />
    </svg>
  )
}

type LibraryView = 'gallery' | 'list'

const VIEW_STORAGE_KEY = 'papercut.library-view.v1'
const CATEGORY_STORAGE_KEY = 'papercut.library-gallery-category.v1'

function emptyMessage(
  documentCount: number,
  audioSavedOnly: boolean,
  bookmarkedOnly: boolean,
  filter: string,
  t: TFunction,
): string {
  if (documentCount === 0) return t('library.documents.empty')
  if (bookmarkedOnly) return t('library.documents.emptyBookmarked')
  if (audioSavedOnly) return t('library.documents.emptySavedAudio')
  return filter.trim() ? t('library.documents.emptyFilter') : t('library.documents.empty')
}

function loadView(): LibraryView {
  try {
    return window.localStorage.getItem(VIEW_STORAGE_KEY) === 'list' ? 'list' : 'gallery'
  } catch {
    return 'gallery'
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
