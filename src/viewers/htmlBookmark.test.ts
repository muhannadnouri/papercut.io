// @vitest-environment jsdom
import {
  afterEach,
  describe,
  expect,
  it,
  vi,
} from 'vitest'
import {
  createHtmlBookmarkApi,
  textPointAtOffset,
} from './htmlBookmark'
import {
  clearSearchTargetHighlight,
  highlightSearchTarget,
} from '../components/DocumentViewer/readerTarget'

afterEach(() => {
  vi.restoreAllMocks()
  vi.useRealTimers()
  document.body.replaceChildren()
})

it('rebuilds bookmark nodes and cached ranges after fallback highlighting and removal', async () => {
  const root = document.createElement('article')
  root.innerHTML = '<p>Alpha Beta Gamma</p>'
  document.body.append(root)
  const measured: Array<{ node: Node;
    offset: number }> = []
  const createRange = document.createRange.bind(document)
  vi.spyOn(document, 'createRange').mockImplementation(() => {
    const range = createRange()
    range.getBoundingClientRect = () => {
      measured.push({
        node: range.startContainer,
        offset: range.startOffset,
      })
      return DOMRect.fromRect({
        y: 200,
        height: 20,
      })
    }
    return range
  })
  const scroll = vi.spyOn(window, 'scrollTo').mockImplementation(() => {})
  const api = createHtmlBookmarkApi(root)
  try {
    await api.restore({ textOffset: 8 })
    highlightSearchTarget(root, 'Beta', 0)
    await api.restore({ textOffset: 8 })
    expect(measured.at(-1)).toEqual({
      node: root.querySelector('mark')!.firstChild,
      offset: 2,
    })
    clearSearchTargetHighlight(root)
    await api.restore({ textOffset: 8 })
    expect(measured.at(-1)).toEqual({
      node: root.querySelector('p')!.firstChild,
      offset: 8,
    })
    expect(scroll).toHaveBeenCalledTimes(3)
  } finally {
    api.dispose()
  }
})

it('restarts an invalidated yielding build and stops disposed restores', async () => {
  vi.useFakeTimers()
  let clock = 0
  vi.spyOn(performance, 'now').mockImplementation(() => (clock += 13))
  const root = document.createElement('article')
  root.innerHTML = '<p>Alpha</p><p>Beta</p><p>Gamma</p>'
  document.body.append(root)
  const createRange = document.createRange.bind(document)
  const nodes: Node[] = []
  vi.spyOn(document, 'createRange').mockImplementation(() => {
    const range = createRange()
    range.getBoundingClientRect = () => {
      nodes.push(range.startContainer)
      return DOMRect.fromRect({
        y: 200,
        height: 20,
      })
    }
    return range
  })
  const scroll = vi.spyOn(window, 'scrollTo').mockImplementation(() => {})
  const api = createHtmlBookmarkApi(root)
  try {
    const pending = api.restore({ textOffset: 6 })
    root.querySelectorAll('p')[1].textContent = 'Changed'
    await vi.runAllTimersAsync()
    await pending
    expect(nodes).toEqual([root.querySelectorAll('p')[1].firstChild])
    root.querySelector('p')!.textContent = 'New Alpha'
    const cancelled = api.restore({ textOffset: 1 })
    api.dispose()
    await vi.runAllTimersAsync()
    await cancelled
    expect(scroll).toHaveBeenCalledTimes(1)
  } finally {
    api.dispose()
  }
})

describe('HTML bookmark text offsets', () => {
  it('resolves an offset across inline text nodes', () => {
    expect(textPointAtOffset([4, 7, 3], 9)).toEqual({
      partIndex: 1,
      offset: 5,
    })
  })
})
