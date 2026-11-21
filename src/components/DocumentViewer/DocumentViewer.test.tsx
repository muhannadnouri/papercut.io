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

it('dismisses an open HTML outline when the next upload reuses its section ordinals', async () => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true)
  vi.stubGlobal('CSS', {})
  const host = document.createElement('div')
  document.body.append(host)
  const root = createRoot(host)
  async function render(url: string, label: string) {
    await act(async () => root.render(<DocumentViewer url={url} format="html"
      content={`<h2 data-papercut-section="0">${label}</h2><h3 data-papercut-section="1">Details</h3>`}
      onClose={() => {}} />))
  }
  async function open() {
    await act(async () => host.querySelector<HTMLButtonElement>('.document-outline-trigger')!.click())
    return document.querySelector('[role="dialog"]')
  }
  try {
    await render('/uploads/aa.html', 'First upload')
    const first = await open()
    expect(first?.textContent).toContain('First upload')
    await act(async () => first!.querySelector<HTMLButtonElement>('button[aria-expanded]')!.click())
    expect(first?.textContent).not.toContain('Details')
    await render('/uploads/bb.html', 'Second upload')
    expect(document.querySelector('[role="dialog"]')).toBeNull()
    const dialog = await open()
    expect(dialog?.textContent).toContain('Second upload')
    expect(dialog?.textContent).toContain('Details')
    expect(dialog?.textContent).not.toContain('First upload')
  } finally {
    await act(async () => root.unmount())
    host.remove()
    vi.unstubAllGlobals()
  }
})

it('toggles a navigable parent from its label or chevron and only jumps from its leaf', async () => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true)
  vi.stubGlobal('CSS', {})
  const scroll = vi.fn()
  vi.stubGlobal('scrollTo', scroll)
  const browse = vi.fn()
  const host = document.createElement('div')
  document.body.append(host)
  const root = createRoot(host)
  try {
    await act(async () => root.render(<DocumentViewer url="/uploads/aa.html" format="html"
      content={bodyHeading + '<h3 data-papercut-section="1">Details</h3>'} onReaderBrowse={browse} onClose={() => {}} />))
    await act(async () => host.querySelector<HTMLButtonElement>('.document-outline-trigger')!.click())
    const nav = document.querySelector<HTMLElement>('[role="dialog"] nav')!
    const toggle = nav.querySelector<HTMLButtonElement>('button[aria-expanded]')!
    expect(toggle.getAttribute('aria-label')).toBe('Collapse Setup')
    expect(toggle.querySelector('bdi')?.textContent).toBe('Setup')
    await act(async () => toggle.querySelector('bdi')!.click())
    expect(toggle.getAttribute('aria-expanded')).toBe('false')
    expect(Array.from(nav.querySelectorAll('button'), button => button.textContent)).toEqual(['Setup'])
    await act(async () => toggle.querySelector('svg')!.dispatchEvent(new MouseEvent('click', { bubbles: true })))
    expect(toggle.getAttribute('aria-expanded')).toBe('true')
    expect(browse).not.toHaveBeenCalled()
    expect(scroll).not.toHaveBeenCalled()
    const leaves = nav.querySelectorAll<HTMLButtonElement>('button:not([aria-expanded])')
    expect(Array.from(leaves, button => button.textContent)).toEqual(['Details'])
    await act(async () => leaves[0].click())
    expect(document.querySelector('[role="dialog"]')).toBeNull()
    expect(document.activeElement).toBe(host.querySelector('article h3'))
    expect(browse).toHaveBeenCalledOnce()
    expect(scroll).toHaveBeenCalled()
  } finally {
    await act(async () => root.unmount())
    host.remove()
    vi.unstubAllGlobals()
  }
})

it('retains nested EPUB choices through parent collapse and a leaf jump, then resets for a new book', async () => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true)
  vi.stubGlobal('CSS', {})
  vi.stubGlobal('scrollTo', vi.fn())
  const host = document.createElement('div')
  document.body.append(host)
  const root = createRoot(host)
  const leaves = Array.from({ length: 10 }, (_, index) => ({
    label: `Passage ${index}`,
    target: `passage-${index}`,
    children: [],
  }))
  // Identical branch labels and leaf targets must still allow independent choices.
  const branch = {
    label: 'Notes',
    target: null,
    children: leaves,
  }
  const outline = [{
    label: 'Part',
    target: null,
    children: [branch, branch],
  }]
  const content = leaves.map(leaf => `<p id="${leaf.target}">${leaf.label}</p>`).join('')
  async function renderBook(url: string) {
    await act(async () => root.render(<DocumentViewer url={url} format="epub"
      content={content} outline={outline} onClose={() => {}} />))
  }
  async function click(button: HTMLButtonElement) {
    await act(async () => button.click())
  }
  async function open() {
    await click(host.querySelector<HTMLButtonElement>('.document-outline-trigger')!)
  }
  const branches = () => Array.from(document.querySelectorAll<HTMLButtonElement>('[role="dialog"] nav button[aria-expanded]'))
  const states = () => branches().map(button => button.getAttribute('aria-expanded'))
  try {
    await renderBook('/uploads/aa.html')
    await open()
    expect(states()).toEqual(['true', 'false', 'false'])
    await click(branches()[2])
    expect(states()).toEqual(['true', 'false', 'true'])
    await click(branches()[0])
    expect(states()).toEqual(['false'])
    await click(branches()[0])
    expect(states()).toEqual(['true', 'false', 'true'])

    await click(document.querySelector<HTMLButtonElement>('[role="dialog"] nav button:not([aria-expanded])')!)
    expect(document.querySelector('[role="dialog"]')).toBeNull()
    expect(document.activeElement?.id).toBe('passage-0')
    await open()
    expect(states()).toEqual(['true', 'false', 'true'])

    await renderBook('/uploads/bb.html')
    expect(document.querySelector('[role="dialog"]')).toBeNull()
    await open()
    expect(states()).toEqual(['true', 'false', 'false'])
  } finally {
    await act(async () => root.unmount())
    host.remove()
    vi.unstubAllGlobals()
  }
})

it('shows every leaf in a long flat HTML outline without disclosure controls', async () => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true)
  vi.stubGlobal('CSS', {})
  const host = document.createElement('div')
  document.body.append(host)
  const root = createRoot(host)
  const labels = Array.from({ length: 21 }, (_, index) => `Section ${index + 1}`)
  const content = labels.map((label, i) => `<h2 data-papercut-section="${i}">${label}</h2>`).join('')
  try {
    await act(async () => root.render(<DocumentViewer url="/uploads/aa.html" format="html"
      content={content} onClose={() => {}} />))
    await act(async () => host.querySelector<HTMLButtonElement>('.document-outline-trigger')!.click())
    const nav = document.querySelector<HTMLElement>('[role="dialog"] nav')!
    expect(nav.querySelector('[aria-expanded]')).toBeNull()
    expect(Array.from(nav.querySelectorAll('button'), row => row.textContent)).toEqual(labels)
  } finally {
    await act(async () => root.unmount())
    host.remove()
    vi.unstubAllGlobals()
  }
})
