// @vitest-environment jsdom
import {
  act,
  useCallback,
  useState,
} from 'react'
import { createRoot } from 'react-dom/client'
import {
  expect,
  it,
  vi,
} from 'vitest'
import '../../i18n'
import { DocumentViewer } from './DocumentViewer'

vi.mock('../../viewers/registry', async () => {
  const { HtmlViewer } = await import('../../viewers/HtmlViewer')
  return {
    resolveViewer: () => ({
      id: 'html',
      Component: HtmlViewer,
    }),
  }
})

it('reveals the same restored HTML viewer and preserves its success notice', async () => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true)
  vi.stubGlobal('CSS', {})
  const frames = new Map<number, FrameRequestCallback>()
  let nextFrame = 0
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
    frames.set(++nextFrame, callback)
    return nextFrame
  })
  vi.stubGlobal('cancelAnimationFrame', (id: number) => frames.delete(id))
  const createRange = document.createRange.bind(document)
  vi.spyOn(document, 'createRange').mockImplementation(() => {
    const range = createRange()
    range.getBoundingClientRect = () => DOMRect.fromRect({
      y: 600,
      height: 20,
    })
    return range
  })
  const scroll = vi.spyOn(window, 'scrollTo').mockImplementation(() => {})
  const host = document.createElement('div')
  document.body.append(host)
  const root = createRoot(host)
  const complete = vi.fn()
  localStorage.setItem('papercut:reader-bookmark:/book.html', JSON.stringify({
    updatedAtMs: 1,
    viewerLocation: { textOffset: 8 },
  }))
  function Reader() {
    const [restoring, setRestoring] = useState(true)
    const onComplete = useCallback(() => {
      if (restoring) {
        complete()
        setRestoring(false)
      }
    }, [restoring])
    return <DocumentViewer url="/book.html" content="<p>Alpha Beta Gamma</p>" restoreBookmark
      restoringDocument={restoring} onRestoringDocumentComplete={onComplete}
      onClose={() => {}} />
  }
  try {
    await act(async () => root.render(<Reader />))
    const article = host.querySelector('article')
    expect(article).not.toBeNull()
    expect(host.querySelector('.document-viewer-hidden')).not.toBeNull()
    expect(host.querySelector('.document-restoring-surface')).not.toBeNull()
    for (let step = 0; step < 8; step++) {
      await act(async () => {
        const batch = [...frames.values()]
        frames.clear()
        batch.forEach(callback => callback(performance.now()))
      })
    }
    expect(complete).toHaveBeenCalledOnce()
    expect(host.querySelector('article')).toBe(article)
    expect(host.querySelector('.document-restoring-surface')).toBeNull()
    expect(host.textContent).toContain('Jumped to bookmark.')
    expect(scroll).toHaveBeenCalledWith({
      top: 600,
      behavior: 'auto',
    })
  } finally {
    await act(async () => root.unmount())
    host.remove()
    localStorage.clear()
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  }
})

it('shows publisher Contents only for EPUB and clears it across document switches', async () => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true)
  vi.stubGlobal('CSS', {})
  const host = document.createElement('div')
  document.body.append(host)
  const root = createRoot(host)
  const entries = [{
    label: 'Section',
    target: 'chapter-0',
    children: [],
  }]
  try {
    // EPUB uploads also use .html URLs. A mounted HTML surface alone is insufficient.
    const content = '<h1 data-papercut-section="0">Opening</h1><h2 data-papercut-section="1">Setup</h2>'
    for (const [url, format, outline, visible] of [
      ['/uploads/aa.html', 'epub', entries, true],
      ['/uploads/bb.html', 'epub', [], false],
      ['/book.html', 'html', entries, false],
      ['/uploads/cc.pdf', 'pdf', [], false],
    ] as const) {
      await act(async () => root.render(<DocumentViewer url={url} format={format}
        content={content} outline={[...outline]} onClose={() => {}} />))
      expect(Boolean(host.querySelector('.document-outline-trigger'))).toBe(visible)
    }
  } finally {
    await act(async () => root.unmount())
    host.remove()
    vi.unstubAllGlobals()
  }
})

const bodyHeading = '<h2 data-papercut-section="0">Setup</h2>'
const navigationHeadings = '<nav><h2 data-papercut-section="1">Menu</h2><h3 data-papercut-section="2">Links</h3></nav>'

it.each([
  {
    example: 'no headings',
    content: '<p>Introduction</p>',
    visible: false,
  },
  {
    example: 'one body heading plus navigation headings',
    content: bodyHeading + navigationHeadings,
    visible: false,
  },
  {
    example: 'two body headings of different ranks',
    content: bodyHeading + '<h3 data-papercut-section="1">Install</h3>',
    visible: true,
  },
])('applies the HTML Contents threshold to $example', async ({ content, visible }) => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true)
  vi.stubGlobal('CSS', {})
  const host = document.createElement('div')
  document.body.append(host)
  const root = createRoot(host)
  try {
    await act(async () => root.render(<DocumentViewer url="/uploads/ab.html" format="html"
      content={content} outline={[]} onClose={() => {}} />))
    expect(host.querySelector('article')?.querySelector('p, h2')).toBeTruthy()
    expect(Boolean(host.querySelector('.document-outline-trigger'))).toBe(visible)
  } finally {
    await act(async () => root.unmount())
    host.remove()
    vi.unstubAllGlobals()
  }
})
