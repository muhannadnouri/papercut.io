import { afterEach, describe, expect, it, vi } from 'vitest'
import { bindPdfSearchTarget } from './pdfSearchTarget'

describe('bindPdfSearchTarget', () => {
  afterEach(() => vi.unstubAllGlobals())

  it('schedules document-wide targets through the current Find adapter', () => {
    const search = vi.fn()
    const cancel = vi.fn()
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      callback(0)
      return 7
    })
    vi.stubGlobal('cancelAnimationFrame', cancel)
    const viewer = {
      ownerDocument: { defaultView: null },
      querySelectorAll: () => [],
    } as unknown as HTMLElement

    const cleanup = bindPdfSearchTarget({
      eventBus: {} as never,
      getFindApi: () => ({ search, next: vi.fn(), previous: vi.fn(), clear: vi.fn() }),
      onProgress: vi.fn(),
      pages: 12,
      pdfViewer: {} as never,
      target: { text: '  whole document  ' },
      viewer,
    })

    expect(search).toHaveBeenCalledWith('whole document')
    cleanup()
    expect(cancel).toHaveBeenCalledWith(7)
  })
})
