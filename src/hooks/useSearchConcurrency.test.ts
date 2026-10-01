import { beforeEach, describe, expect, it, vi } from 'vitest'
import type { UploadedDocumentSearchResponse } from '../uploads/DocumentUploads'

const mocked = vi.hoisted(() => ({
  setters: [] as ReturnType<typeof vi.fn>[],
  searchUploadedDocuments: vi.fn(),
}))

vi.mock('react', () => ({
  useState: (initial: unknown) => {
    const setter = vi.fn()
    mocked.setters.push(setter)
    return [initial, setter]
  },
  useRef: (initial: unknown) => ({ current: initial }),
  useCallback: (callback: unknown) => callback,
}))

vi.mock('../uploads/DocumentUploads', () => ({
  searchUploadedDocuments: mocked.searchUploadedDocuments,
}))

import { useSearch } from './useSearch'

function deferredSearch() {
  let resolve!: (value: UploadedDocumentSearchResponse) => void
  const promise = new Promise<UploadedDocumentSearchResponse>((done) => { resolve = done })
  return { promise, resolve }
}

function response(totalDocuments: number): UploadedDocumentSearchResponse {
  return { results: [], totalDocuments, totalMatchingSections: 0 }
}

describe('search request freshness', () => {
  beforeEach(() => {
    mocked.setters.length = 0
    mocked.searchUploadedDocuments.mockReset()
  })

  it('ignores an old request when the same query is submitted again', async () => {
    const firstAlpha = deferredSearch()
    const beta = deferredSearch()
    const latestAlpha = deferredSearch()
    mocked.searchUploadedDocuments
      .mockReturnValueOnce(firstAlpha.promise)
      .mockReturnValueOnce(beta.promise)
      .mockReturnValueOnce(latestAlpha.promise)

    const search = useSearch({ current: null })
    const setLastSearchInfo = mocked.setters.at(-1)
    search.handleSearch('alpha')
    search.submitSearch()
    search.handleSearch('beta')
    search.submitSearch()
    search.handleSearch('alpha')
    search.submitSearch()

    firstAlpha.resolve(response(1))
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(setLastSearchInfo).not.toHaveBeenCalledWith(
      expect.objectContaining({ uploadedDocuments: 1 }),
    )

    latestAlpha.resolve(response(3))
    await vi.waitFor(() => expect(setLastSearchInfo).toHaveBeenCalledWith(
      expect.objectContaining({ uploadedDocuments: 3 }),
    ))

    beta.resolve(response(2))
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(setLastSearchInfo).not.toHaveBeenCalledWith(
      expect.objectContaining({ uploadedDocuments: 2 }),
    )
  })

  it('sends broad words and exact phrases to the native provider separately', () => {
    mocked.searchUploadedDocuments.mockResolvedValue(response(0))

    const search = useSearch({ current: null })
    search.handleSearch('anne "green gables" orchard')
    search.submitSearch()

    expect(mocked.searchUploadedDocuments).toHaveBeenCalledWith(
      'anne orchard',
      50,
      undefined,
      ['green gables'],
      expect.any(Function),
      'all',
    )
  })

  it('routes broader search only to uploaded documents', () => {
    mocked.searchUploadedDocuments.mockResolvedValue(response(0))
    const pagefind = { search: vi.fn().mockResolvedValue({ results: [] }) }
    const search = useSearch({ current: pagefind }, { mode: 'broader' })

    search.handleSearch('silver compass lantern')
    search.submitSearch()

    expect(pagefind.search).not.toHaveBeenCalled()
    expect(mocked.searchUploadedDocuments).toHaveBeenCalledWith(
      'silver compass lantern', 50, undefined, undefined, expect.any(Function), 'broader',
    )
  })

  it('offers an uploaded typo retry only after the combined result is empty', async () => {
    mocked.searchUploadedDocuments.mockResolvedValue({
      ...response(0), suggestedQuery: 'environment',
    })
    const search = useSearch({ current: null })
    const setLastSearchInfo = mocked.setters.at(-1)
    search.handleSearch('enviroment')
    search.submitSearch()
    await vi.waitFor(() => expect(setLastSearchInfo).toHaveBeenCalledWith(
      expect.objectContaining({ suggestedQuery: 'environment' }),
    ))
  })

  it('passes through a verified related-form retry alongside literal results', async () => {
    mocked.searchUploadedDocuments.mockResolvedValue({
      ...response(1), relatedQuery: 'environmental',
    })
    const search = useSearch({ current: null })
    const setLastSearchInfo = mocked.setters.at(-1)
    search.handleSearch('environment')
    search.submitSearch()
    await vi.waitFor(() => expect(setLastSearchInfo).toHaveBeenCalledWith(
      expect.objectContaining({ relatedQuery: 'environmental', uploadedDocuments: 1 }),
    ))
  })

  it('hides an uploaded suggestion when starter documents already answer the query', async () => {
    mocked.searchUploadedDocuments.mockResolvedValue({ ...response(0), suggestedQuery: 'environment' })
    const pagefind = { search: vi.fn().mockResolvedValue({ results: [{
      id: 'starter', data: async () => ({ id: 'starter', url: '/starter', meta: { title: 'Starter' }, excerpt: 'answer' }),
    }] }) }
    const search = useSearch({ current: pagefind })
    const setLastSearchInfo = mocked.setters.at(-1)
    search.handleSearch('enviroment')
    search.submitSearch()
    await vi.waitFor(() => expect(setLastSearchInfo).toHaveBeenCalledWith(
      expect.objectContaining({ suggestedQuery: undefined }),
    ))
  })

  it('preserves capitalized uploaded terms so names are not silently corrected', () => {
    mocked.searchUploadedDocuments.mockResolvedValue(response(0))
    const search = useSearch({ current: null })
    search.handleSearch('Collonialism')
    search.submitSearch()
    expect(mocked.searchUploadedDocuments).toHaveBeenCalledWith(
      'Collonialism', 50, undefined, undefined, expect.any(Function), 'all',
    )
  })
})
