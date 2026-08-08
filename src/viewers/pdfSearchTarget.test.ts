import { afterEach, describe, expect, it, vi } from 'vitest'
import { bindPdfSearchTarget } from './pdfSearchTarget'

describe('bindPdfSearchTarget', () => {
  afterEach(() => {
    vi.useRealTimers()
    vi.unstubAllGlobals()
  })

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
      querySelector: () => null,
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

  it('falls back and always clears progress when a text layer never renders', () => {
    vi.useFakeTimers()
    const search = vi.fn()
    const onProgress = vi.fn()
    const listeners = new Map<string, (event: never) => void>()
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      callback(0)
      return 8
    })
    vi.stubGlobal('cancelAnimationFrame', vi.fn())
    const viewer = {
      ownerDocument: { defaultView: null },
      querySelector: () => null,
      querySelectorAll: () => [],
    } as unknown as HTMLElement

    const cleanup = bindPdfSearchTarget({
      eventBus: {
        on: (name: string, listener: (event: never) => void) => listeners.set(name, listener),
        off: (name: string) => listeners.delete(name),
      } as never,
      getFindApi: () => ({ search, next: vi.fn(), previous: vi.fn(), clear: vi.fn() }),
      onProgress,
      pages: 12,
      pdfViewer: {} as never,
      target: { text: 'indexed result', pageIndex: 4 },
      viewer,
    })

    expect(onProgress).toHaveBeenLastCalledWith({ pageNumber: 5, phase: 'locating' })
    vi.advanceTimersByTime(1_500)
    expect(search).toHaveBeenCalledWith('indexed result')
    expect(onProgress).toHaveBeenLastCalledWith({ pageNumber: 5, phase: 'verifying' })
    vi.advanceTimersByTime(5_000)
    expect(onProgress).toHaveBeenLastCalledWith(null)

    cleanup()
  })
})
