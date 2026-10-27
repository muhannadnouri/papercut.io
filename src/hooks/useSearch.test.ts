import {
  describe,
  expect,
  it,
  vi,
} from 'vitest'
import type { SearchResult } from '../types/search'
import { resolveDocumentScopeUrls } from './useDocumentFilters'
import {
  pagefindResultsInScope,
  searchStarterDocuments,
} from './useSearch'

describe('document search scope', () => {
  const allUrls = ['/one', '/two', '/three']
  const selected = new Set(['/two'])

  it('resolves selected documents for include and their complement for exclude', () => {
    expect([...resolveDocumentScopeUrls(allUrls, selected, 'include')]).toEqual(['/two'])
    expect([...resolveDocumentScopeUrls(allUrls, selected, 'exclude')]).toEqual(['/one', '/three'])
  })

  it('applies the Pagefind scope before the visible-result limit', async () => {
    let reads = 0
    const results = Array.from({ length: 56 }, (_, index) => {
      const url = index === 55 ? '/allowed' : `/other-${index}`
      return {
        id: url,
        data: async (): Promise<SearchResult> => {
          reads += 1
          return {
            id: url,
            url,
            meta: { title: url },
            excerpt: '',
          }
        },
      }
    })

    const scoped = await pagefindResultsInScope(results, new Set(['/allowed']), true)

    expect(scoped.results.map((result) => result.url)).toEqual(['/allowed'])
    expect(scoped.totalDocuments).toBe(1)
    expect(reads).toBe(56)
  })

  it('does not read provider results when an active scope allows no documents', async () => {
    let reads = 0
    const results = [{
      id: '/one',
      data: async (): Promise<SearchResult> => {
        reads += 1
        return {
          id: '/one',
          url: '/one',
          meta: { title: 'One' },
          excerpt: '',
        }
      },
    }]

    expect(await pagefindResultsInScope(results, new Set(), true)).toEqual({
      results: [],
      totalDocuments: 0,
    })
    expect(reads).toBe(0)
  })

  it('counts scoped Pagefind matches before limiting visible starter documents', async () => {
    const results = Array.from({ length: 60 }, (_, index) => ({
      id: String(index),
      data: async (): Promise<SearchResult> => ({
        id: String(index),
        url: `/allowed-${index}`,
        meta: { title: String(index) },
        excerpt: '',
      }),
    }))
    const allowed = new Set(results.map((_, index) => `/allowed-${index}`))

    const scoped = await pagefindResultsInScope(results, allowed, true)

    expect(scoped.results).toHaveLength(50)
    expect(scoped.totalDocuments).toBe(60)
  })
})

describe('starter All Words eligibility', () => {
  it('rejects loose partial hits, preserves provider order and checks every distinct unquoted term', async () => {
    const rows = ['loose', 'second', 'first', 'missing'].map(id => ({
      id,
      data: vi.fn(),
    }))
    const pagefind = {
      search: vi.fn().mockResolvedValueOnce({ results: rows })
        .mockResolvedValueOnce({ results: rows.slice(1) })
        .mockResolvedValueOnce({ results: [rows[2], rows[1]] }),
    }
    const found = await searchStarterDocuments(pagefind, 'orchard lantern orchard', 'orchard lantern orchard')
    expect(pagefind.search.mock.calls.map(([query]) => query)).toEqual([
      'orchard lantern orchard', '"orchard"', '"lantern"',
    ])
    expect(found.results).toEqual([rows[1], rows[2]])
    expect(rows.every(row => row.data.mock.calls.length === 0)).toBe(true)
  })

  it('short-circuits missing words and leaves phrase verification separate', async () => {
    const row = {
      id: 'one',
      data: vi.fn(),
    }
    const pagefind = {
      search: vi.fn().mockResolvedValueOnce({ results: [row] })
        .mockResolvedValueOnce({ results: [] }),
    }
    expect(await searchStarterDocuments(pagefind, 'orchard lantern', 'orchard lantern')).toEqual({ results: [] })
    expect(pagefind.search).toHaveBeenCalledTimes(2)
    pagefind.search.mockReset().mockResolvedValue({ results: [row] })
    expect(await searchStarterDocuments(pagefind, 'silver compass', '')).toEqual({ results: [row] })
    expect(pagefind.search).toHaveBeenCalledTimes(1)
  })
})

it('stops starter term lookups when a request is superseded during provider work', async () => {
  let release!: (value: { results: { id: string;
    data: ReturnType<typeof vi.fn> }[] }) => void
  const row = {
    id: 'one',
    data: vi.fn(),
  }
  const pagefind = {
    search: vi.fn().mockResolvedValueOnce({ results: [row] })
      .mockImplementationOnce(() => new Promise(resolve => {
        release = resolve
      })),
  }
  let current = true
  const work = searchStarterDocuments(pagefind, 'orchard lantern compass', 'orchard lantern compass', () => current)
  await vi.waitFor(() => expect(pagefind.search).toHaveBeenCalledTimes(2))
  current = false
  release({ results: [row] })
  expect(await work).toEqual({ results: [] })
  expect(pagefind.search).toHaveBeenCalledTimes(2)
  expect(row.data).not.toHaveBeenCalled()
})
