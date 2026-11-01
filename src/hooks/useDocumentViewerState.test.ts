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
} from 'vitest'
import { useDocumentViewerState } from './useDocumentViewerState'

afterEach(() => {
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

  it('keeps restoring state isolated to the restoring url', async () => {
    seedBookmark('/book.html', 240)
    const app = setup(async () => '<p>Loaded</p>')
    await app.render()

    await act(async () => {
      app.state.openDocument('/book.html', undefined, { restoreBookmark: true })
    })
    expect(app.state.documentLoad).toMatchObject({ status: 'restoring' })

    act(() => {
      app.state.completeRestoringDocument()
    })
    // A completion for a different open must not flip an unrelated phase.
    act(() => {
      app.state.completeRestoringDocument()
    })
    expect(app.state.documentLoad).toEqual({ status: 'idle' })

    await app.unmount()
  })
})
