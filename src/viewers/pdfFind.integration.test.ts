// @vitest-environment jsdom
import { readFileSync } from 'node:fs'
import {
  afterEach,
  expect,
  it,
  vi,
} from 'vitest'
import {
  loadPdfJs,
  loadPdfViewer,
} from '../pdf/pdfJs'
import {
  createPdfFindAdapter,
  createPdfFindController,
  pdfSearchTargetPage,
} from './pdfFind'
import type { PDFViewer } from 'pdfjs-dist/legacy/web/pdf_viewer.mjs'

afterEach(() => vi.unstubAllGlobals())

it('finds a quoted-result phrase through real PDF extraction, maps Unicode wraps to source offsets and navigates both pages', async () => {
  // Node runs the real worker handler locally instead of fetching Vite's browser worker URL.
  // @ts-expect-error PDF.js ships no declaration for its worker bundle.
  vi.stubGlobal('pdfjsWorker', await import('pdfjs-dist/legacy/build/pdf.worker.mjs'))
  const pdfjs = await loadPdfJs()
  const module = await loadPdfViewer()
  const task = pdfjs.getDocument({
    data: new Uint8Array(readFileSync('scripts/fixtures/pdf/find-unicode-wraps.pdf')),
    useSystemFonts: true,
  })
  const pdf = await task.promise
  const eventBus = new module.EventBus()
  const linkService = new module.PDFLinkService({ eventBus })
  // Page navigation is the host boundary; parser, Find controller and events are real.
  const viewer = { currentPageNumber: 1 } as PDFViewer
  linkService.setViewer(viewer)
  linkService.setDocument(pdf)
  const controller = createPdfFindController(module.PDFFindController, {
    linkService,
    eventBus,
    delay: 0,
  })
  const result = vi.fn()
  const adapter = createPdfFindAdapter(eventBus, result)
  try {
    controller.setDocument(pdf)
    const phrase = 'straightforward, taking coastal ferries from a single harbor to an island route.'
    const target = {
      text: phrase,
      pageIndex: 0,
    }
    linkService.page = pdfSearchTargetPage(target, pdf.numPages)!
    adapter.api.search(target.text)
    await vi.waitFor(() => expect(result).toHaveBeenLastCalledWith({
      currentIndex: 0,
      matchCount: 2,
    }))
    for (let pageNumber = 1; pageNumber <= 2; pageNumber++) {
      const page = await pdf.getPage(pageNumber)
      const content = await page.getTextContent({ disableNormalization: true })
      const source = content.items.filter(item => 'str' in item).map(item => item.str).join('')
      const hyphen = pageNumber === 1 ? '\u2010' : '\u2011'
      expect(source).toBe(`Prefix 😀 Straightfor${hyphen}ward, taking coastal ferries from a single harborto an island route.`)
      expect(controller.pageMatches![pageNumber - 1]).toEqual(['Prefix 😀 '.length])
      expect(controller.pageMatchesLength![pageNumber - 1]).toEqual([source.length - 'Prefix 😀 '.length])
      const [start] = controller.pageMatches![pageNumber - 1]
      const [length] = controller.pageMatchesLength![pageNumber - 1]
      expect(source.slice(start, start + length)).toBe(source.slice('Prefix 😀 '.length))
    }
    adapter.api.next()
    await vi.waitFor(() => expect(result).toHaveBeenLastCalledWith({
      currentIndex: 1,
      matchCount: 2,
    }))
    expect(linkService.page).toBe(2)
    adapter.api.previous()
    await vi.waitFor(() => expect(result).toHaveBeenLastCalledWith({
      currentIndex: 0,
      matchCount: 2,
    }))
    expect(linkService.page).toBe(1)
    adapter.api.search('a passage absent from both pages')
    await vi.waitFor(() => expect(result).toHaveBeenLastCalledWith({
      currentIndex: 0,
      matchCount: 0,
    }))
    adapter.api.clear()
    await vi.waitFor(() => expect(controller.highlightMatches).toBe(false))
    adapter.dispose()
    const calls = result.mock.calls.length
    eventBus.dispatch('updatefindmatchescount', {
      matchesCount: {
        current: 1,
        total: 99,
      },
    })
    expect(result).toHaveBeenCalledTimes(calls)
  } finally {
    adapter.dispose()
    controller.setDocument(null as never)
    await task.destroy()
  }
})
