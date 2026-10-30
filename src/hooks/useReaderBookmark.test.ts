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

function stubPendingWork() {
  const frames = new Map<number, () => void>()
  const timers = new Map<number, () => void>()
  let nextId = 1
  vi.stubGlobal('requestAnimationFrame', (callback: () => void) => {
    const id = nextId
    nextId += 1
    frames.set(id, callback)
    return id
  })
  vi.stubGlobal('cancelAnimationFrame', (id: number) => {
    frames.delete(id)
  })
  vi.stubGlobal('setTimeout', (callback: () => void) => {
    const id = nextId
    nextId += 1
    timers.set(id, callback)
    return id
  })
  vi.stubGlobal('clearTimeout', (id: number) => {
    timers.delete(id)
  })
  return {
    flushFrames(times = 1) {
      for (let run = 0; run < times; run += 1) {
        const next = frames.entries().next()
        if (next.done) {
          return
        }
        frames.delete(next.value[0])
        next.value[1]()
      }
    },
    runTimers() {
      // Drain the timers queued at call time. React's scheduler may interleave
      // its own render timers ahead of the seek timer, so an exact count never
      // lands on the seek; timers scheduled during the drain (the post-restore
      // notice timer) are left queued.
      let remaining = timers.size
      while (remaining > 0 && timers.size > 0) {
        const next = timers.entries().next()
        if (next.done) {
          return
        }
        timers.delete(next.value[0])
        next.value[1]()
        remaining -= 1
      }
    },
  }
}

function flushAnimationFrames(work: ReturnType<typeof stubPendingWork>, times = 1) {
  act(() => {
    work.flushFrames(times)
  })
}

function runPendingTimers(work: ReturnType<typeof stubPendingWork>) {
  act(() => {
    work.runTimers()
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

  it('keeps the restoring indicator up while restore-on-open seeks, then completes it', () => {
    const work = stubPendingWork()
    const viewerApi = createBookmarkApiStub()
    seedBookmark('/book.html', 240)
    const harness = createBookmarkHarness()

    harness.render({
      url: '/book.html',
      enabled: true,
      viewerApi,
    })
    flushAnimationFrames(work)
    // The indicator is up and the seek waits out the surface reveal delay.
    expect(harness.state.restoringBookmark).toBe(true)
    expect(viewerApi.restore).not.toHaveBeenCalled()

    flushAnimationFrames(work)
    runPendingTimers(work)
    expect(viewerApi.restore).toHaveBeenCalledWith({ textOffset: 240 })
    expect(harness.state.restoringBookmark).toBe(false)
    expect(harness.state.bookmarkNotice).toBe('restored')
    expect(harness.state.isAtBookmark).toBe(true)

    harness.unmount()
  })

  it('shows the restoring indicator for an explicit return to the bookmark too', () => {
    const work = stubPendingWork()
    const viewerApi = createBookmarkApiStub()
    seedBookmark('/book.html', 240)
    const harness = createBookmarkHarness()

    harness.render({
      url: '/book.html',
      enabled: true,
      restoreOnOpen: false,
      viewerApi,
    })
    flushAnimationFrames(work)
    expect(viewerApi.restore).not.toHaveBeenCalled()

    act(() => {
      harness.state.restoreBookmark()
    })
    expect(harness.state.restoringBookmark).toBe(true)

    flushAnimationFrames(work)
    runPendingTimers(work)
    expect(viewerApi.restore).toHaveBeenCalledTimes(1)
    expect(harness.state.restoringBookmark).toBe(false)
    expect(harness.state.bookmarkNotice).toBe('restored')

    harness.unmount()
  })

  it('drops a pending restore when the document changes mid-restore', () => {
    const work = stubPendingWork()
    const viewerApi = createBookmarkApiStub()
    seedBookmark('/book.html', 240)
    const harness = createBookmarkHarness()

    harness.render({
      url: '/book.html',
      enabled: true,
      viewerApi,
    })
    flushAnimationFrames(work)
    expect(harness.state.restoringBookmark).toBe(true)

    harness.render({
      url: '/other.html',
      enabled: true,
      viewerApi,
    })
    expect(harness.state.restoringBookmark).toBe(false)

    flushAnimationFrames(work)
    runPendingTimers(work)
    expect(viewerApi.restore).not.toHaveBeenCalled()

    harness.unmount()
  })
})
