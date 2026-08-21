import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it, vi } from 'vitest'
import { SearchBar } from './SearchBar'

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))

describe('search query errors', () => {
  it('associates an unmatched-quote error with the search input', () => {
    const html = renderToStaticMarkup(
      <SearchBar
        query={'anne "green gables'}
        queryError="unmatchedQuote"
        disabled={false}
        onChange={() => undefined}
        onSubmit={() => undefined}
      />,
    )

    expect(html).toContain('aria-invalid="true"')
    expect(html).toContain('aria-describedby="search-input-help search-input-error"')
    expect(html).toContain('role="alert"')
  })
})
