import { describe, expect, it, vi } from 'vitest'
import type {
  EventBus,
  PDFViewer as PdfJsViewer,
} from 'pdfjs-dist/legacy/web/pdf_viewer.mjs'
import { createPdfBookmarkApi } from './pdfBookmark'

describe('createPdfBookmarkApi', () => {
  it('keeps tracking PDF.js after one subscriber disconnects', () => {
    const listeners = new Map<string, (event: unknown) => void>()
    const eventBus = {
      on: (name: string, listener: (event: unknown) => void) => {
        listeners.set(name, listener)
      },
    } as unknown as EventBus
    const pdfViewer = {
      currentPageNumber: 1,
      pagesCount: 200,
      getPageView: () => ({
        div: { getBoundingClientRect: () => ({ top: 100 }) },
        viewport: { convertToViewportPoint: () => [0, 200] },
      }),
      update: vi.fn(),
      scrollPageIntoView: vi.fn(),
    } as unknown as PdfJsViewer
    const container = {
      scrollTop: 0,
      getBoundingClientRect: () => ({ top: 0, bottom: 600 }),
    } as unknown as HTMLElement
    const api = createPdfBookmarkApi(pdfViewer, container, eventBus)
    const first = vi.fn()
    const second = vi.fn()
    const unsubscribeFirst = api.subscribe(first)
    api.subscribe(second)

    unsubscribeFirst()
    listeners.get('updateviewarea')?.({
      location: { pageNumber: 136, left: 0, top: 720 },
    })

    expect(api.capture()).toEqual({ pageNumber: 136, left: 0, top: 720 })
    expect(first).not.toHaveBeenCalled()
    expect(second).toHaveBeenCalledOnce()
    expect(api.isCurrent({ pageNumber: 136, left: 0, top: 720 })).toBe(true)
  })

  it('keeps a bookmark active across zoom changes while its point remains visible', () => {
    const eventBus = { on: vi.fn() } as unknown as EventBus
    let pageTop = 80
    const pdfViewer = {
      currentPageNumber: 136,
      pagesCount: 200,
      getPageView: () => ({
        div: { getBoundingClientRect: () => ({ top: pageTop }) },
        viewport: { convertToViewportPoint: () => [480, 240] },
      }),
      update: vi.fn(),
    } as unknown as PdfJsViewer
    const api = createPdfBookmarkApi(
      pdfViewer,
      {
        scrollTop: 0,
        getBoundingClientRect: () => ({ top: 0, bottom: 600 }),
      } as unknown as HTMLElement,
      eventBus,
    )
    const bookmark = { pageNumber: 136, left: 0, top: 720 }

    expect(api.isCurrent(bookmark)).toBe(true)
    pageTop = -400
    expect(api.isCurrent(bookmark)).toBe(false)
  })

  it('restores PDF coordinates without changing zoom', () => {
    const eventBus = { on: vi.fn() } as unknown as EventBus
    const scrollPageIntoView = vi.fn()
    const pdfViewer = {
      currentPageNumber: 1,
      pagesCount: 200,
      update: vi.fn(),
      scrollPageIntoView,
    } as unknown as PdfJsViewer
    const api = createPdfBookmarkApi(
      pdfViewer,
      { scrollTop: 0 } as HTMLElement,
      eventBus,
    )

    api.restore({ pageNumber: 136, left: 0, top: 720 })

    expect(scrollPageIntoView).toHaveBeenCalledWith({
      pageNumber: 136,
      destArray: [null, { name: 'XYZ' }, 0, 720, null],
      ignoreDestinationZoom: true,
    })
  })
})
