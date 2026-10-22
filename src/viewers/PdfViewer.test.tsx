// @vitest-environment jsdom
import { act } from 'react'
import { createRoot } from 'react-dom/client'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { PdfViewer } from './PdfViewer'
import { createPdfTextTargetApi } from '../tts/playAnywhere/pdfTextTarget'
import type { ViewerTextTargetApi } from './types'

vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: (key: string) => key }) }))
vi.mock('./PdfControls', () => ({ PdfControls: () => null }))
vi.mock('../pdf/pdfJs', () => ({
  pdfJsAssetRoot: () => '/', pdfLoadErrorMessage: String,
  loadPdfJs: async () => ({ AnnotationMode: { ENABLE: 1 }, getDocument: () => ({
    promise: Promise.resolve({ numPages: 2, getOutline: async () => [] }), destroy: async () => {},
  }) }),
  loadPdfViewer: async () => ({
    SpreadMode: { NONE: 0, ODD: 1 },
    EventBus: class { on() {} off() {} dispatch() {} },
    PDFLinkService: class { setViewer() {} setDocument() {} },
    PDFFindController: class { setDocument() {} },
    PDFViewer: class {
      pagesCount = 2
      currentPageNumber = 1
      onePageRendered = Promise.resolve()
      setDocument() {} update() {} cleanup() {}
    },
  }),
}))
vi.mock('../tts/playAnywhere/pdfTextTarget', () => ({ createPdfTextTargetApi: vi.fn() }))
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
beforeEach(() => {
  vi.stubGlobal('matchMedia', () => ({ matches: false, addEventListener() {}, removeEventListener() {} }))
  vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} })
})
afterEach(() => { vi.clearAllMocks(); vi.unstubAllGlobals(); document.body.replaceChildren() })

it('does not set up PDF capture when no consumer requests it', async () => {
  const container = document.createElement('div')
  document.body.append(container)
  const app = createRoot(container)
  try {
    await act(async () => app.render(<PdfViewer url="one.pdf" />))
    await act(() => vi.dynamicImportSettled())
    expect(createPdfTextTargetApi).not.toHaveBeenCalled()
  } finally { await act(async () => app.unmount()) }
})

it('registers ready PDF capabilities and disposes them on URL replacement, opt-out and unmount', async () => {
  const disposals: Array<ReturnType<typeof vi.fn>> = []
  vi.mocked(createPdfTextTargetApi).mockImplementation(() => {
    const dispose = vi.fn()
    disposals.push(dispose)
    return { api: { captureViewport: async () => ({ status: 'unavailable', reason: 'not-ready' }) }, dispose }
  })
  const container = document.createElement('div')
  document.body.append(container)
  const app = createRoot(container)
  const register = vi.fn<(api: ViewerTextTargetApi | null) => void>()
  const viewport = () => ({ top: 50, bottom: 500, left: 0, right: 600 })
  try {
    await act(async () => app.render(<PdfViewer url="one.pdf" onTextTargetApiChange={register} getTextTargetViewport={viewport} />))
    await act(() => vi.dynamicImportSettled())
    const first = register.mock.calls.at(-1)![0]!
    expect(first.captureViewport).toBeTypeOf('function')
    expect(vi.mocked(createPdfTextTargetApi).mock.calls.at(-1)![3]).toBe(viewport)
    await act(async () => app.render(<PdfViewer url="two.pdf" onTextTargetApiChange={register} getTextTargetViewport={viewport} />))
    await act(() => vi.dynamicImportSettled())
    expect(disposals[0]).toHaveBeenCalledOnce()
    expect(register.mock.calls.some(([api]) => api === null)).toBe(true)
    expect(register.mock.calls.at(-1)![0]).not.toBe(first)
    await act(async () => app.render(<PdfViewer url="two.pdf" />))
    await act(() => vi.dynamicImportSettled())
    expect(disposals.every(dispose => dispose.mock.calls.length === 1)).toBe(true)
    expect(register.mock.calls.at(-1)![0]).toBeNull()
    await act(async () => app.render(<PdfViewer url="two.pdf" onTextTargetApiChange={register} />))
    await act(() => vi.dynamicImportSettled())
  } finally { await act(async () => app.unmount()) }
  expect(disposals.every(dispose => dispose.mock.calls.length === 1)).toBe(true)
  expect(register.mock.calls.at(-1)![0]).toBeNull()
})
