import { useTranslation } from 'react-i18next'
import type { AuthorGroup } from '../../hooks/useDocumentFilters'
import type { DocumentInfo, SearchOpenTarget, SearchResult } from '../../types/search'
import type { UploadedLibraryOrganization } from '../../uploads/DocumentUploads'
import { SearchBar } from '../SearchBar/SearchBar'
import { SearchResults } from '../SearchResults/SearchResults'
import { SearchScope } from '../SearchScope/SearchScope'

interface LastSearchInfo {
  phrases: string[]
}

interface SearchTabProps {
  collapsedAuthors: Set<string>
  disabled: boolean
  docFilterLower: string
  documentFilter: string
  filterTitleByUrl: Map<string, string>
  groupedDocs: AuthorGroup[]
  lastSearchInfo: LastSearchInfo | null
  libraryOrganization: UploadedLibraryOrganization
  loading: boolean
  openingDisabled: boolean
  openingDocumentUrl?: string
  query: string
  results: SearchResult[]
  selectedFilters: Set<string>
  submittedQuery: string
  onChangeQuery: (value: string) => void
  onClearFilters: () => void
  onFilterChange: (value: string) => void
  onSubmitSearch: () => void
  onToggleAllInGroup: (docs: DocumentInfo[]) => void
  onToggleAuthor: (author: string) => void
  onToggleFilter: (url: string) => void
  onViewResult: (result: SearchResult, target?: SearchOpenTarget) => void
}

export function SearchTab({
  collapsedAuthors,
  disabled,
  docFilterLower,
  documentFilter,
  filterTitleByUrl,
  groupedDocs,
  lastSearchInfo,
  libraryOrganization,
  loading,
  openingDisabled,
  openingDocumentUrl,
  query,
  results,
  selectedFilters,
  submittedQuery,
  onChangeQuery,
  onClearFilters,
  onFilterChange,
  onSubmitSearch,
  onToggleAllInGroup,
  onToggleAuthor,
  onToggleFilter,
  onViewResult,
}: SearchTabProps) {
  const { t } = useTranslation()

  return (
    <section className="tab-panel" role="tabpanel" aria-label={t('search.tabLabel')} data-tab="search">
      <SearchBar
        query={query}
        disabled={disabled}
        onChange={onChangeQuery}
        onSubmit={onSubmitSearch}
      />

      <SearchScope
        groupedDocs={groupedDocs}
        collapsedAuthors={collapsedAuthors}
        docFilterLower={docFilterLower}
        documentFilter={documentFilter}
        libraryOrganization={libraryOrganization}
        selectedFilters={selectedFilters}
        filterTitleByUrl={filterTitleByUrl}
        onFilterChange={onFilterChange}
        onToggleFilter={onToggleFilter}
        onToggleAllInGroup={onToggleAllInGroup}
        onToggleAuthor={onToggleAuthor}
        onClearFilters={onClearFilters}
      />

      <SearchResults
        results={results}
        loading={loading}
        submittedQuery={submittedQuery}
        lastSearchInfo={lastSearchInfo}
        selectedFilters={selectedFilters}
        openingDisabled={openingDisabled}
        openingDocumentUrl={openingDocumentUrl}
        onViewResult={onViewResult}
      />
    </section>
  )
}
