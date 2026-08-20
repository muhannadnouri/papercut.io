import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'
import type { SearchPhase } from '../../hooks/useSearch'
import { SearchResults } from './SearchResults'

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { count?: number }) => (
      options?.count === undefined ? key : `${key}:${options.count}`
    ),
  }),
}))

describe('search progress', () => {
  it('reports each real search phase and the searchable document count', () => {
    const expectedKeys: Record<SearchPhase, string> = {
      indexes: 'search.results.searchingDocuments:321',
      candidates: 'search.results.findingCandidates:321',
      evidence: 'search.results.buildingEvidence:321',
      results: 'search.results.loadingRankedResults:321',
      phrases: 'search.results.verifyingExactPhrases:321',
      excerpts: 'search.results.preparingExcerpts:321',
    }

    for (const [searchPhase, expected] of Object.entries(expectedKeys)) {
      const html = renderToStaticMarkup(
        <SearchResults
          results={[]}
          loading
          searchFailed={false}
          searchableDocumentCount={321}
          searchPhase={searchPhase as SearchPhase}
          submittedQuery="orchard"
          lastSearchInfo={null}
          scopeUrls={new Set()}
          scopeActive={false}
          onViewResult={() => undefined}
        />,
      )

      expect(html).toContain('role="status"')
      expect(html).toContain(expected)
      expect(html).toContain('search.results.slowSearch')
    }
  })

  it('does not report a failed provider search as zero results', () => {
    const html = renderToStaticMarkup(
      <SearchResults
        results={[]}
        loading={false}
        searchFailed
        searchableDocumentCount={321}
        searchPhase={null}
        submittedQuery="orchard"
        lastSearchInfo={null}
        scopeUrls={new Set()}
        scopeActive={false}
        onViewResult={() => undefined}
      />,
    )

    expect(html).toContain('role="alert"')
    expect(html).toContain('search.results.searchFailed')
    expect(html).not.toContain('search.results.noResults')
  })
})
