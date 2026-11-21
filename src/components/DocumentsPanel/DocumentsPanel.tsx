import {
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
} from 'react'
import { useTranslation } from 'react-i18next'
import type { TFunction } from 'i18next'
import { Button } from 'react-aria-components'
import type { DocumentInfo } from '../../types/search'
import type { AuthorGroup } from '../../hooks/useDocumentFilters'
import {
  isUploadedDocumentUrl,
  type UploadedDocumentDeleteBatchResult,
  type UploadedLibraryOrganization,
} from '../../uploads/DocumentUploads'
import { BookmarkIcon } from '../BookmarkIndicator/BookmarkIndicator'
import { filterBookmarkedGroups } from '../BookmarkIndicator/bookmarkFilters'
import { BundledDocumentTree } from '../BundledDocumentTree/BundledDocumentTree'
import { Panel } from '../Panel/Panel'
import { DocumentList } from '../DocumentList/DocumentList'
import { UploadedLibraryTree } from '../UploadedLibraryTree/UploadedLibraryTree'
import {
  LibraryGalleryView,
  type LibraryGalleryCategory,
} from '../LibraryGalleryView/LibraryGalleryView'
import { splitDocumentGroupsBySource } from '../DocumentBrowser/documentGroups'
import {
  DocumentImportMenu,
  type DocumentImportOption,
} from './DocumentImportMenu'
import '../DocumentBrowser/DocumentBrowser.css'

export type { DocumentImportOption } from './DocumentImportMenu'

interface DocumentsPanelStatus {
  status: string
  message: ReactNode
  /** Keep Manage-owned operations with the controls that started them. */
  manageContext?: boolean
  onDismiss?: () => void
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
  onDeleteLibraryFolder?: (folderId: string) => Promise<UploadedDocumentDeleteBatchResult | null>
  onFilterChange: (value: string) => void
  onMoveLibraryDocuments?: (documentIds: string[], folderId: string | null) => void | Promise<void>
  onMoveLibraryFolder?: (folderId: string, parentId: string | null) => void | Promise<void>
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
  onMoveLibraryFolder,
  onRenameLibraryFolder,
  onToggleAuthor,
  onToggleShow,
  onViewAudiobooks,
  onViewDocumentInfo,
  onViewDocument,
}: DocumentsPanelProps) {
  const { t } = useTranslation()
  const [bookmarkedOnly, setBookmarkedOnly] = useState(false)
  const [preferredView, setPreferredView] = useState<LibraryView>(loadView)
  const [galleryCategory, setGalleryCategory] = useState<LibraryGalleryCategory>(
    loadCategory,
  )
  const [managing, setManaging] = useState(false)
  const categoryGroup = useRef<HTMLDivElement>(null)
  const manageButton = useRef<HTMLButtonElement>(null)
  const filterInput = useRef<HTMLInputElement>(null)
  const restoreBrowseFocus = useRef(false)
  const view = managing ? 'manage' : preferredView
  const hasImportOptions = importOptions.length > 0
  const importBusy = importStatuses.some((item) => item.status === 'importing')
  const operationBusy = importStatuses.some((item) =>
    item.status === 'importing' || item.status === 'recognizing' || item.status === 'deleting')
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
  const canManage = showUploadedTree && (
    uploadDocs.some((doc) => isUploadedDocumentUrl(doc.url)) ||
    (!contentFilterActive && Boolean(libraryOrganization?.folders.length))
  )

  // Filters and the last deletion can remove the manager while it has focus.
  useLayoutEffect(() => {
    if (managing) {
      restoreBrowseFocus.current = true
      if (!canManage || importBusy || !showDocuments || documentsLoading) {
        setManaging(false)
      }
    } else if (restoreBrowseFocus.current) {
      restoreBrowseFocus.current = false
      if (document.activeElement !== document.body && !document.activeElement?.matches(':disabled')) {
        return
      }
      const trigger = manageButton.current
      const returnTarget = (trigger?.disabled ? null : trigger) ??
        categoryGroup.current?.querySelector<HTMLButtonElement>('[aria-pressed="true"]') ?? filterInput.current
      returnTarget?.focus()
    }
  }, [managing, canManage, importBusy, showDocuments, documentsLoading])

  const switchToView = preferredView === 'gallery' ? 'list' : 'gallery'
  const manageStatusIndex = view !== 'gallery' && showUploadedTree
    ? importStatuses.findIndex((item) => item.manageContext)
    : -1
  const hasFolderTree = bundledDocs.length > 0 ||
    (showUploadedTree && uploadDocs.length > 0)

  const renderStatus = (item: DocumentsPanelStatus, index: number) => (
    item.message && item.status !== 'idle' ? (
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
    ) : null
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
      className={`document-browser-panel documents-panel${view === 'gallery' ? ' documents-panel-gallery' : ''}`}
      ariaLabel={t('library.documents.ariaLabel')}
      title={t('library.documents.title', { count: allDocuments.length })}
      open={showDocuments}
      onToggle={onToggleShow}
    >
      <div className="documents-list-header">
        <input
          ref={filterInput}
          type="text"
          dir="auto"
          className="document-filter-input"
          placeholder={t('library.documents.filterPlaceholder')}
          value={documentFilter}
          onChange={(e) => onFilterChange(e.target.value)}
        />
        {(hasImportOptions || canManage) && (
          <div className="document-upload-actions">
            {hasImportOptions && (
              <DocumentImportMenu importOptions={importOptions} importBusy={importBusy} />
            )}
            {canManage && (
              <Button ref={manageButton} className="document-import-btn document-manage-btn"
                isDisabled={managing || documentOpening || operationBusy} onPress={() => setManaging(true)}>
                {t('library.tree.organize')}
              </Button>
            )}
          </div>
        )}
        <div className="document-view-options">
          <button
            type="button"
            className="library-view-toggle"
            disabled={managing}
            aria-label={switchToView === 'list' ? t('library.documents.listView') : t('library.documents.galleryView')}
            title={switchToView === 'list' ? t('library.documents.listView') : t('library.documents.galleryView')}
            onClick={() => {
              setPreferredView(switchToView)
              savePreference(VIEW_STORAGE_KEY, switchToView)
            }}
          >
            <ViewIcon view={switchToView} />
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

      {importStatuses.map((item, index) => index === manageStatusIndex
        ? null
        : renderStatus(item, index))}

      {view === 'gallery' ? (
        <LibraryGalleryView
          category={galleryCategory}
          categoryGroupRef={categoryGroup}
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
          onDeleteDocument={canManage ? undefined : onDeleteDocument}
          onToggleAuthor={onToggleAuthor}
          onViewDocument={onViewDocument}
        />
      ) : (
        <>
          {showUploadedTree && libraryOrganization && (
            <UploadedLibraryTree
              documents={uploadDocs}
              unfilteredDocuments={allDocuments}
              organization={libraryOrganization}
              filterActive={contentFilterActive}
              documentOpening={documentOpening}
              mutationDisabled={operationBusy}
              onDone={managing ? () => setManaging(false) : undefined}
              openingDocumentUrl={openingDocumentUrl}
              savedAudiobookDocumentUrls={savedAudiobookDocumentUrls}
              managementStatus={manageStatusIndex >= 0
                ? renderStatus(importStatuses[manageStatusIndex], manageStatusIndex)
                : undefined}
              onCreateFolder={onCreateLibraryFolder!}
              onDeleteDocuments={onDeleteDocuments!}
              onDeleteFolder={onDeleteLibraryFolder!}
              onMoveDocuments={onMoveLibraryDocuments!}
              onMoveFolder={onMoveLibraryFolder}
              onRenameFolder={onRenameLibraryFolder!}
              onViewAudiobooks={onViewAudiobooks}
              onViewDocumentInfo={onViewDocumentInfo}
              onViewDocument={onViewDocument}
            />
          )}

          {!managing && bundledDocs.length > 0 && (
            <BundledDocumentTree
              documents={bundledDocs}
              filterActive={contentFilterActive}
              documentOpening={documentOpening}
              openingDocumentUrl={openingDocumentUrl}
              onViewDocument={onViewDocument}
            />
          )}

          {!managing && (documentListGroups.length > 0 || !hasFolderTree) && (
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
  if (documentCount === 0) {
    return t('library.documents.empty')
  }
  if (bookmarkedOnly) {
    return t('library.documents.emptyBookmarked')
  }
  if (audioSavedOnly) {
    return t('library.documents.emptySavedAudio')
  }
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
