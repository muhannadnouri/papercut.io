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
import type { ViewerBookmarkApi } from '../viewers/types'
import {
  parseReaderBookmark,
  readBookmarkedDocumentUrls,
  useReaderBookmark,
} from './useReaderBookmark'

function bookmarkStorage(entries: Record<string, string>) {
  const keys = Object.keys(entries)
  return {
    length: keys.length,
    key: (index: number) => keys[index] ?? null,
    getItem: (key: string) => entries[key] ?? null,
  }
}

describe('readBookmarkedDocumentUrls', () => {
  it('returns only URLs backed by valid current bookmarks', () => {
    const valid = JSON.stringify({
      updatedAtMs: 10,
      viewerLocation: { textOffset: 1842 },
    })
    const urls = readBookmarkedDocumentUrls(bookmarkStorage({
      'papercut:reader-bookmark:/documents/book.html': valid,
      'papercut:reader-bookmark:upload://invalid': '{',
      'unrelated-setting': valid,
    }))

    expect([...urls]).toEqual(['/documents/book.html'])
  })
})

describe('parseReaderBookmark', () => {
  it('keeps PDF page coordinates', () => {
    expect(parseReaderBookmark(JSON.stringify({
      updatedAtMs: 10,
      viewerLocation: {
        pageNumber: 7,
        left: 12,
        top: 416,
      },
    }))).toEqual({
      updatedAtMs: 10,
      viewerLocation: {
        pageNumber: 7,
        left: 12,
        top: 416,
      },
    })
  })

  it('keeps semantic HTML and EPUB text offsets', () => {
    expect(parseReaderBookmark(JSON.stringify({
      updatedAtMs: 10,
      viewerLocation: { textOffset: 1842 },
    }))).toEqual({
      updatedAtMs: 10,
      viewerLocation: { textOffset: 1842 },
    })
  })

  it('rejects legacy window-scroll bookmarks', () => {
    expect(parseReaderBookmark(JSON.stringify({
      scrollRatio: 0.5,
      scrollY: 320,
      updatedAtMs: 10,
    }))).toBeNull()
  })

  it('rejects an invalid viewer location', () => {
    expect(parseReaderBookmark(JSON.stringify({
      updatedAtMs: 10,
      viewerLocation: {
        pageNumber: 7,
        pageOffsetRatio: 0.5,
      },
    }))).toBeNull()
  })

  it.each([
    ['malformed JSON', '{'],
    ['JSON null', 'null'],
    ['a non-finite timestamp', '{"updatedAtMs":1e999,"viewerLocation":{"textOffset":1}}'],
  ])('rejects %s', (_case, raw) => {
    expect(parseReaderBookmark(raw)).toBeNull()
  })
})

interface BookmarkHarnessProps {
  url: string
  enabled: boolean
  restoreOnOpen?: boolean
  viewerApi: ViewerBookmarkApi
}

function createBookmarkApiStub() {
  return {
    capture: vi.fn(() => null),
    isCurrent: vi.fn(() => false),
    isPastStart: vi.fn(() => false),
    restore: vi.fn(),
    scrollToTop: vi.fn(),
    subscribe: vi.fn(() => () => {}),
  }
}

function stubAnimationFrames() {
  const pending = new Map<number, () => void>()
  let nextId = 1
  vi.stubGlobal('requestAnimationFrame', (callback: () => void) => {
    const id = nextId
    nextId += 1
    pending.set(id, callback)
    return id
  })
  vi.stubGlobal('cancelAnimationFrame', (id: number) => {
    pending.delete(id)
  })
  return {
    flush(times = 1) {
      for (let run = 0; run < times; run += 1) {
        const next = pending.entries().next()
        if (next.done) {
          return
        }
        pending.delete(next.value[0])
        next.value[1]()
      }
    },
  }
}

function flushAnimationFrames(frames: ReturnType<typeof stubAnimationFrames>, times = 1) {
  act(() => {
    frames.flush(times)
  })
}

function createBookmarkHarness() {
  const container = document.createElement('div')
  document.body.append(container)
  const root = createRoot(container)
  let latest: ReturnType<typeof useReaderBookmark> | null = null

  function Harness(props: BookmarkHarnessProps) {
    const state = useReaderBookmark(props.url, {
      enabled: props.enabled,
      restoreOnOpen: props.restoreOnOpen,
      viewerApi: props.viewerApi,
    })
    useEffect(() => {
      latest = state
    })
    return null
  }

  return {
    render(props: BookmarkHarnessProps) {
      act(() => {
        root.render(createElement(Harness, props))
      })
    },
    get state() {
      if (!latest) {
        throw new Error('bookmark hook has not rendered yet')
      }
      return latest
    },
    unmount() {
      act(() => {
        root.unmount()
      })
    },
  }
}

function seedBookmark(url: string, textOffset: number) {
  window.localStorage.setItem(`papercut:reader-bookmark:${url}`, JSON.stringify({
    updatedAtMs: 1,
    viewerLocation: { textOffset },
  }))
}

describe('useReaderBookmark restore indicator', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
    document.body.replaceChildren()
  })

  it('keeps the restoring indicator up while restore-on-open seeks, then completes it', async () => {
    const frames = stubAnimationFrames()
    const viewerApi = createBookmarkApiStub()
    seedBookmark('/book.html', 240)
    const harness = createBookmarkHarness()

    harness.render({
      url: '/book.html',
      enabled: true,
      viewerApi,
    })
    // Frame 1 mounts the visible surface; the seek only runs after the next
    // frame, so at least one painted frame of the indicator is guaranteed.
    flushAnimationFrames(frames)
    expect(harness.state.restoringBookmark).toBe(true)
    expect(viewerApi.restore).not.toHaveBeenCalled()

    // Frame 2 is runRestore's paint slot; frame 3 starts the async seek.
    flushAnimationFrames(frames, 2)
    await act(async () => {})
    expect(viewerApi.restore).toHaveBeenCalledWith({ textOffset: 240 })
    expect(harness.state.restoringBookmark).toBe(false)
    expect(harness.state.bookmarkNotice).toBe('restored')
    expect(harness.state.isAtBookmark).toBe(true)

    harness.unmount()
  })

  it('shows the restoring indicator for an explicit return to the bookmark too', async () => {
    const frames = stubAnimationFrames()
    const viewerApi = createBookmarkApiStub()
    seedBookmark('/book.html', 240)
    const harness = createBookmarkHarness()

    harness.render({
      url: '/book.html',
      enabled: true,
      restoreOnOpen: false,
      viewerApi,
    })
    flushAnimationFrames(frames)
    expect(viewerApi.restore).not.toHaveBeenCalled()

    act(() => {
      harness.state.restoreBookmark()
    })
    expect(harness.state.restoringBookmark).toBe(true)

    flushAnimationFrames(frames, 2)
    await act(async () => {})
    expect(viewerApi.restore).toHaveBeenCalledTimes(1)
    expect(harness.state.restoringBookmark).toBe(false)
    expect(harness.state.bookmarkNotice).toBe('restored')

    harness.unmount()
  })

  it('drops a pending restore when the document changes mid-restore', async () => {
    const frames = stubAnimationFrames()
    const viewerApi = createBookmarkApiStub()
    seedBookmark('/book.html', 240)
    const harness = createBookmarkHarness()

    harness.render({
      url: '/book.html',
      enabled: true,
      viewerApi,
    })
    flushAnimationFrames(frames)
    expect(harness.state.restoringBookmark).toBe(true)

    harness.render({
      url: '/other.html',
      enabled: true,
      viewerApi,
    })
    expect(harness.state.restoringBookmark).toBe(false)

    flushAnimationFrames(frames, 2)
    await act(async () => {})
    expect(viewerApi.restore).not.toHaveBeenCalled()

    harness.unmount()
  })
})
