// @vitest-environment jsdom
import {
  act,
  createElement,
  useEffect,
} from 'react'
import { createRoot } from 'react-dom/client'
import {
  afterEach,
  describe,
  expect,
  it,
  vi,
} from 'vitest'
import { useDocumentViewerState } from './useDocumentViewerState'

afterEach(() => {
  vi.useRealTimers()
  window.localStorage.clear()
  document.body.replaceChildren()
})

function setup(loadHtmlDocument: (url: string) => Promise<string>) {
  const container = document.createElement('div')
  document.body.append(container)
  const root = createRoot(container)
  let latest: ReturnType<typeof useDocumentViewerState> | null = null

  function Harness() {
    const state = useDocumentViewerState({
      activeTab: 'library',
      documentsLoading: false,
      loadHtmlDocument,
    })
    useEffect(() => {
      latest = state
    })
    return null
  }

  const render = () => act(async () => {
    root.render(createElement(Harness))
  })

  return {
    render,
    unmount: () => act(async () => root.unmount()),
    get state() {
      if (!latest) {
        throw new Error('document viewer state has not rendered yet')
      }
      return latest
    },
  }
}

function seedBookmark(url: string, textOffset: number) {
  window.localStorage.setItem(`papercut:reader-bookmark:${url}`, JSON.stringify({
    updatedAtMs: 1,
    viewerLocation: { textOffset },
  }))
}

describe('useDocumentViewerState restoring open', () => {
  it('owns the failsafe per restore and clears it on completion and unmount', async () => {
    vi.useFakeTimers()
    seedBookmark('/book.html', 240)
    const app = setup(async () => '<p>Loaded</p>')
    await app.render()
    try {
      await act(async () => {
        await app.state.openDocument('/book.html', undefined, { restoreBookmark: true })
      })
      await act(async () => {
        vi.advanceTimersByTime(10000)
        app.state.clearSelectedDocument()
      })
      await act(async () => {
        await app.state.openDocument('/book.html', undefined, { restoreBookmark: true })
      })
      await act(async () => {
        vi.advanceTimersByTime(10000)
      })
      expect(app.state.documentLoad.status).toBe('restoring')
      await act(async () => {
        vi.advanceTimersByTime(10000)
      })
      expect(app.state.documentLoad.status).toBe('idle')
      await act(async () => {
        await app.state.openDocument('/book.html', undefined, { restoreBookmark: true })
      })
      await act(async () => {
        app.state.completeRestoringDocument()
      })
      expect(vi.getTimerCount()).toBe(0)
      await act(async () => {
        await app.state.openDocument('/book.html', undefined, { restoreBookmark: true })
      })
    } finally {
      await app.unmount()
    }
    expect(vi.getTimerCount()).toBe(0)
  })

  it('holds a bookmarked restore open in the restoring phase until completed', async () => {
    seedBookmark('/book.html', 240)
    const app = setup(async () => '<p>Loaded</p>')
    await app.render()

    await act(async () => {
      app.state.openDocument('/book.html', undefined, { restoreBookmark: true })
    })
    expect(app.state.documentLoad).toMatchObject({
      status: 'restoring',
      url: '/book.html',
    })
    expect(app.state.docContent).toBe('<p>Loaded</p>')

    act(() => {
      app.state.completeRestoringDocument()
    })
    expect(app.state.documentLoad).toEqual({ status: 'idle' })

    await app.unmount()
  })

  it('opens straight to idle when the document has no bookmark', async () => {
    const app = setup(async () => '<p>Loaded</p>')
    await app.render()

    await act(async () => {
      app.state.openDocument('/book.html', undefined, { restoreBookmark: true })
    })
    expect(app.state.documentLoad).toEqual({ status: 'idle' })
    expect(app.state.docContent).toBe('<p>Loaded</p>')

    await app.unmount()
  })

  it('opens straight to idle when restore is not requested', async () => {
    seedBookmark('/book.html', 240)
    const app = setup(async () => '<p>Loaded</p>')
    await app.render()

    await act(async () => {
      app.state.openDocument('/book.html')
    })
    expect(app.state.documentLoad).toEqual({ status: 'idle' })

    await app.unmount()
  })

  it('ignores a completion from an earlier restore of the same URL', async () => {
    seedBookmark('/book.html', 240)
    const app = setup(async () => '<p>Loaded</p>')
    await app.render()

    await act(async () => {
      app.state.openDocument('/book.html', undefined, { restoreBookmark: true })
    })
    expect(app.state.documentLoad).toMatchObject({ status: 'restoring' })
    const completeFirst = app.state.completeRestoringDocument

    act(() => {
      app.state.completeRestoringDocument()
    })
    await act(async () => {
      await app.state.openDocument('/book.html', undefined, { restoreBookmark: true })
    })
    act(() => {
      completeFirst()
    })
    expect(app.state.documentLoad.status).toBe('restoring')
    act(() => {
      app.state.completeRestoringDocument()
    })
    expect(app.state.documentLoad).toEqual({ status: 'idle' })

    await app.unmount()
  })
})
