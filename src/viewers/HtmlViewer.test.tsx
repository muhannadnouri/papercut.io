// @vitest-environment jsdom
import {
  act,
  createRef,
} from 'react'
import { createRoot } from 'react-dom/client'
import {
  afterEach,
  expect,
  it,
  vi,
} from 'vitest'
import { HtmlViewer } from './HtmlViewer'
import type { ViewerTextTargetApi } from './types'
import { createHtmlTextTargetApi } from '../tts/playAnywhere/htmlTextTarget'

vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: (key: string) => key }) }))
Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true })
afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
  document.body.replaceChildren()
})

it('does no capture setup when the optional capability is absent', async () => {
  const observer = vi.spyOn(window, 'MutationObserver')
  const container = document.createElement('div')
  document.body.append(container)
  const app = createRoot(container)
  try {
    await act(async () => app.render(<HtmlViewer url="one" content="<p>One</p>" contentRef={createRef()} />))
    expect(observer).not.toHaveBeenCalled()
    expect(container.textContent).toBe('One')
  } finally {
    await act(async () => app.unmount())
  }
})

it('registers the optional API and invalidates old capabilities on URL replacement/unmount', async () => {
  const container = document.createElement('div')
  document.body.append(container)
  const app = createRoot(container)
  const ref = createRef<HTMLElement>()
  const register = vi.fn<(api: ViewerTextTargetApi | null) => void>()
  try {
    await act(async () => app.render(<HtmlViewer url="one" content="<p>Same text</p>" contentRef={ref} onTextTargetApiChange={register} />))
    const first = register.mock.calls.at(-1)![0]!
    await act(async () => app.render(<HtmlViewer url="two" content="<p>Same text</p>" contentRef={ref} onTextTargetApiChange={register} />))
    expect(register.mock.calls.some(([api]) => api === null)).toBe(true)
    const second = register.mock.calls.at(-1)![0]!
    expect(second).not.toBe(first)
    expect(await first.captureViewport()).toEqual({
      status: 'unavailable',
      reason: 'stale',
    })
    await act(async () => app.unmount())
    expect(register.mock.calls.at(-1)![0]).toBeNull()
    expect(await second.captureViewport()).toEqual({
      status: 'unavailable',
      reason: 'stale',
    })
  } finally {
    if (container.childNodes.length) {
      await act(async () => app.unmount())
    }
  }
})

it('returns a rejected Promise when the viewport provider unexpectedly fails', async () => {
  vi.stubGlobal('IntersectionObserver', class {})
  const root = document.createElement('article')
  document.body.append(root)
  const capture = createHtmlTextTargetApi(root, () => {
    throw new Error('Fixture bounds failed')
  })
  try {
    await expect(capture.api.captureViewport()).rejects.toThrow('Fixture bounds failed')
  } finally {
    capture.dispose()
  }
})

it('preserves source nodes, selection and target snapshots when only audio follow state changes', async () => {
  const container = document.createElement('div')
  document.body.append(container)
  const app = createRoot(container)
  const ref = createRef<HTMLElement>()
  const register = vi.fn<(api: ViewerTextTargetApi | null) => void>()
  const render = async (revision: number, content = '<p>Selected passage.</p>') => act(async () => app.render(
    <HtmlViewer url="one" content={content} contentRef={ref} onTextTargetApiChange={register}
      audioFollowPolicy={{
        active: true,
        allowsReveal: () => false,
        revision,
      }} />,
  ))
  try {
    await render(0)
    const text = container.querySelector('p')!.firstChild!
    document.getSelection()!.setBaseAndExtent(text, 3, text, 8)
    const capture = register.mock.calls.at(-1)![0]!.captureSelection!()
    expect(capture.status).toBe('captured')
    if (capture.status !== 'captured') {
      throw new Error('Expected source selection')
    }
    await render(1)
    expect(container.querySelector('p')!.firstChild).toBe(text)
    expect(document.getSelection()!.toString()).toBe('ected')
    expect(capture.isCurrent()).toBe(true)
    expect(register).toHaveBeenCalledOnce()
    await render(2, '<p>Changed source.</p>')
    expect(capture.isCurrent()).toBe(false)
    expect(container.querySelector('p')!.textContent).toBe('Changed source.')
  } finally {
    await act(async () => app.unmount())
  }
})
