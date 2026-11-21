// @vitest-environment jsdom
import {
  act,
  StrictMode,
  type FormEvent,
  type ReactNode,
} from 'react'
import {
  createRoot,
  type Root,
} from 'react-dom/client'
import {
  afterEach,
  beforeEach,
  expect,
  it,
  vi,
} from 'vitest'
import { AppDialog } from './AppDialog'

vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: (key: string) => key }) }))

let root: Root
let container: HTMLDivElement
let opener: HTMLButtonElement

beforeEach(() => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true)
  document.body.innerHTML = '<button>Open Settings</button><div id="mount"></div>'
  opener = document.querySelector('button')!
  opener.focus()
  container = document.querySelector('#mount')!
  root = createRoot(container)
  // jsdom has no layout; give visible fixtures rectangles without replacing DOM focus/events.
  vi.spyOn(HTMLElement.prototype, 'getClientRects').mockImplementation(function (this: HTMLElement) {
    return (this.closest('[hidden]') || this.style.display === 'none'
      ? [] : [new DOMRect(0, 0, 44, 44)]) as unknown as DOMRectList
  })
})

afterEach(async () => {
  await act(() => root.unmount())
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
  document.body.replaceChildren()
})

async function render(content: ReactNode) {
  await act(() => root.render(content))
}
async function click(selector: string) {
  await act(() => container.querySelector<HTMLElement>(selector)!.click())
}
async function key(key: string, shiftKey = false) {
  const event = new KeyboardEvent('keydown', {
    key,
    shiftKey,
    bubbles: true,
    cancelable: true,
  })
  await act(() => document.activeElement!.dispatchEvent(event))
  return event
}

it('keeps Back separate from Close and ignores clicks inside the dialog', async () => {
  const back = vi.fn()
  const close = vi.fn()
  await render(<AppDialog title="Settings" onBack={back} showCloseButton onCancel={close}>
    <input aria-label="Name" />
  </AppDialog>)
  expect(document.activeElement).toBe(container.querySelector('[aria-label="common.back"]'))
  await click('input')
  expect(close).not.toHaveBeenCalled()
  await click('[aria-label="common.back"]')
  expect(back).toHaveBeenCalledOnce()
  expect(close).not.toHaveBeenCalled()
  await click('[aria-label="common.close"]')
  expect(close).toHaveBeenCalledOnce()
  await click('.app-dialog-backdrop')
  expect(close).toHaveBeenCalledTimes(2)
})

it('uses the latest busy guard and Close handler for Escape, buttons and backdrop after rerender', async () => {
  const back = vi.fn()
  const oldClose = vi.fn()
  const close = vi.fn()
  const dialog = (busy: boolean, cancel: () => void) => <AppDialog title="Transfer"
    onBack={back} showCloseButton onCancel={cancel} navigationDisabled={busy}>
    <input aria-label="Transfer Code" />
  </AppDialog>
  await render(dialog(false, oldClose))
  await render(dialog(true, close))
  expect((await key('Escape')).defaultPrevented).toBe(true)
  await click('[aria-label="common.back"]')
  await click('[aria-label="common.close"]')
  await click('.app-dialog-backdrop')
  expect(back).not.toHaveBeenCalled()
  expect(oldClose).not.toHaveBeenCalled()
  expect(close).not.toHaveBeenCalled()
  await render(dialog(false, close))
  await key('Escape')
  await click('[aria-label="common.close"]')
  await click('.app-dialog-backdrop')
  await click('[aria-label="common.back"]')
  expect(close).toHaveBeenCalledTimes(3)
  expect(back).toHaveBeenCalledOnce()
  expect(oldClose).not.toHaveBeenCalled()
})

it('traps Tab in both directions, skips hidden/disabled controls and restores the opener', async () => {
  await render(<StrictMode><AppDialog title="Settings" onCancel={vi.fn()} showCloseButton>
    <button disabled>Unavailable</button><div hidden><button>Hidden</button></div>
    <input aria-label="Name" /><button>Last</button>
  </AppDialog></StrictMode>)
  const first = container.querySelector<HTMLElement>('.app-dialog-close')!
  const last = container.querySelector<HTMLElement>('.app-dialog-body > button:last-child')!
  expect(document.activeElement).toBe(first)
  expect((await key('Tab', true)).defaultPrevented).toBe(true)
  expect(document.activeElement).toBe(last)
  expect((await key('Tab')).defaultPrevented).toBe(true)
  expect(document.activeElement).toBe(first)
  opener.focus()
  await key('Tab')
  expect(document.activeElement).toBe(first)
  opener.focus()
  await key('Tab', true)
  expect(document.activeElement).toBe(last)
  await render(null)
  expect(document.activeElement).toBe(opener)
})

it('focuses an actionless dialog and removes Escape listeners when unmounted', async () => {
  const close = vi.fn()
  await render(<StrictMode><AppDialog title="Working" onCancel={close} /></StrictMode>)
  const dialog = container.querySelector('[role="dialog"]')
  expect(document.activeElement).toBe(dialog)
  expect((await key('Tab')).defaultPrevented).toBe(true)
  expect(document.activeElement).toBe(dialog)
  await key('Escape')
  expect(close).toHaveBeenCalledOnce()
  await render(null)
  expect(document.activeElement).toBe(opener)
  expect((await key('Escape')).defaultPrevented).toBe(false)
  expect(close).toHaveBeenCalledOnce()
})

it('keeps header navigation non-submitting while Save submits the form and Cancel dismisses it', async () => {
  const close = vi.fn()
  const back = vi.fn()
  const submit = vi.fn((event: FormEvent) => event.preventDefault())
  await render(<AppDialog title="Edit" onCancel={close} onSubmit={submit} onBack={back} showCloseButton
    actions={<><button type="button" onClick={close}>Cancel</button><button type="submit">Save</button></>}>
    <input aria-label="Name" />
  </AppDialog>)
  await click('input')
  await click('.app-dialog-back')
  expect(back).toHaveBeenCalledOnce()
  await click('.app-dialog-close')
  expect(close).toHaveBeenCalledOnce()
  expect(submit).not.toHaveBeenCalled()
  await click('[type="submit"]')
  expect(submit).toHaveBeenCalledOnce()
  expect(close).toHaveBeenCalledOnce()
  await click('.app-dialog-actions [type="button"]')
  expect(close).toHaveBeenCalledTimes(2)
})
