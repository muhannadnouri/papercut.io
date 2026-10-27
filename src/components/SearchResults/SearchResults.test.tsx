import { renderToStaticMarkup } from 'react-dom/server'
import {
  describe,
  expect,
  it,
  vi,
} from 'vitest'
import type { SearchPhase } from '../../hooks/useSearch'
import { indexedSearchOpenTarget } from '../../utils/searchOpenTarget'
import { SearchResults } from './SearchResults'

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { count?: number;
      shown?: number;
      total?: number }) => (
      options?.shown !== undefined ? `${key}:${options.shown}/${options.total}`
        : options?.count === undefined ? key : `${key}:${options.count}`
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
      expect(html).toContain('class="spinner"')
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

describe('explicit typo retry', () => {
  it('shows a retry for an empty search without inventing a result', () => {
    const html = renderToStaticMarkup(
      <SearchResults
        results={[]}
        loading={false}
        searchFailed={false}
        searchableDocumentCount={1}
        searchPhase={null}
        submittedQuery="enviroment"
        lastSearchInfo={{
          mode: 'all',
          phrases: [],
          unquotedText: 'enviroment',
          uploadedDocuments: 0,
          uploadedMatchingSections: 0,
          starterDocuments: 0,
          suggestedQuery: 'environment',
        }}
        scopeUrls={new Set()}
        scopeActive={false}
        onViewResult={() => undefined}
        onRetrySearch={() => undefined}
      />,
    )
    expect(html).toContain('search.results.noResults')
    expect(html).toContain('search.input.button')
    expect(html).toContain('environment')
    expect(html).not.toContain('result-card')
  })

  it('shows an uploaded retry beside starter results', () => {
    const html = renderToStaticMarkup(
      <SearchResults
        results={[{
          id: 'starter',
          url: '/starter',
          meta: { title: 'Starter' },
          excerpt: '<mark>calendar</mark>',
          source: 'starter',
        }]}
        loading={false}
        searchFailed={false}
        searchableDocumentCount={2}
        searchPhase={null}
        submittedQuery="calendar meeting agenda"
        lastSearchInfo={{
          mode: 'all',
          phrases: [],
          unquotedText: 'calendar meeting agenda',
          uploadedDocuments: 0,
          uploadedMatchingSections: 0,
          starterDocuments: 1,
          suggestedQuery: 'calendar meeting agenda',
        }}
        scopeUrls={new Set()}
        scopeActive={false}
        onViewResult={() => undefined}
        onRetrySearch={() => undefined}
      />,
    )
    expect(html).toContain('search.input.button <bdi>“calendar meeting agenda”</bdi>')
    expect(html).toContain('search.results.starterDocument')
    expect(html).not.toContain('search-result-group-starter')
  })
})

describe('search evidence', () => {
  it('labels missing words in broader results without inventing occurrences', () => {
    const html = renderToStaticMarkup(
      <SearchResults
        results={[{
          id: 'upload:section:one:0',
          url: '/uploads/one.html',
          meta: { title: 'One' },
          excerpt: '<mark>compass</mark>',
          source: 'upload',
          termMatches: [
            {
              term: 'silver',
              matched: false,
              matchingSections: 0,
            },
            {
              term: 'compass',
              matched: true,
              matchingSections: 1,
            },
            {
              term: 'lantern',
              matched: true,
              matchingSections: 1,
            },
          ],
        }]}
        loading={false}
        searchFailed={false}
        searchableDocumentCount={1}
        searchPhase={null}
        submittedQuery="silver compass lantern"
        lastSearchInfo={{
          mode: 'broader',
          phrases: [],
          unquotedText: 'silver compass lantern',
          uploadedDocuments: 1,
          uploadedMatchingSections: 1,
          starterDocuments: 0,
        }}
        scopeUrls={new Set()}
        scopeActive={false}
        onViewResult={() => undefined}
      />,
    )

    expect(html).toContain('search.results.wordCoverage')
    expect(html).toContain('search.results.missingWords silver')
    expect(html).toContain('<bdi>silver</bdi> 0')
    expect(html).toContain('search.results.broaderWords')
  })

  it('opens term coverage and distribution evidence with highlight text', () => {
    expect(indexedSearchOpenTarget(
      {
        sectionIndex: 4,
        pageIndex: 3,
        occurrenceIndex: 2,
      },
      'orchard',
    )).toEqual({
      text: 'orchard',
      sectionIndex: 4,
      pageIndex: 3,
      occurrenceIndex: 2,
    })
    expect(indexedSearchOpenTarget({
      sectionIndex: 7,
      pageIndex: null,
      text: 'lantern',
    })).toEqual({
      text: 'lantern',
      sectionIndex: 7,
      pageIndex: undefined,
    })
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
          sectionIndex: 0,
          passages: [
            {
              sectionIndex: 0,
              pageIndex: 2,
              excerpt: 'The <mark>orchard</mark> survey explains the route.',
            },
            {
              sectionIndex: 1,
              excerpt: 'The <mark>lantern</mark> illuminates the map.',
            },
            {
              sectionIndex: 2,
              excerpt: 'A <mark>compass</mark> guides the return.',
            },
          ],
          matchLocations: [
            {
              binIndex: 0,
              sectionIndex: 0,
              matchCount: 2,
            },
            {
              binIndex: 1,
              sectionIndex: 1,
              matchCount: 1,
            },
          ],
          termMatches: [
            {
              term: 'orchard',
              matchingSections: 2,
              sectionIndex: 0,
            },
            {
              term: 'lantern',
              matchingSections: 1,
              sectionIndex: 1,
            },
          ],
        }))}
        loading={false}
        searchFailed={false}
        searchableDocumentCount={1}
        searchPhase={null}
        submittedQuery="orchard lantern"
        lastSearchInfo={{
          mode: 'all',
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
    // The card hides its cross-section excerpt, so Explore must retain it.
    expect(html).toContain('survey explains the route')
    expect(html).toContain('illuminates the map')
    expect(html).toContain('guides the return')
    expect(html.match(/class="result-evidence-passage"/g)).toHaveLength(6)
    expect(html).not.toContain('type="checkbox"')
    expect(html).not.toContain('search.results.comparisonTitle')
  })

  it('keeps the matching-section count when no evidence map is shown', () => {
    const html = renderToStaticMarkup(
      <SearchResults
        results={[{
          id: 'upload:section:one:0',
          url: '/uploads/one.html',
          meta: { title: 'One' },
          excerpt: '<mark>orchard</mark>',
          source: 'upload',
          matchCount: 3,
          matchingSections: 3,
          matchLocations: [{
            binIndex: 0,
            sectionIndex: 0,
            matchCount: 3,
          }],
        }]}
        loading={false}
        searchFailed={false}
        searchableDocumentCount={1}
        searchPhase={null}
        submittedQuery="orchard"
        lastSearchInfo={{
          mode: 'all',
          phrases: [],
          unquotedText: 'orchard',
          uploadedDocuments: 1,
          uploadedMatchingSections: 3,
          starterDocuments: 0,
        }}
        scopeUrls={new Set()}
        scopeActive={false}
        onViewResult={() => undefined}
      />,
    )

    expect(html).toContain('search.results.matchingSections:3')
    expect(html).not.toContain('result-occurrence-map')
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
            {
              binIndex: 0,
              sectionIndex: 0,
              matchCount: 1,
              text: 'anne',
            },
            {
              binIndex: 1,
              sectionIndex: 1,
              matchCount: 1,
              text: 'green',
            },
          ],
        }]}
        loading={false}
        searchFailed={false}
        searchableDocumentCount={1}
        searchPhase={null}
        submittedQuery={'anne "green gables"'}
        lastSearchInfo={{
          mode: 'all',
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

describe('combined keyword results', () => {
  const rows = [
    {
      id: 'upload-one',
      url: '/uploads/one.html',
      meta: { title: 'Orchard Guide' },
      excerpt: '<mark>orchard</mark>',
      source: 'upload' as const,
    },
    {
      id: 'starter',
      url: '/starter.html',
      meta: { title: 'Search Guide' },
      excerpt: '<mark>orchard</mark>',
      source: 'starter' as const,
    },
    {
      id: 'upload-two',
      url: '/uploads/two.html',
      meta: { title: 'Survey Notes' },
      excerpt: '<mark>orchard</mark>',
      source: 'upload' as const,
    },
  ]
  const render = (scopeUrls = new Set<string>(), scopeActive = false, total = 3) => renderToStaticMarkup(
    <SearchResults results={rows} loading={false} searchFailed={false}
      searchableDocumentCount={3} searchPhase={null} submittedQuery="orchard"
      lastSearchInfo={{
        mode: 'all',
        phrases: [],
        unquotedText: 'orchard',
        uploadedDocuments: total - 1,
        uploadedMatchingSections: 2,
        starterDocuments: 1,
      }}
      scopeUrls={scopeUrls} scopeActive={scopeActive} onViewResult={() => undefined} />,
  )
  it('preserves incoming order, labels starter cards and shows one count without source groups', () => {
    const html = render()
    expect(html.indexOf('Orchard Guide')).toBeLessThan(html.indexOf('Search Guide'))
    expect(html.indexOf('Search Guide')).toBeLessThan(html.indexOf('Survey Notes'))
    expect(html.match(/search.results.starterDocument/g)).toHaveLength(1)
    expect(html.match(/search.results.resultCount:3/g)).toHaveLength(1)
    expect(html).not.toContain('search-result-group')
    expect(html).not.toContain('search.results.yourLibrary')
  })
  it('retains scoped cards and makes capped totals explicit in the single summary', () => {
    const filtered = render(new Set(['/uploads/one.html']), true)
    expect(filtered).toContain('Orchard Guide')
    expect(filtered).not.toContain('Search Guide')
    expect(filtered).not.toContain('Survey Notes')
    expect(filtered).toContain('search.results.filtered')
    expect(render(new Set(), false, 70)).toContain('search.results.showingCount:3/70')
  })
})
