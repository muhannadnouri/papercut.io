import { describe, expect, it } from 'vitest'
import type { SearchResult } from '../types/search'
import { resolveDocumentScopeUrls } from './useDocumentFilters'
import { pagefindResultsInScope } from './useSearch'

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
          return { id: url, url, meta: { title: url }, excerpt: '' }
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
        return { id: '/one', url: '/one', meta: { title: 'One' }, excerpt: '' }
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
