import { describe, expect, it, vi } from 'vitest'
import { createPdfFindAdapter, createPdfFindController, pdfFindTextItems, pdfSearchTargetPage } from './pdfFind'
import type { PDFFindController } from 'pdfjs-dist/legacy/web/pdf_viewer.mjs'
import type { PDFDocumentProxy } from 'pdfjs-dist/legacy/build/pdf.mjs'

describe('PDF Find extraction', () => {
  it('keeps Find extraction separate from rendering and forwards errors and reset', async () => {
    let findDocument: PDFDocumentProxy | null = null
    class TestController {
      setDocument(document: PDFDocumentProxy) { findDocument = document }
    }
    const controller = createPdfFindController(TestController as unknown as typeof PDFFindController, {} as never)
    const content = { items: [{ str: 'high‐', hasEOL: true }, { str: 'lights', hasEOL: false }] }
    const getTextContent = vi.fn().mockResolvedValue(content)
    const page = { getTextContent }
    const getPage = vi.fn().mockResolvedValue(page)
    const original = { numPages: 1, getPage } as unknown as PDFDocumentProxy
    controller.setDocument(original)
    const findPage = await findDocument!.getPage(1)
    expect(await findPage.getTextContent({ disableNormalization: true })).toEqual({
      items: [{ str: 'high-', hasEOL: true }, { str: 'lights', hasEOL: false }],
    })
    expect(getPage).toHaveBeenCalledWith(1)
    expect(getTextContent).toHaveBeenCalledWith({ disableNormalization: true })
    expect(await page.getTextContent()).toBe(content)
    expect(content.items[0].str).toBe('high‐')
    getTextContent.mockRejectedValueOnce(new Error('extraction failed'))
    await expect(findPage.getTextContent()).rejects.toThrow('extraction failed')
    controller.setDocument(null as never)
    expect(findDocument).toBeNull()
  })

  it('uses the same conservative uppercase continuation rule as PDF.js', () => {
    expect(pdfFindTextItems([
      { str: 'HTTP‐', hasEOL: true }, { str: 'Server', hasEOL: false },
    ])).toEqual([{ str: 'HTTP-', hasEOL: true }, { str: 'Server', hasEOL: false }])
  })
  it.each(['\u2010', '\u2011'])('repairs a %s wrap without moving original UTF-16 offsets', (hyphen) => {
    const items = [
      { str: `😀 High${hyphen}`, hasEOL: true },
      { str: 'lights guide ferries.', hasEOL: false },
    ]
    const repaired = pdfFindTextItems(items)
    expect(repaired.map((item) => item.str)).toEqual(['😀 High-', 'lights guide ferries.'])
    expect(repaired.map((item) => item.str.length)).toEqual(items.map((item) => item.str.length))
    expect(items[0].str).toBe(`😀 High${hyphen}`)
    expect(repaired[1]).toBe(items[1])
  })

  it('preserves compounds, punctuation and non-continuing lines', () => {
    for (const [before, after, hasEOL] of [
      ['high‐', 'lights', false], ['state‐owned', 'ferries', true],
      ['high—', 'lights', true], ['high–', 'lights', true],
      ['high‐', 'Lights', true], ['high‐', '42 lights', true],
    ] as const) {
      const items = [{ str: before, hasEOL }, { str: after, hasEOL: false }]
      expect(pdfFindTextItems(items)).toEqual(items)
    }
  })

  it('retains line breaks carried by empty items and ignores marked content', () => {
    const items = [
      { str: 'high‐', hasEOL: false }, { str: '', hasEOL: true },
      { type: 'beginMarkedContent' }, { str: 'lights', hasEOL: false },
    ]
    expect(pdfFindTextItems(items)).toEqual([{ str: 'high-', hasEOL: false }, ...items.slice(1)])
    expect(pdfFindTextItems([
      { str: 'high', hasEOL: false }, { str: '‐', hasEOL: true }, { str: 'lights', hasEOL: false },
    ])).toEqual([
      { str: 'high', hasEOL: false }, { str: '-', hasEOL: true }, { str: 'lights', hasEOL: false },
    ])
  })
})

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

  it('treats explicit PDF line-wrap spellings as bounded aliases', () => {
    const eventBus = new TestEventBus()
    const adapter = createPdfFindAdapter(eventBus, vi.fn())

    adapter.api.search('high-lights')
    expect(eventBus.dispatched.at(-1)?.event.query).toEqual(expect.arrayContaining([
      'high-lights',
      'high- lights',
      'highlights',
      'high–lights',
      'high—lights',
    ]))
    expect(eventBus.dispatched.at(-1)?.event.query).toHaveLength(9)

    adapter.api.search('high- lights')
    expect(eventBus.dispatched.at(-1)?.event.query).toEqual(expect.arrayContaining([
      'high- lights',
      'high-lights',
      'highlights',
      'high–lights',
      'high—lights',
    ]))
    expect(eventBus.dispatched.at(-1)?.event.query).toHaveLength(9)

    adapter.api.search('high—lights')
    expect(eventBus.dispatched.at(-1)?.event.query).toEqual(expect.arrayContaining([
      'high-lights',
      'high–lights',
      'high—lights',
      'highlights',
    ]))
    expect(eventBus.dispatched.at(-1)?.event.query).toHaveLength(9)

    adapter.api.search('highlights')
    expect(eventBus.dispatched.at(-1)?.event.query).toBe('highlights')
  })

  it('varies multiple hyphenated compounds independently', () => {
    const eventBus = new TestEventBus()
    const adapter = createPdfFindAdapter(eventBus, vi.fn())

    adapter.api.search('high-lights state-owned')
    const aliases = eventBus.dispatched.at(-1)?.event.query
    expect(aliases).toEqual(expect.arrayContaining([
      'high-lights state-owned',
      'highlights state-owned',
      'high-lights stateowned',
      'highlights stateowned',
      'high–lights state–owned',
      'high—lights state—owned',
      'high–lights state—owned',
      'high—lights state–owned',
    ]))
    expect(aliases).toHaveLength(57)
  })
})
