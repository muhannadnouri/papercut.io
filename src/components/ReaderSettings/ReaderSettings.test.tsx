// @vitest-environment jsdom
import { act } from 'react'
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
import { ReaderSettings } from './ReaderSettings'
import { DEFAULT_READER_SETTINGS } from './useReaderSettings'

vi.mock('react-i18next', () => ({ useTranslation: () => ({ t: (key: string) => key }) }))

let root: Root
let container: HTMLDivElement

beforeEach(async () => {
  vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true)
  document.body.innerHTML = '<p id="page">Document text</p><div id="mount"></div>'
  container = document.querySelector('#mount')!
  root = createRoot(container)
  await act(() => root.render(
    <ReaderSettings applyingFontFamily={null} settings={DEFAULT_READER_SETTINGS} onChange={() => {}} onReset={() => {}} />,
  ))
})

afterEach(async () => {
  await act(() => root.unmount())
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
  document.body.replaceChildren()
})

const button = () => container.querySelector<HTMLButtonElement>('.reader-settings-btn')!
const panel = () => container.querySelector<HTMLElement>('[role="dialog"]')
async function pointerDown(target: Element) {
  await act(() => target.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true })))
}

it('opens a fixed non-modal panel that takes focus and leaves page scrolling alone', async () => {
  await act(() => button().click())
  expect(panel()).not.toBeNull()
  expect(document.activeElement).toBe(panel())
  expect(panel()!.style.top).not.toBe('')
  expect(document.documentElement.style.overflow).toBe('')
  expect(button().getAttribute('aria-expanded')).toBe('true')
})

it('stays open for presses inside and closes for presses outside', async () => {
  await act(() => button().click())
  await pointerDown(panel()!.querySelector('.reader-settings-reset')!)
  expect(panel()).not.toBeNull()
  await pointerDown(document.querySelector('#page')!)
  expect(panel()).toBeNull()
})

it('closes on Escape and returns focus to the button', async () => {
  await act(() => button().click())
  await act(() => panel()!.dispatchEvent(new KeyboardEvent('keydown', {
    key: 'Escape',
    bubbles: true,
  })))
  expect(panel()).toBeNull()
  expect(document.activeElement).toBe(button())
})

it('stays open for presses in the font list, which renders outside the panel', async () => {
  // jsdom has no layout, so AppSelect's scroll-to-active-option call needs a stub.
  Element.prototype.scrollIntoView = vi.fn()
  await act(() => button().click())
  await act(() => panel()!.querySelector<HTMLButtonElement>('.reader-setting-font-select button')!.click())
  const option = document.querySelector('[role="option"]')!
  expect(panel()!.contains(option)).toBe(false)
  await pointerDown(option)
  expect(panel()).not.toBeNull()
})

it('keeps a wide panel inside the viewport on narrow windows', async () => {
  vi.spyOn(document.documentElement, 'clientWidth', 'get').mockReturnValue(600)
  vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockReturnValue(568)
  vi.spyOn(button(), 'getBoundingClientRect').mockReturnValue(new DOMRect(470, 90, 40, 40))
  await act(() => button().click())
  // Lining up with the button's end edge would put the panel at -58px; it is clamped to the 8px margin.
  expect(panel()!.style.left).toBe('8px')
  expect(panel()!.style.top).toBe('138px')
})
