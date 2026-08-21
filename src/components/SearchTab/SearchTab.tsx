import { useTranslation } from 'react-i18next'
import type { AuthorGroup, DocumentScopeMode } from '../../hooks/useDocumentFilters'
import type { LastSearchInfo, SearchPhase, SearchQueryError } from '../../hooks/useSearch'
import type { DocumentInfo, SearchOpenTarget, SearchResult } from '../../types/search'
import type { UploadedLibraryOrganization } from '../../uploads/DocumentUploads'
import { SearchBar } from '../SearchBar/SearchBar'
import { SearchResults } from '../SearchResults/SearchResults'
import { SearchScope } from '../SearchScope/SearchScope'

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
  searchFailed: boolean
  searchableDocumentCount: number
  searchPhase: SearchPhase | null
  openingDisabled: boolean
  openingDocumentUrl?: string
  query: string
  queryError: SearchQueryError | null
  results: SearchResult[]
  selectedFilters: Set<string>
  scopeMode: DocumentScopeMode
  scopeUrls: Set<string>
  scopeActive: boolean
  submittedQuery: string
  onChangeQuery: (value: string) => void
  onClearFilters: () => void
  onScopeModeChange: (mode: DocumentScopeMode) => void
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
  searchFailed,
  searchableDocumentCount,
  searchPhase,
  openingDisabled,
  openingDocumentUrl,
  query,
  queryError,
  results,
  selectedFilters,
  scopeMode,
  scopeUrls,
  scopeActive,
  submittedQuery,
  onChangeQuery,
  onClearFilters,
  onScopeModeChange,
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
        queryError={queryError}
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
        scopeMode={scopeMode}
        filterTitleByUrl={filterTitleByUrl}
        onFilterChange={onFilterChange}
        onToggleFilter={onToggleFilter}
        onToggleAllInGroup={onToggleAllInGroup}
        onToggleAuthor={onToggleAuthor}
        onClearFilters={onClearFilters}
        onScopeModeChange={onScopeModeChange}
      />

      <SearchResults
        results={results}
        loading={loading}
        searchFailed={searchFailed}
        searchableDocumentCount={searchableDocumentCount}
        searchPhase={searchPhase}
        submittedQuery={submittedQuery}
        lastSearchInfo={lastSearchInfo}
        scopeUrls={scopeUrls}
        scopeActive={scopeActive}
        openingDisabled={openingDisabled}
        openingDocumentUrl={openingDocumentUrl}
        onViewResult={onViewResult}
      />
    </section>
  )
}
