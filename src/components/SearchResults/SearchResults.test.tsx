import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'
import type { SearchPhase } from '../../hooks/useSearch'
import { indexedSearchOpenTarget } from '../../utils/searchOpenTarget'
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
    expect(html).toContain('search-feedback-error')
    expect(html).toContain('search.results.searchFailed')
    expect(html).not.toContain('search.results.noResults')
  })

  it('announces an empty search as a non-error status', () => {
    const html = renderToStaticMarkup(
      <SearchResults
        results={[]}
        loading={false}
        searchFailed={false}
        searchableDocumentCount={321}
        searchPhase={null}
        submittedQuery="orchard"
        lastSearchInfo={null}
        scopeUrls={new Set()}
        scopeActive={false}
        onViewResult={() => undefined}
      />,
    )

    expect(html).toContain('no-results search-feedback')
    expect(html).toContain('role="status"')
    expect(html).toContain('search.results.noResults')
    expect(html).not.toContain('search-feedback-error')
  })
})

describe('search evidence', () => {
  it('opens term coverage and distribution evidence with highlight text', () => {
    expect(indexedSearchOpenTarget(
      { sectionIndex: 4, pageIndex: 3, occurrenceIndex: 2 },
      'orchard',
    )).toEqual({ text: 'orchard', sectionIndex: 4, pageIndex: 3, occurrenceIndex: 2 })
    expect(indexedSearchOpenTarget({
      sectionIndex: 7,
      pageIndex: null,
      text: 'lantern',
    })).toEqual({ text: 'lantern', sectionIndex: 7, pageIndex: undefined })
  })

  it('shows compact section coverage without a persistent comparison mode', () => {
    const html = renderToStaticMarkup(
      <SearchResults
        results={['one', 'two'].map((id) => ({
          id: `upload:section:${id}:0`,
          url: `/uploads/${id}.html`,
          meta: { title: id },
          excerpt: '',
          source: 'upload' as const,
          matchScope: 'document' as const,
          matchingSections: 3,
          matchLocations: [
            { binIndex: 0, sectionIndex: 0, matchCount: 2 },
            { binIndex: 1, sectionIndex: 1, matchCount: 1 },
          ],
          termMatches: [
            { term: 'orchard', matchingSections: 2, sectionIndex: 0 },
            { term: 'lantern', matchingSections: 1, sectionIndex: 1 },
          ],
        }))}
        loading={false}
        searchFailed={false}
        searchableDocumentCount={1}
        searchPhase={null}
        submittedQuery="orchard lantern"
        lastSearchInfo={{
          phrases: [],
          unquotedText: 'orchard lantern',
          uploadedDocuments: 2,
          uploadedMatchingSections: 2,
          starterDocuments: 0,
        }}
        scopeUrls={new Set()}
        scopeActive={false}
        onViewResult={() => undefined}
      />,
    )

    expect(html).toContain('search.results.sectionsByTerm')
    expect(html).toContain('orchard')
    expect(html).toContain('lantern')
    expect(html).toContain('search.results.exploreMatches')
    expect(html).toContain('search.results.sectionsWithAnyTerm:3')
    expect(html).not.toContain('type="checkbox"')
    expect(html).not.toContain('search.results.comparisonTitle')
  })
})

describe('search query summary', () => {
  it('shows unquoted words and exact phrases as separate required clauses', () => {
    const html = renderToStaticMarkup(
      <SearchResults
        results={[{
          id: 'one',
          url: '/uploads/one.html',
          meta: { title: 'One' },
          excerpt: '<mark>anne</mark>',
          source: 'upload',
          matchCount: 1,
          matchingSections: 2,
          matchLocations: [
            { binIndex: 0, sectionIndex: 0, matchCount: 1, text: 'anne' },
            { binIndex: 1, sectionIndex: 1, matchCount: 1, text: 'green' },
          ],
        }]}
        loading={false}
        searchFailed={false}
        searchableDocumentCount={1}
        searchPhase={null}
        submittedQuery={'anne "green gables"'}
        lastSearchInfo={{
          phrases: ['green gables'],
          unquotedText: 'anne',
          uploadedDocuments: 1,
          uploadedMatchingSections: 1,
          starterDocuments: 0,
        }}
        scopeUrls={new Set()}
        scopeActive={false}
        onViewResult={() => undefined}
      />,
    )

    expect(html).toContain('search.results.allWords')
    expect(html).toContain('anne')
    expect(html).toContain('search.results.exactPhrase')
    expect(html).toContain('green gables')
    expect(html).toContain('search.results.exactMatch:1')
    expect(html).toContain('class="search-info" role="status"')
    expect(html).not.toContain('search.results.exploreMatches')
    expect(html).not.toContain('search.results.sectionsByTerm')
  })
})
