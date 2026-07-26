import { describe, expect, it, vi } from 'vitest'
import { createPdfFindAdapter, pdfSearchTargetPage } from './pdfFind'

class TestEventBus {
  readonly dispatched: Array<{ name: string; event: Record<string, unknown> }> = []
  private readonly listeners = new Map<string, Set<(event: never) => void>>()

  on(name: string, listener: (event: never) => void) {
    const listeners = this.listeners.get(name) ?? new Set()
    listeners.add(listener)
    this.listeners.set(name, listeners)
  }

  off(name: string, listener: (event: never) => void) {
    this.listeners.get(name)?.delete(listener)
  }

  dispatch(name: string, event: object) {
    this.dispatched.push({ name, event: event as Record<string, unknown> })
    this.listeners.get(name)?.forEach((listener) => listener(event as never))
  }
}

describe('PDF Find adapter', () => {
  it('maps indexed search results to bounded PDF.js page numbers', () => {
    expect(pdfSearchTargetPage({ text: 'rabbit', pageIndex: 6 }, 10)).toBe(7)
    expect(pdfSearchTargetPage({ text: 'rabbit', pageIndex: 12 }, 10)).toBe(10)
    expect(pdfSearchTargetPage({ text: 'rabbit', pageIndex: 0 })).toBe(1)
    expect(pdfSearchTargetPage({ text: '', pageIndex: 6 }, 10)).toBeNull()
    expect(pdfSearchTargetPage({ text: 'rabbit' }, 10)).toBeNull()
  })

  it('maps search, navigation, result counts, and cleanup to PDF.js events', () => {
    const eventBus = new TestEventBus()
    const onResult = vi.fn()
    const adapter = createPdfFindAdapter(eventBus, onResult)

    adapter.api.search('  rabbit hole  ')
    adapter.api.next()
    adapter.api.previous()
    eventBus.dispatch('updatefindmatchescount', {
      matchesCount: { current: 2, total: 5 },
    })

    expect(eventBus.dispatched.slice(0, 3).map(({ name, event }) => ({
      name,
      type: event.type,
      query: event.query,
      findPrevious: event.findPrevious,
    }))).toEqual([
      { name: 'find', type: '', query: 'rabbit hole', findPrevious: false },
      { name: 'find', type: 'again', query: 'rabbit hole', findPrevious: false },
      { name: 'find', type: 'again', query: 'rabbit hole', findPrevious: true },
    ])
    expect(onResult).toHaveBeenLastCalledWith({ currentIndex: 1, matchCount: 5 })

    adapter.api.clear()
    expect(eventBus.dispatched.at(-1)?.name).toBe('findbarclose')
    expect(onResult).toHaveBeenLastCalledWith({ currentIndex: 0, matchCount: 0 })

    adapter.dispose()
    eventBus.dispatch('updatefindmatchescount', {
      matchesCount: { current: 3, total: 5 },
    })
    expect(onResult).toHaveBeenCalledTimes(2)
  })
})
