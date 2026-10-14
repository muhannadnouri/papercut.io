import { useLayoutEffect, useRef, useState } from 'react'
import { Tabs, TabList, Tab, TabPanel } from 'react-aria-components'
import { useTranslation } from 'react-i18next'
import type { AuthorGroup, DocumentScopeMode } from '../../hooks/useDocumentFilters'
import type { LastSearchInfo, SearchPhase, SearchQueryError } from '../../hooks/useSearch'
import type { DocumentInfo, SearchMode, SearchOpenTarget, SearchResult } from '../../types/search'
import type { UploadedLibraryOrganization } from '../../uploads/DocumentUploads'
import { ConceptualResults } from '../../search-semantic/ConceptualResults'
import type { semanticSearchStatus } from '../../search-semantic/semanticSearchStatus'
import type { ConceptualState } from '../../search-semantic/useConceptualSearch'
import './SearchTab.css'
import { SearchBar } from '../SearchBar/SearchBar'
import { SearchOptions } from '../SearchBar/SearchOptions'
import { SearchResults } from '../SearchResults/SearchResults'
import { SearchScope } from '../SearchScope/SearchScope'

export type SearchResultView = 'keyword' | 'conceptual'

interface SearchTabProps {
  semanticEnabled: boolean
  semanticStatus: ReturnType<typeof semanticSearchStatus> | 'downloading'
  hasStoredIndex: boolean
  onOpenSearchSettings: () => void
  conceptualState: ConceptualState
  resultView: SearchResultView
  searchRequest: { revision: number; scope: string }
  onResultViewChange: (view: SearchResultView) => void
  onViewConceptualResult: (url: string, target: SearchOpenTarget) => void
  mode: SearchMode
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
  onModeChange: (mode: SearchMode) => void
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
  semanticEnabled,
  semanticStatus,
  hasStoredIndex,
  onOpenSearchSettings,
  conceptualState,
  resultView,
  searchRequest,
  onResultViewChange,
  onViewConceptualResult,
  mode,
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
  onModeChange,
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
  const view = semanticEnabled ? resultView : 'keyword'
  const [limits, setLimits] = useState({ request: searchRequest, keyword: 10, conceptual: 10 })
  const resultArea = useRef<HTMLDivElement>(null)
  const positions = useRef<Partial<Record<SearchResultView, number>>>({})
  const restoreScroll = useRef<number | null>(null)
  if (limits.request !== searchRequest) {
    setLimits({ request: searchRequest, keyword: 10, conceptual: 10 })
  }
  useLayoutEffect(() => {
    positions.current = {}
    restoreScroll.current = null
  }, [searchRequest])
  useLayoutEffect(() => {
    if (restoreScroll.current !== null) {
      window.scrollTo({ top: restoreScroll.current, behavior: 'instant' })
      restoreScroll.current = null
    }
  }, [view])
  const keywordCount = scopeActive ? results.filter(result => scopeUrls.has(result.url)).length : results.length
  const keywordResults = <SearchResults
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
    visibleLimit={limits.keyword}
    onShowMore={() => setLimits(current => ({ ...current, keyword: current.keyword + 10 }))}
    onRetrySearch={(suggestion) => {
      onChangeQuery(suggestion)
      onSubmitSearch()
    }}
  />

  const resultSelector = semanticEnabled ? (
    <TabList className="search-result-tabs" aria-label={t('search.results.views')}>
      <Tab id="keyword" className="search-result-tab">
        <svg className="search-result-tab-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
          <circle cx="11" cy="11" r="7" />
          <path d="m16 16 5 5" />
        </svg>
        <span dir="auto">{t('search.results.keyword')}{submittedQuery && !loading && !searchFailed
          ? ` (${t('search.results.count', { count: keywordCount })})` : ''}</span>
        {loading && <span className="spinner" role="img" aria-label={t('search.results.searching')} />}
      </Tab>
      <Tab id="conceptual" className="search-result-tab">
        <svg className="search-result-tab-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
          <circle cx="5" cy="12" r="3" />
          <circle cx="18" cy="5" r="3" />
          <circle cx="18" cy="19" r="3" />
          <path d="m8 10.5 7-4m-7 7 7 4" />
        </svg>
        <span dir="auto">{t('search.results.conceptual')}{conceptualState?.phase === 'ready'
          ? ` (${t('search.results.count', { count: conceptualState.response?.results.length ?? 0 })})` : ''}</span>
        {conceptualState?.phase === 'loading' && <span className="spinner" role="img" aria-label={t('search.conceptual.loading')} />}
      </Tab>
    </TabList>
  ) : undefined
  const searchOptions = <SearchOptions resultSelector={resultSelector} keywordOptions={view === 'keyword'}
    mode={mode} onModeChange={onModeChange} query={query} disabled={disabled} onChange={onChangeQuery} />
  const searchScope = <SearchScope
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

  return (
    <section className="tab-panel" role="tabpanel" aria-label={t('search.tabLabel')} data-tab="search">
      <SearchBar
        query={query}
        queryError={queryError}
        disabled={disabled}
        loading={loading}
        submittedQuery={submittedQuery}
        onChange={onChangeQuery}
        onSubmit={onSubmitSearch}
      />

      {semanticEnabled ? <Tabs className="search-result-views" ref={resultArea} selectedKey={view}
        onSelectionChange={(key) => {
          if (key !== 'keyword' && key !== 'conceptual') return
          if (key === view) return
          const top = resultArea.current?.getBoundingClientRect().top ?? 0
          positions.current[view] = window.scrollY
          // Restore each list's place only when switching from the scrolled result bar.
          restoreScroll.current = top <= 0
            ? positions.current[key] ?? Math.max(0, window.scrollY + top) : null
          onResultViewChange(key)
        }}>
        {searchOptions}
        {searchScope}
        <TabPanel id="keyword" className="search-result-panel" hidden={view !== 'keyword'} shouldForceMount>{keywordResults}</TabPanel>
        <TabPanel id="conceptual" className="search-result-panel" hidden={view !== 'conceptual'} shouldForceMount>
          <ConceptualResults state={conceptualState} openingDisabled={openingDisabled}
            setupStatus={!submittedQuery && !queryError ? semanticStatus : undefined}
            hasStoredIndex={hasStoredIndex} onOpenSearchSettings={onOpenSearchSettings}
            openingDocumentUrl={openingDocumentUrl} onViewDocument={onViewConceptualResult}
            visibleLimit={limits.conceptual}
            onShowMore={() => setLimits(current => ({ ...current, conceptual: current.conceptual + 10 }))} />
        </TabPanel>
      </Tabs> : <>{searchOptions}{searchScope}{keywordResults}</>}
    </section>
  )
}
