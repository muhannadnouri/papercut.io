import type { DocumentInfo } from '../../types/search'
import type { AuthorGroup } from '../../hooks/useDocumentFilters'
import type { UploadedLibraryOrganization } from '../../uploads/DocumentUploads'
import { Panel } from '../Panel/Panel'
import { DocumentList } from '../DocumentList/DocumentList'
import { UploadedLibraryTree } from '../UploadedLibraryTree/UploadedLibraryTree'
import { splitDocumentGroupsByUpload } from '../DocumentBrowser/documentGroups'
import '../DocumentBrowser/DocumentBrowser.css'

interface SearchScopeProps {
  collapsedAuthors: Set<string>
  docFilterLower: string
  documentFilter: string
  groupedDocs: AuthorGroup[]
  libraryOrganization?: UploadedLibraryOrganization
  selectedFilters: Set<string>
  onClearFilters: () => void
  onFilterChange: (value: string) => void
  onToggleAllInGroup: (docs: DocumentInfo[]) => void
  onToggleAuthor: (author: string) => void
  onToggleFilter: (title: string) => void
}

/**
 * Search-scope control for the Search tab: active-document chips plus a
 * collapsible selector to narrow which documents the query runs against.
 */
export function SearchScope({
  collapsedAuthors,
  docFilterLower,
  documentFilter,
  groupedDocs,
  libraryOrganization,
  selectedFilters,
  onClearFilters,
  onFilterChange,
  onToggleAllInGroup,
  onToggleAuthor,
  onToggleFilter,
}: SearchScopeProps) {
  const count = selectedFilters.size
  const scopeLabel = count === 0
    ? 'All documents'
    : `${count} document${count === 1 ? '' : 's'}`
  const { uploadDocs, nonUploadGroups } = splitDocumentGroupsByUpload(groupedDocs)
  const showUploadedTree = Boolean(libraryOrganization && uploadDocs.length > 0)

  return (
    <div className="search-scope">
      <Panel
        className="document-browser-panel search-scope-panel"
        ariaLabel="Search scope"
        title="🌪️ Filter By Document"
        meta={scopeLabel}
        defaultOpen={false}
      >
        <div className="documents-list-header">
          <input
            type="text"
            className="document-filter-input"
            placeholder="Filter Documents..."
            value={documentFilter}
            onChange={(e) => onFilterChange(e.target.value)}
          />
          {count > 0 && (
            <button className="clear-filters" onClick={onClearFilters}>
              Clear Filters
            </button>
          )}
        </div>

        {showUploadedTree && libraryOrganization && (
          <UploadedLibraryTree
            mode="filter"
            documents={uploadDocs}
            organization={libraryOrganization}
            selectedFilters={selectedFilters}
            onToggleFilter={onToggleFilter}
            onToggleAllInGroup={onToggleAllInGroup}
          />
        )}

        {(nonUploadGroups.length > 0 || !showUploadedTree) && (
          <DocumentList
            selectable
            groupedDocs={showUploadedTree ? nonUploadGroups : groupedDocs}
            collapsedAuthors={collapsedAuthors}
            docFilterLower={docFilterLower}
            selectedFilters={selectedFilters}
            onToggleAuthor={onToggleAuthor}
            onToggleFilter={onToggleFilter}
            onToggleAllInGroup={onToggleAllInGroup}
          />
        )}
      </Panel>

      {count > 0 && (
        <div className="active-filters" tabIndex={0} aria-label="Selected document filters">
          {Array.from(selectedFilters).map((title) => (
            <span key={title} className="filter-tag">
              {title}
              <button className="filter-tag-remove" onClick={() => onToggleFilter(title)}>
                &times;
              </button>
            </span>
          ))}
        </div>
      )}
    </div>
  )
}
