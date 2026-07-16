import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
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
  filterTitleByUrl: Map<string, string>
  groupedDocs: AuthorGroup[]
  libraryOrganization?: UploadedLibraryOrganization
  selectedFilters: Set<string>
  onClearFilters: () => void
  onFilterChange: (value: string) => void
  onToggleAllInGroup: (docs: DocumentInfo[]) => void
  onToggleAuthor: (author: string) => void
  onToggleFilter: (url: string) => void
}

/**
 * Search-scope control for the Search tab: active-document chips plus a
 * collapsible selector to narrow which documents the query runs against.
 */
export function SearchScope({
  collapsedAuthors,
  docFilterLower,
  documentFilter,
  filterTitleByUrl,
  groupedDocs,
  libraryOrganization,
  selectedFilters,
  onClearFilters,
  onFilterChange,
  onToggleAllInGroup,
  onToggleAuthor,
  onToggleFilter,
}: SearchScopeProps) {
  const { t, i18n } = useTranslation()
  const count = selectedFilters.size
  const scopeLabel = count === 0
    ? t('search.scope.allDocuments')
    : t('search.scope.documentCount', { count })
  const { uploadDocs, nonUploadGroups } = splitDocumentGroupsByUpload(groupedDocs)
  const showUploadedTree = Boolean(libraryOrganization && uploadDocs.length > 0)
  const selectedFilterUrls = useMemo(() => {
    const collator = new Intl.Collator(i18n.resolvedLanguage ?? i18n.language, {
      numeric: true,
      sensitivity: 'base',
    })
    return Array.from(selectedFilters).sort((a, b) => collator.compare(
      filterTitleByUrl.get(a) ?? a,
      filterTitleByUrl.get(b) ?? b,
    ))
  }, [filterTitleByUrl, i18n.language, i18n.resolvedLanguage, selectedFilters])

  return (
    <div className="search-scope">
      <Panel
        className="document-browser-panel search-scope-panel"
        ariaLabel={t('search.scope.ariaLabel')}
        title={(
          <span className="search-scope-title">
            <svg className="search-scope-title-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
              <path d="M4 5h16l-6.5 7.5V19l-3 1.5v-8Z" />
            </svg>
            {t('search.scope.title')}
          </span>
        )}
        meta={scopeLabel}
        defaultOpen={false}
      >
        <div className="documents-list-header">
          <input
            type="text"
            className="document-filter-input"
            placeholder={t('search.scope.filterPlaceholder')}
            value={documentFilter}
            onChange={(e) => onFilterChange(e.target.value)}
          />
          {count > 0 && (
            <button className="clear-filters" onClick={onClearFilters}>
              {t('search.scope.clear')}
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
        <div className="active-filters" tabIndex={0} aria-label={t('search.scope.selectedFiltersLabel')}>
          {selectedFilterUrls.map((url) => {
            const title = filterTitleByUrl.get(url) ?? url
            return (
              <span key={url} className="filter-tag">
                <bdi>{title}</bdi>
                <button
                  className="filter-tag-remove"
                  aria-label={t('search.scope.removeFilter', { title })}
                  onClick={() => onToggleFilter(url)}
                >
                  &times;
                </button>
              </span>
            )
          })}
        </div>
      )}
    </div>
  )
}
