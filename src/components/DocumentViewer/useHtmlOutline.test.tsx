// @vitest-environment jsdom
import {
  act,
  useLayoutEffect,
} from 'react'
import { createRoot } from 'react-dom/client'
import {
  expect,
  it,
  vi,
} from 'vitest'
import '../../i18n'
import { HtmlViewer } from '../../viewers/HtmlViewer'
import { useHtmlOutline } from './useHtmlOutline'

it('exposes entries only for the current ready source and reuses them across unrelated renders', async () => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true)
  const host = document.createElement('div')
  document.body.append(host)
  const root = createRoot(host)
  const committed = vi.fn()
  const chapter = (label: string) => `<h2 data-papercut-section="0">${label}</h2><h3 data-papercut-section="1">Details</h3>`
  function Reader({ url, content }: { url?: string;
    content: string }) {
    const { entries, bodyRef } = useHtmlOutline(url, content)
    // Observe every commit, including the one before the new body ref publishes its result.
    useLayoutEffect(() => {
      committed(entries)
    })
    return url ? <HtmlViewer url={url} content={content} htmlBodyRef={bodyRef} /> : null
  }
  async function render(url: string | undefined, content: string) {
    committed.mockClear()
    await act(async () => root.render(<Reader url={url} content={content} />))
    return committed.mock.calls.map(([entries]) => entries)
  }
  try {
    const first = chapter('First')
    const second = chapter('Second')
    const a = '/uploads/aa.html'
    const b = '/uploads/bb.html'
    const initial = (await render(a, first)).at(-1)
    expect(initial[0].label).toBe('First')
    expect((await render(a, first))[0]).toBe(initial)
    for (const [url, content, label] of [[b, first, 'First'], [b, second, 'Second'], [a, first, 'First']]) {
      const commits = await render(url, content)
      expect(commits[0]).toBeUndefined()
      expect(commits.at(-1)[0]).toMatchObject({
        label,
        target: '0',
      })
    }
    expect((await render(undefined, first)).every(entries => entries === undefined)).toBe(true)
    expect((await render(a, '<p>New source</p>')).at(-1)).toEqual([])
  } finally {
    await act(async () => root.unmount())
    host.remove()
    vi.unstubAllGlobals()
  }
})
