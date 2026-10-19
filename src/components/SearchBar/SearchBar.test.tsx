import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'
import { SearchBar } from './SearchBar'
import { SearchOptions } from './SearchOptions'

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))

describe('search query errors', () => {
  it('associates broader matching help with the query input', () => {
    const html = renderToStaticMarkup(
      <>
        <SearchBar
          query="silver compass lantern"
          queryError={null}
          disabled={false}
          loading={false}
          submittedQuery=""
          onChange={() => undefined}
          onSubmit={() => undefined}
        />
        <SearchOptions keywordOptions mode="broader" query="silver compass lantern"
          disabled={false} onChange={() => undefined} onModeChange={() => undefined} />
      </>,
    )

    expect(html).toContain('search.input.broaderMode')
    expect(html).toContain('search.input.helpBroader')
    expect(html).toContain('<legend>search.input.modeLabel</legend>')
    expect(html).toContain('aria-describedby="search-input-help"')
    expect(html).toContain('checked=""')
  })

  it.each(['unmatchedQuote', 'tooLong'] as const)('associates %s with the search input', (error) => {
    const html = renderToStaticMarkup(
      <SearchBar
        query={'anne "green gables'}
        queryError={error}
        disabled={false}
        loading={false}
        submittedQuery=""
        onChange={() => undefined}
        onSubmit={() => undefined}
      />,
    )

    expect(html).toContain('aria-invalid="true"')
    expect(html).toContain('aria-describedby="search-input-help search-input-error"')
    expect(html).toContain('role="alert"')
    expect(html).toContain(`search.input.${error}`)
  })

  it('shows a busy button only while the submitted query is still in the input', () => {
    const busy = renderToStaticMarkup(
      <SearchBar
        query="orchard"
        queryError={null}
        disabled={false}
        loading
        submittedQuery="orchard"
        onChange={() => undefined}
        onSubmit={() => undefined}
      />,
    )
    const edited = renderToStaticMarkup(
      <SearchBar
        query="orchard lantern"
        queryError={null}
        disabled={false}
        loading
        submittedQuery="orchard"
        onChange={() => undefined}
        onSubmit={() => undefined}
      />,
    )

    expect(busy).toContain('aria-busy="true"')
    expect(busy).toMatch(/<button[^>]*class="search-btn"[^>]*disabled=""/)
    expect(busy).toContain('spinner search-btn-spinner')
    expect(edited).not.toContain('aria-busy="true"')
    expect(edited).not.toContain('disabled=""')
    expect(edited).not.toContain('search-btn-spinner')
  })
})
